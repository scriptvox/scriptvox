//! `scriptvox make --script f`: script in, narrated and captioned video out
//! (PLAN-R1 §2.3). v0.1 first slice: Edge voice, measured karaoke captions,
//! gradient background. Own media (RVX-013), stock (RVX-021), the run store
//! (RVX-015) and detach (RVX-016) build on this path.

use crate::audio;
use crate::captions::{align, paginate, to_ass, to_srt, AssStyle, Layout, Measurer, Word};
use crate::engine_ffmpeg::{part_path, Ffmpeg, RunError};
use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::filterpath::ass_filter;
use crate::fit;
use crate::gates;
use crate::http_rows;
use crate::media;
use crate::pauses::{self, Seg};
use crate::plan;
use crate::render;
use crate::rpc::{RpcError, Worker};
use crate::run::Run;
use crate::store::{self, Cas};
use crate::template::{font_for, gradient, write_fonts};
use crate::textlint;
use crate::{provision, voices};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// How a run's fit to `duration` ended: the rate the voice read at, and the tempo change of each part after it. A
/// child of `resume` that keeps this pace reads its unchanged scenes from the voice cache and gets the same samples.
/// `tempo` is the exact factor, not the rounded `metrics.voice_tempo`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pace {
    pub rate_pct: i32,
    pub tempo: f64,
    /// The `duration` the pace was fitted to. The child's own `duration` is empty: its length follows its words.
    pub duration: Option<f64>,
}

/// Silence between two scenes' speech parts, in seconds.
const SCENE_BREATH_S: f64 = 0.25;

/// The file in a run folder that holds its [`Pace`].
pub const PACE_FILE: &str = "pace.json";

/// The speech parts of a brief with `scenes`: one part per scene (and per pause tag in it), so a change in one scene
/// leaves the cache keys of the others as they are. `None` is the old one-part reading: when no voice is made
/// (`made` false), and when the scenes do not hold the script's words. Scenes with their own `voice` are always cut.
fn scene_segs(
    scenes: &[crate::brief::Scene],
    script: &str,
    voice_id: &str,
    made: bool,
) -> Option<(Vec<Seg>, Vec<String>)> {
    let named = scenes.iter().any(|sc| sc.voice.is_some());
    // A scene with `seconds` has its own time in the voice track, also when no voice is made.
    let timed = scenes.iter().any(|sc| sc.seconds.is_some());
    if !named && !made && !timed {
        return None;
    }
    let joined = scenes
        .iter()
        .map(|s| s.script.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if !named && !joined.split_whitespace().eq(script.split_whitespace()) {
        return None;
    }
    let (mut segs, mut who) = (vec![], vec![]);
    for sc in scenes {
        let id = sc.voice.as_deref().unwrap_or(voice_id);
        // A scene with no words: its `seconds` of silence, exact (no pause-tag limit, no breath around it).
        if let Some(s) = sc.seconds {
            segs.push(Seg::Pause(s));
            who.push(String::new());
            continue;
        }
        let parts = pauses::parse(&sc.script);
        // A short breath between scenes. Each part loses its end silence at the join, so without it one scene runs
        // into the next (measured 13 Nov 2026 with kokoro-rust: 0.3 s at a scene join, 0.8 s at a sentence end).
        // Only a voice that is made has an end silence to lose.
        if made && !parts.is_empty() && matches!(segs.last(), Some(Seg::Speech(_))) {
            segs.push(Seg::Pause(SCENE_BREATH_S));
            who.push(String::new());
        }
        for s in parts {
            segs.push(s);
            who.push(id.to_string());
        }
    }
    // One speaker: `who` stays empty, as for a script with no scenes (the receipt lists no speakers).
    Some((segs, if named { who } else { vec![] }))
}

/// When each scene starts. A scene with words starts at its first word (`default` when the words ran out). A scene
/// with `seconds` and no words ends where the next scene starts (or at `end`), and starts its `seconds` before that.
fn starts_of(scenes: &[crate::brief::Scene], words: &[Word], default: f64, end: f64) -> Vec<f64> {
    let mut idx = 0usize;
    let mut starts: Vec<f64> = scenes
        .iter()
        .map(|sc| {
            let at = words.get(idx).map_or(default, |w| w.start);
            idx += pauses::strip(&sc.script).split_whitespace().count();
            at
        })
        .collect();
    let mut next = end;
    for (i, sc) in scenes.iter().enumerate().rev() {
        if let Some(s) = sc.seconds {
            starts[i] = if i == 0 { 0.0 } else { (next - s).max(0.0) };
        }
        next = starts[i];
    }
    starts
}

/// The length of the silent voice track of `tts: none`: the reading times and every pause. A pause at the end holds
/// the picture for its time; after a last word there is a short tail. No words: the pauses alone, exact.
pub fn reading_len(segs: &[Seg], words: &[Word]) -> f64 {
    let pause = |s: &Seg| if let Seg::Pause(p) = s { *p } else { 0.0 };
    let Some(last) = words.last() else {
        return segs.iter().map(pause).sum::<f64>().max(0.2);
    };
    match segs.iter().rposition(|s| matches!(s, Seg::Speech(_))) {
        Some(i) if i + 1 < segs.len() => last.end + segs[i + 1..].iter().map(pause).sum::<f64>(),
        _ => last.end + 0.3,
    }
}

/// What is wrong with the `seconds` of the scenes and the `duration` of a brief (docs/specs/no-words.md), with a fix.
fn seconds_problem(a: &Args) -> Option<(String, String)> {
    let scenes = a.scenes.as_deref().unwrap_or_default();
    for (i, sc) in scenes.iter().enumerate() {
        let Some(s) = sc.seconds else { continue };
        if !pauses::strip(&sc.script).is_empty() {
            return Some((
                format!("scenes[{i}] has `seconds` and a script: a scene with words takes the time of its words"),
                "Drop `seconds` from that scene, or drop its `script` for a scene with no words. For a hold after the words use a pause tag: \"[pause 2]\".".into(),
            ));
        }
        if !(s.is_finite() && (0.2..=600.0).contains(&s)) {
            return Some((
                format!("scenes[{i}].seconds is a number from 0.2 to 600, not {s}"),
                "Give the time the picture shows, for example \"seconds\": 4.".into(),
            ));
        }
    }
    if !pauses::strip(&a.script).is_empty() {
        return None;
    }
    let sum: f64 = scenes.iter().filter_map(|s| s.seconds).sum();
    let timed = scenes.iter().filter(|s| s.seconds.is_some()).count();
    if a.narration.is_some() || !a.lines.is_empty() {
        return None;
    }
    if timed < scenes.len() || (timed == 0 && a.duration.is_none()) {
        return Some((
            "the script is empty".into(),
            if scenes.is_empty() {
                "Write the narration text into the script. For a video with no words give the brief `duration` (seconds) and no script, or `scenes` that each have `seconds`.".into()
            } else {
                "Write a `script` into each scene, or give each scene with no words its `seconds` (how long its picture shows).".into()
            },
        ));
    }
    match a.duration {
        Some(d) if timed > 0 && (d - sum).abs() > 0.05 => Some((
            format!("duration is {d} s, and the `seconds` of the scenes add up to {sum} s: a video with no words has the length of its scenes"),
            format!("Drop `duration`, or change the scenes so that their `seconds` add up to {d}."),
        )),
        _ => None,
    }
}

/// Scene numbers (from 1) a child of `resume` must voice again, and those it keeps, against its parent's scenes. A
/// scene is kept when the parent has a scene with the same words and voice.
pub fn scenes_voiced_again(
    old: &[crate::brief::Scene],
    new: &[crate::brief::Scene],
) -> (Vec<usize>, Vec<usize>) {
    let key = |s: &crate::brief::Scene| {
        (
            s.script.split_whitespace().collect::<Vec<_>>().join(" "),
            s.voice.clone(),
        )
    };
    let had: Vec<_> = old.iter().map(key).collect();
    (1..=new.len()).partition(|n| !had.contains(&key(&new[n - 1])))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Args {
    /// The narration text (read from `--script` or the brief).
    pub script: String,
    pub lang: Option<String>,
    pub tts: Option<String>,
    pub voice: Option<String>,
    pub local_only: bool,
    pub out: Option<PathBuf>,
    /// Target length in seconds: the narration is fitted before render.
    pub duration: Option<f64>,
    pub aspect: String,
    /// Frames per second: 24, 25, 30 (default) or 60.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Folder of your own clips and stills, used in name order.
    pub media: Option<PathBuf>,
    /// Stock video: comma-separated search terms (one per shot, cycled), or `auto`.
    pub stock: Option<String>,
    /// Music: `calm`, `pulse` (built-in beds), `library:<name>`, a file inside the project, an https address, or none.
    pub music: Option<String>,
    /// The cut of music from an address: start and end, seconds or a clock.
    #[serde(default)]
    pub music_from: Option<String>,
    #[serde(default)]
    pub music_to: Option<String>,
    /// A music bed made by a music row from this description.
    #[serde(default)]
    pub music_prompt: Option<String>,
    /// A free music bed found with no key from these words (`media search`).
    #[serde(default)]
    pub music_search: Option<String>,
    /// Facts of the script with their sources (brief `sources`). No effect on the picture or the plan id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<crate::brief::Source>,
    /// A video that plays from the start under the narration, its sound dropped (re-voice, translate).
    #[serde(default)]
    pub base_video: Option<PathBuf>,
    /// The base video's sound under the voice: none, keep or background.
    #[serde(default)]
    pub base_sound: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub say: std::collections::BTreeMap<String, String>,
    /// A dub on the base video's times: each line starts at its own time and fits its own length.
    #[serde(default)]
    pub lines: Vec<crate::brief::Line>,
    /// A headline shown big for the first 1.6 s.
    #[serde(default)]
    pub hook: Option<String>,
    /// Caption style: `karaoke` (default, up to 2 lines), `pop` (1 to 3 big words at a time) or `reveal`.
    #[serde(default)]
    pub captions: Option<String>,
    /// The one transition between shots: fade (default), slide or cut.
    #[serde(default)]
    pub transition: Option<String>,
    /// The colour look of the picture (`media::look`).
    #[serde(default)]
    pub look: Option<String>,
    /// Built-in sound effects on cuts and cards.
    #[serde(default)]
    pub sfx: bool,
    /// `beat`: each scene boundary moves to the nearest beat of the music bed, at most 0.25 s (`cut-on-beat.md`).
    #[serde(default)]
    pub cut_on: Option<String>,
    /// The end card after the narration.
    #[serde(default)]
    pub outro: Option<crate::brief::Outro>,
    /// The brand colour (#RRGGBB) for the word highlight and the bars.
    #[serde(default)]
    pub accent: Option<String>,
    /// How loud the bed sits under the voice.
    #[serde(default)]
    pub music_level: Option<String>,
    /// The plain background: a name or colours.
    #[serde(default)]
    pub background: Option<String>,
    /// A waveform of the video's sound over the plain background (G107). Old requests have none.
    #[serde(default)]
    pub waveform: bool,
    /// A look added to every generate prompt.
    #[serde(default)]
    pub style: Option<String>,
    /// A seed for generated stills and clips, on rows that take one (G075). Old requests have none.
    #[serde(default)]
    pub seed: Option<u64>,
    /// The brand's font file.
    #[serde(default)]
    pub font: Option<PathBuf>,
    /// A recorded narration file instead of a voice.
    pub narration: Option<PathBuf>,
    /// Where captions sit: bottom, middle or top.
    #[serde(default)]
    pub caption_at: Option<String>,
    /// Caption size: small, normal or large.
    #[serde(default)]
    pub caption_size: Option<String>,
    /// Brief `card_style` (`card_style.rs`). Always in the request, so `resume --patch` can set it.
    #[serde(default)]
    pub card_style: Option<String>,
    /// Brief `size`: the short side of the frame (`size.rs`).
    #[serde(default)]
    pub size: Option<u32>,
    /// Brief `caption_outline` and `caption_box` (`caption_style.rs`).
    #[serde(default)]
    pub caption_outline: Option<f64>,
    #[serde(default)]
    pub caption_box: Option<f64>,
    /// Brief `loop`: asks for the gate `loop_seam`.
    #[serde(default, rename = "loop")]
    pub loop_seam: bool,
    /// The voice's pace: 1.0 is the engine's own, 0.75 to 1.3. Old requests have none.
    #[serde(default)]
    pub voice_rate: Option<f64>,
    /// A logo in a top corner of every frame.
    #[serde(default)]
    pub logo: Option<crate::brief::Logo>,
    /// The same person or thing in every generated scene.
    #[serde(default)]
    pub character: Option<crate::brief::Character>,
    /// Two or more people, each with a name.
    #[serde(default)]
    pub characters: Vec<crate::brief::Character>,
    /// Pick a file from `media` for each line of the script.
    #[serde(default)]
    pub media_match: bool,
    /// Key words or phrases that stay in the accent colour on their caption page.
    #[serde(default)]
    pub emphasis: Vec<String>,
    /// A small second caption line under the main one, one entry per sentence.
    #[serde(default)]
    pub captions_second: Vec<String>,
    /// A fast look: half the size at 15 fps, no upscale or smoothing. `resume --final` makes the full video.
    #[serde(default)]
    pub draft: bool,
    /// `make --until voice`: stop after this stage. Not in a request that has none, so old requests and recipes keep
    /// their text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// `make --storyboard`: one still per clip scene and a sheet, then stop (docs/specs/storyboard.md). Not in a
    /// request that has none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub storyboard: bool,
    /// Upscale generated clips with an `upscale` row.
    #[serde(default)]
    pub upscale: bool,
    /// Interpolate generated clips with an `interpolate` row (more frames, smooth slow-down).
    #[serde(default)]
    pub smooth: bool,
    /// Sound from each generated clip (an `sfx.gen` row that takes a video), mixed under the voice.
    #[serde(default)]
    pub ambience: bool,
    /// Scenes from the brief: each scene's visual and where its words start.
    #[serde(default)]
    pub scenes: Option<Vec<crate::brief::Scene>>,
    /// Attach to a run the parent created (`make --detach`).
    pub run_id: Option<String>,
    /// Use the FFmpeg on PATH instead of the pinned build.
    pub use_system: bool,
    /// The plan id the user approved (needed only when the plan costs money).
    pub approve: Option<String>,
    /// Spend cap for this run in USD (default: the approved plan cost).
    pub max_spend: Option<f64>,
    /// Print the plan and stop.
    pub plan_only: bool,
    /// The same command with the same key is the same run (returned or resumed).
    pub idempotency_key: Option<String>,
    /// Set on a child run made by `resume`.
    #[serde(default)]
    pub parent_run: Option<String>,
    /// Set by `resume` when the child keeps the voice of its parent's unchanged scenes (docs/specs/resume-scene-voice.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pace: Option<Pace>,
    /// Do not embed the recipe in the MP4 metadata.
    #[serde(default)]
    pub no_embed: bool,
    /// `replay`: use only the local cache; a miss is an error, never a provider call.
    #[serde(default)]
    pub cache_only: bool,
    /// The stock clips a recipe recorded (replay uses them with no search).
    #[serde(default)]
    pub stock_picks: Option<Vec<StockPick>>,
}

/// One line of credits.txt for a file from the keyless search.
fn open_credit(what: &str, p: &StockPick) -> String {
    let note = p.terms.as_ref().map_or("", |t| t.note.as_str());
    format!(
        "{what}: by {} through Openverse, {} ({note})",
        p.credit, p.page
    )
}

/// One stock clip of a recipe: enough to fetch it again, or find it in the cache.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StockPick {
    pub provider: String,
    pub id: String,
    pub url: String,
    pub credit: String,
    pub page: String,
    /// The file's own terms, for a row with terms per file. An old recipe has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms: Option<http_rows::Terms>,
    /// Only for music from an address (`fetch`): the title, the licence text of the site when it gives one, and the
    /// layer of fetch that gave the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
}

/// One line of credits.txt for music from an address.
pub(crate) fn fetch_credit(p: &StockPick) -> String {
    let by = if p.credit.is_empty() {
        String::new()
    } else {
        format!(" by {}", p.credit)
    };
    let note = p.terms.as_ref().map_or("", |t| t.note.as_str());
    format!(
        "Music: \"{}\"{by}, {} ({note}; downloaded by scriptvox fetch, layer {})",
        p.title.as_deref().unwrap_or(""),
        p.url,
        p.layer.as_deref().unwrap_or("native")
    )
}

/// Frame size for an aspect name: 9:16 (default, shorts), 16:9, 1:1 or 4:5 (feed posts).
/// `--draft` frame rate: half the frames of the usual 30.
const DRAFT_FPS: u32 = 15;

pub fn frame(aspect: &str) -> Option<(u32, u32)> {
    match aspect {
        "9:16" => Some((1080, 1920)),
        "16:9" => Some((1920, 1080)),
        "1:1" => Some((1080, 1080)),
        "4:5" => Some((1080, 1350)),
        "3:4" => Some((1080, 1440)),
        "4:3" => Some((1440, 1080)),
        "2:3" => Some((1080, 1620)),
        "21:9" => Some((2520, 1080)),
        _ => None,
    }
}

/// The shape asked of generator rows for a frame: models offer 9:16, 16:9 and 1:1, so the other shapes ask for the nearest
/// (the same width on tall frames) and the media graph crops the middle.
pub fn gen_aspect(aspect: &str) -> &str {
    match aspect {
        "4:5" | "3:4" | "2:3" => "9:16",
        "4:3" | "21:9" => "16:9",
        other => other,
    }
}
/// The default frame rate; a brief may ask for 24, 25 or 60.
const FPS: u32 = 30;
/// Silence after the last word, so the video does not cut the voice off.
const TAIL_S: f64 = 0.6;
/// How long the hook headline stays on screen; a scene-one card waits for it.
const HOOK_S: f64 = 1.6;
/// How long `make` waits for one generated clip; past it the job stays on record and a rerun resumes it.
const GEN_WAIT_S: u64 = 900;
// Edge RPC error codes (python/scriptvox_worker/tts_edge.py).
pub(crate) const PROVIDER_REFUSED: i64 = -32010;
pub(crate) const PROVIDER_RETRYABLE: i64 = -32011;
/// `replay` found no cached voice.
const CACHE_MISS: i64 = -32030;

/// Arabic when more than 30% of the letters are Arabic script.
/// A script in a Latin-letter language with no voice here reads as English: an English voice would read it badly.
/// Common short words of each language, and the letters only it writes; the language with most hits, and over a tenth
/// of the words. A French word after an elision ("l'eau", "qu'en") is a hit.
pub fn foreign_script(text: &str) -> Option<&'static str> {
    // (name, words, letters)
    const WORDS: [(&str, &[&str], &str); 6] = [
        (
            "Indonesian",
            &[
                "yang", "dan", "di", "dari", "ini", "itu", "untuk", "dengan", "tidak", "pada",
                "adalah", "ke", "akan",
            ],
            "",
        ),
        (
            "Spanish",
            &[
                "el", "la", "los", "las", "de", "que", "en", "una", "por", "para", "es", "del",
                "su", "más", "muy", "pero", "como",
            ],
            "ñ¿¡",
        ),
        (
            "French",
            &[
                "le", "la", "les", "des", "et", "est", "une", "dans", "que", "qui", "du", "au",
                "de", "en", "un", "à", "ce", "il", "avec", "aux", "nous", "vous",
            ],
            "èêëœùûîï",
        ),
        (
            "German",
            &[
                "der", "die", "das", "und", "ist", "ein", "eine", "nicht", "mit", "auf", "für",
                "zu", "von", "im", "dem", "sich", "auch", "sie", "wir",
            ],
            "ßäöü",
        ),
        (
            "Portuguese",
            &[
                "os", "de", "que", "em", "uma", "para", "com", "não", "da", "dos", "das", "na",
                "ao", "aos", "é", "você", "muito",
            ],
            "ãõ",
        ),
        (
            "Italian",
            &[
                "il", "lo", "gli", "di", "che", "e", "una", "per", "con", "non", "della", "sono",
                "del", "è", "nel", "alla", "dei", "delle", "più", "la", "un", "le",
            ],
            "",
        ),
    ];
    // Each word in lower case, and whether it was written so: "Los" in "Los Angeles" is a name, not a Spanish word.
    let words: Vec<(String, bool)> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .map(|w| (w.to_lowercase(), !w.chars().any(char::is_uppercase)))
        .collect();
    if words.len() < 5 {
        return None;
    }
    let english = [
        "the", "and", "of", "to", "is", "in", "a", "that", "for", "with", "this", "on", "it",
        "are", "was", "were", "be", "been", "as", "at", "by", "from", "have", "has", "had", "not",
        "you", "your", "we", "our", "they", "their", "he", "she", "his", "her", "or", "an", "but",
        "if", "so", "do", "does", "did", "why", "what", "how", "when", "who", "which", "will",
        "can", "more", "much", "i", "my", "me", "no", "all", "one", "its", "there", "than", "then",
        "about", "up", "out", "into", "over", "just", "now", "here", "every", "each", "these",
        "those", "them", "us", "am", "would", "should", "could", "very", "too", "also", "only",
    ];
    let en = words
        .iter()
        .filter(|(w, _)| english.contains(&w.as_str()))
        .count();
    let elided = |w: &str| {
        ["l", "d", "qu", "c", "j", "n", "s"].iter().any(|p| {
            w.strip_prefix(p)
                .is_some_and(|r| r.starts_with(['\'', '’']))
        })
    };
    let (name, most) = WORDS
        .iter()
        .map(|(n, list, letters)| {
            let hit = |(w, lower): &&(String, bool)| {
                (*lower && list.contains(&w.as_str()))
                    || w.chars().any(|c| letters.contains(c))
                    || (*n == "French" && elided(w))
            };
            (*n, words.iter().filter(hit).count())
        })
        .max_by_key(|x| x.1)?;
    // Two hits at least: one word that two languages share ("die", "la") is not a language.
    (most >= 2 && most > en && most * 10 > words.len()).then_some(name)
}

pub fn detect_lang(text: &str) -> &'static str {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    let arabic = letters
        .iter()
        .filter(|c| ('\u{0600}'..='\u{06FF}').contains(c))
        .count();
    if !letters.is_empty() && arabic * 10 > letters.len() * 3 {
        "ar"
    } else {
        foreign_script(text).and_then(latin_code).unwrap_or("en")
    }
}

/// The code of a language [`foreign_script`] names, when the registry has a voice for it (docs/specs/latin-languages.md).
/// Italian has no row: it stays `en` with the `lang` warning.
fn latin_code(name: &str) -> Option<&'static str> {
    Some(match name {
        "Spanish" => "es",
        "French" => "fr",
        "German" => "de",
        "Portuguese" => "pt",
        "Indonesian" => "id",
        _ => return None,
    })
}

/// The stages `make --until` may stop after. `captions` or `clips` would be added here and at their own stop.
const UNTIL: [&str; 1] = ["voice"];

/// `--until` with a stage that is not offered: a usage error that names it and the ones that are.
pub fn until_error(until: Option<&str>) -> Option<Envelope> {
    let bad = until.filter(|u| !UNTIL.contains(u))?;
    Some(Envelope::fail(
        "make",
        Exit::Usage,
        format!("--until takes {}, not {bad}", UNTIL.join(", ")),
        Some(
            "Use `--until voice` to stop after the voice and the caption files, or drop --until."
                .into(),
        ),
    ))
}

/// The request, so `resume <run> --brief f` can make a child run from it.
fn save_request(dir: &Path, a: &Args) -> Args {
    let mut saved = a.clone();
    saved.run_id = None;
    saved.approve = None;
    saved.plan_only = false;
    let _ = std::fs::write(
        dir.join("request.json"),
        serde_json::to_string_pretty(&saved).unwrap_or_default(),
    );
    saved
}

impl Args {
    /// Brief `caption_size`, `caption_outline` and `caption_box` as the one style (`caption_style.rs`).
    pub fn caption_style(&self) -> crate::caption_style::Style {
        crate::caption_style::Style {
            size: self.caption_size.clone(),
            outline: self.caption_outline,
            box_: self.caption_box,
        }
    }
}

/// `resume --patch` (G066): a JSON merge patch of brief fields on a run's request. Only brief fields that the request
/// keeps under the same name; `null` takes a field back to its default. New words or scenes set the script as `resume
/// --brief` does. Returns the request and the fields changed.
pub fn patch_request(a: &Args, patch: &Value) -> Result<(Args, Vec<String>), String> {
    let Some(p) = patch.as_object().filter(|p| !p.is_empty()) else {
        return Err("the patch must be a JSON object with at least one field".into());
    };
    let brief = crate::brief::schema("brief").unwrap_or_default();
    let mut req = serde_json::to_value(a).map_err(|e| e.to_string())?;
    let mut keys = vec![];
    for (k, v) in p {
        // The run keeps the Sources card, not the field: set after the other fields, on the card (below).
        if k == "sources_card" {
            if !matches!(v, Value::Bool(_) | Value::Null) {
                return Err("`sources_card` takes true or false".into());
            }
            keys.push(k.clone());
            continue;
        }
        // `sources` is left out of a request that has none, and it is still a field a run keeps.
        let known =
            brief["properties"].get(k).is_some() && (req.get(k).is_some() || k == "sources");
        if !known {
            return Err(format!(
                "`{k}` is not a brief field a run keeps (script_file and brand cannot be patched: give `script`, or the fields)"
            ));
        }
        let v = match (k.as_str(), v) {
            ("stock", Value::Array(t)) => json!(t
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")),
            (_, Value::Null) => match &req[k] {
                Value::Array(_) => json!([]),
                Value::Bool(_) => json!(false),
                Value::Object(_) if k == "say" => json!({}),
                _ if k == "sources" => json!([]),
                _ => Value::Null,
            },
            _ => v.clone(),
        };
        req[k] = v;
        keys.push(k.clone());
    }
    let mut out: Args =
        serde_json::from_value(req).map_err(|e| format!("the patch does not fit: {e}"))?;
    if p.contains_key("lines") {
        out.script = out
            .lines
            .iter()
            .map(|l| l.text.trim())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if let Some(on) = p.get("sources_card") {
        // New words with no scenes drop the scenes (below), and the card has no scene to stand on.
        if on == &Value::Bool(true)
            && !p.contains_key("scenes")
            && (p.contains_key("script") || p.contains_key("lines"))
        {
            return Err("`sources_card` with new words needs the words as `scenes`: the card is on the last scene".into());
        }
        crate::sources_card::patch(
            &mut out.scenes,
            &out.sources,
            out.lang.as_deref(),
            &out.aspect,
            on == &Value::Bool(true),
        )?;
    }
    // Patched scenes get the brief's check of their cards (`card` and `cards`).
    if let Some((i, e)) = out
        .scenes
        .iter()
        .flatten()
        .enumerate()
        .filter(|_| p.contains_key("scenes"))
        .find_map(|(i, sc)| Some((i, crate::scene_cards::problem(sc)?)))
    {
        return Err(format!("scene {}: {e}", i + 1));
    }
    if let Some(sc) = out.scenes.as_ref().filter(|_| p.contains_key("scenes")) {
        out.script = sc
            .iter()
            .map(|s| s.script.trim())
            .collect::<Vec<_>>()
            .join(" ");
    } else if (p.contains_key("script") || p.contains_key("lines"))
        && (out.media_match || out.script.trim() != a.script.trim())
    {
        // New words: the scenes, cards and picks of the old ones no longer fit.
        out.scenes = None;
    }
    Ok((out, keys))
}

/// The first of the brief's or recipe's own files that lies outside the project. Each of these ends up in the video.
fn file_outside_project(a: &Args) -> Option<PathBuf> {
    let root = Path::new(".").canonicalize().ok()?;
    let scenes = a.scenes.iter().flatten();
    a.narration
        .iter()
        .chain(a.font.iter())
        .chain(a.logo.iter().map(|l| &l.file))
        .chain(scenes.clone().filter_map(|s| s.overlay.as_ref()))
        .chain(scenes.filter_map(|s| s.inset.as_ref().map(|i| &i.file)))
        .filter_map(|p| p.canonicalize().ok())
        .find(|p| !p.starts_with(&root))
}

/// Every picture, clip and sound file that the brief, the flags or a recipe names. Each goes to FFmpeg or ffprobe, so
/// each must be plain media by its first bytes (`media-inputs.md`), checked before a plan is priced or a row is called.
fn media_files(a: &Args) -> Vec<PathBuf> {
    let scenes = || a.scenes.iter().flatten();
    let mut files: Vec<PathBuf> = a
        .narration
        .iter()
        .chain(a.base_video.iter())
        .chain(a.logo.iter().map(|l| &l.file))
        .chain(
            a.character
                .iter()
                .chain(&a.characters)
                .filter_map(|c| c.file.as_ref()),
        )
        .chain(scenes().filter_map(|s| s.media.as_ref()))
        .chain(scenes().filter_map(|s| s.overlay.as_ref()))
        .chain(scenes().filter_map(|s| s.inset.as_ref().map(|i| &i.file)))
        .chain(scenes().filter_map(|s| s.generate.as_ref()?.image.as_ref()))
        .chain(scenes().filter_map(|s| s.generate.as_ref()?.last_frame.as_ref()))
        .cloned()
        .collect();
    // A built-in bed has a name, not a file.
    files.extend(
        a.music
            .iter()
            .filter(|m| !crate::fetch::is_address(m) && !crate::music_lib::is_library(m))
            .map(PathBuf::from),
    );
    // The media folder: each clip or photo in it (the kinds `media::list` takes).
    if let Some(dir) = a.media.as_ref().and_then(|d| std::fs::read_dir(d).ok()) {
        files.extend(
            dir.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| media::kind_of(p).is_some()),
        );
    }
    // A file outside the project is refused further on by its own rule, with its own message, and is not read here.
    let root = Path::new(".").canonicalize().ok();
    files.retain(|p| {
        p.canonicalize()
            .ok()
            .zip(root.as_ref())
            .is_some_and(|(p, r)| p.starts_with(r))
    });
    files
}

/// A recipe comes from an MP4, which may be untrusted: a recorded pick must name a known provider, a plain id, and an
/// https URL on that provider's hosts. Checked when the request arrives (so before a detached run starts), and again
/// at the fetch.
fn stock_pick_refused(p: &StockPick, rows: &[http_rows::Row]) -> Option<Envelope> {
    let row = rows
        .iter()
        .find(|r| r.name == p.provider)
        .cloned()
        .or_else(|| http_rows::rows().into_iter().find(|r| r.name == p.provider));
    let ok = row
        .as_ref()
        .is_some_and(|r| http_rows::url_allowed(&p.url, &r.hosts))
        && http_rows::safe_token(&p.provider)
        && http_rows::safe_token(&p.id);
    (!ok).then(|| {
        Envelope::fail_code(
            "make",
            Exit::Usage,
            "SVX_BAD_RECIPE",
            format!(
                "the recipe names a stock clip that is not allowed: {} {}",
                p.provider, p.id
            ),
            Some("Only remake videos you trust.".into()),
        )
    })
}

pub fn run(mut a: Args) -> Envelope {
    if let Some(c) = a.cut_on.as_deref() {
        let no_bed = a.music.as_deref().is_none_or(|m| m == "none");
        // Music from an address is fetched before the voice when `cut_on` is set (docs/specs/reel-gaps.md).
        let late = a.music_prompt.is_some() || a.music_search.is_some();
        let (msg, fix) = if c != "beat" {
            (
                format!("unknown cut_on {c}"),
                "Use \"cut_on\": \"beat\", or drop the field.",
            )
        } else if a.scenes.as_ref().is_none_or(|s| s.len() < 2) {
            (
                "cut_on moves scene boundaries, and this brief has no scenes".to_string(),
                "Give the video as `scenes` (2 or more), or drop cut_on.",
            )
        } else if late {
            (
                "cut_on needs the music before the voice is made, and music from a search (music_search) or a music row (music_prompt) comes later".to_string(),
                "Get the file first (`scriptvox media search`, or `scriptvox generate`) and give it as `music`; or give `music` a built-in bed, a file or an https address; or drop cut_on.",
            )
        } else if no_bed {
            (
                "cut_on needs a music bed: the beats come from it".to_string(),
                "Set music to pulse or to a music file inside the project, or drop cut_on.",
            )
        } else {
            (String::new(), "")
        };
        if !msg.is_empty() {
            return Envelope::fail("make", Exit::Usage, msg, Some(fix.into()));
        }
    }
    // A request can get `card_style` from `resume --patch`, which does not read a brief.
    if let Some(e) = crate::card_style::problem(
        a.card_style.as_deref(),
        a.background.as_deref(),
        a.accent.as_deref(),
    ) {
        return Envelope::fail(
            "make",
            Exit::Usage,
            e,
            Some("Use \"card_style\": \"paper\" with a light `background` and an `accent` that is dark enough, or drop card_style.".into()),
        );
    }
    // A library track: a known name, and with --local-only a copy in the store (docs/specs/music-library.md).
    if let Some(m) = a
        .music
        .as_deref()
        .filter(|m| crate::music_lib::is_library(m))
    {
        match crate::music_lib::find(m) {
            Err((msg, fix)) => return Envelope::fail("make", Exit::Usage, msg, Some(fix)),
            Ok(t) if a.local_only && !crate::music_lib::cached(&t) => {
                return needs(voices::needs_input(
                    "music",
                    "dependency",
                    "This library track is not downloaded yet, and that needs the network. Run once without --local-only, or use a built-in bed.",
                    vec![json!({"args": ["--music", "calm"], "cost_usd": 0, "note": "built-in bed, offline"})],
                ));
            }
            Ok(_) => {}
        }
    }
    // Music from an address: the checks of `fetch` that cost no request, before any other work.
    let music_url = a.music.clone().filter(|m| crate::fetch::is_address(m));
    match &music_url {
        Some(url) => {
            if let Err(f) =
                crate::fetch::music_check(url, a.music_from.as_deref(), a.music_to.as_deref())
            {
                return Envelope::fail_code("make", f.exit, f.code, f.message, f.fix);
            }
            if a.local_only {
                return needs(voices::needs_input(
                    "music",
                    "dependency",
                    "Music from an address needs the network. Run `scriptvox fetch <address> --audio` first and give that file as music, or run without --local-only.",
                    vec![json!({"args": ["--music", "calm"], "cost_usd": 0, "note": "built-in bed, offline"})],
                ));
            }
        }
        // A cut of the user's own music file is made in the run folder (docs/specs/user-music.md).
        None if (a.music_from.is_some() || a.music_to.is_some())
            && a.music
                .as_deref()
                .is_none_or(|m| m == "none" || audio::bed(m).is_some()) =>
        {
            return Envelope::fail(
                "make",
                Exit::Usage,
                "music_from and music_to cut a music file or music from an address, and this brief has none",
                Some("Give `music` a file in the project or an https address, or drop music_from and music_to.".into()),
            );
        }
        None => {}
    }
    if let Some(env) = a
        .stock_picks
        .iter()
        .flatten()
        .find_map(|p| stock_pick_refused(p, &[]))
    {
        return env;
    }
    if let Some(p) = file_outside_project(&a) {
        return Envelope::fail_code(
            "make",
            Exit::Error,
            "SVX_PATH_OUTSIDE_ROOT",
            format!("{} is outside this project folder", p.display()),
            Some("Copy the file into the project.".into()),
        );
    }
    if let Some(env) = crate::media_gate::refuse("make", media_files(&a)) {
        return env;
    }
    let fail = |exit, msg: String, fix: Option<&str>| {
        Envelope::fail("make", exit, msg, fix.map(String::from))
    };
    // `sound: none` is no field: the same plan id and recipe as a brief without it.
    for sc in a.scenes.iter_mut().flatten() {
        sc.sound = sc.sound.take().filter(|s| s != "none");
    }
    if let Some(env) = until_error(a.until.as_deref()) {
        return env;
    }
    // `--until voice`: only the voice stage is planned, approved and run.
    let voice_only = a.until.is_some();
    // A detached run saves its request first: a check that stops it early (script too long) is fixed by `resume`.
    if let Some(dir) = a
        .run_id
        .as_ref()
        .map(|id| Path::new(".scriptvox/runs").join(id))
        .filter(|d| d.is_dir())
    {
        let _ = save_request(&dir, &a);
    }
    // Frames leave at the pick, so approval comes first. The picks do not change the cost, and the plan sends nothing.
    // A storyboard sends no frame of the user's media anywhere: it is treated as a plan here.
    if a.media_match && !a.plan_only && !a.storyboard {
        let plan = run(Args {
            plan_only: true,
            ..a.clone()
        });
        if let (Some(id), Some(cost)) = (
            &plan.plan_id,
            plan.metrics.get("cost_usd").and_then(Value::as_f64),
        ) {
            let stages = serde_json::to_value(&plan)
                .map(|v| v["stages"].clone())
                .unwrap_or_default();
            if let Some(env) = approval_stop(id, cost, a.approve.as_deref(), a.max_spend, stages) {
                return env;
            }
        }
    }
    // The picks do not change the cost, and a plan may not have them yet: an approval holds for the plan without them.
    let id_scenes = a.scenes.clone();
    // Not before a stop at the voice: the pick sends frames to the vision service, and the `resume` makes it.
    let (matched, match_failed) = if voice_only {
        (None, None)
    } else {
        match pick_media(&mut a) {
            Ok(m) => m,
            Err(env) => return *env,
        }
    };
    // SSML is not a script form here: a break tag becomes a pause tag, any other tag stops the run.
    let ssml_breaks = match crate::cues::take_ssml(&mut a) {
        Ok(n) => n,
        Err(e) => return fail(Exit::Usage, e, Some(crate::cues::SSML_FIX)),
    };
    // A video with no words (docs/specs/no-words.md): its length is the `seconds` of its scenes, or `duration`.
    if let Some((msg, fix)) = seconds_problem(&a) {
        return fail(Exit::Usage, msg, Some(&fix));
    }
    let no_words =
        pauses::strip(&a.script).is_empty() && a.narration.is_none() && a.lines.is_empty();
    if no_words {
        // No voice is made, so no voice row is picked: `--local-only` passes, and no key is asked for.
        a.tts = Some("none".into());
        a.voice = None;
    }
    let script = a.script.clone();
    // Steps that send the user's own media to a model need the network and a new call: refuse them up front.
    let sends_media = a.ambience || a.base_sound.as_deref() == Some("background");
    if sends_media && (a.local_only || a.cache_only) {
        return fail(
            Exit::Usage,
            "ambience and base_sound background send clips to a model, so they need the network and a new call".into(),
            Some("Drop --local-only or --cache-only, or drop ambience and base_sound background."),
        );
    }
    let fps = a.fps.unwrap_or(FPS);
    if ![24, 25, 30, 60].contains(&fps) {
        return fail(
            Exit::Usage,
            format!("unsupported fps {fps}"),
            Some("Use fps 24, 25, 30 or 60."),
        );
    }
    let Some((w, h)) = frame(&a.aspect) else {
        return fail(
            Exit::Usage,
            format!("unknown aspect {}", a.aspect),
            Some("Use aspect 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9."),
        );
    };
    let (w, h) = match crate::size::check(a.size) {
        Ok(()) => crate::size::scale((w, h), a.size),
        Err(e) => return fail(Exit::Usage, e, Some(crate::size::FIX)),
    };
    let (w, h, fps) = if a.draft {
        (w / 2, (h / 2 + 1) & !1, DRAFT_FPS)
    } else {
        (w, h, fps)
    };
    let lang = a
        .lang
        .as_deref()
        .map(voices::lang_code)
        .unwrap_or_else(|| detect_lang(&script).to_string());
    if let Some(d) = a.duration.filter(|d| !d.is_finite() || *d <= 0.0) {
        return Envelope::fail(
            "make",
            Exit::Usage,
            format!("duration must be a number of seconds above 0, not {d}"),
            Some("Give `duration` in seconds, for example 30.".into()),
        );
    }
    if let Some(f) = &a.narration {
        let usage =
            |m: String, fix: &str| Envelope::fail("make", Exit::Usage, m, Some(fix.to_string()));
        if !f.is_file() {
            return usage(
                format!("no narration file: {}", f.display()),
                "Check the path in `narration`.",
            );
        }
        if a.duration.is_some() {
            return usage(
                "`narration` sets the length, so `duration` cannot".to_string(),
                "Drop `duration`, or trim the recording.",
            );
        }
    }
    let voice = match voices::choose(&lang, a.tts.as_deref(), a.voice.as_deref(), a.local_only) {
        Ok(v) => voices::Voice {
            say: a.say.clone(),
            ..v
        },
        Err(ni) => return needs(ni),
    };
    // Delivery cues and IPA: what this voice's row does not take leaves the script here, with one warning each.
    let cue_warnings = match crate::cues::for_run(&mut a, &voice, ssml_breaks) {
        Ok(w) => w,
        Err(e) => return fail(Exit::Usage, e, Some(crate::cues::IPA_FIX)),
    };
    let script = a.script.clone();
    // Brief `voice_rate` as the percent the engine gets; `None` is the engine's own pace. Checked here too, because
    // `resume --patch` does not pass the brief check.
    let rate0 = match fit::voice_rate_pct(a.voice_rate, &voice.engine) {
        Ok(r) if r.is_some() && (a.narration.is_some() || !a.lines.is_empty()) => {
            return fail(
                Exit::Usage,
                "voice_rate does not go with `narration` (a recording has its own pace) or `lines` (each line is fitted to its own time)".into(),
                Some("Drop voice_rate."),
            )
        }
        Ok(r) => r,
        Err(e) => {
            return fail(
                Exit::Usage,
                e,
                Some("Give \"voice_rate\" from 0.75 (slower) to 1.3 (faster), or drop it."),
            )
        }
    };
    // A kept pace can come from a recipe in a file, which the user may not have made: the engine's own rate limits,
    // and a tempo `atempo` can do.
    if let Some(p) = &a.pace {
        let (lo, hi) = fit::rate_limits(&voice.engine);
        if !(lo..=hi).contains(&p.rate_pct)
            || !(p.tempo.is_finite() && (0.5..=2.0).contains(&p.tempo))
        {
            return fail(
                Exit::Usage,
                format!(
                    "the kept pace is out of range: rate {} percent, tempo {}",
                    p.rate_pct, p.tempo
                ),
                Some("Make the video again from its brief."),
            );
        }
    }
    // A child of `resume` that keeps its parent's pace reads at the rate the parent's fit ended at.
    let rate0 = a
        .pace
        .as_ref()
        .map_or(rate0, |p| (p.rate_pct != 0).then_some(p.rate_pct));
    // What the voice is estimated to read in a second, at that pace.
    let pace_wps =
        fit::words_per_s(&lang, &voice.engine) * (1.0 + f64::from(rate0.unwrap_or(0)) / 100.0);
    // A scene's or a dub line's own speaker: the same engine, and it speaks the language.
    let own_voices = a
        .scenes
        .iter()
        .flatten()
        .filter_map(|s| s.voice.as_deref())
        .chain(a.lines.iter().filter_map(|l| l.voice.as_deref()));
    for id in own_voices {
        // An id can be in two engines (the two Kokoro rows): take the row of this video's engine first.
        let all = voices::all();
        let v = all
            .iter()
            .find(|v| v.id == id && v.engine == voice.engine)
            .or_else(|| all.iter().find(|v| v.id == id));
        let why = match &v {
            None => Some(format!("unknown voice {id}")),
            Some(v) if v.engine != voice.engine || voice.engine == "selfhost" => Some(format!(
                "voice {id} is not usable with engine {}: all speakers come from one engine, and selfhost has one speaker",
                voice.engine
            )),
            Some(v) if !v.speaks(&lang) => Some(format!("voice {id} does not speak {lang}")),
            _ => None,
        };
        if let Some(why) = why {
            return fail(Exit::Usage, why, Some("Run `scriptvox voices --json`."));
        }
    }
    // Scenes: one speech part per scene, so `resume` with one scene changed voices that scene alone. Not for a silent
    // video or a recording (no voice is made: the breath between scenes would only move their times), and not with
    // `media_match`: its scenes come from the picks, which a plan and `--until voice` do not have yet, so the run
    // would ask for other cache keys than its plan and than the stopped run.
    let voice_made = voice.engine != "none" && a.narration.is_none() && !a.media_match;
    let (segs, who) = a
        .scenes
        .as_ref()
        .and_then(|sc| scene_segs(sc, &script, &voice.id, voice_made))
        .unwrap_or_else(|| (pauses::parse(&script), vec![]));
    // No words and no scene with `seconds`: the `duration` is the one silence (pause tags in the script do not count).
    let segs = match a.duration {
        Some(d) if no_words && a.scenes.iter().flatten().all(|s| s.seconds.is_none()) => {
            vec![Seg::Pause(d)]
        }
        _ => segs,
    };
    let pause_s: f64 = segs
        .iter()
        .map(|s| if let Seg::Pause(p) = s { *p } else { 0.0 })
        .sum();
    let spoken = pauses::strip(&script);
    if !a.captions_second.is_empty() {
        let tokens: Vec<&str> = spoken.split_whitespace().collect();
        let n = crate::captions::sentences(&tokens).len();
        if n != a.captions_second.len() {
            return fail(
                Exit::Usage,
                format!(
                    "captions_second has {} entries, the script has {n} sentences",
                    a.captions_second.len()
                ),
                Some("Give one captions_second entry per sentence of the script, in order."),
            );
        }
    }
    // The estimates and the price count what the voice says.
    let voiced = crate::say::apply(&spoken, &a.say);
    // Text lint before any provider call: boxes and broken Arabic never reach a render.
    let card_text: Vec<&str> = a
        .scenes
        .iter()
        .flatten()
        .flat_map(crate::scene_cards::of)
        .flat_map(|c| c.texts())
        .collect();
    let lint = textlint::check(&format!(
        "{} {spoken} {}",
        a.hook.as_deref().unwrap_or(""),
        card_text.join(" ")
    ));
    if let Some(bg) = &a.background {
        if let Err(e) = crate::template::set_background(bg) {
            return fail(Exit::Usage, e, Some("Fix `background` in the brief."));
        }
    }
    if let Some(f) = &a.font {
        if let Err(e) = crate::template::set_brand_font(f) {
            return fail(
                Exit::Usage,
                e,
                Some("Give a TTF or OTF file inside the project."),
            );
        }
        let missing = crate::template::missing_letters(
            font_for(&lang).1,
            &format!(
                "{} {spoken} {}",
                a.hook.as_deref().unwrap_or(""),
                card_text.join(" ")
            ),
        );
        if !missing.is_empty() {
            let shown: String = missing.iter().take(12).collect();
            return fail(
                Exit::Usage,
                format!("font {} has no letters for: {shown}", f.display()),
                Some("Pick a font that covers the script, or drop `font`."),
            );
        }
    }
    if let Some(first) = lint.first() {
        let mut env = Envelope::fail_code(
            "make",
            Exit::GateFailed,
            first.code,
            first.detail.clone(),
            Some(first.fix.clone()),
        );
        env.findings = lint.iter().map(|f| json!(f)).collect();
        return env;
    }
    // `tts: none` has no voice to measure: its length is known here, so `plan` and `make` stop at the same place with
    // the same answer. The other voices: only a clear miss stops here, and the real length decides after TTS.
    if let Some(target) = a.duration.filter(|_| a.lines.is_empty() && !no_words) {
        if voice.engine == "none" && a.narration.is_none() && a.pace.is_none() {
            let pace = silent_lang(&segs, &voice);
            let len = |rate: Option<i32>| reading_len(&segs, &reading_times(&segs, pace, rate));
            let fitted = fit::ladder(
                len(rate0),
                target,
                spoken.split_whitespace().count(),
                fit::words_per_s(&lang, "none"),
                rate0.is_none(),
                fit::rate_limits("none"),
                &mut |pct| Some(len(Some(pct))),
            );
            if let Some((_, f @ (fit::Fit::TooLong { .. } | fit::Fit::TooShort { .. }))) = fitted {
                return fit_failure(
                    f,
                    target,
                    "the words at reading pace and the pauses; nothing was rendered",
                );
            }
        } else if let Some(f) = fit::precheck(
            fit::spoken_words(&voiced, &lang, &voice.engine),
            pause_s,
            target,
            pace_wps,
        ) {
            return fit_failure(
                f,
                target,
                "estimated before TTS; nothing was synthesized or rendered",
            );
        }
    }
    // A pause tag above the limit is cut: said, not silent.
    let mut cue_warnings = cue_warnings;
    if pauses::capped(&script) {
        cue_warnings.push(gates::Finding {
            gate: "pauses",
            status: "warn",
            detail: "a pause tag asks for more than 10 s: it is cut to 10 s".into(),
            fix: Some(
                "For a longer time with no words give a scene `seconds` and no script.".into(),
            ),
        });
    }
    // Named options, checked before any voice or render work (an unknown transition used to panic after TTS).
    let checked = media::transition(a.transition.as_deref(), false, fps)
        .and_then(|_| media::look(a.look.as_deref()))
        .and_then(|_| {
            a.scenes
                .iter()
                .flatten()
                .enumerate()
                .try_for_each(|(i, s)| match &s.transition {
                    Some(_) if i == 0 => Err(
                        "the first scene has no transition: it has no scene before it".to_string(),
                    ),
                    Some(t) => media::transition(Some(t), false, fps).map(|_| ()),
                    None => Ok(()),
                })
        })
        .and_then(|_| {
            a.scenes
                .iter()
                .flatten()
                .filter_map(|s| s.camera.as_deref())
                .try_for_each(|c| media::camera(c, (1080, 1920), 1.0, fps).map(|_| ()))
        });
    if let Err(e) = checked {
        let fix = if e.starts_with("the first scene") {
            "Remove `transition` from the first scene."
        } else {
            "Fix the name in the brief."
        };
        return fail(Exit::Usage, e, Some(fix));
    }
    // An overlay with no alpha would hide its scene: refused before any voice is made.
    if let Ok(ff) = Ffmpeg::locate(a.use_system, false) {
        let opaque = a
            .scenes
            .iter()
            .flatten()
            .filter_map(|s| s.overlay.as_ref())
            .find(|o| !ff.has_alpha(o));
        if let Some(o) = opaque {
            return fail(
                Exit::Usage,
                format!("overlay {} has no alpha channel, so it would hide the scene", o.display()),
                Some("Render it with transparency: WebM VP9 with alpha, MOV ProRes 4444 or Animation, or PNG. Or use `inset`."),
            );
        }
    }
    // A voice or a recording speaks: the music goes under it. Else the music is the only sound.
    let has_sound_voice = voice.engine != "none" || a.narration.is_some();
    // A waveform of silence is a flat line: refused here, before any work.
    let has_sound = voice.engine != "none"
        || a.narration.is_some()
        || a.music.as_deref().is_some_and(|m| m != "none")
        || a.music_prompt.is_some()
        || a.music_search.is_some()
        || a.base_sound.as_deref().is_some_and(|m| m != "none")
        || a.sfx
        || a.ambience;
    if a.waveform && !has_sound {
        return fail(
            Exit::Usage,
            "waveform needs sound: this video has no voice and no music".into(),
            Some("Set `tts` to a voice, give `narration` or `music`, or drop `waveform`."),
        );
    }
    match a.base_sound.as_deref() {
        None | Some("none") => {}
        Some("keep" | "background") if a.base_video.is_none() => {
            return fail(
                Exit::Usage,
                "base_sound needs base_video".into(),
                Some("Set base_video, or drop base_sound."),
            )
        }
        Some("keep" | "background")
            if a.music.as_deref().is_some_and(|m| m != "none")
                || a.music_prompt.is_some()
                || a.music_search.is_some() =>
        {
            return fail(
                Exit::Usage,
                "base_sound replaces the music bed".into(),
                Some("Drop music, music_prompt and music_search, or drop base_sound."),
            )
        }
        Some("keep" | "background") => {}
        Some(other) => {
            return fail(
                Exit::Usage,
                format!("unknown base_sound {other}: use none, keep or background"),
                Some("Fix the name in the brief."),
            )
        }
    }
    if a.base_video.is_some() && (a.media.is_some() || a.stock.is_some() || a.scenes.is_some()) {
        return fail(
            Exit::Usage,
            "base_video replaces the other visuals".into(),
            Some("Drop media, stock and scenes, or drop base_video."),
        );
    }
    let open_terms: Vec<String> = a
        .scenes
        .iter()
        .flatten()
        .filter_map(|s| s.open_image.clone())
        .collect();
    // Sources are checked by form only, before any work: nothing is fetched.
    if a.sources.len() > 20 {
        return fail(
            Exit::Usage,
            format!("sources has {} entries, at most 20", a.sources.len()),
            Some("Keep the facts the script states."),
        );
    }
    if let Some(p) = a.sources.iter().find_map(|s| s.problem()) {
        return fail(
            Exit::Usage,
            format!("sources: {p}"),
            Some("Each source needs `text` (the fact) and a `url` or a `title`."),
        );
    }
    if !open_terms.is_empty() || a.music_search.is_some() {
        if a.local_only {
            return needs(voices::needs_input(
                "stock",
                "dependency",
                "A search for a free picture or sound needs the network. Drop open_image and music_search, or run without --local-only.",
                vec![json!({"args": ["--media", "<your clips folder>"], "cost_usd": 0, "note": "your own media, offline"})],
            ));
        }
        if a.music_search
            .as_deref()
            .is_some_and(|t| t.trim().is_empty())
            || open_terms.iter().any(|t| t.trim().is_empty())
        {
            return fail(
                Exit::Usage,
                "open_image or music_search is empty".into(),
                Some("Give the words to search for."),
            );
        }
        if a.music_search.is_some()
            && (a.music_prompt.is_some() || a.music.as_deref().is_some_and(|m| m != "none"))
        {
            return fail(
                Exit::Usage,
                "give one of `music`, `music_prompt` and `music_search`".into(),
                Some("Keep one of them."),
            );
        }
    }
    let scene_stock = a
        .scenes
        .as_ref()
        .is_some_and(|s| s.iter().any(|sc| sc.stock.is_some()));
    let stock_req = a
        .stock
        .clone()
        .or(scene_stock.then(|| "scenes".to_string()));
    let stock_rows: Vec<http_rows::Row> = match &stock_req {
        None => vec![],
        Some(_) if a.local_only => {
            return needs(voices::needs_input(
                "stock",
                "dependency",
                "Stock video needs the network. Drop --stock, or run without --local-only.",
                vec![
                    json!({"args": ["--media", "<your clips folder>"], "cost_usd": 0, "note": "your own media, offline"}),
                ],
            ))
        }
        Some(_) => {
            let rows: Vec<_> = http_rows::rows()
                .into_iter()
                .filter(|r| r.kind == "stock.video" && r.ready())
                .collect();
            if rows.is_empty() && a.stock_picks.is_none() {
                return needs(voices::needs_input(
                    "stock.pexels.api_key",
                    "secret",
                    "Get a free key at pexels.com/api, then run `scriptvox keys set pexels` in your terminal (or pixabay).",
                    vec![
                        json!({"args": [], "cost_usd": 0, "note": "no --stock: the narrated kinetic gradient template, no key needed"}),
                        json!({"args": ["--media", "<your clips folder>"], "cost_usd": 0, "note": "your own clips and photos"}),
                    ],
                ));
            }
            rows
        }
    };
    // A base video: one clip inside the project, played straight through. It replaces every other visual.
    let base_item = match &a.base_video {
        None => None,
        Some(p) => {
            let real = match p.canonicalize() {
                Ok(r) => r,
                Err(_) => {
                    return fail(
                        Exit::Usage,
                        format!("base_video {} does not exist", p.display()),
                        Some("Check the path."),
                    )
                }
            };
            let root = Path::new(".")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from("."));
            if !real.starts_with(&root) {
                return Envelope::fail_code(
                    "make",
                    Exit::Error,
                    "SVX_PATH_OUTSIDE_ROOT",
                    format!("{} is outside this project folder", real.display()),
                    Some("Copy the video into the project.".into()),
                );
            }
            if media::kind_of(&real) != Some(media::Kind::Video) {
                return fail(
                    Exit::Usage,
                    format!("{} is not a video", p.display()),
                    Some("Use .mp4 or .mov."),
                );
            }
            Some(media::Item {
                path: real,
                kind: media::Kind::Video,
                from: 0.0,
                to: None,
            })
        }
    };
    // Dub lines: the times must make sense on this video, and the video sets the length.
    let base_len = base_item.as_ref().and_then(|b| {
        Ffmpeg::locate(a.use_system, false)
            .ok()
            .and_then(|f| f.video_duration(&b.path).ok())
    });
    if !a.lines.is_empty() {
        if let Some(why) = line_problem(&a, base_len) {
            return fail(
                Exit::Usage,
                why,
                Some("Each line needs text and start below end, in time order, without overlap, inside the video."),
            );
        }
        a.duration = a.duration.or(base_len);
    }
    // A generated bed: a music row with a key, planned now, made after the voice sets the length.
    if let Some(c) = a
        .captions
        .as_deref()
        .filter(|c| !["karaoke", "pop", "reveal"].contains(c))
    {
        return fail(
            Exit::Usage,
            format!("unknown caption style {c}"),
            Some("Use \"captions\": \"karaoke\", \"pop\" or \"reveal\"."),
        );
    }
    if let Some((e, fix)) = a.caption_style().check(a.captions.as_deref()) {
        return fail(Exit::Usage, e, Some(fix));
    }
    let music_row = match &a.music_prompt {
        None => None,
        Some(_) if a.music.as_deref().is_some_and(|m| m != "none") => {
            return fail(
                Exit::Usage,
                "give `music` or `music_prompt`, not both".into(),
                Some("Keep the one you want."),
            )
        }
        Some(p) if p.trim().is_empty() => {
            return fail(Exit::Usage, "music_prompt is empty".into(), Some("Describe the bed: style, instruments, mood, tempo."))
        }
        Some(_) if a.local_only => {
            return needs(voices::needs_input(
                "music",
                "dependency",
                "A generated music bed needs the network. Use --music calm, or run without --local-only.",
                vec![json!({"args": ["--music", "calm"], "cost_usd": 0, "note": "built-in bed, offline"})],
            ))
        }
        Some(_) => match crate::generators::all().into_iter().find(|g| {
            g.kind == "music.gen" && !g.unknown_price() && crate::keys::resolve(&g.key).is_some()
        }) {
            Some(g) => Some(g),
            None => {
                return needs(voices::needs_input(
                    "generate.music_row",
                    "secret",
                    "No music row with a key on this install. Use --music calm, or see `scriptvox providers`.",
                    vec![json!({"args": ["--music", "calm"], "cost_usd": 0, "note": "built-in bed"})],
                ))
            }
        },
    };
    let library = a
        .music
        .as_deref()
        .filter(|m| crate::music_lib::is_library(m))
        .and_then(|m| crate::music_lib::find(m).ok());
    let mut music = match a.music.as_deref() {
        None | Some("none") => audio::Music::None,
        // Downloaded after the plan, with the stock files.
        Some(_) if music_url.is_some() => audio::Music::None,
        // A library track: from the store, or downloaded once (free, checked by its sha256). A plan sends no request.
        Some(_) if library.is_some() => match library.as_ref() {
            Some(t) if a.plan_only && !crate::music_lib::cached(t) => audio::Music::None,
            Some(t) => {
                match crate::music_lib::get(t, Path::new(".scriptvox/music"), a.cache_only) {
                    Ok(p) => audio::Music::File(std::path::absolute(&p).unwrap_or(p)),
                    Err(e) => {
                        return fail(
                            Exit::ProviderRetryable,
                            e,
                            Some("Run it again; a download resumes. Or use music calm."),
                        )
                    }
                }
            }
            None => audio::Music::None,
        },
        Some(name) => match audio::pick_music(name) {
            Ok(m) => m,
            Err(audio::MusicRefused::Missing) => {
                return fail(
                    Exit::Usage,
                    format!("no music {name}"),
                    Some("Use --music calm, --music pulse, library:<name> (`scriptvox media music`), or a music file inside this project."),
                );
            }
            Err(audio::MusicRefused::Outside) => {
                return Envelope::fail_code(
                    "make",
                    Exit::Error,
                    "SVX_PATH_OUTSIDE_ROOT",
                    format!("{name} is outside this project folder"),
                    Some("Copy the music file into the project.".into()),
                );
            }
        },
    };
    // The user's own music file: any file with sound (a song, or a video for its sound). A file that cannot be read
    // stops here with its name, not later inside the mix.
    if let (audio::Music::File(p), Ok(ff)) = (&music, Ffmpeg::locate(a.use_system, false)) {
        if !ff.has_stream(p, "audio") || ff.duration(p).is_err() {
            return fail(
                Exit::Usage,
                format!("music {} has no sound that can be read", p.display()),
                Some("Give a sound file (mp3, m4a, wav, flac, ogg) or a video with sound. `scriptvox analyze <file>` shows what a file holds. Or use music `calm` or `pulse`."),
            );
        }
    }
    // `music_from` and `music_to` on that file: seconds or a clock, the start before the end.
    let music_cut = match (&music, &a.music_from, &a.music_to) {
        (audio::Music::File(_), from, to) if from.is_some() || to.is_some() => {
            let time = |t: &Option<String>| t.as_deref().map(crate::fetch::parse_time);
            match (time(from).unwrap_or(Some(0.0)), time(to)) {
                (Some(f), None) => Some((f, None)),
                (Some(f), Some(Some(t))) if t > f => Some((f, Some(t))),
                _ => {
                    return fail(
                        Exit::Usage,
                        "music_from and music_to are seconds (12.5) or a clock (0:12), and music_to is after music_from".into(),
                        Some("For example \"music_from\": 12.5, \"music_to\": \"0:24\"."),
                    )
                }
            }
        }
        _ => None,
    };
    // Scene files: each must exist, be a clip or a photo, and live inside the project.
    let root = Path::new(".")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));
    let mut scene_items: Vec<Option<media::Item>> = Vec::new();
    for (i, sc) in a.scenes.iter().flatten().enumerate() {
        if sc.media.is_none() && (sc.media_from.is_some() || sc.media_to.is_some()) {
            return fail(
                Exit::Usage,
                format!("scenes[{i}].media_from and media_to need a `media` file in that scene"),
                Some("Add `media`, or drop them."),
            );
        }
        // The scene clip's own sound: only for the user's own clip, and only a scene with words has time for it.
        if let Some(mode) = sc.sound.as_deref() {
            let own = sc.media.is_some()
                && sc.stock.is_none()
                && sc.open_image.is_none()
                && sc.generate.is_none();
            let problem = if !matches!(mode, "keep" | "under") {
                Some((
                    format!("unknown sound {mode}: use keep, under or none"),
                    "Fix `sound` in that scene.",
                ))
            } else if !own {
                Some((
                    "sound is for the scene's own `media` clip, not for stock, open_image or generate".to_string(),
                    "Give the scene your own clip as `media`, or drop `sound`.",
                ))
            } else if sc.seconds.is_none() && pauses::strip(&sc.script).is_empty() {
                Some((
                    "sound needs a script or `seconds` in that scene: a scene with neither has no time in the video".to_string(),
                    "Give the scene a short script or `seconds`, or drop `sound`.",
                ))
            } else {
                None
            };
            if let Some((msg, fix)) = problem {
                return fail(Exit::Usage, format!("scenes[{i}].sound: {msg}"), Some(fix));
            }
        }
        scene_items.push(match &sc.media {
            None => None,
            Some(p) => {
                let real = match p.canonicalize() {
                    Ok(r) => r,
                    Err(_) => return fail(
                        Exit::Usage,
                        format!(
                            "scenes[{i}].media {} does not exist",
                            std::path::absolute(p).unwrap_or(p.clone()).display()
                        ),
                        Some("Check the `media` path of that scene: it is relative to the brief."),
                    ),
                };
                if !real.starts_with(&root) {
                    return Envelope::fail_code(
                        "make",
                        Exit::Error,
                        "SVX_PATH_OUTSIDE_ROOT",
                        format!("{} is outside this project folder", real.display()),
                        Some("Copy the file into the project.".into()),
                    );
                }
                let kind = media::kind_of(&real);
                let from = sc.media_from.unwrap_or(0.0);
                let to = sc.media_to;
                let parted = from > 0.0 || to.is_some();
                if parted && kind != Some(media::Kind::Video) {
                    return fail(
                        Exit::Usage,
                        format!("scenes[{i}].media_from and media_to are for a clip"),
                        Some("Drop them for a photo."),
                    );
                }
                if !(from.is_finite() && from >= 0.0) || to.is_some_and(|t| t <= from) {
                    return fail(
                        Exit::Usage,
                        format!("scenes[{i}]: media_from must be 0 or more, and media_to after it"),
                        Some("Give seconds inside the clip."),
                    );
                }
                // A start past the end would show nothing: say so before the voice is made.
                if from > 0.0 {
                    let len = Ffmpeg::locate(a.use_system, false)
                        .ok()
                        .and_then(|f| f.video_duration(&real).ok());
                    if let Some(l) = len.filter(|l| from >= *l) {
                        return fail(
                            Exit::Usage,
                            format!("scenes[{i}].media_from {from} is past the end of the clip ({l:.1} s)"),
                            Some("Pick a time inside the clip."),
                        );
                    }
                }
                if sc.sound.is_some() {
                    let no_sound = Ffmpeg::locate(a.use_system, false)
                        .is_ok_and(|f| !f.has_stream(&real, "audio"));
                    if kind != Some(media::Kind::Video) || no_sound {
                        return fail(
                            Exit::Usage,
                            format!(
                                "scenes[{i}].sound: {} {}",
                                real.display(),
                                if kind == Some(media::Kind::Video) { "has no sound" } else { "is not a clip" }
                            ),
                            Some("Use a clip that has sound, or drop `sound` from that scene."),
                        );
                    }
                }
                match kind {
                    Some(kind) => Some(media::Item {
                        path: real,
                        kind,
                        from,
                        to,
                    }),
                    None => {
                        return fail(
                            Exit::Usage,
                            format!("{} is not a clip or a photo", p.display()),
                            Some("Use .mp4, .mov, .jpg or .png."),
                        )
                    }
                }
            }
        });
    }
    let media = match &a.media {
        None => None,
        Some(d) => match media::list(d, Path::new(".")) {
            Ok(items) => Some(items),
            Err(media::MediaError::OutsideRoot(p)) => {
                return Envelope::fail_code(
                    "make",
                    Exit::Error,
                    "SVX_PATH_OUTSIDE_ROOT",
                    format!("{} is outside this project folder", p.display()),
                    Some("Copy or link the media into the project, or run scriptvox from a folder that contains it.".into()),
                )
            }
            Err(media::MediaError::Missing(p)) => return fail(Exit::Error, format!("media folder {} does not exist", p.display()), Some("Check the --media path.")),
            Err(media::MediaError::Empty(p)) => {
                return fail(Exit::Error, format!("no clips or stills in {}", p.display()), Some("Add .mp4, .mov, .jpg or .png files, or drop --media."))
            }
        },
    };
    let gen = match plan_generated(&a, &lang) {
        Ok(g) => g,
        Err(env) => return *env,
    };
    // The plan: stages, cost and downloads, before any call or download.
    // With --duration a paid voice may be read twice (one rate retry): the plan prices both.
    // A `voice_rate` keeps its pace: one read.
    let reads = if a.duration.is_some() && rate0.is_none() {
        2.0
    } else {
        1.0
    };
    // A paid voice is priced at what this run has to buy: a part that is in the voice cache is no call and no charge
    // (docs/specs/voice-plan-cache.md). Dub lines and a recording keep the full price: no lookup.
    let mut voice_note = String::new();
    // Set when a fit to `duration` may re-read: the run writes there how its fit ended.
    let mut fit_note: Option<String> = None;
    let tts_cost = voice.usd_per_1k_chars.map_or(0.0, |p| {
        let usd = |chars: usize| chars as f64 / 1000.0 * p;
        let full = usd(voiced.chars().count());
        let cents = |x: f64| (x * 100.0).ceil() / 100.0;
        if a.narration.is_some() || !a.lines.is_empty() || voice.engine == "none" {
            return cents(full * reads);
        }
        // With nothing in the cache, the price of a read is the one of every earlier version.
        let buy = |parts: &[VoicePart]| {
            if parts.iter().any(|p| p.cached) {
                usd(parts.iter().filter(|p| !p.cached).map(|p| p.chars).sum())
            } else {
                full
            }
        };
        let parts = voice_parts(&voice, &who, &segs, rate0);
        let (made, all) = (parts.iter().filter(|p| p.cached).count(), parts.len());
        let first = buy(&parts);
        let mut reread = 0.0;
        if let (true, Some(target)) = (reads > 1.0, a.duration) {
            let key = fit_note_key(&parts, &segs, target, spoken.split_whitespace().count());
            // Only a first read that comes whole from the cache gives the same fit again.
            let known = Cas::open().note(&key).filter(|_| made == all && all > 0);
            fit_note = Some(key);
            reread = match known.as_ref().and_then(|n| n.get("rate")) {
                Some(Value::Null) => 0.0,
                Some(Value::Number(pct)) if pct.is_i64() => buy(&voice_parts(
                    &voice,
                    &who,
                    &segs,
                    pct.as_i64().map(|p| p as i32),
                )),
                // The pace of the re-read is not known before the first read is measured (or the note is not one
                // this build wrote): one whole read is priced.
                _ => full,
            };
        }
        if made > 0 {
            voice_note = if made < all {
                format!("; {made} of {all} parts already made: only the rest is priced")
            } else if reread > 0.0 {
                format!(
                    "; already made: no call and no charge for the first read; a re-read at another pace may be needed to reach the duration: up to ${:.2}",
                    cents(reread)
                )
            } else {
                "; already made: no call and no charge".to_string()
            };
        }
        cents(first + reread)
    });
    // A child of `resume` with `scenes`: which scenes get a new voice, from the voice cache. Only where the rate is
    // known before the voice: a kept pace, or no `duration`.
    let mut voiced_again: Option<Vec<usize>> = None;
    let mut kept_note = String::new();
    if let (Some(sc), true) = (
        a.scenes
            .as_ref()
            .filter(|sc| scene_segs(sc, &script, &voice.id, voice_made).is_some()),
        a.parent_run.is_some() && a.duration.is_none() && a.lines.is_empty() && voice_made,
    ) {
        let (again, kept): (Vec<usize>, Vec<usize>) = (1..=sc.len()).partition(|n| {
            let s = &sc[n - 1];
            let who = vec![s.voice.clone().unwrap_or_else(|| voice.id.clone())];
            pauses::parse(&s.script).iter().any(|seg| {
                voice_parts(&voice, &who, std::slice::from_ref(seg), rate0)
                    .iter()
                    .any(|p| !p.cached)
            })
        });
        let list = |v: &[usize]| {
            v.iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        kept_note = match (again.is_empty(), kept.is_empty()) {
            (true, _) => "; every scene keeps its voice: no scene is voiced again".to_string(),
            (false, true) => "; every scene is voiced again".to_string(),
            (false, false) => format!(
                "; voiced again: scene {}; voice kept: scene {}",
                list(&again),
                list(&kept)
            ),
        };
        if let Some(p) = &a.pace {
            kept_note += &format!(
                "; the pace of the parent run is kept (rate {:+} percent, tempo {:.3}), so the length follows the words{}",
                p.rate_pct,
                p.tempo,
                p.duration
                    .map(|d| format!(" and may not be {d} s: `resume --duration {d}` fits all scenes again"))
                    .unwrap_or_default()
            );
        }
        voiced_again = Some(again);
    }
    let mut stages = vec![plan::Stage {
        stage: "voice",
        tool: format!("tts.{}", voice.engine),
        cost_usd: tts_cost,
        network: voice.network,
        explain: match &a.narration {
            Some(f) => format!("your own recording {}: no voice is made", f.display()),
            None => {
                (match rate0 {
                    None => voice.explain(),
                    Some(_) if a.pace.is_some() => voice.explain(),
                    Some(pct) => format!(
                        "{}; pace {pct:+} percent (voice_rate){}",
                        voice.explain(),
                        if a.duration.is_some() {
                            "; voice_rate keeps this pace, so duration is reached by a tempo change only, within 16 percent, or the run stops after the voice"
                        } else {
                            ""
                        }
                    ),
                }) + &voice_note
                    + &kept_note
                    // The length of a made voice is known only after it is read; `tts: none` is checked above.
                    + if a.duration.is_some() && voice.engine != "none" && a.lines.is_empty() {
                        "; the fit to `duration` is checked at make, after the voice is read: a plan cannot measure it"
                    } else {
                        ""
                    }
            }
        },
    }];
    // Frames of the user's own video leave this machine: the plan says where, before they do.
    if let Some(host) = crate::judge::vision_url() {
        let mut vision = |explain: String| {
            stages.push(plan::Stage {
                stage: "vision",
                tool: "vision".into(),
                cost_usd: 0.0,
                network: true,
                explain,
            })
        };
        if a.base_video.is_some() && !a.local_only && !a.cache_only {
            vision(format!(
                "3 frames of the base video go to {host} to look for burned text"
            ));
        }
        // Nothing went at this plan. Picks already kept send nothing more, so they need no notice.
        let picks_open = a
            .scenes
            .as_ref()
            .is_none_or(|s| s.iter().any(crate::match_media::is_open));
        if a.plan_only && a.media_match && matched.is_none() && match_failed.is_none() && picks_open
        {
            vision(format!(
                "`make` sends one frame of each media file (up to 4 from a clip over 12 s) and the script's lines to {host}, to pick a file for each line; `make` lists the picks in media_match. This plan sent nothing"
            ));
        }
    }
    if !open_terms.is_empty() || a.music_search.is_some() {
        stages.push(plan::Stage {
            stage: "stock",
            tool: "open.search (Openverse)".into(),
            cost_usd: 0.0,
            network: true,
            explain: "free search with no key".into(),
        });
    }
    if let Some(url) = &music_url {
        stages.push(plan::Stage {
            stage: "music",
            tool: "fetch".into(),
            cost_usd: 0.0,
            network: true,
            explain: format!(
                "downloads the sound of {url}{} one time and keeps it for a rerun{}{}",
                match (&a.music_from, &a.music_to) {
                    (None, None) => String::new(),
                    (f, t) => format!(
                        " (from {} to {})",
                        f.as_deref().unwrap_or("its start"),
                        t.as_deref().unwrap_or("its end")
                    ),
                },
                if a.cut_on.is_some() {
                    ", before the voice: the cuts follow its beat"
                } else {
                    ""
                },
                crate::fetch::sign_in_note(url)
            ),
        });
    }
    if let Some(t) = &library {
        let here = crate::music_lib::cached(t);
        stages.push(plan::Stage {
            stage: "music",
            tool: "music.library".into(),
            cost_usd: 0.0,
            network: !here,
            explain: format!(
                "the library track \"{}\" by {} ({}, {}){}",
                t.title,
                t.artist,
                t.licence,
                t.source,
                if here {
                    String::new()
                } else {
                    format!(
                        "; downloads {:.1} MB one time and keeps it for a rerun",
                        t.bytes as f64 / 1e6
                    )
                }
            ),
        });
    }
    // The user's own music file: what the mix does with it. No stage for a built-in bed, as before.
    if let (audio::Music::File(p), None) = (&music, &music_url) {
        let cut = match music_cut {
            Some((f, Some(t))) => format!(", from {f} s to {t} s"),
            Some((f, None)) => format!(", from {f} s"),
            None => String::new(),
        };
        let gap = audio::music_gap_db(a.music_level.as_deref()).unwrap_or(10.0);
        stages.push(plan::Stage {
            stage: "music",
            tool: "audio.mix".into(),
            cost_usd: 0.0,
            network: false,
            explain: format!(
                "{}{cut}: it starts again when it is shorter than the video and fades out in the last second; {}; the video is set to {} LUFS",
                match &library {
                    Some(t) => format!("the library track {}", t.name),
                    None => format!("your file {}", p.file_name().unwrap_or_default().to_string_lossy()),
                },
                if has_sound_voice {
                    format!("{gap:.0} dB under the voice, and lower while the voice speaks")
                } else {
                    "it is the only sound".to_string()
                },
                audio::TARGET_LUFS
            ),
        });
    }
    if a.stock.is_some() {
        let names: Vec<String> = stock_rows.iter().map(|r| r.name.clone()).collect();
        stages.push(plan::Stage {
            stage: "stock",
            tool: format!("stock.video ({})", names.join(", ")),
            cost_usd: 0.0,
            network: true,
            explain: format!(
                "free external API, {}",
                if stock_rows.iter().all(|r| r.keyless()) {
                    "no key"
                } else {
                    "your own key, then the rows with no key"
                }
            ),
        });
    }
    // Each character's picture: one call to an image row, before the first scene that shows the character.
    let cast = cast_of(&a);
    for (c, who) in cast.iter().enumerate() {
        let shown = gen.iter().flatten().any(|g| !g.made && g.cast.contains(&c));
        if who.file.is_some() || !shown {
            continue;
        }
        // No row: the run says so with its fix line.
        let Some((row, _, _, made)) = character_pick(&a, who) else {
            continue;
        };
        stages.push(plan::Stage {
            stage: "character",
            tool: format!("image.gen.{}", row.name),
            cost_usd: if made { 0.0 } else { row.cost_for(1.0, 0.0, 0) },
            network: !made && row.gateway != "local",
            explain: if made {
                format!(
                    "the picture of {}: already made, no call and no charge",
                    who.name.as_deref().unwrap_or("the character")
                )
            } else {
                format!(
                    "the picture of {}: one still through {}, {}{}{}",
                    who.name.as_deref().unwrap_or("the character"),
                    row.name,
                    row.cost_words(1.0),
                    row.external_note(),
                    answers_words(&a, &row)
                )
            },
        });
    }
    for (i, g) in gen.iter().enumerate() {
        // The first picture of a clip is a call to an image row of its own. Its price is in the line of the scene.
        if let Some((g, (fr, _))) = g.as_ref().and_then(|g| Some((g, g.frame.as_ref()?))) {
            if !g.made {
                stages.push(plan::Stage {
                    stage: "first_frame",
                    tool: format!("image.gen.{}", fr.name),
                    cost_usd: 0.0,
                    network: fr.gateway != "local",
                    explain: format!(
                        "scene {}: its first picture through {} ({}; the price is in the line of the scene){}{}",
                        i + 1,
                        fr.name,
                        fr.cost_words(1.0),
                        fr.external_note(),
                        answers_words(&a, fr)
                    ),
                });
            }
        }
        if let Some(g) = g {
            stages.push(plan::Stage {
                stage: "generate",
                tool: format!("video.gen.{}", g.row.name),
                cost_usd: g.cost,
                network: g.row.gateway != "local",
                explain: if g.made {
                    format!(
                        "scene {}: already made with {}, no call and no charge",
                        i + 1,
                        g.row.name
                    )
                } else if g.still {
                    format!(
                        "scene {}: a still{} through {}{}{}{}",
                        i + 1,
                        if !g.cast.is_empty() {
                            " with the character"
                        } else {
                            ""
                        },
                        g.row.name,
                        g.row.takes_s.map_or(String::new(), |t| format!(
                            "; takes about {t:.0} s, measured"
                        )),
                        g.row.external_note() + answers_words(&a, &g.row),
                        g.row.seed_note(a.seed)
                    )
                } else {
                    format!(
                        "scene {}: {} s through {} at ${}/s{} ({} price, read {}){}{}{}{}",
                        i + 1,
                        g.seconds,
                        g.row.gateway,
                        g.row.usd_per_s,
                        g.row.shape_words(),
                        g.row.price,
                        g.row.price_checked,
                        g.row.stale_note(),
                        g.row.takes_s.map_or(String::new(), |t| format!(
                            "; takes about {t:.0} s, measured"
                        )),
                        g.row.external_note() + answers_words(&a, &g.row),
                        g.row.seed_note(a.seed)
                    )
                },
            });
        }
    }
    // Steps through free rows: listed, and refused here (not after the voice) when no row can run them.
    // A still is a picture: upscale, smooth and ambience work on clips only.
    let any_gen = gen.iter().flatten().any(|g| !g.still);
    type Fits = fn(&crate::generators::Generator) -> bool;
    let row_steps: [(bool, &str, &str, &str, Fits); 5] = [
        (
            a.upscale && !a.draft && any_gen,
            "upscale",
            "upscale",
            "each generated clip",
            |g| g.kind == "upscale",
        ),
        (
            a.smooth && !a.draft && any_gen,
            "smooth",
            "interpolate",
            "each generated clip, 24 to 48 fps",
            |g| g.kind == "interpolate",
        ),
        (
            a.ambience,
            "ambience",
            "sound-from-video",
            "sound from each video scene",
            is_v2a,
        ),
        (
            a.base_sound.as_deref() == Some("background"),
            "base_sound background",
            "stem",
            "the base video's sound without its voice",
            |g| g.kind == "stems",
        ),
        // The run calls this row in `cut_out`: the plan lists it, and a missing row stops here, before the voice.
        (
            a.scenes
                .iter()
                .flatten()
                .any(|s| s.card.as_ref().is_some_and(|c| c.behind == Some(true))),
            "behind",
            "matte",
            "the subject of each shot with a title behind it, cut out",
            |g| g.kind == "matte",
        ),
    ];
    for (on, field, what, explain, fits) in row_steps {
        if !on {
            continue;
        }
        match pick_row(field, what, fits) {
            Ok(g) => stages.push(plan::Stage {
                stage: field.split(' ').next().unwrap_or(field),
                tool: format!("{}.{}", g.kind, g.name),
                cost_usd: 0.0,
                network: true,
                explain: explain.to_string(),
            }),
            Err(e) => return fail(e.exit, e.message, Some(&e.fix)),
        }
    }
    if let Some(mr) = &music_row {
        // Asked longer than the video: a generated piece may fade out early (seen on ACE-Step, 2 Nov 2026).
        let want = a.duration.unwrap_or(
            fit::spoken_words(&voiced, &lang, &voice.engine) as f64 / pace_wps + pause_s,
        ) + 10.0;
        let s = mr.duration_for(want);
        stages.push(plan::Stage {
            stage: "music",
            tool: format!("music.gen.{}", mr.name),
            cost_usd: mr.cost(s),
            network: true,
            explain: format!(
                "{s} s bed through {} at ${}/s{}",
                mr.gateway,
                mr.usd_per_s,
                mr.stale_note() + &mr.external_note()
            ),
        });
    }
    // The length the plan expects: the asked duration, else the words at the voice's pace and the pauses (with the
    // scenes that have `seconds`).
    let est_secs = a
        .duration
        .unwrap_or(fit::spoken_words(&voiced, &lang, &voice.engine) as f64 / pace_wps + pause_s);
    let est_scenes = a.scenes.as_ref().map_or(1, Vec::len);
    stages.push(plan::Stage {
        stage: "render",
        tool: "engine.ffmpeg".into(),
        cost_usd: 0.0,
        network: false,
        explain: crate::size::explain((w, h), fps, est_secs, est_scenes),
    });
    stages.push(plan::Stage {
        stage: "check",
        tool: "gates".into(),
        cost_usd: 0.0,
        network: false,
        explain: "black, slideshow, silence, loudness, A/V drift".into(),
    });
    if voice_only {
        // The plan, its cost and its approval are for what runs.
        stages.retain(|s| s.stage == "voice");
    }
    let cost: f64 = stages.iter().map(|s| s.cost_usd).sum();
    let pending = provision::pending(a.use_system, true);
    // A LUT counts by its content, like the font and the logo.
    let look_id = a
        .look
        .as_ref()
        .map(|l| crate::store::hash_file(Path::new(l)).unwrap_or_else(|_| l.clone()));
    let mut id_input = json!({
        "script": script, "lang": lang, "voice": [voice.engine, voice.id], "duration": a.duration, "aspect": a.aspect, "fps": fps,
        "media": a.media, "stock": a.stock, "music": a.music,
        "price": voice.usd_per_1k_chars, "model": voice.model, "stock_picks": a.stock_picks,
        "generate": gen.iter().flatten().map(|g| &g.id_input).collect::<Vec<_>>(),
        // The full price of each shot and of the music: an approval is for this bill. Not the store's discount, or a run id would change once its clip is made.
        "prices": gen.iter().flatten().map(|g| g.listed).collect::<Vec<_>>(),
        "music_usd": stages.iter().find(|s| s.stage == "music").map(|s| s.cost_usd),
        "music_prompt": a.music_prompt, "sfx": a.sfx, "upscale": a.upscale, "smooth": a.smooth, "ambience": a.ambience, "accent": a.accent, "background": a.background, "music_level": a.music_level, "font": a.font.as_ref().and_then(|f| crate::store::hash_file(f).ok()), "caption_at": a.caption_at, "narration": a.narration.as_ref().and_then(|f| crate::store::hash_file(f).ok()), "outro": a.outro, "logo": a.logo.as_ref().map(|l| json!({"file": crate::store::hash_file(&l.file).ok(), "corner": l.corner, "width": l.width, "opacity": l.opacity})), "captions": a.captions, "transition": a.transition, "look": look_id, "emphasis": a.emphasis, "scenes": id_scenes, "draft": a.draft, "music_row": music_row.as_ref().map(|m| &m.name), "base_video": a.base_video, "base_sound": a.base_sound,
        "pins": [provision::pin("ffmpeg").map(|p| p.version), provision::pin("uv").map(|p| p.version)],
    });
    if !a.captions_second.is_empty() {
        id_input["captions_second"] = json!(a.captions_second);
    }
    // Only when set: a brief with no `cut_on` keeps its old id.
    if let Some(c) = &a.cut_on {
        id_input["cut_on"] = json!(c);
    }
    // Only a frame size that is not the default: a brief with no `size`, or 1080, keeps its old id.
    if let Some(s) = a.size.filter(|s| *s != crate::size::DEFAULT) {
        id_input["size"] = json!(s);
    }
    // Only a size that changes the picture: a brief with no size, or `normal`, keeps its old id.
    if let Some(s) = a.caption_size.as_deref().filter(|s| *s != "normal") {
        id_input["caption_size"] = json!(s);
    }
    // Only a style that changes the picture: a brief with no `card_style`, or `dark`, keeps its old id.
    if let Some(s) = a.card_style.as_deref().filter(|s| *s != "dark") {
        id_input["card_style"] = json!(s);
    }
    // Only when set: an old brief keeps its id.
    if let Some(o) = a.caption_outline {
        id_input["caption_outline"] = json!(o);
    }
    if let Some(b) = a.caption_box {
        id_input["caption_box"] = json!(b);
    }
    if a.waveform {
        id_input["waveform"] = json!(true);
    }
    // Its own plan and run: the whole video of the same brief is another id, so it never answers for this stop.
    if let Some(u) = &a.until {
        id_input["until"] = json!(u);
    }
    // Only a pace that changes the voice: no field, or 1.0, keeps the old id.
    if let Some(pct) = rate0 {
        id_input["voice_rate"] = json!(pct);
    }
    if let Some(p) = &a.pace {
        id_input["pace"] = json!([p.rate_pct, p.tempo]);
    }
    if !open_terms.is_empty() {
        id_input["open_image"] = json!(open_terms);
    }
    if let Some(m) = &a.music_search {
        id_input["music_search"] = json!(m);
    }
    // Only when given: a brief with no cut keeps its old id.
    if a.music_from.is_some() || a.music_to.is_some() {
        id_input["music_cut"] = json!([a.music_from, a.music_to]);
    }
    let plan = plan::Plan {
        plan_id: plan::plan_id(&id_input),
        cost_usd: cost,
        stages,
        downloads: pending
            .iter()
            .map(|p| json!({"name": p.name, "version": p.version, "bytes": p.size}))
            .collect(),
    };
    // Stills first: its own plan and approval, and nothing of the video is made.
    if a.storyboard {
        return storyboard(&a, &gen, &plan);
    }
    if a.plan_only {
        if let Some(env) = plan::over_cap("plan", &plan.plan_id, plan.cost_usd, a.max_spend, vec![])
        {
            return env.with("stages", json!(plan.stages));
        }
    }
    if a.plan_only {
        let mut env = Envelope::ok("plan");
        env.plan_id = Some(plan.plan_id.clone());
        env.findings.extend(crate::generators::plan_finding());
        env.findings.extend(not_answering_finding());
        env.findings
            .extend(crate::size::memory_warning((w, h), est_secs, est_scenes));
        // A generated clip shorter than its scene holds its last frame, or slows down when it is close.
        let wps = fit::words_per_s(&lang, &voice.engine);
        for (i, g) in gen.iter().enumerate() {
            let scene = a.scenes.as_ref().and_then(|s| s.get(i));
            if let (Some(g), Some(sc)) = (g.as_ref().filter(|g| !g.still), scene) {
                let need = sc.seconds.unwrap_or(
                    fit::spoken_words(&pauses::strip(&sc.script), &lang, &voice.engine) as f64
                        / wps,
                );
                if media::stretch((g.seconds, 24.0), need).is_none() && g.seconds < need {
                    env.findings.push(json!({"gate": "clips", "status": "warn",
                        "detail": format!("scene {}: the clip runs {} s, the narration about {need:.0} s: the clip holds its last frame for the rest", i + 1, g.seconds),
                        "fix": "Split the scene, shorten its words, give it `smooth: true`, or use `still: true`: a picture lasts as long as its scene."}));
                }
            }
        }
        let said: Vec<Word> = spoken
            .split_whitespace()
            .map(|t| Word {
                text: t.into(),
                start: 0.0,
                end: 0.0,
            })
            .collect();
        let (_, missing) = crate::captions::find_emphasis(&said, &a.emphasis);
        if !missing.is_empty() {
            env.findings.push(json!(emphasis_warning(&missing)));
        }
        env.findings
            .extend(picture_shape_warnings(&a).iter().map(|f| json!(f)));
        for (i, l) in a.lines.iter().enumerate() {
            let est = fit::line_seconds(&pauses::strip(&l.text), &lang, &voice.engine);
            let room = line_room(&a.lines, i, base_len);
            if let Err(need) = fit::fit_line(est, l.end - l.start, room) {
                let chars = l.text.chars().count();
                env.findings.push(json!({"gate": "line_fit", "status": "warn",
                    "detail": format!("line {} is estimated at {need:.2}x speed, and the limit is {}x", i + 1, fit::LINE_MAX),
                    "fix": format!("Cut it to about {} characters.", fit::line_budget(chars, est, room))}));
            }
        }
        if let Some(target) = a.duration.filter(|_| a.lines.is_empty()) {
            let words_n = fit::spoken_words(&voiced, &lang, &voice.engine);
            let rate_free = fit::takes_rate(&voice.engine) && rate0.is_none();
            let or_rate = if rate0.is_some() {
                ", or drop voice_rate"
            } else {
                ""
            };
            let fix = match fit::plan_warn(words_n, pause_s, target, pace_wps, rate_free) {
                Some(fit::Fit::TooShort { ratio, add_words }) => Some((
                    ratio,
                    format!("Add about {add_words} words, or lower duration{or_rate}."),
                )),
                Some(fit::Fit::TooLong { ratio, cut_words }) => Some((
                    ratio,
                    format!("Cut about {cut_words} words, or raise duration{or_rate}."),
                )),
                _ => None,
            };
            if let Some((ratio, fix)) = fix {
                let why = if rate0.is_some() {
                    "voice_rate keeps the voice's pace, so it fits only within 16 percent and make would likely stop after the voice".to_string()
                } else if voice.engine == "none" {
                    format!(
                        "the words would show at about {:+.0} percent of reading pace",
                        (ratio - 1.0) * 100.0
                    )
                } else if rate_free {
                    // The same number the fitter gives the voice (`voice_rate_pct` after make).
                    format!(
                        "the voice would read at about {:+.0} percent speed to fit; it may sound {}",
                        (ratio - 1.0) * 100.0,
                        if ratio < 1.0 { "dragged" } else { "rushed" }
                    )
                } else {
                    "the open voice takes no speaking rate and fits only within 16 percent, so make would likely stop after the voice".to_string()
                };
                env.findings.push(
                    json!({"gate": "fit", "status": "warn", "fix": fix, "detail": format!(
                    "the narration is estimated at {ratio:.2}x the {target:.1} s target; {why}")}),
                );
            }
        }
        if cost > 0.0 {
            env.next = vec![format!("scriptvox make ... --approve {}", plan.plan_id)];
        }
        env.metrics.insert("cost_usd".into(), json!(cost));
        if let Some(v) = &voiced_again {
            env = env.with("voiced_again", json!(v));
        }
        if let Some(w) = foreign_warning(&a, &lang, &spoken) {
            env.findings.push(json!(w));
        }
        env.findings.extend(cue_warnings.iter().map(|w| json!(w)));
        if let Some(w) = &match_failed {
            env.findings.push(json!(w));
        }
        if let Some(m) = matched {
            env = env.with("media_match", m);
        }
        if a.draft {
            // What the final adds; the agent can tell a draft's fault from a real one.
            let mut left = vec![format!("half size ({w}x{h}), 15 fps")];
            if will_listen(&a, &voice) {
                left.push("listening check (gate words)".to_string());
            }
            if a.upscale && any_gen {
                left.push("upscale".into());
            }
            if a.smooth && any_gen {
                left.push("smooth".into());
            }
            env = env.with("draft_leaves_out", json!(left));
        }
        return env
            .with("stages", json!(plan.stages))
            .with("downloads", json!(plan.downloads));
    }
    if let Some(env) = approval_stop(
        &plan.plan_id,
        cost,
        a.approve.as_deref(),
        a.max_spend,
        json!(plan.stages),
    ) {
        return env;
    }
    // Idempotency: the same plan and key is the same run.
    // A plan with generated scenes or paid music always gets a run id from the plan, so a rerun finds its provider jobs
    // and never pays twice.
    let has_gen = gen.iter().any(Option::is_some)
        || plan
            .stages
            .iter()
            .any(|s| s.stage == "music" && s.cost_usd > 0.0);
    let run_id = a.run_id.clone().or_else(|| {
        a.idempotency_key
            .as_deref()
            .or(has_gen.then_some("make"))
            .map(|k| plan::run_id_for(&plan.plan_id, k))
    });
    if let Some(id) = &run_id {
        if let Some(prev) =
            std::fs::read_to_string(Path::new(".scriptvox/runs").join(id).join("result.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            if crate::supervise::replayable(Path::new("."), &prev) {
                let mut env = crate::supervise::replay("make", prev);
                env.plan_id = Some(plan.plan_id.clone());
                return env.with("idempotent", json!(true));
            }
        }
    }
    let bytes: u64 = pending.iter().map(|p| p.size).sum();
    if bytes > provision::AGENT_DOWNLOAD_LIMIT && crate::agent_mode::from_env() {
        let names: Vec<String> = pending
            .iter()
            .map(|p| format!("{} {} ({} MB)", p.name, p.version, p.size / 1_000_000))
            .collect();
        return needs(voices::needs_input(
            "tools",
            "download",
            &format!(
                "ScriptVox must download {} MB first: {}. Run `scriptvox setup` once in a terminal.",
                bytes / 1_000_000,
                names.join(", ")
            ),
            vec![
                json!({"args": ["--use-system"], "cost_usd": 0, "note": "use the FFmpeg and uv already on this machine"}),
            ],
        ));
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("make", e),
    };
    let run = match &run_id {
        Some(id) if Path::new(".scriptvox/runs").join(id).is_dir() => {
            Run::attach(Path::new("."), id)
        }
        Some(id) => Run::create_with_id(Path::new("."), "make", id),
        None => Run::create(Path::new("."), "make"),
    };
    let run = match run {
        Ok(r) => r,
        Err(e) => {
            return fail(
                Exit::Error,
                format!("cannot create the run folder: {e}"),
                None,
            )
        }
    };
    let (id, dir) = (run.id.clone(), run.dir.clone());
    let saved = save_request(&dir, &a);
    let milestone = |msg: &str| {
        eprintln!("scriptvox: {msg}");
        run.event("progress", json!({"explain": msg}));
    };
    // Every return below lands in result.json, events.jsonl and state.db.
    let env = (|| -> Envelope {
        // A foreground run stops when the caller's shell stops it (an agent's shell limit). Say so in the first second.
        if a.run_id.is_none() {
            milestone(&format!("{id}: runs in this command. If your shell stops it: `scriptvox resume {id}`. For a long run add `--detach`, then `scriptvox wait <run_id>`"));
        }
        milestone(&format!("{id}: {}", voice.explain()));
        // `cut_on` with music from an address: the beats come from that file, so it is fetched first, before the
        // voice. A download that fails then costs nothing. The same call as the later one: same store, same credit.
        // The cut of the user's own music file (`music_from`, `music_to`): one bounded pass into the run folder, so
        // the beat grid, the loop and the fade all work on the cut.
        if let (audio::Music::File(src), Some((from, to))) = (&music, music_cut) {
            let cut = dir.join("music-cut.wav");
            let mut args: Vec<String> = vec!["-ss".into(), format!("{from:.3}")];
            if let Some(to) = to {
                args.extend(["-t".into(), format!("{:.3}", to - from)]);
            }
            args.extend(["-i".into(), src.to_string_lossy().into_owned()]);
            args.extend(["-vn", "-ac", "2", "-ar", "48000"].map(String::from));
            args.push(cut.to_string_lossy().into_owned());
            let made =
                ff.run(&dir, &args, None).is_ok() && ff.duration(&cut).is_ok_and(|d| d > 0.05);
            if !made {
                let mut env = fail(
                    Exit::Usage,
                    format!(
                        "music_from {from} is past the end of {}, or the file has no sound there",
                        src.display()
                    ),
                    Some("Give music_from and music_to inside the file: `scriptvox analyze <file>` shows its length."),
                );
                env.run_id = Some(id.clone());
                return env;
            }
            music = audio::Music::File(cut);
        }
        let mut music_first: Option<StockPick> = None;
        if let (Some(url), true) = (&music_url, a.cut_on.is_some() && !voice_only) {
            milestone(&format!(
                "{id}: getting the music from its address first: the cuts follow its beat"
            ));
            match crate::fetch::music(
                url,
                a.music_from.as_deref(),
                a.music_to.as_deref(),
                &dir,
                a.cache_only,
                &ff,
            ) {
                Ok((path, p)) => {
                    music = audio::Music::File(path);
                    music_first = Some(p);
                }
                Err(f) => {
                    let mut env = Envelope::fail_code("make", f.exit, f.code, f.message, f.fix);
                    env.run_id = Some(id.clone());
                    env.plan_id = Some(plan.plan_id.clone());
                    return env;
                }
            }
        }

        // Voice: the worker writes the audio and words.json into the run folder.
        // Paid tools ask `ledger.reserve` before any HTTP call; the cap is enforced here, in Rust.
        // Without an approval of this plan the voice may spend the plan's cost and no more: `--max-spend` alone is
        // not an approval. So a part the plan found in the cache that cannot be read back is never bought unasked.
        // The cap of the run: what this plan asks, plus what the run has paid before. A rerun's plan holds only the parts
        // that are still to make, and the ledger counts the whole run: without the second term the next paid part of a
        // rerun was refused (seen 13 Nov 2026: two clips of $0.34, the rerun's cap was $0.34 with $0.34 spent).
        let run_cost = plan.cost_usd + plan::spent(&id);
        let approved = a.approve.as_deref() == Some(plan.plan_id.as_str());
        let ledger = plan::Ledger::new(&id, a.max_spend.filter(|_| approved).unwrap_or(run_cost));
        let synth_at = |rate: Option<i32>,
                        tempo: f64,
                        segs: &[Seg],
                        who: &[String],
                        dir: &Path|
         -> Result<(PathBuf, Vec<Word>), Box<Envelope>> {
            let made = match &a.narration {
                Some(file) => narration_voice(&ff, file, segs, dir, &lang).map_err(|m| RpcError {
                    code: -32050,
                    message: m,
                }),
                None => synthesize(
                    &ff,
                    &voice,
                    who,
                    segs,
                    dir,
                    a.local_only,
                    rate,
                    tempo,
                    &ledger,
                    a.cache_only,
                ),
            };
            made.map_err(|e| {
                Box::new(match e.code {
                    CACHE_MISS => Envelope::fail_code("make", Exit::Error, "SVX_CACHE_MISS", e.message, Some("Use `scriptvox make --from <video>` to make it again with provider calls.".into())),
            plan::RESERVE_REFUSED => Envelope::fail_code(
                        "make",
                        Exit::ApprovalNeeded,
                        "SVX_SPEND_CAP",
                        e.message,
                        Some("Raise --max-spend, or approve a cheaper plan.".into()),
                    ),
                    PROVIDER_REFUSED => needs(voices::needs_input(
                        "tts.voice",
                        "dependency",
                        &format!("{}. Pick a voice that speaks '{lang}'.", e.message),
                        voices::refused_alternatives(&lang, &voice.engine),
                    )),
                    PROVIDER_RETRYABLE => {
                        let mut env = fail(
                            Exit::ProviderRetryable,
                            e.message,
                            Some("Run the same command again."),
                        );
                        env.run_id = Some(id.clone());
                        env
                    }
                    _ => fail(
                        Exit::Error,
                        e.message,
                        Some("Run `scriptvox doctor` (or `scriptvox worker`) to check the worker."),
                    ),
                })
            })
        };
        let synth = |rate: Option<i32>, tempo: f64| synth_at(rate, tempo, &segs, &who, &dir);
        // The tempo change goes on each speech part before the join, not on the joined file: a part then gives the
        // same samples in a child run that changes another part (docs/specs/resume-scene-voice.md).
        let by_part = voice.engine != "none"
            && a.narration.is_none()
            && !matches!(segs.as_slice(), [Seg::Speech(_)]);
        let pin_tempo = a.pace.as_ref().map_or(1.0, |p| p.tempo);
        let mut line_report = None;
        let first = if a.lines.is_empty() {
            synth(rate0, pin_tempo)
        } else {
            let len = base_len.or_else(|| {
                base_item
                    .as_ref()
                    .and_then(|b| ff.video_duration(&b.path).ok())
            });
            line_voice(&ff, &a.lines, len, &voice, &dir, &|r, s, w, d| {
                synth_at(r, 1.0, s, w, d)
            })
            .map(|(wav, ws, report)| {
                line_report = Some(report);
                (wav, ws)
            })
        };
        let (mut audio, mut events) = match first {
            Ok(r) => r,
            Err(env) => return *env,
        };
        let mut explain_fit = match &a.pace {
            Some(p) => Some(format!(
                "pace kept from the parent run: TTS rate {:+}%, atempo {:.3}",
                p.rate_pct, p.tempo
            )),
            None => rate0.map(|p| format!("pace: voice_rate {p:+}%")),
        };
        // The pace of the voice as numbers for a reviewer: TTS rate in percent (brief `voice_rate`, or the fit's
        // re-read), then atempo.
        let (mut fit_rate_pct, mut fit_tempo) = (rate0.unwrap_or(0), pin_tempo);
        let write_pace = |p: &Pace| {
            let _ = std::fs::write(
                dir.join(PACE_FILE),
                serde_json::to_string(p).unwrap_or_default(),
            );
        };
        if let Some(p) = &a.pace {
            write_pace(p);
        }
        if let Some(target) = a.duration.filter(|_| a.lines.is_empty() && !no_words) {
            let words_n = spoken.split_whitespace().count();
            let actual = ff.duration(&audio).unwrap_or(0.0);
            let wps = fit::words_per_s(&lang, &voice.engine);
            // A voice without a rate, or with the pace the brief set, is not read again.
            let rate_free = fit::takes_rate(&voice.engine) && rate0.is_none();
            // The pace is one rate and one tempo for every part: a child of `resume` can keep it.
            let mut pace_known = by_part;
            let mut stopped = None;
            let fitted = fit::ladder(
                actual,
                target,
                words_n,
                wps,
                rate_free,
                fit::rate_limits(&voice.engine),
                &mut |pct| {
                    // Before the re-read: a run that stops there still leaves the true pace for the next plan.
                    if let Some(key) = &fit_note {
                        let _ = Cas::open().put(key, "voice.fit", "1", &[], json!({"rate": pct}));
                    }
                    milestone(&format!(
                        "{id}: narration {actual:.1} s for {target:.1} s, speaking rate {pct:+}%"
                    ));
                    match synth(Some(pct), 1.0) {
                        Ok(r) => {
                            (audio, events) = r;
                            Some(ff.duration(&audio).unwrap_or(0.0))
                        }
                        Err(env) => {
                            stopped = Some(env);
                            None
                        }
                    }
                },
            );
            let Some((reread, decision)) = fitted else {
                return *stopped.expect("the ladder stops only when the re-read failed");
            };
            if let Some(pct) = reread {
                explain_fit = Some(format!("fit: TTS rate {pct:+}%"));
                fit_rate_pct = pct;
            }
            if let (None, Some(key)) = (reread, &fit_note) {
                let _ = Cas::open().put(key, "voice.fit", "1", &[], json!({"rate": null}));
            }
            match decision {
                fit::Fit::Keep => {}
                fit::Fit::Tempo(f) => {
                    // Every part is in the cache now: join them again, each at the new tempo. From the cache only,
                    // so this step can never buy a voice; if a part is not there, the joined file is changed.
                    let reused = VOICE_REUSED.load(std::sync::atomic::Ordering::Relaxed);
                    let parts = by_part
                        .then(|| {
                            synthesize(
                                &ff,
                                &voice,
                                &who,
                                &segs,
                                &dir,
                                a.local_only,
                                reread.or(rate0),
                                f,
                                &ledger,
                                true,
                            )
                            .ok()
                        })
                        .flatten();
                    VOICE_REUSED.store(reused, std::sync::atomic::Ordering::Relaxed);
                    if let Some(r) = parts {
                        (audio, events) = r;
                    } else {
                        pace_known = false;
                        let fitted = dir.join("voice_fit.wav");
                        let args: Vec<String> = vec![
                            "-i".into(),
                            audio.to_string_lossy().into(),
                            "-af".into(),
                            fit::atempo_chain(f),
                            fitted.to_string_lossy().into(),
                        ];
                        if let Err(e) = ff.run(&dir, &args, None) {
                            return fail(Exit::Error, e, None);
                        }
                        events = events
                            .into_iter()
                            .map(|w| Word {
                                start: w.start / f,
                                end: w.end / f,
                                ..w
                            })
                            .collect();
                        audio = fitted;
                    }
                    fit_tempo = f;
                    explain_fit = Some(format!(
                        "{}atempo {f:.3}",
                        explain_fit
                            .map(|s| s + ", ")
                            .unwrap_or_else(|| "fit: ".into())
                    ));
                }
                fit::Fit::Resynth(_) => unreachable!("handled above"),
                f => {
                    let mut env =
                        fit_failure(f, target, "measured after TTS; nothing was rendered");
                    if let (Some(_), Some(e)) = (rate0, env.error.as_mut()) {
                        e.fix = e.fix.take().map(|f| {
                            format!("{f} Or drop voice_rate: it keeps the voice's pace, so only a tempo change within 16 percent is left to reach the duration.")
                        });
                    }
                    env.run_id = Some(id);
                    return env;
                }
            }
            if pace_known {
                write_pace(&Pace {
                    rate_pct: fit_rate_pct,
                    tempo: fit_tempo,
                    duration: Some(target),
                });
            }
        }

        // Captions: script words on TTS times, broken by measured width.
        let words = align(&spoken, &events);
        let (family, font, size, mut pages, mut style) = caption_pages(
            &words,
            &lang,
            (w, h),
            a.captions.as_deref().unwrap_or("karaoke"),
            crate::captions::size_scale(a.caption_size.as_deref()).unwrap_or((1, 1)),
        );
        // The second line (brief `captions_second`): wrapped at its own size, under the main captions, which move up.
        let second = (!a.captions_second.is_empty()).then(|| {
            second_layout(
                &a.captions_second,
                a.font.is_some().then_some((family, font)),
                font,
                size,
                w,
            )
        });
        match a.caption_at.as_deref() {
            // The bottom of the page at 62 percent of the height: under a face, above the platform's buttons.
            Some("middle") => style.margin_v = h * 38 / 100,
            // Under the status bar and a logo.
            Some("top") => {
                style.alignment = 8;
                style.margin_v = h * 12 / 100;
            }
            _ => {}
        }
        let base_margin = style.margin_v;
        if let Some((_, size2, lines)) = &second {
            style.margin_v += second_raise(lines, *size2, style.size);
        }
        // Where the narration's picture ends: the audio plus its tail, or the asked duration. The end card starts
        // here. The last caption page and the last scene card stay until then, so the frames before the end card
        // are not bare background (seen 4 Nov 2026: about 0.4 s of empty frame before the outro).
        // A video that ends on a scene with `seconds` ends there: that time is exact, with no tail.
        let ends_timed = no_words
            || a.scenes
                .as_ref()
                .and_then(|s| s.last())
                .is_some_and(|s| s.seconds.is_some());
        let speech_end = match a.duration {
            Some(target) => target,
            None => {
                ff.duration(&audio)
                    .unwrap_or_else(|_| words.last().map_or(1.0, |w| w.end))
                    + if ends_timed { 0.0 } else { TAIL_S }
            }
        };
        // Not through a long time with no words at the end (a pause tag, a scene with `seconds`): the page leaves
        // after its words, as it does at a long pause in the middle.
        let hold_to = match (segs.last(), words.last()) {
            (Some(Seg::Pause(p)), Some(w)) if *p > 1.0 => (w.end + TAIL_S).min(speech_end),
            _ => speech_end,
        };
        if let Some(p) = pages.last_mut() {
            p.end = p.end.max(hold_to);
        }
        // `cut_on: beat`: the grid of the bed, read once; the report is filled where the picture cuts are made.
        let beat_grid = a
            .cut_on
            .is_some()
            .then(|| crate::beats::Grid::of_bed(&ff, &dir, &music, speech_end + 2.0));
        let mut cut_on_report: Option<serde_json::Value> = None;
        let mut sfx_cues: Vec<(f64, crate::audio::Sfx)> = Vec::new();
        // When each scene starts, for the text bounds gate.
        let mut scene_starts: Vec<f64> = Vec::new();
        // Title cards that stand behind the subject: their time windows, drawn from `behind.ass`.
        let mut behind: Vec<(f64, f64)> = Vec::new();
        let (found, missing) = crate::captions::find_emphasis(&words, &a.emphasis);
        style.emphasis = found;
        // A dialogue: the words of every scene that is not the first voice.
        if let Some(sc) = a
            .scenes
            .as_ref()
            .filter(|s| s.iter().any(|x| x.voice.is_some()))
        {
            let first = sc[0].voice.as_deref().unwrap_or(voice.id.as_str());
            let mut at = 0;
            for s in sc {
                let n = pauses::strip(&s.script).split_whitespace().count();
                if s.voice.as_deref().unwrap_or(voice.id.as_str()) != first {
                    style.second_speaker.extend(at..(at + n).min(words.len()));
                }
                at += n;
            }
        }
        // Brief `card_style: paper`: the cards and the hook of a document for a light page (`card_style.rs`).
        let on_paper = crate::card_style::is_paper(a.card_style.as_deref());
        let paper = |ass: String| match on_paper {
            true => crate::card_style::paper(&ass, a.accent.as_deref()),
            false => ass,
        };
        let written = (|| {
            let ass = to_ass(&pages, &words, &style);
            let ass = crate::caption_style::apply(&ass, a.caption_outline, a.caption_box, w.min(h));
            let ass = match &second {
                Some((fam2, size2, lines)) => crate::captions::with_second(
                    &ass,
                    &second_cues(&words, &pages, lines),
                    fam2,
                    *size2,
                    base_margin,
                ),
                None => ass,
            };
            let ass = match &a.hook {
                Some(hook) if !hook.trim().is_empty() => {
                    hook_ass(&ass, hook, family, font, size, w)
                }
                _ => ass,
            };
            // Scene cards, timed like the scene cuts: from the scene's first word to the next scene's.
            let mut card_events = Vec::new();
            let mut behind_events = Vec::new();
            // Each card's window and a time it has landed, for `snapshot --cards`.
            let mut card_times: Vec<serde_json::Value> = Vec::new();
            sfx_cues.clear();
            behind.clear();
            if let Some(scenes) = &a.scenes {
                let mut starts = starts_of(scenes, &words, 0.0, speech_end);
                // The words of a scene are its own by their times; the picture, the card and the whoosh follow
                // the cut, which `cut_on` may have moved.
                let word_starts = starts.clone();
                if let Some(g) = &beat_grid {
                    starts = g.snap(&starts, speech_end).0;
                }
                scene_starts.clone_from(&starts);
                let last = speech_end;
                let wrap = |t: &str, s: u32, max: f32| {
                    let m = Measurer::new(font.to_vec(), s as f32).expect("embedded font parses");
                    crate::captions::wrap(t, max, |x| m.width(x))
                };
                let frame = crate::cards::Frame { w, h, size };
                if a.sfx {
                    sfx_cues.extend(
                        starts
                            .iter()
                            .skip(1)
                            .map(|&t| (t, crate::audio::Sfx::Whoosh)),
                    );
                }
                // A card waits for the hook to go (seen 4 Nov 2026: a code card half under the hook at 0.8 s).
                let hook_end = if a.hook.as_deref().is_some_and(|h| !h.trim().is_empty()) {
                    HOOK_S
                } else {
                    0.0
                };
                for (i, sc) in scenes.iter().enumerate() {
                    let list: Vec<&crate::cards::Card> = crate::scene_cards::of(sc).collect();
                    if !list.is_empty() {
                        let end = starts.get(i + 1).copied().unwrap_or(last);
                        let word_end = word_starts.get(i + 1).copied().unwrap_or(last);
                        let said: Vec<_> = words
                            .iter()
                            .filter(|w| w.start >= word_starts[i] && w.start < word_end)
                            .cloned()
                            .collect();
                        let from = starts[i].max(hook_end).min(end - 0.5).max(starts[i]);
                        // `on`, `at` and `hold` move a card inside its scene; without them these are the same times.
                        // The cards of a `cards` list take turns (`scene_cards.rs`).
                        let wins =
                            crate::scene_cards::windows(i + 1, &list, starts[i], from, end, &said)?;
                        for (j, (c, (from, end))) in list.into_iter().zip(wins).enumerate() {
                            // A list item said after the card has gone is not waited for.
                            let said: Vec<_> =
                                said.iter().filter(|w| w.start < end).cloned().collect();
                            let mut ev = crate::cards::events(c, from, end, &frame, &wrap, &said);
                            // Marks on a phrase of a text card (`card_mark.rs`); the card's size is in each event.
                            let marks = crate::card_mark::events(c, &ev, font, size, &said)
                                .map_err(|e| {
                                    std::io::Error::other(format!("scene {}: {e}", i + 1))
                                })?;
                            ev.extend(marks);
                            let mut row = json!({"scene": i + 1, "kind": c.kind, "start": from, "end": end,
                            "landed": (crate::cards::landed_in(c, &ev, from, end) * 100.0).round() / 100.0});
                            if sc.cards.is_some() {
                                row["card"] = json!(j + 1);
                            }
                            card_times.push(row);
                            if c.behind == Some(true) {
                                // The title shows for at most 2.5 s (cards::events).
                                behind.push((from, end.max(from + 0.5).min(from + 2.5)));
                                behind_events.extend(ev);
                            } else {
                                card_events.extend(ev);
                            }
                        }
                    }
                }
            }
            if !behind_events.is_empty() {
                // Same header and styles, no caption lines: only the titles under the subject.
                let head: String = ass
                    .lines()
                    .filter(|l| !l.starts_with("Dialogue:"))
                    .map(|l| format!("{l}\n"))
                    .collect();
                std::fs::write(
                    dir.join("behind.ass"),
                    paper(crate::cards::behind_style(&crate::cards::insert(
                        &head,
                        family,
                        size,
                        &behind_events,
                    ))),
                )?;
            }
            // Brief `card_style: paper`, before the brief's accent takes the place of the default one.
            let mut ass = paper(crate::cards::insert(&ass, family, size, &card_events));
            if let Some(c) = a.accent.as_deref().and_then(crate::captions::ass_colour) {
                ass = ass.replace(crate::captions::ACCENT, &c);
            }
            std::fs::write(dir.join("captions.ass"), ass)?;
            std::fs::write(dir.join("captions.srt"), to_srt(&pages, &words))?;
            std::fs::write(
                dir.join("captions.vtt"),
                crate::captions::to_vtt(&pages, &words),
            )?;
            // The final word times, after any fit: chapters read these (the voice's own file holds the times before the
            // tempo change: chapters came 1.07 s late, fresh-agent test 3).
            std::fs::write(
                dir.join("words.json"),
                serde_json::to_string(&words).unwrap_or_default(),
            )?;
            if !scene_starts.is_empty() {
                std::fs::write(dir.join("scenes.json"), json!(scene_starts).to_string())?;
            }
            if !card_times.is_empty() {
                std::fs::write(
                    dir.join("cards.json"),
                    serde_json::Value::from(card_times).to_string(),
                )?;
            }
            write_fonts(&dir)
        })();
        let fc = match written {
            Ok(fc) => fc,
            Err(e) => {
                // Cards of one scene that do not fit their times: a fault of the brief, with its fix.
                if let Some(r) = e
                    .get_ref()
                    .and_then(|x| x.downcast_ref::<crate::scene_cards::Refusal>())
                {
                    return fail(Exit::Usage, r.msg.clone(), Some(&r.fix));
                }
                return fail(Exit::Error, format!("cannot write captions: {e}"), None);
            }
        };
        milestone(&format!(
            "{id}: {} words, {} caption pages",
            words.len(),
            pages.len()
        ));

        // Render.
        let secs = speech_end;
        // Whole frames: the video track is frame-exact, so the audio is cut to the
        // same length (an off-grid length gave a drift above one frame).
        // The end card adds its seconds after the narration.
        let outro_at = a
            .outro
            .as_ref()
            .map(|_| (secs * f64::from(fps)).round() / f64::from(fps));
        let secs = secs + a.outro.as_ref().map_or(0.0, |o| o.seconds.unwrap_or(2.5));
        let secs = (secs * f64::from(fps)).round() / f64::from(fps);
        if let (Some(o), Some(at)) = (&a.outro, outro_at) {
            if let Some(t) = o.text.as_deref() {
                // The line under the logo: a title card event at 64 percent of the height (60 without a logo).
                let card = crate::cards::Card {
                    kind: crate::cards::Kind::Title,
                    text: Some(t.into()),
                    label: None,
                    items: None,
                    values: None,
                    behind: None,
                    then: None,
                    on: None,
                    at: None,
                    ease: None,
                    stagger: None,
                    from: None,
                    hold: None,
                    mark: None,
                    y: None,
                    grid: None,
                    trace: None,
                    rows: None,
                    points: None,
                    nodes: None,
                    edges: None,
                };
                let (family, font, size, _, _) =
                    caption_pages(&[], &lang, (w, h), "karaoke", (1, 1));
                let wrap = |t: &str, s: u32, max: f32| {
                    let m = Measurer::new(font.to_vec(), s as f32).expect("embedded font parses");
                    crate::captions::wrap(t, max, |x| m.width(x))
                };
                let y = if a.logo.is_some() { 0.64 } else { 0.5 };
                let ev: Vec<String> = crate::cards::events(
                    &card,
                    at + 0.3,
                    secs,
                    &crate::cards::Frame { w, h, size },
                    &wrap,
                    &[],
                )
                .into_iter()
                .map(|e| crate::cards::until(&crate::cards::move_to(&e, h as f32 * y), secs))
                // The file has the brief's accent by now, so only the new lines and the styles are changed.
                .map(|e| match on_paper {
                    true => crate::card_style::event(&e, a.accent.as_deref()),
                    false => e,
                })
                .collect();
                let p = dir.join("captions.ass");
                let ass = std::fs::read_to_string(&p).unwrap_or_default();
                let ass = crate::cards::insert(&ass, family, size, &ev);
                let _ = std::fs::write(
                    &p,
                    match on_paper {
                        true => crate::card_style::styles(&ass),
                        false => ass,
                    },
                );
            }
        }
        if voice_only {
            // `--until voice`: the audio, the fit and the caption files are done. Nothing after this line ran: no
            // clip, stock, music, render or gate.
            let mut env = Envelope::ok("make");
            env.run_id = Some(id.clone());
            for (k, p) in [
                ("audio", audio.clone()),
                ("words", dir.join("words.json")),
                ("captions_srt", dir.join("captions.srt")),
                ("captions_vtt", dir.join("captions.vtt")),
                ("captions_ass", dir.join("captions.ass")),
            ] {
                env.artifacts.insert(k.into(), json!(p));
            }
            for (k, v) in [
                ("until", json!("voice")),
                ("duration_s", json!((secs * 100.0).round() / 100.0)),
                (
                    "voice_s",
                    json!((ff.duration(&audio).unwrap_or(0.0) * 100.0).round() / 100.0),
                ),
                ("words", json!(words.len())),
                ("caption_pages", json!(pages.len())),
                ("cost_usd", json!(plan::spent(&id))),
            ] {
                env.metrics.insert(k.into(), v);
            }
            if let Some(r) = line_report.take() {
                env.metrics.insert("lines".into(), r);
            }
            if voice.engine != "none" {
                env.metrics
                    .insert("voice_rate_pct".into(), json!(fit_rate_pct));
                env.metrics.insert(
                    "voice_tempo".into(),
                    json!((fit_tempo * 1000.0).round() / 1000.0),
                );
            }
            if VOICE_REUSED.load(std::sync::atomic::Ordering::Relaxed) {
                env.metrics.insert("voice_reused".into(), json!(true));
            }
            env.next = vec![
                "Listen to artifacts.audio and read artifacts.captions_srt.".to_string(),
                format!("To make the video with this voice: scriptvox resume {id} --wait --json"),
            ];
            let mut explain = vec![voice.explain()];
            explain.extend(explain_fit);
            explain.push(
                "stopped after the voice (--until voice): no clip, stock, music, render or gate ran"
                    .to_string(),
            );
            return env
                .with("lang", json!(lang))
                .with("explain", json!(explain));
        }
        // Default: scriptvox-out/<run>.mp4 in the project, with the SRT next to it, written after a good render.
        let out = a
            .out
            .clone()
            .unwrap_or_else(|| PathBuf::from("scriptvox-out").join(format!("{id}.mp4")));
        if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(parent);
        }
        let out = std::path::absolute(&out).unwrap_or(out);
        // Before any paid step. The final rename would replace the source and answer ok.
        let music_file = match &music {
            audio::Music::File(p) => Some(p.clone()),
            _ => None,
        };
        let source = scene_items
            .iter()
            .flatten()
            .chain(media.iter().flatten())
            .chain(base_item.iter())
            .map(|i| i.path.clone())
            .chain(a.narration.clone())
            .chain(a.logo.as_ref().map(|l| l.file.clone()))
            .chain(music_file)
            .find(|p| crate::engine_ffmpeg::same_file(&out, p));
        if let Some(p) = source {
            let mut env = fail(
                Exit::Usage,
                format!("--out is an input file: {}", p.display()),
                Some("Give another --out: a make cannot write over a file it reads."),
            );
            env.run_id = Some(id.clone());
            return env;
        }
        let t0 = std::time::Instant::now();
        milestone(&format!("{id}: rendering {secs:.1} s"));
        let mut credits: Vec<String> = Vec::new();
        let mut stock_picks: Vec<StockPick> = Vec::new();
        // Picks from the keyless search: in the receipt and the terms note, not the recipe (each URL may be on any host).
        let mut open_picks: Vec<StockPick> = Vec::new();
        let mut open_used: Vec<(String, String)> = Vec::new();
        let media = if a.stock.is_some() && media.is_none() && a.scenes.is_none() {
            let starts: Vec<f64> = pages.iter().map(|p| p.start).collect();
            let cuts = media::shots(&starts, secs);
            let page_texts: Vec<String> = pages
                .iter()
                .map(|p| {
                    p.lines
                        .iter()
                        .flatten()
                        .map(|&i| words[i].text.clone())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            let terms: Vec<String> = match a.stock.as_deref() {
                Some("auto") | Some("") => http_rows::auto_terms(&page_texts),
                Some(list) => list
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                None => unreachable!(),
            };
            match fetch_stock(
                &stock_rows,
                &terms,
                &cuts,
                (w, h),
                &dir,
                &run,
                &mut credits,
                a.stock_picks.as_deref(),
                a.cache_only,
                &mut stock_picks,
            ) {
                Ok(items) => Some(items),
                Err(env) => return *env,
            }
        } else {
            media
        };
        if !credits.is_empty() {
            let body = format!("{}\n", credits.join("\n"));
            let _ = std::fs::write(dir.join("credits.txt"), &body);
        }
        // Sound made from each generated clip (brief `ambience`): (scene start, wav).
        let mut ambience: Vec<(f64, PathBuf)> = Vec::new();
        // Each scene clip that keeps its own sound (scene `sound`), cut to its scene.
        let mut clip_sounds: Vec<audio::ClipSound> = Vec::new();
        // Scenes: cuts on scene boundaries; each scene shows its own visual.
        let scene_plan = match &a.scenes {
            Some(scenes) if !scenes.is_empty() => {
                let mut starts = starts_of(scenes, &words, secs, speech_end);
                if let Some(g) = &beat_grid {
                    // The same end as for the cards, so both get the same times.
                    let (moved, report) = g.snap(&starts, speech_end);
                    starts = moved;
                    cut_on_report = Some(report);
                }
                let cuts = media::shots(&starts, secs);
                let mut items: Vec<Option<media::Item>> = scene_items.clone();
                items.truncate(cuts.len());
                // Each character's picture, made once when the first scene needs it.
                let cast = cast_of(&a);
                let mut pictures: Vec<Option<String>> = vec![None; cast.len()];
                // Generated scenes, one provider job each, resumed from the run folder on a rerun.
                for (i, g) in gen.iter().enumerate().take(cuts.len()) {
                    let Some(g) = g else { continue };
                    if items[i].is_some() {
                        continue;
                    }
                    milestone(&format!(
                        "{id}: generating scene {} with {}",
                        i + 1,
                        g.row.name
                    ));
                    let made = g
                        .made
                        .then(|| Cas::open().get(&g.key, &dir))
                        .flatten()
                        .and_then(|f| f.into_iter().next().map(|x| x.1));
                    if made.is_none() {
                        for &c in &g.cast {
                            if pictures[c].is_some() {
                                continue;
                            }
                            let budget = a.max_spend.unwrap_or(run_cost);
                            match character_picture(&a, cast[c], c, &dir, &id, budget) {
                                Ok(uri) => pictures[c] = Some(uri),
                                Err(mut env) => {
                                    env.run_id = Some(id.clone());
                                    return *env;
                                }
                            }
                        }
                    }
                    let with_ref = |req: &Value| {
                        let mut r = req.clone();
                        let uris: Vec<&String> = g
                            .cast
                            .iter()
                            .filter_map(|&c| pictures[c].as_ref())
                            .collect();
                        if !uris.is_empty() {
                            r["image_urls"] = json!(uris);
                        }
                        r
                    };
                    let keep = |p: &Path| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("clip");
                        let terms = serde_json::to_value(&g.row.terms).unwrap_or_default();
                        let _ = Cas::open().put(&g.key, &g.row.name, "1", &[(name, p)], terms);
                    };
                    if g.still {
                        let got = match &made {
                            Some(p) => Ok((p.clone(), 0.0)),
                            None => crate::genjob::run_clip(
                                &g.row,
                                &with_ref(&g.input),
                                false,
                                Path::new("."),
                                &dir,
                                &format!("scene{}_still", i + 1),
                                &id,
                                a.max_spend.unwrap_or(run_cost),
                                GEN_WAIT_S,
                            ),
                        };
                        match got {
                            Ok((path, _)) => {
                                keep(&path);
                                items[i] = Some(media::Item {
                                    path,
                                    kind: media::Kind::Image,
                                    from: 0.0,
                                    to: None,
                                });
                                continue;
                            }
                            Err(e) => {
                                let mut env = fail(e.exit, e.message, Some(&e.fix));
                                env.run_id = Some(id.clone());
                                return env;
                            }
                        }
                    }
                    // A first frame from the image row, then image to video from it.
                    let mut input = g.input.clone();
                    if let Some((fr, req)) = g.frame.as_ref().filter(|_| made.is_none()) {
                        // A first frame already made with this row and request (a storyboard still, or an earlier
                        // run) comes from the store: no call. Not with a character: its reference picture is part
                        // of the request, and the storyboard makes none.
                        let sent = with_ref(req);
                        let fkey = frame_key(fr, &sent);
                        let shot = format!("scene{}_frame", i + 1);
                        let stored = g
                            .cast
                            .is_empty()
                            .then(|| Cas::open().get(&fkey, &dir.join(format!(".{shot}"))))
                            .flatten()
                            .and_then(|f| f.into_iter().next());
                        let png = match stored {
                            Some((_, p)) => p,
                            None => match crate::genjob::run_clip(
                                fr,
                                &sent,
                                false,
                                Path::new("."),
                                &dir,
                                &shot,
                                &id,
                                a.max_spend.unwrap_or(run_cost),
                                GEN_WAIT_S,
                            ) {
                                Ok((p, _)) => {
                                    let name = format!("{shot}.{}", fr.ext());
                                    let terms = serde_json::to_value(&fr.terms).unwrap_or_default();
                                    if g.cast.is_empty() {
                                        let _ = Cas::open().put(
                                            &fkey,
                                            &fr.name,
                                            "1",
                                            &[(name.as_str(), &p)],
                                            terms,
                                        );
                                    }
                                    p
                                }
                                Err(e) => {
                                    let mut env = fail(e.exit, e.message, Some(&e.fix));
                                    env.run_id = Some(id.clone());
                                    return env;
                                }
                            },
                        };
                        match crate::generate::image_uri(&png) {
                            Ok((uri, _)) => {
                                input = g.row.with_seed(
                                    g.row.input(
                                        &g.prompt,
                                        g.seconds,
                                        gen_aspect(&a.aspect),
                                        Some(&uri),
                                    ),
                                    a.seed,
                                )
                            }
                            Err(env) => return *env,
                        }
                    }
                    let got = match &made {
                        Some(p) => Ok((p.clone(), 0.0)),
                        None => crate::genjob::run_clip(
                            &g.row,
                            &input,
                            g.image,
                            Path::new("."),
                            &dir,
                            &format!("scene{}", i + 1),
                            &id,
                            a.max_spend.unwrap_or(run_cost),
                            GEN_WAIT_S,
                        ),
                    };
                    match got {
                        Ok((path, _)) => {
                            keep(&path);
                            let found = crate::gates::clip_check(
                                &ff,
                                &path,
                                g.seconds,
                                gen_aspect(&a.aspect),
                                g.image,
                            );
                            if let Some(bad) = found.iter().find(|f| f.status == "fail") {
                                let mut env = Envelope::fail_code(
                                    "make",
                                    Exit::GateFailed,
                                    "SVX_CLIP_CHECK",
                                    format!(
                                        "scene {}: the generated clip failed a check: {}",
                                        i + 1,
                                        bad.detail
                                    ),
                                    bad.fix.clone(),
                                );
                                env.findings = found.iter().map(|f| json!(f)).collect();
                                env.run_id = Some(id.clone());
                                env.plan_id = Some(plan.plan_id.clone());
                                return env;
                            }
                            if a.ambience {
                                milestone(&format!("{id}: sound for scene {}", i + 1));
                                match ambience_clip(
                                    &ff,
                                    &path,
                                    0.0,
                                    cuts[i].1 - cuts[i].0,
                                    &g.prompt,
                                    &dir,
                                    i + 1,
                                    &id,
                                ) {
                                    Ok(p) => ambience.push((cuts[i].0, p)),
                                    Err(e) => {
                                        let mut env = fail(e.exit, e.message, Some(&e.fix));
                                        env.run_id = Some(id.clone());
                                        env.plan_id = Some(plan.plan_id.clone());
                                        return env;
                                    }
                                }
                            }
                            let mut path = path;
                            for (on, kind, verb) in [
                                (a.upscale && !a.draft, "upscale", "upscaling"),
                                (a.smooth && !a.draft, "interpolate", "smoothing"),
                            ] {
                                if !on {
                                    continue;
                                }
                                milestone(&format!("{id}: {verb} scene {}", i + 1));
                                match row_clip(&path, &dir, i + 1, &id, kind) {
                                    Ok(p) => path = p,
                                    Err(e) => {
                                        let mut env = fail(e.exit, e.message, Some(&e.fix));
                                        env.run_id = Some(id.clone());
                                        env.plan_id = Some(plan.plan_id.clone());
                                        return env;
                                    }
                                }
                            }
                            items[i] = Some(media::Item {
                                path,
                                kind: media::Kind::Video,
                                from: 0.0,
                                to: None,
                            })
                        }
                        Err(e) => {
                            let mut env = fail(e.exit, e.message, Some(&e.fix));
                            env.run_id = Some(id.clone());
                            env.plan_id = Some(plan.plan_id.clone());
                            return env;
                        }
                    }
                }
                // Stock scenes, fetched in one call in scene order.
                let stock_idx: Vec<usize> = (0..cuts.len())
                    .filter(|&i| items[i].is_none() && scenes[i].stock.is_some())
                    .collect();
                if !stock_idx.is_empty() {
                    let terms: Vec<String> = stock_idx
                        .iter()
                        .map(|&i| scenes[i].stock.clone().unwrap_or_default())
                        .collect();
                    let sub: Vec<(f64, f64)> = stock_idx.iter().map(|&i| cuts[i]).collect();
                    match fetch_stock(
                        &stock_rows,
                        &terms,
                        &sub,
                        (w, h),
                        &dir,
                        &run,
                        &mut credits,
                        a.stock_picks.as_deref(),
                        a.cache_only,
                        &mut stock_picks,
                    ) {
                        Ok(got) => {
                            for (k, it) in stock_idx.iter().zip(got) {
                                items[*k] = Some(it);
                            }
                        }
                        Err(env) => return *env,
                    }
                }
                for (i, sc) in scenes.iter().enumerate() {
                    let Some(term) = sc.open_image.as_deref().filter(|_| items[i].is_none()) else {
                        continue;
                    };
                    match crate::open_search::pick(
                        "image",
                        term,
                        &mut open_used,
                        &dir,
                        a.cache_only,
                    ) {
                        Ok((path, p)) => {
                            credits.push(open_credit(&format!("Shot {}: picture", i + 1), &p));
                            open_picks.push(p);
                            items[i] = Some(media::Item {
                                path,
                                kind: media::Kind::Image,
                                from: 0.0,
                                to: None,
                            });
                        }
                        Err(e) => {
                            let mut env = fail(
                                Exit::ProviderRetryable,
                                e,
                                Some("Try other words in open_image, or run again later."),
                            );
                            env.run_id = Some(id.clone());
                            env.plan_id = Some(plan.plan_id.clone());
                            return env;
                        }
                    }
                }
                // A scene with no visual: the media folder in turn, else the gradient.
                let mut pool = media.clone().unwrap_or_default().into_iter().cycle();
                let has_pool = media.is_some();
                let items: Vec<media::Item> = items
                    .into_iter()
                    .map(|it| {
                        it.unwrap_or_else(|| {
                            if has_pool {
                                pool.next().unwrap()
                            } else {
                                media::Item {
                                    path: PathBuf::new(),
                                    kind: media::Kind::Gradient,
                                    from: 0.0,
                                    to: None,
                                }
                            }
                        })
                    })
                    .collect();
                // The clip's own sound: the part the picture shows, from the scene's start, at the clip's own speed. It
                // stops with the scene, with `media_to` or with the file, whichever is first; a slowed or held picture
                // does not stretch it.
                for (i, it) in items.iter().enumerate() {
                    let Some(mode) = scenes.get(i).and_then(|s| s.sound.as_deref()) else {
                        continue;
                    };
                    let len = (cuts[i].1 - cuts[i].0).min(it.to.map_or(f64::MAX, |t| t - it.from));
                    match audio::clip_sound(&ff, &dir, &it.path, it.from, len, i + 1) {
                        Ok(file) => clip_sounds.push(audio::ClipSound {
                            at: cuts[i].0,
                            file,
                            keep: mode == "keep",
                        }),
                        Err(e) => {
                            let mut env = fail(
                                Exit::Usage,
                                format!(
                                    "scenes[{i}].sound: {} gave no sound: {e}",
                                    it.path.display()
                                ),
                                Some("Use a clip that has sound, or drop `sound` from that scene."),
                            );
                            env.run_id = Some(id.clone());
                            return env;
                        }
                    }
                }
                // Ambience for the other video scenes (stock, own media), from the picture and the stock term.
                if a.ambience {
                    for (i, it) in items.iter().enumerate() {
                        let done = ambience.iter().any(|(t, _)| *t == cuts[i].0);
                        if it.kind != media::Kind::Video || done {
                            continue;
                        }
                        milestone(&format!("{id}: sound for scene {}", i + 1));
                        let prompt = scenes[i].stock.clone().unwrap_or_default();
                        match ambience_clip(
                            &ff,
                            &it.path,
                            it.from,
                            cuts[i].1 - cuts[i].0,
                            &prompt,
                            &dir,
                            i + 1,
                            &id,
                        ) {
                            Ok(p) => ambience.push((cuts[i].0, p)),
                            Err(e) => {
                                let mut env = fail(e.exit, e.message, Some(&e.fix));
                                env.run_id = Some(id.clone());
                                return env;
                            }
                        }
                    }
                }
                Some((items, cuts))
            }
            _ => None,
        };
        if let Some(term) = &a.music_search {
            match crate::open_search::pick("sound", term, &mut open_used, &dir, a.cache_only) {
                Ok((path, p)) => {
                    credits.push(open_credit("Music", &p));
                    open_picks.push(p);
                    music = audio::Music::File(path);
                }
                Err(e) => {
                    let mut env = fail(
                        Exit::ProviderRetryable,
                        e,
                        Some("Try other words in music_search, or use music `calm`."),
                    );
                    env.run_id = Some(id.clone());
                    env.plan_id = Some(plan.plan_id.clone());
                    return env;
                }
            }
        }
        if let Some(t) = &library {
            credits.push(crate::music_lib::credit(t));
            open_picks.push(crate::music_lib::pick(t));
        }
        if let Some(p) = music_first.take() {
            credits.push(fetch_credit(&p));
            open_picks.push(p);
        } else if let Some(url) = &music_url {
            milestone(&format!("{id}: getting the music from its address"));
            match crate::fetch::music(
                url,
                a.music_from.as_deref(),
                a.music_to.as_deref(),
                &dir,
                a.cache_only,
                &ff,
            ) {
                Ok((path, p)) => {
                    credits.push(fetch_credit(&p));
                    open_picks.push(p);
                    music = audio::Music::File(path);
                }
                Err(f) => {
                    let mut env = Envelope::fail_code("make", f.exit, f.code, f.message, f.fix);
                    env.run_id = Some(id.clone());
                    env.plan_id = Some(plan.plan_id.clone());
                    return env;
                }
            }
        }
        if !credits.is_empty() {
            let body = format!("{}\n", credits.join("\n"));
            let _ = std::fs::write(dir.join("credits.txt"), &body);
        }
        // One place for every video that plays: an HDR clip becomes SDR before the graph sees it.
        let (mut scene_plan, mut media, mut base_item) = (scene_plan, media, base_item);
        let mut tone_mapped = 0;
        for it in scene_plan
            .iter_mut()
            .flat_map(|(items, _)| items.iter_mut())
            .chain(media.iter_mut().flatten())
            .chain(base_item.iter_mut())
            .filter(|it| it.kind == media::Kind::Video)
        {
            match ff.sdr_source(&it.path) {
                Ok(p) => {
                    tone_mapped += usize::from(p != it.path);
                    it.path = p;
                }
                Err(e) => {
                    let mut env = fail(
                        Exit::Error,
                        e,
                        Some("Check the file, and that the disk has room."),
                    );
                    env.run_id = Some(id.clone());
                    return env;
                }
            }
        }
        let mut plain = false;
        let mut shot_count = 1;
        let tr = media::transition(a.transition.as_deref(), lang == "ar", fps)
            .expect("checked at plan time");
        // The shots, kept for a render in groups of scenes (docs/specs/render-split.md).
        let mut shots: Option<crate::split::Shots> = None;
        let visual = match (&scene_plan, &media) {
            _ if base_item.is_some() => {
                let base = std::slice::from_ref(base_item.as_ref().unwrap());
                // With its own sound kept, the picture must not slow down (the sound plays at 1x).
                let lens = if a.base_sound.as_deref().is_some_and(|m| m != "none") {
                    vec![]
                } else {
                    clip_lens(&ff, base)
                };
                let (inputs, graph) =
                    media::graph(base, &lens, &[], &[(0.0, secs)], (w, h), fps, tr, &[]);
                milestone(&format!("{id}: base video under the new voice"));
                Visual {
                    inputs,
                    graph,
                    label: "[bg]".into(),
                }
            }
            (Some((items, cuts)), _) => {
                let s = crate::split::Shots {
                    items: items.clone(),
                    lens: clip_lens(&ff, items),
                    moves: a
                        .scenes
                        .as_ref()
                        .map_or(vec![], |s| s.iter().map(|sc| sc.camera.clone()).collect()),
                    cuts: cuts.clone(),
                    frame: (w, h),
                    fps,
                    tr,
                    into: a.scenes.as_ref().map_or(vec![], |s| {
                        s.iter()
                            .map(|sc| {
                                let t = sc.transition.as_deref()?;
                                media::transition(Some(t), lang == "ar", fps).ok()
                            })
                            .collect()
                    }),
                };
                let (inputs, graph) = s.graph((w, h), None);
                shots = Some(s);
                milestone(&format!("{id}: {} scenes", cuts.len()));
                shot_count = cuts.len();
                Visual {
                    inputs,
                    graph,
                    label: "[bg]".into(),
                }
            }
            (None, None) => {
                plain = true;
                Visual::gradient((w, h), fps, secs)
            }
            (None, Some(items)) => {
                let starts: Vec<f64> = pages.iter().map(|p| p.start).collect();
                let cuts = media::spread(media::shots(&starts, secs), items.len());
                let s = crate::split::Shots {
                    items: items.clone(),
                    lens: clip_lens(&ff, items),
                    moves: vec![],
                    cuts,
                    frame: (w, h),
                    fps,
                    tr,
                    into: vec![],
                };
                let (inputs, graph) = s.graph((w, h), None);
                milestone(&format!(
                    "{id}: {} shots from {} media files",
                    s.cuts.len(),
                    items.len()
                ));
                shot_count = s.cuts.len();
                shots = Some(s);
                Visual {
                    inputs,
                    graph,
                    label: "[bg]".into(),
                }
            }
        };
        let (enc, skipped) = match ff.pick_encoder() {
            Ok(r) => r,
            Err(e) => {
                return fail(
                    Exit::Error,
                    e,
                    Some("Install an FFmpeg build with libx264 or libopenh264."),
                )
            }
        };
        if let (Some(mr), Some(mp)) = (&music_row, &a.music_prompt) {
            milestone(&format!("{id}: making the music bed with {}", mr.name));
            let input = mr.request(&crate::generators::GenInput {
                prompt: mp.clone(),
                seconds: secs + 10.0,
                ..Default::default()
            });
            // A bed already made comes back free and the same, like a clip (`--final` after `--draft`).
            let key = store::key("gen", "1", &json!({"model": mr.name, "input": input}));
            let cas = Cas::open();
            let made = cas.get(&key, &dir).and_then(|f| f.into_iter().next());
            let got = match made {
                Some((_, p)) => Ok((p, 0.0)),
                None => crate::genjob::run_clip(
                    mr,
                    &input,
                    false,
                    Path::new("."),
                    &dir,
                    "music",
                    &id,
                    a.max_spend.unwrap_or(run_cost),
                    GEN_WAIT_S,
                ),
            };
            match got {
                Ok((p, _)) => {
                    let name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("music.wav");
                    let terms = serde_json::to_value(&mr.terms).unwrap_or_default();
                    let _ = cas.put(&key, &mr.name, "1", &[(name, &p)], terms);
                    music = audio::Music::File(p)
                }
                Err(e) => {
                    let mut env = fail(e.exit, e.message, Some(&e.fix));
                    env.run_id = Some(id.clone());
                    return env;
                }
            }
        }
        // The base video's own sound takes the bed's place (ducked under the voice); `background` drops its voice.
        if let (Some(mode @ ("keep" | "background")), Some(base)) =
            (a.base_sound.as_deref(), &base_item)
        {
            milestone(&format!("{id}: base video sound ({mode})"));
            match base_sound(&ff, &base.path, mode == "background", &dir, &id) {
                Ok(p) => music = audio::Music::Base(p),
                Err(e) => {
                    let mut env = fail(e.exit, e.message, Some(&e.fix));
                    env.run_id = Some(id.clone());
                    return env;
                }
            }
        }
        let mastered = match audio::master(
            &ff,
            &dir,
            &audio,
            &music,
            &sfx_cues,
            &ambience,
            &clip_sounds,
            secs,
            audio::music_gap_db(a.music_level.as_deref()).unwrap_or(10.0),
        ) {
            Ok(m) => m,
            Err(e) => return fail(Exit::Error, format!("audio master failed: {e}"), None),
        };
        let part = part_path(&out);
        let mut visual = visual;
        // The colour look goes on the picture before titles, cards and the logo.
        // Checked before TTS, but the file can go in between.
        let look = match media::look(a.look.as_deref()) {
            Ok(l) => l,
            Err(e) => return fail(Exit::Usage, e, Some("Fix `look` in the brief.")),
        };
        if let Some(f) = look {
            let sep = if visual.graph.is_empty() { "" } else { ";" };
            visual.graph = format!("{}{sep}{}{f}[look]", visual.graph, visual.label);
            visual.label = "[look]".into();
        }
        let mut extra: Vec<gates::Finding> = Vec::new();
        if !behind.is_empty() {
            match cut_out(&ff, &dir, &visual, &behind, (w, h), &id) {
                Ok(masks) => {
                    milestone(&format!("{id}: {} titles behind the subject", masks.len()));
                    // Each title moves to the height where the subject crosses a small part of it.
                    let ass_path = dir.join("behind.ass");
                    let ass = std::fs::read_to_string(&ass_path).unwrap_or_default();
                    let mut lines: Vec<String> = ass.lines().map(String::from).collect();
                    let dialogue: Vec<usize> = (0..lines.len())
                        .filter(|&i| lines[i].starts_with("Dialogue:"))
                        .collect();
                    let line_h = (size * crate::cards::BEHIND_SCALE) as f64 * 1.2 / h as f64;
                    let (_, font, _, _, _) = caption_pages(&[], &lang, (w, h), "karaoke", (1, 1));
                    let meter =
                        Measurer::new(font.to_vec(), (size * crate::cards::BEHIND_SCALE) as f32)
                            .expect("embedded font parses");
                    for (k, m) in masks.iter().enumerate() {
                        let Some(&li) = dialogue.get(k) else { continue };
                        let rows = lines[li].matches("\\N").count() + 1;
                        // Only the columns the text spans count: a subject beside a short word does not hide it.
                        let text = lines[li].rsplit('}').next().unwrap_or("");
                        let span = text
                            .split("\\N")
                            .map(|l| f64::from(meter.width(l)) / f64::from(w))
                            .fold(0.0, f64::max);
                        let Some(profile) = row_profile(&ff, m, span) else {
                            continue;
                        };
                        // Arabic letters join on the baseline: hiding it costs more than in Latin script.
                        let (aim, max) = if lang == "ar" {
                            (0.08, 0.25)
                        } else {
                            (0.2, BEHIND_MAX_COVER)
                        };
                        let (y, c) =
                            place(&profile, line_h, rows, crate::cards::BEHIND_Y as f64, aim);
                        lines[li] = crate::cards::move_to(&lines[li], (y * h as f64) as f32);
                        let warn = c > max;
                        extra.push(gates::Finding {
                            gate: "behind",
                            status: if warn { "warn" } else { "pass" },
                            detail: format!("title {}: placed at {:.0}% of the height, the subject hides {:.0}% of it", k + 1, y * 100.0, c * 100.0),
                            fix: warn.then(|| "Drop `behind` on that card, or use a shot where the subject covers less of the frame.".into()),
                        });
                    }
                    let _ = std::fs::write(&ass_path, lines.join("\n") + "\n");
                    let first = visual.inputs.iter().filter(|x| *x == "-i").count();
                    for m in &masks {
                        visual
                            .inputs
                            .extend(["-i".into(), m.to_string_lossy().into_owned()]);
                    }
                    visual.graph = behind_graph(&visual, &behind, first, (w, h), fps);
                    visual.label = "[behind]".into();
                }
                Err(e) => {
                    let mut env = fail(e.exit, e.message, Some(&e.fix));
                    env.run_id = Some(id.clone());
                    return env;
                }
            }
        }
        // The waveform of the final mix over the plain background (G107): after the look, so the accent stays true.
        let mut wave_tmp = None;
        if a.waveform {
            // Where the plain background shows with nothing in the middle: no picture, no card, after the hook.
            let after_hook = if a.hook.is_some() { HOOK_S } else { 0.0 };
            let spans: Vec<(f64, f64)> = match (&a.scenes, &scene_plan) {
                (Some(scenes), Some((items, cuts))) => items
                    .iter()
                    .zip(cuts)
                    .zip(scenes)
                    .filter(|((it, _), sc)| {
                        it.kind == media::Kind::Gradient
                            && crate::scene_cards::of(sc).next().is_none()
                    })
                    .map(|((_, c), _)| (c.0.max(after_hook), c.1))
                    .collect(),
                _ if plain => vec![(after_hook, secs)],
                _ => vec![],
            };
            if !spans.is_empty() {
                let (y, bh) = wave_band(a.caption_at.as_deref(), h);
                let file = match wave_file(&ff, &dir, &mastered, (w, bh), fps, a.accent.as_deref())
                {
                    Ok(f) => f,
                    Err(e) => return fail(Exit::Error, format!("waveform failed: {e}"), None),
                };
                let first = visual.inputs.iter().filter(|x| *x == "-i").count();
                // By its bare name: the render runs in the run folder, and a run id in the render key would stop a
                // rerun from finding its chunks. The name holds the hash of the sound, so a new sound is a new key.
                wave_tmp = Some(dir.join(&file));
                visual.inputs.extend(["-i".into(), file]);
                let on: Vec<String> = spans
                    .iter()
                    .map(|(a, b)| format!("between(t,{a:.3},{b:.3})"))
                    .collect();
                let sep = if visual.graph.is_empty() { "" } else { ";" };
                // eof_action=pass: the mix can end before the picture, and the background goes on.
                visual.graph = format!(
                    "{}{sep}{}[{first}:v]overlay=0:{y}:eof_action=pass:enable='{}'[wave]",
                    visual.graph,
                    visual.label,
                    on.join("+")
                );
                visual.label = "[wave]".into();
            }
        }
        // Picture in picture, under captions and logo. Every scene's time comes from the scene cuts.
        if let (Some(scenes), Some((_, cuts))) = (&a.scenes, &scene_plan) {
            for (i, (sc, span)) in scenes.iter().zip(cuts).enumerate() {
                let Some(inset) = &sc.inset else { continue };
                let first = visual.inputs.iter().filter(|x| *x == "-i").count();
                let path = std::path::absolute(&inset.file).unwrap_or(inset.file.clone());
                if media::kind_of(&path) == Some(media::Kind::Image) {
                    // A looped still, or it would show for one frame only.
                    visual.inputs.extend([
                        "-loop".into(),
                        "1".into(),
                        "-t".into(),
                        format!("{:.3}", span.1 - span.0 + 0.1),
                    ]);
                }
                visual
                    .inputs
                    .extend(["-i".into(), path.display().to_string()]);
                let sep = if visual.graph.is_empty() { "" } else { ";" };
                visual.graph = format!(
                    "{}{sep}{}",
                    visual.graph,
                    media::inset_graph(
                        inset,
                        first,
                        &visual.label,
                        &format!("[in{i}]"),
                        *span,
                        (w, h)
                    )
                );
                visual.label = format!("[in{i}]");
            }
        }
        // Full-frame layers with alpha, over the picture and the insets, under captions and logo (G048).
        if let (Some(scenes), Some((_, cuts))) = (&a.scenes, &scene_plan) {
            for (i, (sc, span)) in scenes.iter().zip(cuts).enumerate() {
                let Some(file) = &sc.overlay else { continue };
                let first = visual.inputs.iter().filter(|x| *x == "-i").count();
                let path = std::path::absolute(file).unwrap_or(file.clone());
                if media::kind_of(&path) == Some(media::Kind::Image) {
                    visual.inputs.extend([
                        "-loop".into(),
                        "1".into(),
                        "-t".into(),
                        format!("{:.3}", span.1 - span.0 + 0.1),
                    ]);
                }
                // FFmpeg's own VP8 and VP9 decoders drop the alpha side stream.
                match ff.video_codec(&path).as_deref() {
                    Some("vp9") => visual.inputs.extend(["-c:v".into(), "libvpx-vp9".into()]),
                    Some("vp8") => visual.inputs.extend(["-c:v".into(), "libvpx".into()]),
                    _ => {}
                }
                visual
                    .inputs
                    .extend(["-i".into(), path.display().to_string()]);
                let sep = if visual.graph.is_empty() { "" } else { ";" };
                visual.graph = format!(
                    "{}{sep}{}",
                    visual.graph,
                    media::overlay_graph(first, &visual.label, &format!("[ovo{i}]"), *span, (w, h))
                );
                visual.label = format!("[ovo{i}]");
            }
        }
        // The end card first, so the corner logo is not blurred into it; the corner logo stops where it starts.
        if let (Some(o), Some(at)) = (&a.outro, outro_at) {
            let first = visual.inputs.iter().filter(|x| *x == "-i").count();
            if let Some(l) = &a.logo {
                // A looped still for the card's length: a single frame would hold its faded-out first state.
                visual.inputs.extend([
                    "-loop".into(),
                    "1".into(),
                    "-t".into(),
                    format!("{:.2}", o.seconds.unwrap_or(2.5) + 0.5),
                    "-i".into(),
                    std::path::absolute(&l.file)
                        .unwrap_or(l.file.clone())
                        .display()
                        .to_string(),
                ]);
            }
            visual.graph = outro_graph(&visual, at, a.logo.as_ref().map(|_| first), (w, h));
            if crate::card_style::is_paper(a.card_style.as_deref()) {
                visual.graph = crate::card_style::outro(&visual.graph);
            }
            visual.label = "[outro]".into();
        }
        if let Some(l) = &a.logo {
            let first = visual.inputs.iter().filter(|x| *x == "-i").count();
            visual.inputs.extend([
                "-i".into(),
                std::path::absolute(&l.file)
                    .unwrap_or(l.file.clone())
                    .display()
                    .to_string(),
            ]);
            visual.graph = logo_graph(&visual, l, first, (w, h), outro_at);
            visual.label = "[logo]".into();
        }
        let graph = video_graph(&visual);
        // The recipe that remakes this video (no keys: the request never holds one).
        let mut recipe_req = saved.clone();
        recipe_req.stock_picks = (!stock_picks.is_empty()).then(|| stock_picks.clone());
        // The file may be shared: music from an address keeps no query and no user part (a query can be signed).
        if let Some(m) = recipe_req
            .music
            .as_mut()
            .filter(|m| crate::fetch::is_address(m))
        {
            *m = crate::fetch::public_url(m);
        }
        let recipe = json!({"scriptvox": env!("CARGO_PKG_VERSION"), "plan_id": plan.plan_id, "request": recipe_req}).to_string();
        // Many scenes on a large frame: the shot track is made first, in groups of scenes, each in its own FFmpeg
        // process, and this render reads the group files (docs/specs/render-split.md).
        // `per`: the size of a group for the second attempt, after a first attempt in one process failed for memory.
        let mut per: Option<usize> = None;
        let (split, chunk_dir, stats) = loop {
            let split = match shots.as_ref().map(|s| {
                let layers = crate::split::Layers {
                    inputs: &visual.inputs,
                    graph: &graph,
                    fontconfig: &fc,
                };
                let work = std::path::absolute(".scriptvox/chunks").unwrap_or_default();
                crate::split::render(&ff, &dir, &work, s, secs, &layers, per)
            }) {
                Some(Err(e)) => {
                    let (exit, msg, fix) = crate::split::failure(e);
                    return fail(exit, msg, Some(fix));
                }
                Some(Ok(Ok(sp))) => Some(sp),
                Some(Ok(Err(Some(why)))) => {
                    milestone(&format!(
                        "{id}: picture in one process, not in groups of scenes: {why}"
                    ));
                    None
                }
                Some(Ok(Err(None))) | None => None,
            };
            if let Some(sp) = &split {
                milestone(&format!(
                    "{id}: picture made in {} groups of scenes ({} kept from an earlier attempt)",
                    sp.groups, sp.kept
                ));
            }
            let (job_inputs, job_graph) = split
                .as_ref()
                .map_or((&visual.inputs, &graph), |sp| (&sp.inputs, &sp.graph));
            let job = render::Job {
                inputs: job_inputs,
                graph: job_graph,
                enc: &enc,
                fps,
                secs,
                audio: &mastered,
                out: &part,
                fontconfig: &fc,
                metadata: (!a.no_embed).then_some(recipe.as_str()),
            };
            let captions = std::fs::read_to_string(dir.join("captions.ass")).unwrap_or_default();
            let chunk_dir = std::path::absolute(
                Path::new(".scriptvox/chunks").join(render::key(&job, &captions)),
            )
            .unwrap_or_default();
            match render::render(&ff, &dir, &chunk_dir, &job) {
                Ok(s) => break (split, chunk_dir, s),
                Err(RunError::Stalled(s)) => {
                    let _ = std::fs::remove_file(&part);
                    return fail(
                        Exit::WatchdogKill,
                        format!("FFmpeg made no progress for {s} s and was stopped"),
                        Some("Run the same command again: finished chunks are kept and the render resumes."),
                    );
                }
                Err(RunError::Failed(e)) => {
                    let _ = std::fs::remove_file(&part);
                    // The picture in one process failed for memory, and the load was under the limit for groups
                    // (a draft frame with many scenes and overlays, 14 Nov 2026): one more attempt, in groups.
                    let again = shots
                        .as_ref()
                        .filter(|_| split.is_none() && per.is_none())
                        .and_then(|s| crate::split::retry_size(&e, s.frame, s.cuts.len()));
                    if let Some(n) = again {
                        milestone(&format!(
                            "{id}: the render in one process ran out of memory; one more attempt, with the picture in groups of {n} scenes"
                        ));
                        let _ = std::fs::remove_dir_all(&chunk_dir);
                        per = Some(n);
                        continue;
                    }
                    // Out of memory: the same command fails the same way (docs/specs/output-size.md).
                    // FFmpeg can also end with no error and too few frames when a source fails for memory (seen at
                    // 3840x2160 with 8 scenes): missing chunks on a frame that the estimate puts near the limit.
                    let scenes = a.scenes.as_ref().map_or(1, Vec::len);
                    let heavy = crate::size::estimate((w, h), fps, secs, scenes).memory_gb
                        > crate::size::MEMORY_WARN_GB;
                    if e.contains("Cannot allocate memory")
                        || e.contains("malloc of size")
                        || (heavy && e.contains("chunks were written"))
                    {
                        return fail(
                            Exit::Error,
                            format!("{e} (the render ran out of memory)"),
                            Some(crate::split::memory_fix(split.is_none() && shots.is_some())),
                        );
                    }
                    return fail(Exit::Error, e, Some("Run the same command again: finished chunks are kept and the render resumes."));
                }
            }
        };
        if stats.resumed_from > 0 {
            milestone(&format!(
                "{id}: kept {} finished chunks of {}, rendering the rest",
                stats.resumed_from, stats.chunks
            ));
        }
        if let Err(e) = std::fs::rename(&part, &out) {
            return fail(
                Exit::Error,
                format!("cannot move the video into place: {e}"),
                None,
            );
        }
        // The final file exists: the chunks are no longer needed.
        let _ = std::fs::remove_dir_all(&chunk_dir);
        // And the group files of a render in groups.
        if let Some(sp) = &split {
            let _ = std::fs::remove_dir_all(&sp.dir);
        }
        // The wave file too: about 1.6 MB a second at 1080 px, and a rerun makes it again from the sound.
        if let Some(f) = wave_tmp {
            let _ = std::fs::remove_file(f);
        }
        let rb = match ff.readback(&out) {
            Ok(rb) => rb,
            Err(e) => return fail(Exit::Error, e, None),
        };
        let planned = (secs * f64::from(fps)).round() as u64;
        let mut env = if rb.frames.abs_diff(planned) > 1 {
            Envelope::fail(
                "make",
                Exit::GateFailed,
                format!("readback: {} frames, planned {planned}", rb.frames),
                // The chunk folder is removed above, so the next run renders every frame new (`docs/specs/RVX-017.md`).
                Some(if stats.resumed_from > 0 {
                    "A chunk kept from the earlier try had the wrong length. The kept chunks are removed now: run the same command again, and every frame is rendered new.".into()
                } else {
                    "Run the same command again. If the count is wrong again, run `scriptvox report` and send its file with the problem.".into()
                }),
            )
        } else {
            Envelope::ok("make")
        };
        // Quality gates over every frame: a failure is exit 3, and the video stays for inspection.
        milestone(&format!("{id}: checking"));
        let checked = gates::check(&ff, &out);
        if let Err(e) = &checked {
            env = Envelope::fail(
                "make",
                Exit::GateFailed,
                format!("gate failed: {e}"),
                Some("The video stays for inspection. Run the same command again.".into()),
            );
        }
        if let Ok(mut findings) = checked {
            // No voice and no music: silence is the design, and so is a missing loudness. With music the silence
            // finding stays: a bed that ended at 9.3 s of a 15 s video was not reported (11 Nov 2026).
            if voice.engine == "none" && a.music.is_none() && a.music_prompt.is_none() {
                findings.retain(|f| f.gate != "silence" && f.gate != "loudness");
            }
            let fails: Vec<&gates::Finding> =
                findings.iter().filter(|f| f.status == "fail").collect();
            if env.ok && !fails.is_empty() {
                let names: Vec<&str> = fails.iter().map(|f| f.gate).collect();
                env = Envelope::fail(
                    "make",
                    Exit::GateFailed,
                    format!("gate failed: {}", names.join(", ")),
                    fails[0].fix.clone(),
                );
            }
            env.findings = findings.iter().map(|f| json!(f)).collect();
        }
        // Listen to the result when a speech to text row is free to use: the script words heard in order.
        if !a.draft && will_listen(&a, &voice) {
            milestone(&format!("{id}: listening"));
            let script: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
            // The master, not the video: the transcript cache is keyed on the file, so a change to the picture alone
            // (a card, a colour, `--final`) needs no second listen (16 to 25 s on an 84 s video).
            let heard =
                crate::analyze::transcribe(&ff, &dir.join("master.wav"), Some(lang.as_str()), secs);
            // A failed listen is said, not hidden (4 Nov 2026 eval: one of two like runs had no words gate).
            if let Err(e) = &heard {
                extra.push(gates::Finding {
                    gate: "words",
                    status: "warn",
                    detail: format!(
                        "not checked: the speech to text row failed ({})",
                        e.error
                            .as_ref()
                            .map_or("no message", |x| x.message.as_str())
                    ),
                    fix: Some("Run `scriptvox check <run> --hear` to listen again.".into()),
                });
            }
            if let Ok((doc, _)) = heard {
                let f = gates::words_heard(
                    &crate::say::apply(&script.join(" "), &a.say),
                    doc["text"].as_str().unwrap_or(""),
                );
                if env.ok && f.status == "fail" {
                    let kept = std::mem::take(&mut env.findings);
                    env = Envelope::fail(
                        "make",
                        Exit::GateFailed,
                        "gate failed: words",
                        f.fix.clone(),
                    );
                    env.findings = kept;
                }
                extra.push(f);
            }
        }
        // A base video's own text (subtitles, a title) stays in the picture under the new captions: say so.
        if let (Some(base), false) = (&base_item, a.local_only || a.cache_only) {
            milestone(&format!("{id}: looking for text in the base video"));
            let span = ff.duration(&base.path).map_or(secs, |l| l.min(secs));
            match crate::judge::burned_text(&ff, &base.path, span, &dir) {
                Ok(Some(what)) => extra.push(gates::Finding {
                    gate: "base_text",
                    status: "warn",
                    detail: format!("the base video has text in its picture ({what}); it stays under the new captions"),
                    fix: Some("Use a copy of the video without burned-in text, or accept it.".into()),
                }),
                Ok(None) => extra.push(gates::Finding {
                    gate: "base_text",
                    status: "pass",
                    detail: "no added text seen in 3 frames of the base video".into(),
                    fix: None,
                }),
                // ponytail: the check is advice only; an unreachable vision model leaves no finding.
                Err(_) => {}
            }
        }
        if !pages.is_empty() {
            let found = gates::caption_pages(&pages);
            if env.ok && found.status == "fail" {
                let kept = std::mem::take(&mut env.findings);
                env = Envelope::fail(
                    "make",
                    Exit::GateFailed,
                    format!("gate failed: captions: {}", found.detail),
                    found.fix.clone(),
                );
                env.findings = kept;
            }
            extra.push(found);
        }
        // Every box placed on the frame: captions, hook, cards, end card, and titles behind the subject.
        let burned = ["captions.ass", "behind.ass"]
            .map(|n| std::fs::read_to_string(dir.join(n)).unwrap_or_default());
        if let Some(found) = gates::text_bounds(&burned.join("\n"), (w, h), &scene_starts) {
            if env.ok && found.status == "fail" {
                let kept = std::mem::take(&mut env.findings);
                env = Envelope::fail(
                    "make",
                    Exit::GateFailed,
                    format!("gate failed: text_bounds: {}", found.detail),
                    found.fix.clone(),
                );
                env.findings = kept;
            }
            extra.push(found);
        }
        // The letters against the picture around them, and the seam of a loop (`contrast.rs`).
        extra.extend(crate::contrast::gate(&ff, &dir, &out, &fc));
        if a.loop_seam {
            extra.push(crate::contrast::loop_seam(&ff, &dir, &out));
        }
        if !missing.is_empty() {
            extra.push(emphasis_warning(&missing));
        }
        extra.extend(foreign_warning(&a, &lang, &spoken));
        extra.extend(cue_warnings.iter().cloned());
        extra.extend(picture_shape_warnings(&a));
        extra.extend(match_failed.clone());
        // Not for a pace the brief chose.
        let fitted_rate = if rate0.is_some() { 0 } else { fit_rate_pct };
        if let Some(n) = fit::rate_warn(fitted_rate, spoken.split_whitespace().count()) {
            extra.push(gates::Finding {
                gate: "fit",
                status: "warn",
                detail: if voice.engine == "none" {
                    format!("the words show at {fit_rate_pct:+} percent of reading pace to fill the duration")
                } else {
                    format!(
                        "the voice reads at {fit_rate_pct:+} percent speed to fill the duration; it may sound {}",
                        if fit_rate_pct < 0 { "dragged" } else { "rushed" }
                    )
                },
                fix: Some(if n > 0 {
                    format!("Add about {n} words, or lower duration.")
                } else {
                    format!("Cut about {} words, or raise duration.", -n)
                }),
            });
        }
        // The kept pace sets the length: say so when it is not the length the first run was asked for.
        if let Some(d) = a
            .pace
            .as_ref()
            .and_then(|p| p.duration)
            .filter(|d| (speech_end - d).abs() > 0.5)
        {
            extra.push(gates::Finding {
                gate: "fit",
                status: "warn",
                detail: format!("the narration part of the video is {speech_end:.1} s, not the {d} s of `duration`: the scenes that did not change keep their voice and pace, so the length follows the new words"),
                fix: Some(format!("For {d} s: `scriptvox resume {id} --duration {d}`. It fits the voice of all scenes again.")),
            });
        }
        env.findings.extend(extra.iter().map(|f| json!(f)));
        if let Some(rep) = cut_on_report.take() {
            if rep["moved"] == 0 {
                let why = if rep["boundaries"][0]["stayed"] == "the music has no steady beat" {
                    "the music has no steady beat, so no scene boundary moved"
                } else {
                    "no scene boundary has a beat within its limit, so none moved"
                };
                env.findings.push(json!({"gate": "cut_on", "status": "warn", "detail": why,
                    "fix": "Use music with a clear beat (the built-in `pulse`, or a track with drums), or drop cut_on. `scriptvox analyze <music file> --beats` shows the grid."}));
            }
            env = env.with("cut_on", rep);
        }
        crate::captions::sidecars(&dir, &out, a.out.is_some(), &mut env);
        if let Some(m) = matched.clone() {
            env = env.with("media_match", m);
        }
        env.run_id = Some(id.clone());
        for (k, p) in [
            ("video", &out),
            ("audio", &audio),
            ("captions_srt", &dir.join("captions.srt")),
            ("captions_vtt", &dir.join("captions.vtt")),
            ("captions_ass", &dir.join("captions.ass")),
        ] {
            env.artifacts.insert(k.into(), json!(p));
        }
        env.artifacts
            .insert("master".into(), json!(dir.join("master.wav")));
        for (k, v) in [
            ("duration_s", json!((secs * 100.0).round() / 100.0)),
            ("frames", json!(rb.frames)),
            ("words", json!(words.len())),
            ("caption_pages", json!(pages.len())),
            ("shots", json!(shot_count)),
            ("chunks", json!(stats.chunks)),
            ("resumed_from_chunk", json!(stats.resumed_from)),
            ("render_ms", json!(t0.elapsed().as_millis() as u64)),
            (
                "video_bytes",
                json!(std::fs::metadata(&out).map_or(0, |m| m.len())),
            ),
            ("cost_usd", json!(plan::spent(&id))),
        ] {
            env.metrics.insert(k.into(), v);
        }
        if a.draft {
            env.metrics.insert("draft".into(), json!(true));
        }
        if let Some(sp) = &split {
            env.metrics.insert("scene_groups".into(), json!(sp.groups));
            env.metrics
                .insert("scene_groups_kept".into(), json!(sp.kept));
        }
        if let Some(r) = line_report.take() {
            env.metrics.insert("lines".into(), r);
        }
        if voice.engine != "none" {
            env.metrics
                .insert("voice_rate_pct".into(), json!(fit_rate_pct));
            env.metrics.insert(
                "voice_tempo".into(),
                json!((fit_tempo * 1000.0).round() / 1000.0),
            );
        }
        let reused = gen.iter().flatten().filter(|g| g.made).count();
        if reused > 0 {
            env.metrics.insert("reused_scenes".into(), json!(reused));
        }
        if VOICE_REUSED.load(std::sync::atomic::Ordering::Relaxed) {
            env.metrics.insert("voice_reused".into(), json!(true));
        }
        let mut explain = vec![
            voice.explain(),
            if dir.join(NARRATION_HEARD).exists() {
                "timing: from speech to text of your recording".to_string()
            } else if voice.timing == "exact" {
                "timing: exact (TTS word events)".to_string()
            } else {
                "timing: estimated (spread by characters; this engine gives no word events)"
                    .to_string()
            },
        ];
        explain.extend(explain_fit);
        explain.extend(skipped);
        explain.push(format!("encoder: {}", enc.name));
        if tone_mapped > 0 {
            explain.push(format!(
                "HDR clips tone mapped to SDR: {tone_mapped}, copies in .scriptvox/hdr/"
            ));
        }
        if env.ok {
            // A dialogue names each voice.
            let mut speakers: Vec<&str> = who
                .iter()
                .map(String::as_str)
                .filter(|w| !w.is_empty())
                .collect();
            speakers.sort_unstable();
            speakers.dedup();
            // `explain` names one voice with its terms: the others are from the same engine, on the same terms.
            if speakers.len() > 1 {
                explain.push(format!("speakers: {}", speakers.join(", ")));
            }
            let receipt = json!({
                "run_id": env.run_id, "plan_id": plan.plan_id, "video": out, "duration_s": secs, "cost_usd": plan.cost_usd,
                "voice": {"engine": voice.engine, "id": voice.id, "terms": voice.terms, "timing": if dir.join(NARRATION_HEARD).exists() { "from speech to text" } else { voice.timing.as_str() }},
                "speakers": speakers,
                // A first frame's image row too: one that came from the store left no job file here, and its terms count.
                "generators": gen.iter().flatten().flat_map(|g| std::iter::once(g.row.name.clone()).chain(g.frame.as_ref().map(|(fr, _)| fr.name.clone()))).collect::<Vec<_>>(),
                "encoder": enc.name, "pins": {"ffmpeg": provision::pin("ffmpeg").map(|p| p.version), "uv": provision::pin("uv").map(|p| p.version)},
                "credits": env.artifacts.get("credits"),
                "stock": stock_picks.iter().chain(&open_picks).collect::<Vec<_>>(), "embedded_recipe": !a.no_embed,
            });
            let _ = std::fs::write(
                dir.join("receipt.json"),
                serde_json::to_string_pretty(&receipt).unwrap_or_default(),
            );
            env.artifacts
                .insert("receipt".into(), json!(dir.join("receipt.json")));
        }
        if let Some(p) = &a.parent_run {
            env = env.with("parent_run", json!(p));
        }
        if let Some(v) = &voiced_again {
            env = env.with("voiced_again", json!(v));
        }
        env.with("lang", json!(lang))
            .with("explain", json!(explain))
    })();
    let mut env = env;
    env.run_id.get_or_insert_with(|| run.id.clone());
    env.events_file = Some(run.dir.join("events.jsonl").to_string_lossy().into_owned());
    if env.artifacts.contains_key("video") {
        let how = gates::hear_how();
        let words_why = if a.draft {
            "a draft does not listen".to_string()
        } else if a.local_only || a.cache_only {
            format!("--local-only or --cache-only stops the speech row in make; {how}")
        } else {
            format!("no speech to text row, or it failed; {how}")
        };
        let mut want = vec![("words", words_why.as_str())];
        if a.base_video.is_some() {
            want.push((
                "base_text",
                "no vision service answered, so burned text in the base video was not looked for",
            ));
        }
        let not_checked = gates::not_checked(&env.findings, &want);
        env = env.with("not_checked", json!(not_checked));
        let (coverage, limitations) = gates::limits(&env.findings);
        env = env
            .with("coverage", json!(coverage))
            .with("limitations", json!(limitations));
    }
    run.finish(Path::new("."), &env);
    env
}

/// One stock clip per shot, in script order. A rate limit or server error moves
/// to the next provider (a `fallback` event). Clips go through the cache.
#[allow(clippy::too_many_arguments)]
fn fetch_stock(
    rows: &[http_rows::Row],
    terms: &[String],
    cuts: &[(f64, f64)],
    (w, h): (u32, u32),
    dir: &Path,
    run: &Run,
    credits: &mut Vec<String>,
    recorded: Option<&[StockPick]>,
    cache_only: bool,
    picked: &mut Vec<StockPick>,
) -> Result<Vec<media::Item>, Box<Envelope>> {
    let orientation = if h > w {
        "portrait"
    } else if w > h {
        "landscape"
    } else {
        "square"
    };
    let cas = Cas::open();
    let dl = provision::data_dir().join("downloads").join("stock");
    let stock_dir = dir.join("stock");
    let mut used: Vec<String> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut refused_why = String::new();
    let mut found: std::collections::HashMap<String, Vec<http_rows::Clip>> =
        std::collections::HashMap::new();
    let mut items = Vec::new();
    for (i, (a, b)) in cuts.iter().enumerate() {
        let term = &terms[i % terms.len().max(1)];
        let recorded_pick = recorded.and_then(|r| r.get(i % r.len().max(1))).cloned();
        if let Some(env) = recorded_pick
            .as_ref()
            .and_then(|p| stock_pick_refused(p, rows))
        {
            return Err(Box::new(env));
        }
        let (pick, path) = loop {
            if recorded_pick.is_none() && !found.contains_key(term) {
                // Every row's reason goes in the final error, not only the last one (audit H4).
                let mut why: Vec<String> = Some(refused_why.clone())
                    .filter(|w| !w.is_empty())
                    .into_iter()
                    .collect();
                let mut clips = None;
                let live: Vec<&http_rows::Row> =
                    rows.iter().filter(|r| !refused.contains(&r.name)).collect();
                // A term of 2 or more words that no row has a clip for is tried again as its first word
                // alone. `--stock auto` makes 2-word terms ("script write"), and the keyless rows often have
                // nothing for a pair, which failed the whole run (found on a real run, 10 Nov 2026).
                let first = term.split_whitespace().next().unwrap_or(term).to_string();
                let tries: Vec<&str> = if first == *term {
                    vec![term.as_str()]
                } else {
                    vec![term.as_str(), first.as_str()]
                };
                'tries: for (t, q) in tries.iter().enumerate() {
                    if t > 0 {
                        run.event("fallback", json!({"from": term, "to": q, "reason": "no row has a clip for the full term"}));
                    }
                    for (n, row) in live.iter().enumerate() {
                        let key = crate::keys::resolve(&row.key)
                            .map(|(k, _)| k)
                            .unwrap_or_default();
                        let last = match http_rows::search(row, &key, q, orientation, w.min(h)) {
                            Ok(c) if !c.is_empty() => {
                                clips = Some(c);
                                break 'tries;
                            }
                            Ok(_) => format!("{} found nothing for '{q}'", row.name),
                            Err(e) if e.fallback() => e.message.clone(),
                            Err(e) => {
                                return Err(Box::new(Envelope::fail(
                                    "make",
                                    Exit::Error,
                                    e.message,
                                    Some("Check the key with `scriptvox keys list`.".into()),
                                )))
                            }
                        };
                        why.push(last.clone());
                        if let Some(next) = live.get(n + 1) {
                            run.event(
                                "fallback",
                                json!({"from": row.name, "to": next.name, "reason": last}),
                            );
                        }
                    }
                }
                let Some(c) = clips else {
                    return Err(Box::new(Envelope::fail_code(
                        "make",
                        Exit::ProviderRetryable,
                        "SVX_STOCK_UNAVAILABLE",
                        format!("no stock clip for '{term}': {}", why.join("; ")),
                        Some("Try other --stock terms, or run again later.".into()),
                    )));
                };
                found.insert(term.clone(), c);
            }
            let need = b - a;
            let provider = recorded_pick
                .as_ref()
                .map(|p| p.provider.clone())
                .unwrap_or_default();
            let pick = match recorded_pick.clone() {
                Some(p) => http_rows::Clip {
                    provider: p.provider.clone(),
                    id: p.id,
                    url: p.url,
                    width: 0,
                    height: 0,
                    duration: need,
                    credit: p.credit,
                    page: p.page,
                    terms: p
                        .terms
                        .or_else(|| {
                            rows.iter()
                                .find(|r| r.name == provider)
                                .map(|r| r.terms.clone())
                        })
                        .unwrap_or(http_rows::Terms {
                            commercial: Some(true),
                            attribution_required: false,
                            redistribution: false,
                            note: String::new(),
                        }),
                },
                None => found[term]
                    .iter()
                    .find(|c| !used.contains(&c.url) && c.duration >= need)
                    .or_else(|| found[term].iter().find(|c| !used.contains(&c.url)))
                    .unwrap_or(&found[term][0])
                    .clone(),
            };
            let ext = if pick.url.ends_with(".webm") {
                "webm"
            } else {
                "mp4"
            };
            let name = format!("{}_{}.{ext}", pick.provider, pick.id);
            let ck = store::key(
                "stock.video",
                &pick.provider,
                &json!({"id": pick.id, "url": pick.url}),
            );
            let path = match cas.get(&ck, &stock_dir) {
                Some(files) => files.into_iter().next().map(|(_, p)| p).unwrap(),
                None if cache_only => {
                    return Err(Box::new(Envelope::fail_code(
                        "make",
                        Exit::Error,
                        "SVX_CACHE_MISS",
                        format!(
                            "stock clip {} {} is not in the cache",
                            pick.provider, pick.id
                        ),
                        Some("Use `scriptvox make --from <video>` to fetch it again.".into()),
                    )));
                }
                None => {
                    let dest = stock_dir.join(&name);
                    let _ = std::fs::create_dir_all(&stock_dir);
                    if let Err(e) = http_rows::download(&pick.url, &dl, &dest, None) {
                        // A host that refuses (a rate limit on this address, measured on Commons) refuses the next
                        // clip too, so the next row answers. A broken transfer stays: a rerun resumes it.
                        if recorded_pick.is_none() && e.starts_with("download answered HTTP") {
                            run.event("fallback", json!({"from": pick.provider, "reason": e}));
                            refused_why = format!("{} refused the download: {e}", pick.provider);
                            refused.push(pick.provider.clone());
                            found.clear();
                            continue;
                        }
                        return Err(Box::new(Envelope::fail(
                            "make",
                            Exit::ProviderRetryable,
                            e,
                            Some("Run the same command again: the download resumes.".into()),
                        )));
                    }
                    // A stock clip is a file from a service: only plain media stays (`media-inputs.md`).
                    if let Err(e) = crate::media_gate::downloaded(&pick.provider, &dest) {
                        return Err(Box::new(Envelope::fail(
                            "make",
                            Exit::Error,
                            format!("{e}. {}", crate::media_gate::ACCEPTED),
                            Some(
                                "Nothing was kept. Give another stock term, or your own clip."
                                    .into(),
                            ),
                        )));
                    }
                    let _ = cas.put(
                        &ck,
                        "stock.video",
                        &pick.provider,
                        &[(name.as_str(), &dest)],
                        json!(pick.terms),
                    );
                    dest
                }
            };
            break (pick, path);
        };
        used.push(pick.url.clone());
        picked.push(StockPick {
            provider: pick.provider.clone(),
            id: pick.id.clone(),
            url: pick.url.clone(),
            credit: pick.credit.clone(),
            page: pick.page.clone(),
            terms: rows
                .iter()
                .any(|r| r.name == pick.provider && r.license.is_some())
                .then(|| pick.terms.clone()),
            title: None,
            licence: None,
            layer: None,
        });
        let site = rows
            .iter()
            .find(|r| r.name == pick.provider)
            .map_or(pick.provider.clone(), |r| r.site.clone());
        credits.push(format!(
            "Shot {}: video by {} on {site}, {} ({})",
            i + 1,
            pick.credit,
            pick.page,
            pick.terms.note
        ));
        items.push(media::Item {
            path,
            kind: media::Kind::Video,
            from: 0.0,
            to: None,
        });
    }
    Ok(items)
}

/// A script that cannot fit: exit 3 with a word-count fix, before any render.
/// One generated scene, planned: the row, the request body, its cost, and the plan-id form of the request.
struct GenShot {
    /// One picture from an image row, not a clip.
    still: bool,
    /// Key of this clip in the content store: same row, same input.
    key: String,
    /// Already made in this project or on this machine: no call and no charge.
    made: bool,
    /// The image row and its request, when the scene asks for a first frame.
    frame: Option<(crate::generators::Generator, Value)>,
    /// The characters whose pictures this still or first frame starts from.
    cast: Vec<usize>,
    prompt: String,
    row: crate::generators::Generator,
    input: Value,
    image: bool,
    seconds: f64,
    cost: f64,
    /// The cost before the content store's discount, for the plan id.
    listed: f64,
    id_input: Value,
}

/// Plans every `generate` scene before any call: picks the row, the length and the request, and prices it.
fn plan_generated(a: &Args, lang: &str) -> Result<Vec<Option<GenShot>>, Box<Envelope>> {
    // The rows as before. What the store has comes from there, on the row that made it: its service is not asked.
    let first = plan_generated_once(a, lang, None)?;
    let cas = Cas::open();
    let down = |g: &GenShot| {
        !g.made
            && (is_down(a, &g.row)
                || g.frame
                    .as_ref()
                    .is_some_and(|(fr, req)| is_down(a, fr) && !cas.has(&frame_key(fr, req))))
    };
    if !first.iter().flatten().any(down) {
        return Ok(first);
    }
    // A shot that must still be made, on a service that does not answer: the pick again, with each row asked. The
    // other scenes keep their first pick and are not asked: a scene that is in the store never stops the plan.
    let settled: Vec<usize> = (0..first.len())
        .filter(|&i| !first[i].as_ref().is_some_and(down))
        .collect();
    let again = plan_generated_once(a, lang, Some(&settled));
    ASKING.store(false, std::sync::atomic::Ordering::Relaxed);
    let again = again?;
    Ok(first
        .into_iter()
        .zip(again)
        .map(|(f, s)| if f.as_ref().is_some_and(down) { s } else { f })
        .collect())
}

/// `ask`: the pick again; each scene that is not in this list asks its rows if their service answers.
fn plan_generated_once(
    a: &Args,
    lang: &str,
    ask: Option<&[usize]>,
) -> Result<Vec<Option<GenShot>>, Box<Envelope>> {
    let usage =
        |msg: String, fix: String| Box::new(Envelope::fail("make", Exit::Usage, msg, Some(fix)));
    let rows = crate::generators::all();
    let mut out = Vec::new();
    // What each reference picture is, for the content keys: a new character makes new pictures.
    let cast = cast_of(a);
    let mut char_ids = Vec::new();
    for c in &cast {
        char_ids.push(match &c.file {
            Some(f) => format!(
                "{} {}",
                crate::generate::image_uri(f)?.1,
                c.prompt.as_deref().unwrap_or("")
            ),
            None => c.prompt.clone().unwrap_or_default(),
        });
    }
    for (i, sc) in a.scenes.iter().flatten().enumerate() {
        if let Some(settled) = ask {
            ASKING.store(!settled.contains(&i), std::sync::atomic::Ordering::Relaxed);
        }
        let Some(gs) = &sc.generate else {
            out.push(None);
            continue;
        };
        let n = i + 1;
        for name in &gs.who {
            if !cast
                .iter()
                .any(|c| c.name.as_deref() == Some(name.as_str()))
            {
                return Err(usage(
                    format!("scene {n}: who names {name}, who is not in `characters`"),
                    "Use the names from `characters`.".into(),
                ));
            }
        }
        if gs.who.len() > 4 {
            return Err(usage(
                format!("scene {n}: who names more than 4 people"),
                "A picture takes at most 4 references.".into(),
            ));
        }
        let in_scene: Vec<usize> = (0..cast.len())
            .filter(|&c| {
                gs.character != Some(false)
                    && gs.image.is_none()
                    && (gs.who.is_empty()
                        || cast[c].name.as_ref().is_some_and(|m| gs.who.contains(m)))
            })
            .collect();
        if in_scene.len() > 4 {
            return Err(usage(
                format!("scene {n}: {} characters in one picture", in_scene.len()),
                "A picture takes at most 4 references: name them with `who`.".into(),
            ));
        }
        // The service takes 40 MB a request, and base64 adds a third.
        let ref_bytes: u64 = in_scene
            .iter()
            .filter_map(|&c| cast[c].file.as_ref())
            .filter_map(|f| std::fs::metadata(f).ok())
            .map(|m| m.len())
            .sum();
        if ref_bytes > 28_000_000 {
            return Err(usage(
                format!(
                    "scene {n}: the reference pictures are too big together ({} MB)",
                    ref_bytes / 1_000_000
                ),
                "Make them smaller: 2 MB each is plenty.".into(),
            ));
        }
        let with_character = !in_scene.is_empty();
        let char_key: Vec<&String> = in_scene.iter().map(|&c| &char_ids[c]).collect();
        // A still can come from this machine, after `setup image-local`. A clip cannot yet.
        if a.local_only && !(gs.still == Some(true) && crate::provision::image_ready()) {
            return Err(Box::new(needs(voices::needs_input(
                "generate",
                "dependency",
                "A generated scene needs the network, except a still after `scriptvox setup image-local`. Drop `generate` from the brief, or run without --local-only.",
                vec![json!({"args": [], "cost_usd": 0, "note": "use `media` or `stock` for that scene"})],
            ))));
        }
        if a.cache_only {
            return Err(usage(
                format!("scene {n}: replay cannot remake a generated clip from the cache"),
                "Run `scriptvox make --brief` again with the same brief: it finds the saved clip in its run.".into(),
            ));
        }
        if let Some((why, fix)) = gs.model.as_deref().and_then(crate::generators::problem) {
            return Err(usage(format!("scene {n}: {why}"), fix));
        }
        if gs.prompt.trim().is_empty() {
            return Err(usage(
                format!("scene {n}: the generate prompt is empty"),
                "Describe the shot: subject, action, camera.".into(),
            ));
        }
        let (image, hash) = match &gs.image {
            Some(p) => match crate::generate::image_uri(p) {
                Ok((u, h)) => (Some(u), Some(h)),
                Err(env) => return Err(env),
            },
            None => (None, None),
        };
        let (last, last_hash) = match &gs.last_frame {
            Some(p) => match crate::generate::image_uri(p) {
                Ok((u, h)) => (Some(u), Some(h)),
                Err(env) => return Err(env),
            },
            None => (None, None),
        };
        if last.is_some()
            && (gs.still == Some(true) || (image.is_none() && gs.first_frame.is_none()))
        {
            return Err(usage(
                format!("scene {n}: last_frame needs a clip with `image` or `first_frame`"),
                "Give the first frame too: the clip goes from it to the last frame.".into(),
            ));
        }
        if image.is_some() && gs.first_frame.is_some() {
            return Err(usage(
                format!("scene {n}: give `image` or `first_frame`, not both"),
                "Keep the one you want as the first frame.".into(),
            ));
        }
        let styled = match &a.style {
            Some(st) => format!("{}, {st}", gs.prompt.trim_end_matches(['.', ' '])),
            None => gs.prompt.clone(),
        };
        // A clip carries the character through its first frame, so it gets one.
        let first_frame = gs
            .first_frame
            .clone()
            .or_else(|| (with_character && gs.still != Some(true)).then(|| styled.clone()));
        let frame_row = match &first_frame {
            None => None,
            Some(fp) if fp.trim().is_empty() => {
                return Err(usage(format!("scene {n}: first_frame is empty"), "Describe the opening picture.".into()))
            }
            Some(_) => match frame_row(&rows, with_character, &a.aspect) {
                Some(g) => Some(g),
                None => {
                    return Err(Box::new(needs(voices::needs_input(
                        "generate.image_row",
                        "dependency",
                        &format!("Scene {n} asks for a first frame, but no image row with a key fits on this install. Give `image` instead, run `scriptvox setup image-local` for a free local one, or see `scriptvox providers`.{}", not_answering_note()),
                        vec![json!({"args": [], "cost_usd": 0, "note": "drop first_frame: text to video"})],
                    ))))
                }
            },
        };
        // A still: one picture from an image row, with no video row at all.
        if gs.still == Some(true) {
            let found = rows
                .iter()
                .filter(|g| {
                    g.kind == "image.gen"
                        && (!with_character || g.takes_reference)
                        && gs.model.as_deref().is_none_or(|m| g.name == m)
                        && g.aspects.iter().any(|x| x == gen_aspect(&a.aspect))
                        && g.ready()
                        && (!a.local_only || g.gateway == "local")
                        && answers(g)
                })
                .min_by(|x, y| x.cost(1.0).total_cmp(&y.cost(1.0)));
            let Some(row) = found else {
                return Err(Box::new(needs(voices::needs_input(
                    "generate.image_row",
                    "dependency",
                    &format!("Scene {n} asks for a still, but no image row with a key fits on this install. Use `media` or `stock`, run `scriptvox setup image-local` for a free local one, or see `scriptvox providers`.{}", not_answering_note()),
                    vec![json!({"args": [], "cost_usd": 0, "note": "use `media` or `stock` for that scene"})],
                ))));
            };
            let prompt = styled;
            let input = row.with_seed(
                row.request(&crate::generators::GenInput {
                    prompt: prompt.clone(),
                    aspect: gen_aspect(&a.aspect).to_string(),
                    ..Default::default()
                }),
                a.seed,
            );
            out.push(Some(GenShot {
                cost: row.cost_for(1.0, 0.0, in_scene.len()),
                listed: 0.0,
                row: row.clone(),
                image: false,
                still: true,
                key: String::new(),
                made: false,
                seconds: 0.0,
                id_input: with_cast(
                    json!({"model": row.name, "input": input, "usd_per_s": row.usd_per_s, "usd_each": row.usd_each}),
                    &char_key,
                ),
                prompt,
                frame: None,
                cast: in_scene.clone(),
                input,
            }));
            continue;
        }
        let needs_i2v = image.is_some() || frame_row.is_some();
        let want = gs.seconds.or(sc.seconds).unwrap_or_else(|| {
            pauses::strip(&sc.script).split_whitespace().count() as f64
                / fit::words_per_s(lang, a.tts.as_deref().unwrap_or("edge"))
        });
        let fits = |g: &crate::generators::Generator| {
            g.kind == "video.gen"
                // Priced by the length of a source video: a scene has none, so a plan cannot price it.
                && g.usd_per_input_s == 0.0
                && g.aspects.iter().any(|x| x == gen_aspect(&a.aspect))
                && (!needs_i2v || g.i2v.is_some())
                && (last.is_none() || g.takes_last_frame.is_some())
        };
        let row = match &gs.model {
            Some(m) => match rows.iter().find(|g| &g.name == m) {
                None => {
                    let names: Vec<_> = rows.iter().map(|g| g.name.as_str()).collect();
                    return Err(usage(
                        format!("scene {n}: unknown model {m}"),
                        format!("Use one of: {}. To add one: `scriptvox models add <hf id, local file or fal endpoint>`.", names.join(", ")),
                    ));
                }
                Some(g)
                    if g.admission(None)
                        .is_some_and(|b| b.kind == "retired" || b.kind == "price") =>
                {
                    let b = g.admission(None).unwrap();
                    let fix = if b.kind == "price" {
                        "A plan cannot price this row. Add its price to the row, or make the clip with `scriptvox generate --max-spend` first.".to_string()
                    } else {
                        b.fix
                    };
                    return Err(usage(format!("scene {n}: {}", b.why), fix));
                }
                Some(g) if !fits(g) => {
                    // The rows that can make this scene, by name: the user need not read the whole list.
                    let can: Vec<_> = rows
                        .iter()
                        .filter(|g| fits(g))
                        .map(|g| g.name.as_str())
                        .collect();
                    let which = if needs_i2v && g.i2v.is_none() {
                        "Rows that take a start picture"
                    } else {
                        "Rows that fit"
                    };
                    return Err(usage(
                        format!("scene {n}: {m} does not fit: {}", misfit(g, gen_aspect(&a.aspect), needs_i2v)),
                        format!(
                            "{which} for this scene: {}. Name one as `model`, or drop `model` to let scriptvox pick. `scriptvox providers --kind video` shows `takes_start_picture` for each row.",
                            if can.is_empty() { "none on this install".to_string() } else { can.join(", ") }
                        ),
                    ));
                }
                Some(g) => g,
            },
            None => {
                let mut ok: Vec<_> = rows
                    .iter()
                    .filter(|g| fits(g) && g.ready() && answers(g))
                    .collect();
                ok.sort_by(|x, y| {
                    x.cost(x.duration_for(want))
                        .total_cmp(&y.cost(y.duration_for(want)))
                });
                // The only rows that fit are on a service that does not answer: stop here, with what to do.
                let dead = rows.iter().find(|g| fits(g) && g.ready() && !answers(g));
                match ok.first() {
                    Some(g) => g,
                    None if dead.is_some() => return Err(service_down(n, dead.unwrap())),
                    None => match rows.iter().find(|g| fits(g)) {
                        Some(g) => g,
                        None if last.is_some() => {
                            return Err(usage(
                                format!("scene {n}: no generator row takes a last frame"),
                                "Drop last_frame, or add a row with `takes_last_frame` (see `scriptvox providers`).".into(),
                            ))
                        }
                        None => {
                            return Err(usage(
                                format!("scene {n}: no generator row fits aspect {}", a.aspect),
                                "Change the aspect.".into(),
                            ))
                        }
                    },
                }
            }
        };
        // A row named in the brief, on a service that does not answer: no other row is taken in its place.
        if gs.model.is_some() && row.ready() && !answers(row) {
            return Err(service_down(n, row));
        }
        if let Some(b) = row.why_not().filter(|b| b.kind == "dependency") {
            return Err(Box::new(needs(voices::needs_input(
                "generate.row",
                b.kind,
                &format!("Scene {n}: {}. {}", b.why, b.fix),
                vec![
                    json!({"args": [], "cost_usd": 0, "note": "use `media` or `stock` for that scene"}),
                ],
            ))));
        }
        if crate::keys::resolve(&row.key).is_none() {
            return Err(Box::new(needs(voices::needs_input(
                &format!("generate.{}.api_key", row.key),
                "secret",
                &format!(
                    "Scene {n} needs a {} key. Run `scriptvox keys set {}` in your terminal.",
                    row.key, row.key
                ),
                vec![
                    json!({"args": [], "cost_usd": 0, "note": "use `media` or `stock` for that scene"}),
                ],
            ))));
        }
        let seconds = row.duration_for(want);
        // A clip row never sees the reference picture, and leaves the first frame by its own words (5 Nov 2026: by frame
        // 15 a navy abaya turned grey and glasses became sunglasses). So its prompt names the character too.
        let words: Vec<&str> = in_scene
            .iter()
            .filter_map(|&c| cast[c].prompt.as_deref())
            .map(|w| w.trim_end_matches(['.', ' ']))
            .collect();
        let prompt = if words.is_empty() {
            styled
        } else {
            format!("{}. {styled}", words.join(", and "))
        };
        let input = row.with_seed(
            row.input(&prompt, seconds, gen_aspect(&a.aspect), image.as_deref()),
            a.seed,
        );
        // `fits` and the named-row check above make sure the row takes it.
        let input = row
            .with_last_frame(input.clone(), last.as_deref())
            .unwrap_or(input);
        let mut id_input = input.clone();
        if let Some(h) = &hash {
            id_input[row.image_key()] = json!(format!("blake3:{h}"));
        }
        if let (Some(h), Some(f)) = (&last_hash, &row.takes_last_frame) {
            id_input[f.as_str()] = json!(format!("blake3:{h}"));
        }
        let frame = frame_row.map(|fr| {
            let req = fr.with_seed(
                fr.request(&crate::generators::GenInput {
                    prompt: first_frame.clone().unwrap_or_default(),
                    aspect: gen_aspect(&a.aspect).to_string(),
                    ..Default::default()
                }),
                a.seed,
            );
            (fr, req)
        });
        out.push(Some(GenShot {
            // A paid first frame is part of the bill.
            cost: row.cost_for(seconds, 0.0, usize::from(image.is_some() && row.takes_reference))
                + frame
                    .as_ref()
                    .map_or(0.0, |(fr, _)| fr.cost_for(1.0, 0.0, in_scene.len())),
            listed: 0.0,
            row: row.clone(),
            image: needs_i2v,
            still: false,
            key: String::new(),
            made: false,
            seconds,
            id_input: with_cast(
                json!({"model": row.name, "input": id_input, "usd_per_s": row.usd_per_s,
                    "first_frame": frame.as_ref().map(|(fr, req): &(crate::generators::Generator, Value)| json!({"model": fr.name, "input": req, "usd_each": fr.usd_each}))}),
                &char_key,
            ),
            prompt,
            frame,
            cast: in_scene.clone(),
            input,
        }));
    }
    // A clip already made (same row, same input) comes back free: a `resume` that changes only the look must not make
    // new pictures or pay again (fresh-agent test 4). Change a word of the prompt for a new take.
    let cas = Cas::open();
    for g in out.iter_mut().flatten() {
        g.key = store::key("gen", "1", &g.id_input);
        g.listed = g.cost;
        if cas.has(&g.key) {
            g.made = true;
            g.cost = 0.0;
        }
    }
    Ok(out)
}

/// The stop for a scene whose row is on a service of the user's own that does not answer now.
fn service_down(n: usize, g: &crate::generators::Generator) -> Box<Envelope> {
    Box::new(needs(voices::needs_input(
        "generate.row",
        "dependency",
        &format!(
            "Scene {n}: the row {} is on your own GPU service ({}), and it does not answer now. Start the service (`scriptvox providers test {}` checks it), or name another row as `model` (`scriptvox providers --kind video --ready`).",
            g.name,
            g.base.as_deref().unwrap_or("its `base`"),
            g.name
        ),
        vec![json!({"args": [], "cost_usd": 0, "note": "use `media` or `stock` for that scene"})],
    )))
}

/// On while a pick is made again because a row of the first pick does not answer: [`answers`] then asks each row.
/// ponytail: a process flag, one `make` in a process; a parameter through the pickers if that ever changes.
static ASKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The pick made by `pick` again, with every row asked if its service answers.
fn asking<T>(pick: impl FnOnce() -> T) -> T {
    ASKING.store(true, std::sync::atomic::Ordering::Relaxed);
    let out = pick();
    ASKING.store(false, std::sync::atomic::Ordering::Relaxed);
    out
}

/// A row that must run now and is on a service of the user's own that does not answer. Not asked for a run that
/// may not use the network.
fn is_down(a: &Args, g: &crate::generators::Generator) -> bool {
    !a.local_only && !a.cache_only && g.answers_now() == Some(false)
}

/// Rows that were not chosen because their own GPU service does not answer now, for the plan's finding.
static NOT_ANSWERING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// False for a row on the user's own GPU service that does not answer now: such a row is never chosen silently
/// (docs/specs/reel-gaps.md). The row is kept for the plan's finding and for the error when no other row fits.
fn answers(g: &crate::generators::Generator) -> bool {
    // The rows are picked as before first, and what is in the store is taken from there. Only for what must still be
    // made is the service asked, and the pick made again (`plan_generated`, `character_pick`, `storyboard`).
    if !ASKING.load(std::sync::atomic::Ordering::Relaxed) || g.answers_now() != Some(false) {
        return true;
    }
    if let Ok(mut l) = NOT_ANSWERING.lock() {
        if !l.contains(&g.name) {
            l.push(g.name.clone());
        }
    }
    false
}

/// The rows [`answers`] refused, as a sentence for an error that found no row. Empty when there are none.
fn not_answering_note() -> String {
    let l = NOT_ANSWERING.lock().map(|l| l.clone()).unwrap_or_default();
    if l.is_empty() {
        return String::new();
    }
    format!(
        " Your own GPU service does not answer now, so {} cannot run: start the service, or check its address with `scriptvox providers test {}`.",
        l.join(", "),
        l[0]
    )
}

/// The plan's finding for the rows [`answers`] refused: said, with the row that is used in the stages.
fn not_answering_finding() -> Option<Value> {
    let l = NOT_ANSWERING.lock().map(|l| l.clone()).unwrap_or_default();
    (!l.is_empty()).then(|| json!({"gate": "services", "status": "warn",
        "detail": format!("{} is on your own GPU service, and it does not answer now: it is not used. The stages name the row that is used in its place", l.join(", ")),
        "fix": format!("Start the service to use it (`scriptvox providers test {}` checks it), or go on with the rows in the stages.", l[0])}))
}

/// `; your own service answers now` for the plan line of a row on the user's own GPU service.
fn answers_words(a: &Args, g: &crate::generators::Generator) -> &'static str {
    match g.answers_now() {
        Some(true) if !a.local_only && !a.cache_only => "; your own service answers now",
        _ => "",
    }
}

/// The row, the request and the store key of a character's picture, and whether the store has it. The first row
/// as before; when the picture must still be made and that row's service does not answer, the next row that does.
fn character_pick(
    a: &Args,
    c: &crate::brief::Character,
) -> Option<(crate::generators::Generator, Value, String, bool)> {
    let of = |row: crate::generators::Generator| {
        let (req, key) = character_request(a, c, &row);
        let made = Cas::open().has(&key);
        (row, req, key, made)
    };
    let first = of(character_row(a)?);
    if first.3 || !is_down(a, &first.0) {
        return Some(first);
    }
    asking(|| character_row(a)).map(of)
}

/// The image row that makes a character's picture: the first one that takes a reference picture and can run now.
/// The plan and the run both ask here.
fn character_row(a: &Args) -> Option<crate::generators::Generator> {
    crate::generators::all().into_iter().find(|g| {
        g.takes_reference
            && g.ready()
            && !g.unknown_price()
            && (!a.local_only || g.gateway == "local")
            && answers(g)
    })
}

/// The request for a character's picture and its key in the content store.
fn character_request(
    a: &Args,
    c: &crate::brief::Character,
    row: &crate::generators::Generator,
) -> (Value, String) {
    let mut prompt = format!(
        "{}, full body, standing, plain grey background",
        c.prompt
            .as_deref()
            .unwrap_or_default()
            .trim_end_matches(['.', ' '])
    );
    if let Some(st) = &a.style {
        prompt = format!("{prompt}, {st}");
    }
    let req = row.request(&crate::generators::GenInput {
        prompt,
        aspect: gen_aspect(&a.aspect).to_string(),
        ..Default::default()
    });
    let key = store::key(
        "gen",
        "1",
        &json!({"model": row.name, "input": req, "character": true}),
    );
    (req, key)
}

/// The image row for a first frame, and for a storyboard still: the cheapest row that fits the frame and can run, so
/// a free row is used before a paid one.
fn frame_row(
    rows: &[crate::generators::Generator],
    with_character: bool,
    aspect: &str,
) -> Option<crate::generators::Generator> {
    rows.iter()
        .filter(|g| {
            g.kind == "image.gen"
                && (!with_character || g.takes_reference)
                && g.aspects.iter().any(|x| x == gen_aspect(aspect))
                && g.ready()
                && answers(g)
        })
        .min_by(|x, y| x.cost(1.0).total_cmp(&y.cost(1.0)))
        .cloned()
}

/// The store key of a first frame: the image row and the request it gets. `make --storyboard` keeps its stills under
/// it, and `make` looks there before it makes a first frame.
fn frame_key(row: &crate::generators::Generator, req: &Value) -> String {
    store::key(
        "gen",
        "1",
        &json!({"model": row.name, "input": req, "first_frame": true}),
    )
}

/// `make --storyboard` (G054): one still per clip scene, then one sheet with the scene numbers, and stop. `video` is
/// the plan of the whole video, for `next`; the storyboard has a plan of its own that holds the stills only, so an
/// approval here never covers a clip. No voice, no clip and no render: this runs before any of them.
fn storyboard(a: &Args, gen: &[Option<GenShot>], video: &plan::Plan) -> Envelope {
    struct Panel {
        scene: usize,
        /// The image row and its request. None: the scene has its own `image`, which is the panel.
        job: Option<(crate::generators::Generator, Value, String)>,
        own: Option<PathBuf>,
        prompt: String,
        from: &'static str,
        made: bool,
    }
    let fail = |exit, msg: String, fix: Option<&str>| {
        Envelope::fail("make", exit, msg, fix.map(String::from))
    };
    let rows = crate::generators::all();
    let cas = Cas::open();
    let mut panels = Vec::new();
    for (i, g) in gen.iter().enumerate() {
        let Some(g) = g.as_ref().filter(|g| !g.still) else {
            continue;
        };
        let own = a
            .scenes
            .as_ref()
            .and_then(|s| s.get(i))
            .and_then(|s| s.generate.as_ref())
            .and_then(|gs| gs.image.clone());
        let (job, prompt, from) = match (&own, &g.frame) {
            (Some(_), _) => (None, g.prompt.clone(), "image"),
            (None, Some((fr, req))) => {
                let prompt = req["prompt"].as_str().unwrap_or(&g.prompt).to_string();
                (Some((fr.clone(), req.clone())), prompt, "first_frame")
            }
            (None, None) => {
                let Some(fr) = frame_row(&rows, false, &a.aspect) else {
                    return needs(voices::needs_input(
                        "generate.image_row",
                        "dependency",
                        &format!("A storyboard needs an image row, and none with a key fits on this install. Run `scriptvox setup image-local` for a free local one, or see `scriptvox providers --kind image`.{}", not_answering_note()),
                        vec![],
                    ));
                };
                let req = fr.with_seed(
                    fr.request(&crate::generators::GenInput {
                        prompt: g.prompt.clone(),
                        aspect: gen_aspect(&a.aspect).to_string(),
                        ..Default::default()
                    }),
                    a.seed,
                );
                (Some((fr, req)), g.prompt.clone(), "prompt")
            }
        };
        // A still that must be made, on a service that does not answer: the next image row that does.
        let job = match job {
            Some((fr, req))
                if from == "prompt" && is_down(a, &fr) && !cas.has(&frame_key(&fr, &req)) =>
            {
                let Some(fr) = asking(|| frame_row(&rows, false, &a.aspect)) else {
                    return needs(voices::needs_input(
                        "generate.image_row",
                        "dependency",
                        &format!("A storyboard needs an image row, and none that can run now fits on this install. Run `scriptvox setup image-local` for a free local one, or see `scriptvox providers --kind image`.{}", not_answering_note()),
                        vec![],
                    ));
                };
                let req = fr.with_seed(
                    fr.request(&crate::generators::GenInput {
                        prompt: g.prompt.clone(),
                        aspect: gen_aspect(&a.aspect).to_string(),
                        ..Default::default()
                    }),
                    a.seed,
                );
                Some((fr, req))
            }
            job => job,
        };
        let job = job.map(|(fr, req)| {
            let key = frame_key(&fr, &req);
            (fr, req, key)
        });
        panels.push(Panel {
            scene: i + 1,
            made: job.as_ref().is_some_and(|(_, _, k)| cas.has(k)),
            job,
            own,
            prompt,
            from,
        });
    }
    if panels.is_empty() {
        return fail(
            Exit::Usage,
            "nothing to storyboard: no scene of this brief generates a clip".into(),
            Some("A storyboard shows the scenes with `generate` (and not `still`). Run `make` without --storyboard."),
        );
    }
    // The storyboard's own plan: the stills, and nothing of the video.
    let price = |fr: &crate::generators::Generator| fr.cost_for(1.0, 0.0, 0);
    let stages: Vec<plan::Stage> = panels
        .iter()
        .filter_map(|p| Some((p, p.job.as_ref()?)))
        .map(|(p, (fr, _, _))| plan::Stage {
            stage: "storyboard",
            tool: format!("image.gen.{}", fr.name),
            cost_usd: if p.made { 0.0 } else { price(fr) },
            network: !p.made && fr.gateway != "local",
            explain: if p.made {
                format!("scene {}: already made: no call and no charge", p.scene)
            } else {
                format!(
                    "scene {}: one still with {} through {}, {}{}",
                    p.scene,
                    fr.name,
                    fr.gateway,
                    fr.cost_words(1.0),
                    fr.external_note() + answers_words(a, fr)
                )
            },
        })
        .collect();
    let cost: f64 = stages.iter().map(|s| s.cost_usd).sum();
    // The id holds the full price of each still, not the store's discount (the rule of generated clips).
    let plan_id = plan::plan_id(&json!({
        "command": "storyboard",
        "stills": panels.iter().map(|p| match &p.job {
            Some((fr, req, _)) => json!({"scene": p.scene, "model": fr.name, "input": req, "usd_each": fr.usd_each, "price": price(fr)}),
            None => json!({"scene": p.scene, "image": p.own}),
        }).collect::<Vec<_>>(),
    }));
    let stages_json = json!(stages);
    let video_plan = json!({"plan_id": video.plan_id, "cost_usd": video.cost_usd});
    let next = format!(
        "Run the same command without --storyboard: the video, ${:.2} by its own plan {}{}.",
        video.cost_usd,
        video.plan_id,
        if video.cost_usd > 0.0 {
            ", which needs its own --approve"
        } else {
            ""
        }
    );
    if a.plan_only {
        if let Some(env) = plan::over_cap("plan", &plan_id, cost, a.max_spend, vec![]) {
            return env.with("stages", stages_json);
        }
        let mut env = Envelope::ok("plan");
        env.plan_id = Some(plan_id.clone());
        env.findings.extend(not_answering_finding());
        env.metrics.insert("cost_usd".into(), json!(cost));
        env.next = vec![if cost > 0.0 {
            format!("scriptvox make ... --storyboard --approve {plan_id}")
        } else {
            "Run the same command without --plan.".to_string()
        }];
        return env
            .with("stages", stages_json)
            .with("make_plan", video_plan);
    }
    if let Some(mut env) = approval_stop(
        &plan_id,
        cost,
        a.approve.as_deref(),
        a.max_spend,
        stages_json.clone(),
    ) {
        if env
            .next
            .first()
            .is_some_and(|n| n.starts_with("scriptvox make ..."))
        {
            env.next = vec![format!(
                "scriptvox make ... --storyboard --approve {plan_id}"
            )];
        }
        return env;
    }
    let id = plan::run_id_for(
        &plan_id,
        a.idempotency_key.as_deref().unwrap_or("storyboard"),
    );
    let dir = Path::new(".scriptvox/runs").join(&id);
    let out = PathBuf::from("scriptvox-out").join(format!("{id}-storyboard"));
    let stop = |e: String| {
        let mut env = fail(Exit::Error, e, Some("Check the folder, then run the command again: a still that is made is not made again."));
        env.plan_id = Some(plan_id.clone());
        env
    };
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::create_dir_all(&out)) {
        return stop(e.to_string());
    }
    let cap = a.max_spend.unwrap_or(cost);
    let mut spent = 0.0;
    let mut stills = Vec::new();
    let mut files = Vec::new();
    for p in &panels {
        let (path, paid, reused) = match (&p.job, &p.own) {
            (Some((fr, req, key)), _) => {
                let shot = format!("scene{}_board", p.scene);
                // Out of the store into a folder of its own: its name there may be another scene's.
                let stored = cas
                    .get(key, &dir.join(format!(".{shot}")))
                    .and_then(|f| f.into_iter().next());
                match stored {
                    Some((_, path)) => (path, 0.0, true),
                    None => {
                        eprintln!(
                            "scriptvox: {id}: storyboard still for scene {} with {}",
                            p.scene, fr.name
                        );
                        match crate::genjob::run_clip(
                            fr,
                            req,
                            false,
                            Path::new("."),
                            &dir,
                            &shot,
                            &id,
                            cap,
                            GEN_WAIT_S,
                        ) {
                            Ok((path, paid)) => {
                                let name = format!("{shot}.{}", fr.ext());
                                let terms = serde_json::to_value(&fr.terms).unwrap_or_default();
                                let _ =
                                    cas.put(key, &fr.name, "1", &[(name.as_str(), &path)], terms);
                                (path, paid, false)
                            }
                            Err(e) => {
                                let mut env = fail(e.exit, e.message, Some(&e.fix));
                                env.plan_id = Some(plan_id.clone());
                                env.run_id = Some(id.clone());
                                return env;
                            }
                        }
                    }
                }
            }
            (None, Some(own)) => (own.clone(), 0.0, false),
            (None, None) => continue,
        };
        spent += paid;
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("png");
        let file = out.join(format!("scene{}.{ext}", p.scene));
        if let Err(e) = std::fs::copy(&path, &file) {
            return stop(format!("{}: {e}", file.display()));
        }
        let row = p.job.as_ref().map(|(fr, _, _)| fr.name.clone());
        stills.push(json!({"scene": p.scene, "file": std::path::absolute(&file).unwrap_or(file.clone()),
            "row": row, "prompt": p.prompt, "from": p.from, "cost_usd": paid, "reused": reused,
            // Only a `first_frame` scene starts from its still in `make`: the brief asked for image to video there.
            "make_reuses": p.from == "first_frame" && gen[p.scene - 1].as_ref().is_some_and(|g| g.cast.is_empty())}));
        files.push((p.scene, file));
    }
    // The sheet: the stills in a grid, the scene number on each. Only numbers go into the graph and the ASS file.
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("make", e),
    };
    let fc = match write_fonts(&dir) {
        Ok(fc) => fc,
        Err(e) => return stop(e.to_string()),
    };
    let (w, h) = match gen_aspect(&a.aspect) {
        "16:9" => (360, 202),
        "1:1" => (360, 360),
        _ => (360, 640),
    };
    let n = files.len();
    let cols = if n <= 3 {
        n
    } else {
        (n as f64).sqrt().ceil() as usize
    };
    let gap = if n > 1 { 12 } else { 0 };
    let mut ass = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {}\nPlayResY: {}\nScaledBorderAndShadow: yes\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: N,{},44,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,4,0,7,0,0,0,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        cols * (w + gap),
        n.div_ceil(cols) * (h + gap),
        font_for("en").0
    );
    let mut args: Vec<String> = Vec::new();
    for (i, (scene, file)) in files.iter().enumerate() {
        args.extend([
            "-i".into(),
            std::path::absolute(file)
                .unwrap_or(file.clone())
                .to_string_lossy()
                .into_owned(),
        ]);
        ass.push_str(&format!(
            "Dialogue: 0,0:00:00.00,0:10:00.00,N,,0,0,0,,{{\\pos({},{})}}{scene}\n",
            (i % cols) * (w + gap) + 10,
            (i / cols) * (h + gap) + 6
        ));
    }
    if let Err(e) = std::fs::write(dir.join("board.ass"), ass) {
        return stop(e.to_string());
    }
    let sheet = out.join("storyboard.png");
    let sheet_abs = std::path::absolute(&sheet).unwrap_or(sheet);
    args.extend([
        "-filter_complex".into(),
        format!(
            "{};[o]{}[s]",
            gates::tile_graph(
                n,
                cols,
                &format!(
                    "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1"
                )
            ),
            ass_filter("board.ass", "fonts")
        ),
        "-map".into(),
        "[s]".into(),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
        sheet_abs.to_string_lossy().into_owned(),
    ]);
    // The run folder is the cwd: the ASS file and the fonts are relative names.
    if let Err(e) = ff.run(&dir, &args, Some(&std::path::absolute(&fc).unwrap_or(fc))) {
        return stop(format!("the sheet could not be written: {e}"));
    }
    let mut env = Envelope::ok("make");
    env.run_id = Some(id);
    env.plan_id = Some(plan_id);
    env.artifacts.insert("storyboard".into(), json!(sheet_abs));
    env.artifacts.insert(
        "stills".into(),
        json!(stills.iter().map(|s| s["file"].clone()).collect::<Vec<_>>()),
    );
    env.metrics.insert("cost_usd".into(), json!(spent));
    env.metrics.insert("stills".into(), json!(stills.len()));
    env.next = vec![next];
    env.with("stills", json!(stills))
        .with("stages", stages_json)
        .with("make_plan", video_plan)
}

/// A scene's content key with its cast. No field when there is none: a clip made before this field keeps its key, or
/// it would be made and charged again.
fn with_cast(mut id: Value, cast: &[&String]) -> Value {
    if !cast.is_empty() {
        id["character"] = json!(cast);
    }
    id
}

/// The brief's `character`, or its `characters`.
fn cast_of(a: &Args) -> Vec<&crate::brief::Character> {
    a.character.iter().chain(a.characters.iter()).collect()
}

/// One character as a data URI: its file, or one picture from an image row. The picture is kept in the content
/// store, so `resume` and later runs of the same prompt show the same person.
fn character_picture(
    a: &Args,
    c: &crate::brief::Character,
    n: usize,
    dir: &Path,
    id: &str,
    budget: f64,
) -> Result<String, Box<Envelope>> {
    // With both, the file is the picture and the prompt is for the clip rows.
    if let Some(f) = &c.file {
        return Ok(crate::generate::image_uri(f)?.0);
    }
    let Some((row, req, key, _)) = character_pick(a, c) else {
        return Err(Box::new(Envelope::fail(
            "make",
            Exit::NeedsInput,
            "no image row that takes a reference picture has a key",
            Some(format!(
                "Run `scriptvox setup image-local` for a free local one, or see `scriptvox providers`.{}",
                not_answering_note()
            )),
        )));
    };
    // `character.png`, or one per person.
    let job = match n {
        0 => "character".to_string(),
        n => format!("character{}", n + 1),
    };
    let stored = format!("{job}.png");
    let cas = Cas::open();
    // Out of the store into a folder of its own: the name it was kept under may belong to another person of this video.
    let got = cas
        .get(&key, &dir.join(".cast").join(&job))
        .and_then(|f| f.into_iter().next());
    let path = match got {
        Some((_, p)) => {
            let here = dir.join(&stored);
            if std::fs::copy(&p, &here).is_ok() {
                here
            } else {
                p
            }
        }
        None => {
            let (p, _) = crate::genjob::run_clip(
                &row,
                &req,
                false,
                Path::new("."),
                dir,
                &job,
                id,
                budget,
                GEN_WAIT_S,
            )
            .map_err(|e| Box::new(Envelope::fail("make", e.exit, e.message, Some(e.fix))))?;
            let terms = serde_json::to_value(&row.terms).unwrap_or_default();
            let _ = cas.put(&key, &row.name, "1", &[(stored.as_str(), &p)], terms);
            p
        }
    };
    Ok(crate::generate::image_uri(&path)?.0)
}

/// The first row that `fits` a brief field and has a key. Licence terms play no part: they are data. `make` and the
/// plan both ask here, so a brief that cannot run fails at `plan` with the same message.
fn pick_row(
    field: &str,
    what: &str,
    fits: impl Fn(&crate::generators::Generator) -> bool,
) -> Result<crate::generators::Generator, crate::genjob::JobErr> {
    let rows: Vec<_> = crate::generators::all()
        .into_iter()
        .filter(|g| fits(g) && !g.paid())
        .collect();
    if let Some(g) = rows.iter().find(|g| crate::keys::resolve(&g.key).is_some()) {
        return Ok(g.clone());
    }
    // A free row runs only on the user's own GPU service; say so, a key alone cannot help.
    let own = rows
        .iter()
        .find(|g| g.gateway == "selfhost")
        .map_or(String::new(), |g| {
            format!(
                " The free {what} rows run on your own GPU service: {}.",
                g.deploy_step()
            )
        });
    Err(crate::genjob::JobErr {
        exit: Exit::NeedsInput,
        message: format!("the brief asks for {field}, but no free {what} row has a key"),
        fix: format!("Drop `{field}` from the brief, or see `scriptvox providers`.{own}"),
        detail: None,
    })
}

fn is_v2a(g: &crate::generators::Generator) -> bool {
    g.kind == "sfx.gen" && g.t2v.as_deref().is_some_and(|e| e.ends_with("/v2a"))
}

/// One generated clip through the first free row of `kind` that takes a clip and gives one back (`upscale` for
/// brief `upscale: true`, `interpolate` for `smooth: true`); resumed from the run folder on a rerun.
fn row_clip(
    clip: &Path,
    dir: &Path,
    n: usize,
    run: &str,
    kind: &str,
) -> Result<PathBuf, crate::genjob::JobErr> {
    use base64::Engine as _;
    let field = if kind == "upscale" {
        "upscale"
    } else {
        "smooth"
    };
    let row = pick_row(field, kind, |g| g.kind == kind)?;
    let bytes = std::fs::read(clip).map_err(|e| crate::genjob::JobErr {
        exit: Exit::Error,
        message: format!("{}: {e}", clip.display()),
        fix: "Run make again; the clip is fetched again.".into(),
        detail: None,
    })?;
    let req = row.request(&crate::generators::GenInput {
        video: Some(format!(
            "data:video/mp4;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )),
        ..Default::default()
    });
    crate::genjob::run_clip(
        &row,
        &req,
        false,
        Path::new("."),
        dir,
        &format!(
            "scene{n}_{}",
            if kind == "upscale" { "up" } else { "smooth" }
        ),
        run,
        0.0,
        GEN_WAIT_S,
    )
    .map(|(p, _)| p)
}

/// Sound made from one scene's clip by a row that takes a video (brief `ambience: true`), as long as the shorter of
/// the clip and the scene (`want`), at most 10 s. The row gets a small copy (480 p, no sound, cut to that length), so
/// a long 4K stock clip stays under the upload limit. Resumed from the run folder on a rerun.
#[allow(clippy::too_many_arguments)]
fn ambience_clip(
    ff: &Ffmpeg,
    clip: &Path,
    from: f64,
    want: f64,
    prompt: &str,
    dir: &Path,
    n: usize,
    run: &str,
) -> Result<PathBuf, crate::genjob::JobErr> {
    use base64::Engine as _;
    let err = |exit, message: String, fix: &str| crate::genjob::JobErr {
        exit,
        message,
        fix: fix.into(),
        detail: None,
    };
    let row = pick_row("ambience", "sound-from-video", is_v2a)?;
    let secs = ff
        .duration(clip)
        .unwrap_or(want)
        .min(want)
        .round()
        .clamp(1.0, 10.0);
    let small = dir.join(format!("scene{n}_sound_in.mp4"));
    let args: Vec<String> = [
        "-ss".to_string(),
        format!("{from:.3}"),
        "-i".into(),
        clip.to_string_lossy().into_owned(),
        "-t".into(),
        format!("{secs:.3}"),
        "-an".into(),
        "-vf".into(),
        "scale=-2:480".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "28".into(),
        small.to_string_lossy().into_owned(),
    ]
    .to_vec();
    let bytes = ff
        .run(dir, &args, None)
        .and_then(|_| std::fs::read(&small).map_err(|e| e.to_string()))
        .map_err(|e| {
            err(
                Exit::Error,
                format!("{}: {e}", clip.display()),
                "Run make again; the clip is read again.",
            )
        })?;
    let req = row.request(&crate::generators::GenInput {
        prompt: prompt.into(),
        seconds: secs,
        video: Some(format!(
            "data:video/mp4;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )),
        ..Default::default()
    });
    crate::genjob::run_clip(
        &row,
        &req,
        false,
        Path::new("."),
        dir,
        &format!("scene{n}_sound"),
        run,
        0.0,
        GEN_WAIT_S,
    )
    .map(|(p, _)| p)
}

/// The base video's sound as a WAV for the bed slot; with `strip_voice`, sent to a stem row first so only
/// its music and ambience come back. Resumed from the run folder on a rerun.
fn base_sound(
    ff: &Ffmpeg,
    base: &Path,
    strip_voice: bool,
    dir: &Path,
    run: &str,
) -> Result<PathBuf, crate::genjob::JobErr> {
    use base64::Engine as _;
    let err = |exit, message: String, fix: &str| crate::genjob::JobErr {
        exit,
        message,
        fix: fix.into(),
        detail: None,
    };
    let wav = dir.join("base_sound.wav");
    let args: Vec<String> = [
        "-i".to_string(),
        base.to_string_lossy().into_owned(),
        "-vn".into(),
        "-ac".into(),
        "2".into(),
        "-ar".into(),
        "44100".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        wav.to_string_lossy().into_owned(),
    ]
    .to_vec();
    ff.run(dir, &args, None).map_err(|e| {
        err(
            Exit::Usage,
            format!("the base video has no sound to keep: {e}"),
            "Drop base_sound, or use a base video with sound.",
        )
    })?;
    if !strip_voice {
        return Ok(wav);
    }
    let row = pick_row("base_sound background", "stem", |g| g.kind == "stems")?;
    // FLAC halves the upload; the server takes 40 MB, so about 6 minutes of stereo sound.
    let flac = dir.join("base_sound.flac");
    let args: Vec<String> = [
        "-i".to_string(),
        wav.to_string_lossy().into_owned(),
        "-c:a".into(),
        "flac".into(),
        flac.to_string_lossy().into_owned(),
    ]
    .to_vec();
    ff.run(dir, &args, None)
        .map_err(|e| err(Exit::Error, e, "Run make again."))?;
    let bytes =
        std::fs::read(&flac).map_err(|e| err(Exit::Error, e.to_string(), "Run make again."))?;
    if bytes.len() > 28_000_000 {
        return Err(err(
            Exit::Usage,
            format!(
                "the base video's sound is too long to send ({} MB)",
                bytes.len() / 1_000_000
            ),
            "Use base_sound keep, or a base video under about 5 minutes.",
        ));
    }
    let req = row.request(&crate::generators::GenInput {
        audio: Some(format!(
            "data:audio/flac;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )),
        ..Default::default()
    });
    crate::genjob::run_clip(
        &row,
        &req,
        false,
        Path::new("."),
        dir,
        "base_stems",
        run,
        0.0,
        GEN_WAIT_S,
    )
    .map(|(p, _)| p)
}

fn foreign_warning(a: &Args, lang: &str, script: &str) -> Option<gates::Finding> {
    let found = foreign_script(script).filter(|_| a.lang.is_none() && lang == "en")?;
    Some(gates::Finding {
        gate: "lang",
        status: "warn",
        detail: format!("the script looks {found}, and an English voice will read it"),
        fix: Some("`scriptvox voices` lists the languages that have a voice. Use your own narration file (`narration`), or write the script in one of them.".into()),
    })
}

/// The full video is listened to (gate `words`): there is a voice, a network and a free speech to text row.
fn will_listen(a: &Args, voice: &crate::voices::Voice) -> bool {
    !a.local_only
        && !a.cache_only
        && (voice.engine != "none" || a.narration.is_some())
        && crate::opentts::rows("stt").is_some()
}

/// Why the dub lines cannot run on this video, if they cannot.
fn line_problem(a: &Args, base_len: Option<f64>) -> Option<String> {
    if a.base_video.is_none() {
        return Some("lines needs base_video".into());
    }
    if a.narration.is_some() {
        return Some("lines and narration cannot be used together".into());
    }
    let mut last = 0.0;
    for (i, l) in a.lines.iter().enumerate() {
        let n = i + 1;
        if l.text.trim().is_empty() {
            return Some(format!("line {n} has no text"));
        }
        if !(l.start >= last && l.end > l.start) {
            return Some(format!(
                "line {n} runs {} to {} s, and the line before it ends at {last} s",
                l.start, l.end
            ));
        }
        if base_len.is_some_and(|t| l.end > t + 0.1) {
            return Some(format!(
                "line {n} ends at {} s, past the end of the video",
                l.end
            ));
        }
        last = l.end;
    }
    None
}

/// Seconds from the start of line `i` to the next line, or to the end of the video.
fn line_room(lines: &[crate::brief::Line], i: usize, base_len: Option<f64>) -> f64 {
    let until = lines
        .get(i + 1)
        .map(|n| n.start)
        .or(base_len)
        .unwrap_or(lines[i].end);
    until.max(lines[i].end) - lines[i].start
}

type ReadVoice<'a> = dyn Fn(Option<i32>, &[Seg], &[String], &Path) -> Result<(PathBuf, Vec<Word>), Box<Envelope>>
    + 'a;

/// Each line is read once and measured. If any line cannot fit, all budgets come back before a second read is paid
/// for. Then a voice with a rate reads again at the speed its line needs, and `atempo` takes what is left.
/// ponytail: the third step of the row, a small slowdown of the picture, is not here; a line over the limit stops the
/// run with its budget. Add it when a real dub needs it: it moves every later time.
fn line_voice(
    ff: &Ffmpeg,
    lines: &[crate::brief::Line],
    base_len: Option<f64>,
    voice: &voices::Voice,
    dir: &Path,
    read: &ReadVoice,
) -> Result<(PathBuf, Vec<Word>, Value), Box<Envelope>> {
    let read_line = |i: usize, rate: Option<i32>| {
        let d = dir.join(format!("line{:03}", i + 1));
        let _ = std::fs::create_dir_all(&d);
        let segs = pauses::parse(&lines[i].text);
        let id = lines[i].voice.clone().unwrap_or_else(|| voice.id.clone());
        let who = vec![id; segs.len()];
        read(rate, &segs, &who, &d)
    };
    // Speech, not file length: a voice file ends with silence that may lie under the next line.
    let seconds = |wav: &Path, words: &[Word]| {
        ff.duration(wav)
            .map(|f| words.last().map_or(f, |w| w.end.min(f)))
            .map_err(|e| Box::new(Envelope::fail("make", Exit::Error, e, None)))
    };
    let (mut first, mut over) = (vec![], vec![]);
    for (i, l) in lines.iter().enumerate() {
        let (wav, words) = read_line(i, None)?;
        let spoken = seconds(&wav, &words)?;
        let room = line_room(lines, i, base_len);
        match fit::fit_line(spoken, l.end - l.start, room) {
            Ok(f) => first.push((wav, words, f, spoken)),
            Err(need) => over.push(json!({
                "line": i + 1, "start": l.start, "end": l.end, "speed_needed": (need * 100.0).round() / 100.0,
                "chars": l.text.chars().count(),
                "budget_chars": fit::line_budget(l.text.chars().count(), spoken, room),
            })),
        }
    }
    if !over.is_empty() {
        return Err(Box::new(
            Envelope::fail_code(
                "make",
                Exit::GateFailed,
                "SVX_LINES_TOO_LONG",
                format!(
                    "{} of {} lines cannot fit at {}x speed (measured after TTS; nothing was rendered)",
                    over.len(),
                    lines.len(),
                    fit::LINE_MAX
                ),
                Some("Cut each line in `line_budgets` to its `budget_chars`.".into()),
            )
            .with("line_budgets", json!(over)),
        ));
    }
    let (mut inputs, mut filters, mut mix, mut events, mut report) =
        (vec![], vec![], String::new(), vec![], vec![]);
    for (i, (wav, words, f, spoken)) in first.into_iter().enumerate() {
        let (mut wav, mut words, mut read_s) = (wav, words, spoken);
        // Within 3 percent a re-read changes nothing a listener hears.
        // No further than the engine goes; the tempo below takes what is left.
        let pct = (((f.speed - 1.0) * 100.0).round() as i32).min(fit::rate_limits(&voice.engine).1);
        let reread = pct > 3 && fit::takes_rate(&voice.engine);
        if reread {
            (wav, words) = read_line(i, Some(pct))?;
            read_s = seconds(&wav, &words)?;
        }
        let tempo = (read_s / f.run_s).max(1.0);
        let tempo = if tempo > 1.01 { tempo } else { 1.0 };
        let start = lines[i].start;
        inputs.extend(["-i".to_string(), wav.to_string_lossy().into_owned()]);
        let speed = if tempo > 1.0 {
            format!("{},", fit::atempo_chain(tempo))
        } else {
            String::new()
        };
        let ms = (start * 1000.0).round() as u64;
        filters.push(format!(
            "[{i}:a]aresample={RATE},aformat=channel_layouts=mono,{speed}adelay={ms}:all=1[l{i}]"
        ));
        mix.push_str(&format!("[l{i}]"));
        events.extend(words.into_iter().map(|w| Word {
            start: w.start / tempo + start,
            end: w.end / tempo + start,
            ..w
        }));
        let round = |x: f64| (x * 1000.0).round() / 1000.0;
        let ran = read_s / tempo;
        report.push(json!({
            "start": start, "end": lines[i].end, "ran_s": round(ran),
            "borrowed_s": round((ran - (lines[i].end - start)).max(0.0)),
            "rate_pct": if reread { pct } else { 0 }, "tempo": round(tempo),
        }));
    }
    let out = dir.join("voice_lines.wav");
    filters.push(format!(
        "{mix}amix=inputs={}:normalize=0:duration=longest[v]",
        lines.len()
    ));
    let mut args = inputs;
    args.extend([
        "-filter_complex".to_string(),
        filters.join(";"),
        "-map".into(),
        "[v]".into(),
        "-y".into(),
        out.to_string_lossy().into_owned(),
    ]);
    ff.run(dir, &args, None)
        .map_err(|e| Box::new(Envelope::fail("make", Exit::Error, e, None)))?;
    Ok((out, events, json!(report)))
}

fn fit_failure(f: fit::Fit, target: f64, when: &str) -> Envelope {
    let (code, msg, fix) = match f {
        fit::Fit::TooLong { ratio, cut_words } => (
            "SVX_NEEDS_SHORTER_SCRIPT",
            format!("narration is {ratio:.2}x the {target:.1} s target ({when})"),
            format!("Cut about {cut_words} words, or raise --duration."),
        ),
        fit::Fit::TooShort { ratio, add_words } => (
            "SVX_NEEDS_LONGER_SCRIPT",
            format!("narration is {ratio:.2}x the {target:.1} s target ({when})"),
            format!("Add about {add_words} words, or lower --duration."),
        ),
        _ => unreachable!("only failures reach fit_failure"),
    };
    Envelope::fail_code("make", Exit::GateFailed, code, msg, Some(fix))
}

fn needs(item: Value) -> Envelope {
    let mut env = Envelope::fail(
        "make",
        Exit::NeedsInput,
        item["human_action"]
            .as_str()
            .unwrap_or("input needed")
            .to_string(),
        None,
    );
    env.needs_input = vec![item];
    env
}

/// Audio parts of a script: TTS files and exact silences (in samples at 24 kHz).
enum Part {
    File(PathBuf),
    Silence(u64),
}

const RATE: u64 = 24_000;

/// TTS for every speech segment, joined with exact silences at the pause tags.
/// Word times move by each segment's start, so captions stay in sync.
/// The voice of this run came from the cache: said in the metrics (test 4 could prove it only from the run folder).
static VOICE_REUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Word times at reading pace, a little longer for a longer word. A speaking rate from the fitter changes the pace.
pub fn reading_times(segs: &[Seg], lang: &str, rate: Option<i32>) -> Vec<Word> {
    let pace = fit::words_per_s(lang, "none") * (1.0 + f64::from(rate.unwrap_or(0)) / 100.0);
    let (mut t, mut out) = (0.1, Vec::new());
    for seg in segs {
        match seg {
            Seg::Pause(p) => t += p,
            Seg::Speech(text) => {
                let ws: Vec<&str> = text.split_whitespace().collect();
                let chars: usize = ws.iter().map(|w| w.chars().count() + 3).sum();
                let total = ws.len() as f64 / pace;
                for w in ws {
                    let d = total * (w.chars().count() + 3) as f64 / chars as f64;
                    out.push(Word {
                        text: w.to_string(),
                        start: t,
                        end: t + d,
                    });
                    t += d;
                }
            }
        }
    }
    out
}

/// Marks a run whose recorded narration has word times from speech to text.
const NARRATION_HEARD: &str = "narration_heard";

/// `tts: none`: silence as long as the reading takes, and the word times.
/// A recorded narration as the voice track. Word times from speech to text when a row is there, else the script
/// spread evenly over the file (brief `narration`, MoneyPrinterTurbo #custom-audio-file).
fn narration_voice(
    ff: &Ffmpeg,
    file: &Path,
    segs: &[Seg],
    dir: &Path,
    lang: &str,
) -> Result<(PathBuf, Vec<Word>), String> {
    let wav = dir.join("voice.wav");
    let abs = std::path::absolute(file).unwrap_or(file.to_path_buf());
    let args: Vec<String> = [
        "-i",
        &abs.to_string_lossy(),
        "-vn",
        "-ac",
        "1",
        "-ar",
        &RATE.to_string(),
        "voice.wav",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    ff.run(dir, &args, None)?;
    let secs = ff
        .duration(&wav)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let heard = crate::analyze::transcribe(ff, &wav, Some(lang), secs)
        .ok()
        .and_then(|(doc, _)| crate::analyze::timed_words(&doc))
        .filter(|w| !w.is_empty());
    // The receipt and `explain` say which: a resumed run does not come through here again.
    if heard.is_some() {
        let _ = std::fs::write(dir.join(NARRATION_HEARD), "");
    }
    let words = heard.unwrap_or_else(|| {
        let even = reading_times(segs, lang, None);
        let k = (secs - 0.1).max(0.1) / even.last().map_or(1.0, |w| w.end).max(0.01);
        even.iter()
            .map(|w| Word {
                text: w.text.clone(),
                start: w.start * k,
                end: w.end * k,
            })
            .collect()
    });
    Ok((wav, words))
}

/// The reading pace of `tts: none`: Arabic when the script has Arabic letters, else the first language of the row.
/// `plan` and the run both ask here.
fn silent_lang<'a>(segs: &[Seg], voice: &'a voices::Voice) -> &'a str {
    let arabic = |t: &String| t.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
    if segs
        .iter()
        .any(|s| matches!(s, Seg::Speech(t) if arabic(t)))
    {
        "ar"
    } else {
        voice.lang.split(',').next().unwrap_or("en").trim()
    }
}

fn silent_voice(
    ff: &Ffmpeg,
    segs: &[Seg],
    dir: &Path,
    voice: &voices::Voice,
    rate: Option<i32>,
) -> Result<(PathBuf, Vec<Word>), String> {
    let words = reading_times(segs, silent_lang(segs, voice), rate);
    let secs = reading_len(segs, &words);
    let wav = dir.join("voice.wav");
    let args: Vec<String> = [
        "-f",
        "lavfi",
        "-i",
        &format!("anullsrc=r={RATE}:cl=mono"),
        "-t",
        &format!("{secs:.3}"),
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([wav.to_string_lossy().into_owned()])
    .collect();
    ff.run(dir, &args, None)?;
    Ok((wav, words))
}

/// The cache key of one speech part, and the text the engine gets for it. The plan and `synthesize` both ask here, so
/// a plan prices the parts a run would have to buy and no other (docs/specs/voice-plan-cache.md).
fn voice_key(voice: &voices::Voice, id: &str, text: &str, rate: Option<i32>) -> (String, String) {
    // Symbols are read as words ("≤" as "at most"); the captions keep the symbol.
    let said = crate::say::spoken(
        &if voice.takes_ipa {
            crate::say::apply_ipa(text, &voice.say)
        } else {
            crate::say::apply(text, &voice.say)
        },
        voice.lang.split(',').next().unwrap_or("en").trim(),
    );
    let rate_s = rate.map(|r| format!("{r:+}%"));
    let mut ck_in = json!({"text": said, "voice": id, "rate": rate_s});
    if voice.engine == "selfhost" {
        // The open voice pipeline changes faster than the crate version; old cache entries must not come back.
        ck_in["open"] = json!(crate::opentts::VERSION);
    }
    if voice.engine == "elevenlabs" && rate_s.is_some() {
        // Before 10 Nov 2026 the worker dropped the rate: an entry stored under a rate then holds the voice at its
        // own pace, and must not come back as the slower or faster one.
        ck_in["speed"] = json!(1);
    }
    if voice.takes_ipa && !crate::say::ipa_terms(&voice.say).is_empty() {
        // The sounds of each stand-in word: another IPA form is another voice file.
        ck_in["ipa"] = json!(crate::say::ipa_words(&voice.say));
    }
    if voice.engine == crate::kokoro_rust::ENGINE {
        // Before 13 Nov 2026 the word times of this voice ran into the silence at the end of the clip: an entry of
        // that time must not come back with them.
        ck_in["words"] = json!(2);
    }
    let tool = format!("tts.{}", voice.engine);
    (store::key(&tool, env!("CARGO_PKG_VERSION"), &ck_in), said)
}

/// One speech part of a voice at plan time: its cache key, the characters the engine would get, and whether the
/// store has it. Reads the store only: no call, no download, no write.
pub(crate) struct VoicePart {
    pub key: String,
    pub chars: usize,
    pub cached: bool,
}

/// The speech parts `synthesize` would ask for, in order. `who` is the speaker of each segment, as in `synthesize`.
pub(crate) fn voice_parts(
    voice: &voices::Voice,
    who: &[String],
    segs: &[Seg],
    rate: Option<i32>,
) -> Vec<VoicePart> {
    let cas = Cas::open();
    segs.iter()
        .enumerate()
        .filter_map(|(n, seg)| match seg {
            Seg::Speech(text) => {
                let id = who.get(n).map_or(voice.id.as_str(), |s| s.as_str());
                let (key, said) = voice_key(voice, id, text, rate);
                Some(VoicePart {
                    cached: cas.has(&key),
                    chars: said.chars().count(),
                    key,
                })
            }
            Seg::Pause(_) => None,
        })
        .collect()
}

/// The key of the note that says how the fit to `duration` ended for this first read: no re-read, or a re-read at
/// one rate. With it a later plan of the same request can look that re-read up in the store too.
fn fit_note_key(first: &[VoicePart], segs: &[Seg], target: f64, words: usize) -> String {
    let pauses: Vec<f64> = segs
        .iter()
        .filter_map(|s| match s {
            Seg::Pause(p) => Some(*p),
            Seg::Speech(_) => None,
        })
        .collect();
    store::key(
        "voice.fit",
        env!("CARGO_PKG_VERSION"),
        &json!({"reads": first.iter().map(|p| &p.key).collect::<Vec<_>>(), "pauses": pauses, "duration": target, "words": words}),
    )
}

/// The words of one speech part on the joined timeline: `at` is where the part starts, `len` the length of its audio
/// after the end silence is cut and the tempo is changed. No word ends after the audio of its part: an engine that
/// estimates its word times over the whole clip (the Python `kokoro`, an old cache entry of `kokoro-rust`) put the
/// last words in the cut silence, so they were on screen with the first words of the next part (13 Nov 2026). Such
/// words are scaled to the length of the part. Measured word times end before the cut and do not change.
fn part_words(ws: Vec<Word>, tempo: f64, at: f64, len: f64) -> Vec<Word> {
    let last = ws.iter().map(|w| w.end).fold(0.0, f64::max) / tempo;
    let k = if last > len { len / last } else { 1.0 } / tempo;
    ws.into_iter()
        .map(|w| Word {
            start: w.start * k + at,
            end: w.end * k + at,
            ..w
        })
        .collect()
}

#[allow(clippy::too_many_arguments)] // two call sites (`make`, `voices --preview`); a struct would only rename these
pub(crate) fn synthesize(
    ff: &Ffmpeg,
    voice: &voices::Voice,
    who: &[String],
    segs: &[Seg],
    dir: &Path,
    local_only: bool,
    rate: Option<i32>,
    // The tempo change of each speech part after it is read (1.0: none). Pauses and word times follow it.
    tempo: f64,
    ledger: &plan::Ledger,
    cache_only: bool,
) -> Result<(PathBuf, Vec<Word>), RpcError> {
    let err = |code: i64| move |m: String| RpcError { code, message: m };
    if voice.engine == "none" {
        return silent_voice(ff, segs, dir, voice, rate).map_err(err(-32050));
    }
    let cas = Cas::open();
    // The worker starts on the first cache miss: a fully cached script makes no call.
    let mut worker: Option<Worker> = None;
    // The Rust voice loads its model on the first cache miss, once per run.
    let mut rust_voice: Option<scriptvox_kokoro::Engine> = None;
    let mut tts = |text: &str,
                   id: &str,
                   out_dir: &Path|
     -> Result<(PathBuf, Vec<Word>), RpcError> {
        let (ck, said) = voice_key(voice, id, text, rate);
        let text = said.as_str();
        // Contract: the core creates out_dir before a tool call.
        std::fs::create_dir_all(out_dir)
            .map_err(|e| err(-32050)(format!("cannot create {}: {e}", out_dir.display())))?;
        let rate_s = rate.map(|r| format!("{r:+}%"));
        let tool = format!("tts.{}", voice.engine);
        let read_words = |p: &Path| -> Option<Vec<Word>> {
            serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
        };
        if let Some(files) = cas.get(&ck, out_dir) {
            VOICE_REUSED.store(true, std::sync::atomic::Ordering::Relaxed);
            let audio = files
                .iter()
                .find(|(n, _)| n == "voice.mp3")
                .map(|(_, p)| p.clone());
            if let (Some(audio), Some(words)) = (audio, read_words(&out_dir.join("words.json"))) {
                return Ok((audio, words));
            }
        }
        if cache_only {
            return Err(RpcError {
                code: CACHE_MISS,
                message: format!(
                    "the voice for \"{}\" is not in the cache",
                    text.chars().take(40).collect::<String>()
                ),
            });
        }
        // Open voices run on self-hosted GPUs through the job runner, not in the Python worker.
        if voice.engine == "selfhost" {
            let lang = voice.lang.split(',').next().unwrap_or("ar").trim();
            let (audio, words) = crate::opentts::speak(ff, lang, text, out_dir, &ledger.run, None)?;
            let words_file = out_dir.join("words.json");
            let _ = std::fs::write(
                &words_file,
                serde_json::to_string(&words).unwrap_or_default(),
            );
            let terms = json!({"commercial": voice.commercial, "attribution_required": false, "redistribution": false, "note": voice.terms});
            let _ = cas.put(
                &ck,
                &tool,
                env!("CARGO_PKG_VERSION"),
                &[("voice.mp3", &audio), ("words.json", &words_file)],
                terms,
            );
            return Ok((audio, words));
        }
        if voice.engine == crate::kokoro_rust::ENGINE {
            if rust_voice.is_none() {
                rust_voice =
                    Some(crate::kokoro_rust::load().map_err(err(crate::rpc::WORKER_CRASHED))?);
            }
            let engine = rust_voice.as_ref().unwrap();
            let sounds = crate::say::ipa_words(&voice.say);
            let (audio, words) =
                crate::kokoro_rust::speak(engine, text, id, rate, out_dir, &sounds)
                    .map_err(err(crate::rpc::WORKER_CRASHED))?;
            let words_file = out_dir.join("words.json");
            let _ = std::fs::write(
                &words_file,
                serde_json::to_string(&words).unwrap_or_default(),
            );
            let terms = json!({"commercial": voice.commercial, "attribution_required": false, "redistribution": false, "note": voice.terms});
            let _ = cas.put(
                &ck,
                &tool,
                env!("CARGO_PKG_VERSION"),
                &[("voice.mp3", &audio), ("words.json", &words_file)],
                terms,
            );
            return Ok((audio, words));
        }
        if worker.is_none() {
            let wdir = provision::worker_dir().map_err(err(crate::rpc::WORKER_CRASHED))?;
            provision::prepare_worker(&wdir, &|m| eprintln!("scriptvox: {m}"))
                .map_err(err(PROVIDER_RETRYABLE))?;
            // Kokoro runs in its own plugin process (GPL G2P); every other engine in the worker.
            let mut cmd = if voice.engine == "kokoro" {
                provision::kokoro_command().map_err(err(crate::rpc::WORKER_CRASHED))?
            } else {
                provision::worker_command(&wdir).map_err(err(crate::rpc::WORKER_CRASHED))?
            };
            // Keys reach the worker only as env vars, and only for the engine in use.
            for (p, var) in crate::keys::all() {
                cmd.env_remove(&var);
                if p == voice.engine {
                    if let Some((k, _)) = crate::keys::resolve(&p) {
                        cmd.env(&var, k);
                    }
                }
            }
            let mut w = Worker::spawn(cmd)?;
            w.hello(local_only)?;
            worker = Some(w);
        }
        let w = worker.as_mut().unwrap();
        let mut input = json!({"text": text, "voice": id, "out_dir": out_dir, "model": voice.model, "usd_per_1k_chars": voice.usd_per_1k_chars});
        if let Some(r) = &rate_s {
            input["rate"] = json!(r);
        }
        let r = w.request_with(
            "tool.call",
            json!({"tool": tool, "input": input}),
            |_| {},
            |m, p| ledger.handle(m, p),
        );
        ledger.finish(match &r {
            Ok(v) => Ok(v["cost_usd"].as_f64()),
            Err(e) => Err(e.code == plan::RESERVE_REFUSED || e.code == PROVIDER_REFUSED),
        });
        let r = r?;
        let audio = PathBuf::from(r["artifacts"]["audio"].as_str().unwrap_or_default());
        let words_file = PathBuf::from(r["artifacts"]["words"].as_str().unwrap_or_default());
        let words = read_words(&words_file).ok_or_else(|| {
            err(crate::rpc::WORKER_CRASHED)(format!(
                "the worker wrote no words file at {}",
                words_file.display()
            ))
        })?;
        // Edge output terms: personal or non-commercial, never redistributed in a shared cache.
        let terms = json!({"commercial": false, "attribution_required": false, "redistribution": false, "note": voice.terms});
        let _ = cas.put(
            &ck,
            &tool,
            env!("CARGO_PKG_VERSION"),
            &[("voice.mp3", &audio), ("words.json", &words_file)],
            terms,
        );
        Ok((audio, words))
    };
    if let ([Seg::Speech(text)], true) = (segs, tempo == 1.0) {
        let id = who.first().map_or(voice.id.as_str(), |s| s.as_str());
        let r = tts(text, id, dir);
        drop(tts);
        if let Some(w) = worker {
            w.shutdown();
        }
        return r;
    }
    let join_err = err(-32050);
    let (mut parts, mut words, mut t) = (Vec::new(), Vec::new(), 0.0);
    for (n, seg) in segs.iter().enumerate() {
        match seg {
            Seg::Speech(text) => {
                let sd = dir.join(format!("seg_{n:02}"));
                let id = who.get(n).map_or(voice.id.as_str(), |s| s.as_str());
                let (mp3, ws) = tts(text, id, &sd)?;
                // Not "voice.wav": the local Rust voice writes its own file under that name in this folder.
                let wav = sd.join("trimmed.wav");
                let args: Vec<String> = vec![
                    "-i".into(),
                    mp3.to_string_lossy().into(),
                    // TTS clips end with up to 1 s of silence; trim it (keep 50 ms) so a
                    // pause tag is the gap the listener hears. Word times do not move.
                    "-af".into(),
                    format!(
                        "areverse,silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05,areverse{}",
                        if tempo == 1.0 { String::new() } else { format!(",{}", fit::atempo_chain(tempo)) }
                    ),
                    "-ar".into(),
                    RATE.to_string(),
                    "-ac".into(),
                    "1".into(),
                    wav.to_string_lossy().into(),
                ];
                ff.run(dir, &args, None).map_err(join_err)?;
                let d = ff.duration(&wav).map_err(join_err)?;
                words.extend(part_words(ws, tempo, t, d));
                t += d;
                parts.push(Part::File(wav));
            }
            Seg::Pause(s) => {
                let samples = (s / tempo * RATE as f64).round() as u64;
                t += samples as f64 / RATE as f64;
                parts.push(Part::Silence(samples));
            }
        }
    }
    drop(tts);
    if let Some(w) = worker {
        w.shutdown();
    }
    let mut args: Vec<String> = Vec::new();
    for p in &parts {
        match p {
            Part::File(f) => args.extend(["-i".into(), f.to_string_lossy().into_owned()]),
            Part::Silence(n) => args.extend(
                ["-f", "lavfi", "-i"]
                    .map(String::from)
                    .into_iter()
                    .chain([format!("anullsrc=r={RATE}:cl=mono,atrim=end_sample={n}")]),
            ),
        }
    }
    let inputs: String = (0..parts.len()).map(|i| format!("[{i}:a]")).collect();
    let voice_wav = dir.join("voice.wav");
    args.extend([
        "-filter_complex".into(),
        format!("{inputs}concat=n={}:v=0:a=1[a]", parts.len()),
        "-map".into(),
        "[a]".into(),
    ]);
    args.extend([
        "-c:a".into(),
        "pcm_s16le".into(),
        voice_wav.to_string_lossy().into_owned(),
    ]);
    ff.run(dir, &args, None).map_err(join_err)?;
    Ok((voice_wav, words))
}

/// The render argv. The run folder is the cwd: captions and fonts are relative.
/// The visual track: FFmpeg inputs, graph text that ends in `label`.
pub struct Visual {
    pub inputs: Vec<String>,
    pub graph: String,
    pub label: String,
}

impl Visual {
    pub fn gradient((w, h): (u32, u32), fps: u32, secs: f64) -> Self {
        let inputs = vec![
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            gradient(w, h, fps, secs),
        ];
        Self {
            inputs,
            graph: String::new(),
            label: "[0:v]".into(),
        }
    }
}

/// The waveform band in pixels, (top, height): a quarter of the height, clear of the captions wherever they sit.
/// Captions at the bottom: 28 to 53 percent. In the lower middle: 12 to 37 percent. At the top: 40 to 65 percent.
fn wave_band(caption_at: Option<&str>, h: u32) -> (u32, u32) {
    let top = match caption_at {
        Some("middle") => 5,
        Some("top") => 16,
        _ => 11,
    };
    (h * top / 40, h / 4)
}

/// The waveform's colour as 6 hex digits: the accent when it is a checked `#RRGGBB`, else the captions' yellow.
/// Only checked text reaches the filter graph: the accent can come from a recipe in an MP4, and any other text
/// there would be read by FFmpeg as part of the graph.
fn wave_colour(accent: Option<&str>) -> &str {
    accent
        .filter(|c| crate::captions::ass_colour(c).is_some())
        .map_or("FFE500", |c| c.trim_start_matches('#'))
}

#[cfg(test)]
mod wave_colour_tests {
    #[test]
    fn only_a_checked_hex_colour_reaches_the_filter_text() {
        assert_eq!(super::wave_colour(Some("#00c2a8")), "00c2a8");
        assert_eq!(super::wave_colour(None), "FFE500");
        for bad in [
            "#00c2a8:scale=lin[v];movie=/etc/passwd[x]",
            "00c2a8",
            "#00c2a",
            "#zzzzzz",
            "red",
            "#00c2a8 ",
        ] {
            assert_eq!(super::wave_colour(Some(bad)), "FFE500", "{bad}");
        }
    }
}

/// Draws the sound of `audio` as a small video with alpha in the run folder `dir`, and returns its file name.
/// Its own FFmpeg pass, never a part of the render graph: there the sound and the picture ran at different speeds and
/// FFmpeg kept full frames in memory with no limit (10 Nov 2026: 19 GB). The name is a hash of the sound and the
/// settings, so the same sound gives the same name in every run folder and the render key follows the sound.
fn wave_file(
    ff: &Ffmpeg,
    dir: &Path,
    audio: &Path,
    (w, bh): (u32, u32),
    fps: u32,
    accent: Option<&str>,
) -> Result<String, String> {
    // The default is the captions' yellow (`captions::ACCENT`, written there as ASS BGR).
    let colour = wave_colour(accent);
    let draw = format!("aformat=channel_layouts=mono,showwaves=s={w}x{bh}:mode=cline:rate={fps}:colors=0x{colour}:scale=sqrt:draw=full,format=argb");
    let sound = crate::store::hash_file(audio).map_err(|e| e.to_string())?;
    let name = format!(
        "wave-{}.mov",
        &blake3::hash(format!("{sound} {draw}").as_bytes()).to_hex()[..16]
    );
    let args: Vec<String> = [
        "-i",
        &audio.to_string_lossy(),
        "-filter_complex",
        &format!("[0:a]{draw}[v]"),
        "-map",
        "[v]",
        "-r",
        &fps.to_string(),
        "-c:v",
        "qtrle",
        &dir.join(&name).to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ff.run(dir, &args, None)?;
    Ok(name)
}

/// Caption pages and style for word-timed captions, the same in `make` and `edit --captions`: 84 px on a 1080 px
/// short side (scaled so every script looks the same size), 2 lines (or 1 big line for "pop"), above platform UI.
/// `scale` (brief `caption_size`, `captions::size_scale`) makes the captions smaller or larger; the hook, the cards and
/// the second line keep the size of the frame.
/// Returns (font family, font bytes, caption size before "pop" and `scale`, pages, style).
pub fn caption_pages(
    words: &[Word],
    lang: &str,
    (w, h): (u32, u32),
    style: &str,
    scale: (u32, u32),
) -> (
    &'static str,
    &'static [u8],
    u32,
    Vec<crate::captions::Page>,
    AssStyle<'static>,
) {
    let (family, font) = font_for(lang);
    // 84 px on a 1080 px short side; the margin keeps captions above platform UI.
    let size = 84 * w.min(h) / 1080;
    // Same visual size for every script: scale by the font's win-height-to-em ratio
    // against Noto Sans (Noto Sans Arabic has much taller win metrics).
    let latin = crate::captions::em_factor(crate::template::FONTS[0].2).unwrap_or(1.0);
    let size =
        (size as f32 * crate::captions::em_factor(font).unwrap_or(latin) / latin).round() as u32;
    // "pop": one short line of 1 to 3 words, bigger, a new page at least every second.
    let (pop, reveal) = (style == "pop", style == "reveal");
    let cap_size = if pop {
        size * 27 / 20
    } else if reveal {
        size * 6 / 5
    } else {
        size
    };
    // Brief `caption_size`, on top of the style's own size. The lines are measured at this size, so they still fit.
    let cap_size = cap_size * scale.0 / scale.1;
    let m = Measurer::new(font.to_vec(), cap_size as f32).expect("embedded font parses");
    let layout = if pop {
        Layout {
            max_width: w.min(h) as f32 * 0.62,
            max_lines: 1,
            max_gap: 0.4,
            max_page: 1.0,
        }
    } else {
        Layout {
            // About 42 characters a line on wide frames: long lines are hard to read.
            max_width: (w as f32 * 0.83).min(w.min(h) as f32 * 1.04),
            max_lines: if reveal { 3 } else { 2 },
            max_gap: 0.6,
            max_page: 4.0,
        }
    };
    let pages = paginate(words, &layout, |s| m.width(s));
    let style = AssStyle {
        font: family,
        size: cap_size,
        play_res: (w, h),
        alignment: 2,
        margin_v: if h > w { h * 19 / 100 } else { h * 9 / 100 },
        pop,
        reveal,
        emphasis: vec![],
        second_speaker: vec![],
    };
    (family, font, size, pages, style)
}

/// The second caption line (brief `captions_second`, `edit`/`dub --captions-second`): its font family, size and
/// wrapped lines. Three fifths of the main `size`, at the same look in any script (as `caption_pages` scales it).
/// `brand`: the user's own font, which the second line keeps; else Noto Sans, or Noto Sans Arabic for Arabic entries.
pub fn second_layout(
    texts: &[String],
    brand: Option<(&'static str, &'static [u8])>,
    font: &[u8],
    size: u32,
    w: u32,
) -> (&'static str, u32, Vec<Vec<String>>) {
    let arabic = |t: &str| t.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
    let (fam2, font2) = brand.unwrap_or_else(|| {
        let (_, f, b) = crate::template::FONTS[usize::from(texts.iter().any(|t| arabic(t)))];
        (f, b)
    });
    let em = |f: &[u8]| crate::captions::em_factor(f).unwrap_or(1.0);
    let size2 = ((size * 3 / 5) as f32 * em(font2) / em(font))
        .round()
        .max(14.0) as u32;
    let m2 = Measurer::new(font2.to_vec(), size2 as f32).expect("embedded font parses");
    let lines = texts
        .iter()
        .map(|t| crate::captions::wrap(t, w as f32 * 0.86, |s| m2.width(s)))
        .collect();
    (fam2, size2, lines)
}

/// How far the main captions (at `main` px) move up for the second line: its longest entry, both boxes' padding and
/// a small gap.
pub fn second_raise(lines: &[Vec<String>], size2: u32, main: u32) -> u32 {
    let most = lines.iter().map(Vec::len).max().unwrap_or(1) as u32;
    most * size2 + size2 / 3 + main / 6 + size2 / 4
}

/// Each sentence's second line, from its first word until the main page with its last word leaves. A short pause
/// holds that page, so the line does not blink; never past the next sentence's first word, when one page holds both.
pub fn second_cues(
    words: &[Word],
    pages: &[crate::captions::Page],
    lines: &[Vec<String>],
) -> Vec<(f64, f64, Vec<String>)> {
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    let spans = crate::captions::sentences(&texts);
    spans
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let last = r.end - 1;
            let page_end = pages
                .iter()
                .find(|p| p.lines.iter().flatten().any(|&w| w == last))
                .map_or(words[last].end, |p| p.end);
            let to = spans
                .get(i + 1)
                .map_or(page_end, |n| page_end.min(words[n.start].start));
            (
                words[r.start].start,
                to,
                lines.get(i).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

/// Lines of a `--captions-second` file, one per sentence: blank lines at the end go, an empty one inside is an error.
pub fn read_second(file: &std::path::Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let lines: Vec<String> = text
        .trim_end()
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    if lines.iter().any(String::is_empty) {
        return Err(format!(
            "{} has an empty line: give one line per sentence",
            file.display()
        ));
    }
    Ok(lines)
}

/// Brief `media_match`: fills the scenes with picks from the folder. A failed call is a warning: the video still works with
/// the files in folder order, and it says so.
/// Exit 4 with the plan id to approve, when this plan costs money and has no valid approval.
pub fn approval_stop(
    plan_id: &str,
    cost: f64,
    approve: Option<&str>,
    max_spend: Option<f64>,
    stages: Value,
) -> Option<Envelope> {
    let why = plan::needs_approval(plan_id, cost, approve)?;
    // Over the cap, an id to approve would only be refused at the reserve. A valid approval never gets here.
    if let Some(env) = plan::over_cap("make", plan_id, cost, max_spend, vec![]) {
        return Some(env.with("stages", stages));
    }
    let mut env = Envelope::fail(
        "make",
        Exit::ApprovalNeeded,
        format!(
            "approval needed: {} (${cost:.2})",
            why["reason"].as_str().unwrap_or("")
        ),
        Some(format!("Run the same command with --approve {plan_id}")),
    );
    env.status = "approval_needed";
    env.plan_id = Some(plan_id.to_string());
    env.next = vec![format!("scriptvox make ... --approve {plan_id}")];
    env.metrics.insert("cost_usd".into(), json!(cost));
    Some(env.with("stages", stages))
}

fn pick_media(a: &mut Args) -> Result<(Option<Value>, Option<gates::Finding>), Box<Envelope>> {
    if !a.media_match {
        return Ok((None, None));
    }
    // Picked already (`resume`, `replay`, `remake`): no call, and no report of a pick that did not happen.
    if a.scenes
        .as_ref()
        .is_some_and(|s| !s.iter().any(crate::match_media::is_open))
    {
        return Ok((None, None));
    }
    let usage = |msg: &str, fix: &str| {
        Box::new(Envelope::fail(
            "make",
            Exit::Usage,
            msg.to_string(),
            Some(fix.to_string()),
        ))
    };
    let Some(dir) = a.media.clone() else {
        return Err(usage(
            "media_match needs a media folder",
            "Add \"media\": \"<folder>\" to the brief.",
        ));
    };
    if a.local_only || a.cache_only {
        return Err(usage(
            "media_match asks a vision row, so it needs the network",
            "Drop --local-only or --cache-only, or drop media_match: the files then play in folder order.",
        ));
    }
    // A bad folder gets its own error later, from the same list.
    let Ok(items) = media::list(&dir, Path::new(".")) else {
        return Ok((None, None));
    };
    let warn = |e: String| gates::Finding {
        gate: "media_match",
        status: "warn",
        detail: format!("not matched, the files play in folder order: {e}"),
        fix: Some("Run it again, or write `scenes` yourself.".into()),
    };
    let sends = !a.plan_only && !a.storyboard;
    let ff = match Ffmpeg::locate(a.use_system, sends) {
        Ok(f) => f,
        Err(e) => return Ok((None, Some(warn(e)))),
    };
    let mut scenes = a
        .scenes
        .clone()
        .unwrap_or_else(|| crate::match_media::scenes_from_script(&a.script));
    // A plan sends no frame: it shows picks already kept, and its `vision` stage names the host for the rest.
    match crate::match_media::apply(&mut scenes, &ff, &items, sends) {
        Ok(v) => {
            a.scenes = Some(scenes);
            Ok((Some(v), None))
        }
        Err(_) if a.plan_only && crate::judge::vision_url().is_some() => Ok((None, None)),
        Err(e) => Ok((None, Some(warn(e)))),
    }
}

/// The hook headline added to an ASS document. Measured like the captions: libass never re-wraps (WrapStyle 2).
pub fn hook_ass(ass: &str, hook: &str, family: &str, font: &[u8], size: u32, w: u32) -> String {
    let hook_size = size * 3 / 2;
    let hm = Measurer::new(font.to_vec(), hook_size as f32).expect("embedded font parses");
    // Balanced, so the hook never ends on one lone word.
    let wrap = |t: &str, _: u32, max: f32| crate::captions::wrap(t, max, |s| hm.width(s));
    let lines = crate::cards::balanced(&wrap, hook.trim(), hook_size, w as f32 * 0.86);
    crate::captions::with_hook(ass, &lines, family, hook_size, HOOK_S)
}

/// Why a row named in a brief cannot make this scene.
fn misfit(g: &crate::generators::Generator, aspect: &str, needs_i2v: bool) -> String {
    if g.kind != "video.gen" {
        format!("it is not a video row ({})", g.kind)
    } else if g.usd_per_input_s > 0.0 {
        "it is priced by the length of a source video, and a generated scene has none".into()
    } else if !g.aspects.iter().any(|x| x == aspect) {
        format!("it makes {}, not {aspect}", g.aspects.join(", "))
    } else if needs_i2v && g.i2v.is_none() {
        "the scene starts from a picture (the brief's `character`, or the scene's `image` or `first_frame`), and this row takes no start picture (it has no image to video)".into()
    } else if g.takes_last_frame.is_none() {
        "the scene ends on a picture (last_frame), and it takes no last frame".into()
    } else {
        "unknown".into()
    }
}

/// The share of a picture that stays when a clip of another shape fills its frame from it.
fn shape_keeps(w: u32, h: u32, aspect: &str) -> Option<f64> {
    let (x, y) = aspect.split_once(':')?;
    let want = x.parse::<f64>().ok()? / y.parse::<f64>().ok()?;
    let r = f64::from(w) / f64::from(h.max(1));
    Some((r / want).min(want / r))
}

/// A `generate.image` of another shape than the clip loses its edges, with no other sign (a round logo lost both sides).
fn picture_shape_warnings(a: &Args) -> Vec<gates::Finding> {
    let Ok(ff) = Ffmpeg::locate(false, false) else {
        return vec![];
    };
    let aspect = gen_aspect(&a.aspect);
    a.scenes
        .iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, sc)| {
            let (w, h) = ff.shown_size(sc.generate.as_ref()?.image.as_ref()?)?;
            let keep = shape_keeps(w, h, aspect).filter(|k| *k < 0.9)?;
            Some(gates::Finding {
                gate: "picture_shape",
                status: "warn",
                detail: format!(
                    "scene {}: the picture is {w}x{h} and the clip fills {aspect}, so about {:.0} percent of it stays",
                    i + 1,
                    keep * 100.0
                ),
                fix: Some(format!("Give a picture in {aspect}, or look at the first frame and accept the crop.")),
            })
        })
        .collect()
}

/// An `emphasis` entry the script does not say: it would light nothing, with no other sign.
fn emphasis_warning(missing: &[String]) -> gates::Finding {
    gates::Finding {
        gate: "emphasis",
        status: "warn",
        detail: format!("not in the script: {}", missing.join(", ")),
        fix: Some("Write each entry as the script says it.".into()),
    }
}

/// The logo over the visual track: scaled to its share of the width, faded to its opacity, in a top corner with a
/// margin of 4 percent of the width (5 percent of the height from the top, under platform status bars).
pub fn logo_graph(
    visual: &Visual,
    l: &crate::brief::Logo,
    input: usize,
    (w, h): (u32, u32),
    until: Option<f64>,
) -> String {
    let sep = if visual.graph.is_empty() { "" } else { ";" };
    let lw = ((w as f32 * l.width.unwrap_or(0.16)) as u32 / 2 * 2).max(2);
    let m = w * 4 / 100;
    let x = if l.corner.as_deref() == Some("left") {
        m.to_string()
    } else {
        format!("W-w-{m}")
    };
    format!(
        "{}{sep}[{input}:v]scale={lw}:-2,format=rgba,colorchannelmixer=aa={:.2}[lg];{}[lg]overlay={x}:{}{}[logo]",
        visual.graph,
        l.opacity.unwrap_or(0.9).clamp(0.2, 1.0),
        visual.label,
        h * 5 / 100,
        until.map_or(String::new(), |t| format!(":enable='lt(t,{t:.3})'"))
    )
}

/// The end card from `at` seconds: the picture blurs and darkens in 0.4 s, and the logo (FFmpeg input `logo`), 40
/// percent of the width, fades in above the middle. The text comes from the captions file. The darkest pixels stay
/// at luma 40, just above the black gate's level (38): on a dark plain background the end card read as 2.2 s of
/// black and failed the gate (seen 13 Nov 2026, 9:16, `slate`).
/// The card branch runs for the whole video and is transparent before `at`. A branch that starts at `at`
/// (`trim=start`) made the overlay hold every earlier frame of the video in memory while it waited for the branch's
/// first frame: a 51 s video at 1920x1080 needed more than 5 GB and failed (seen 13 Nov 2026).
pub fn outro_graph(visual: &Visual, at: f64, logo: Option<usize>, (w, h): (u32, u32)) -> String {
    // 24 px on a 1080 px short side: the blur looks the same at each `size`.
    let blur = (24 * w.min(h) / 1080).max(1);
    let sep = if visual.graph.is_empty() { "" } else { ";" };
    let mut g = format!(
        "{}{sep}{}split[ob][oe];[oe]boxblur={blur}:2:enable='gte(t,{at:.3})',eq=brightness=-0.22,lutyuv=y='max(val,40)',format=yuva420p,fade=t=in:st={at:.3}:d=0.4:alpha=1",
        visual.graph, visual.label
    );
    match logo {
        Some(i) => g.push_str(&format!(
            "[oc];[{i}:v]scale={}:-2,format=rgba,fade=t=in:st=0.15:d=0.4:alpha=1,setpts=PTS+{at:.3}/TB[ol];[oc][ol]overlay=(W-w)/2:H*0.44-h/2[od]",
            (w * 2 / 5) / 2 * 2
        )),
        None => g.push_str("[od]"),
    }
    g.push_str(";[ob][od]overlay=eof_action=pass[outro]");
    g
}

/// The visual track with titles behind the subject: draw `behind.ass` on the picture, then lay the cut-out subject
/// (picture where the mask is white) back over it in each window. Mask `k` is FFmpeg input `first + k`.
/// Each subject branch runs for the whole video, and the overlay is switched on only inside its window: the mask
/// gets black frames in front, one for each frame of the video before the window. A branch that starts at the
/// window (`trim=start`) makes the overlay hold every earlier frame of the video in memory while it waits for the
/// branch's first frame (the end card failed this way on 13 Nov 2026, see `outro_graph`).
// ponytail: alphamerge now runs on every frame of the video for each window, not only inside it. If that time
// matters, end each branch at its window's end.
pub fn behind_graph(
    visual: &Visual,
    wins: &[(f64, f64)],
    first: usize,
    (w, h): (u32, u32),
    fps: u32,
) -> String {
    let sep = if visual.graph.is_empty() { "" } else { ";" };
    let n = wins.len();
    let fgs: String = (0..n).map(|k| format!("[fg{k}]")).collect();
    let mut g = format!(
        "{}{sep}{}split={}[bgb]{fgs};[bgb]{}[t0]",
        visual.graph,
        visual.label,
        n + 1,
        ass_filter("behind.ass", "fonts")
    );
    for (k, (a, b)) in wins.iter().enumerate() {
        // The frames before the window, as `cut_out` counts them (`trim=start` with the same 3 decimals).
        let (a, b) = ((a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0);
        let before = (a * f64::from(fps) - 1e-6).ceil().max(0.0);
        g.push_str(&format!(
            ";[{i}:v]fps={fps},format=gray,scale={w}:{h},tpad=start={before}[m{k}];[fg{k}][m{k}]alphamerge[s{k}];\
             [t{k}][s{k}]overlay=eof_action=pass:enable='gte(t,{a:.3})*lt(t,{b:.3})'[t{}]",
            k + 1,
            i = first + k
        ));
    }
    g.push_str(&format!(";[t{n}]null[behind]"));
    g
}

/// Above this share of the title band hidden by the subject, the title is hard to read (checked by eye on 3 Nov:
/// towers hiding about two thirds of "ABU DHABI" left "DHA").
const BEHIND_MAX_COVER: f64 = 0.4;

/// Rows of a mask profile: one value per 1/ROWS of the height.
const ROWS: usize = 192;

/// The share of each row, across the middle `span` of the width (the title's measured width, 15 to 100 percent),
/// that the mask marks as subject, averaged over all frames, from one FFmpeg pass that shrinks every frame to
/// 1 x ROWS grey pixels. Limited range: 16 to 235.
/// Each video item's length in seconds (None for stills and when the probe fails), for `media::graph`.
fn clip_lens(ff: &Ffmpeg, items: &[media::Item]) -> Vec<Option<(f64, f64)>> {
    items
        .iter()
        .map(|it| match it.kind {
            // What is left to play from its start point.
            media::Kind::Video => ff
                .duration(&it.path)
                .ok()
                .map(|l| (it.to.unwrap_or(l).min(l) - it.from).max(0.1))
                .zip(ff.frame_rate(&it.path).ok()),
            _ => None,
        })
        .collect()
}

fn row_profile(ff: &Ffmpeg, mask: &Path, span: f64) -> Option<Vec<f64>> {
    let span = span.clamp(0.15, 1.0);
    let o = std::process::Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin", "-v", "error"])
        .args(crate::media_gate::input(mask).ok()?)
        .args([
            "-vf",
            &format!(
                "crop=iw*{span:.3}:ih:iw*{:.3}:0,scale=1:{ROWS}:flags=area,format=gray",
                (1.0 - span) / 2.0
            ),
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .ok()?;
    let n = o.stdout.len() / ROWS;
    (n > 0).then(|| {
        (0..ROWS)
            .map(|r| {
                let sum: f64 = (0..n).map(|f| o.stdout[f * ROWS + r] as f64).sum();
                ((sum / n as f64 - 16.0) / 219.0).clamp(0.0, 1.0)
            })
            .collect()
    })
}

/// Where a title of `lines` lines, each `line` high (share of the height), goes: the centre between 15 and 65
/// percent whose worst line is hidden nearest `aim` (the subject clearly in front, every line still readable),
/// ties to `default`. Returns (centre, hidden share of the worst line). A subject that crosses nowhere keeps
/// `default`.
pub fn place(profile: &[f64], line: f64, lines: usize, default: f64, aim: f64) -> (f64, f64) {
    let n = profile.len();
    let rows = ((line * n as f64).ceil() as usize).clamp(1, n);
    let one = |c: f64| {
        let top = ((c - line / 2.0) * n as f64)
            .round()
            .clamp(0.0, (n - rows) as f64) as usize;
        profile[top..top + rows].iter().sum::<f64>() / rows as f64
    };
    let cover = |c: f64| {
        (0..lines)
            .map(|k| one(c + (k as f64 - (lines as f64 - 1.0) / 2.0) * line))
            .fold(0.0, f64::max)
    };
    let mut best = (default, cover(default));
    if best.1 < 0.05 && (15..=65).all(|p| cover(p as f64 / 100.0) < 0.05) {
        return best;
    }
    for p in 15..=65 {
        let c = p as f64 / 100.0;
        let v = cover(c);
        let (d, bd) = ((v - aim).abs(), (best.1 - aim).abs());
        if d < bd - 1e-9
            || ((d - bd).abs() < 1e-9 && (c - default).abs() < (best.0 - default).abs())
        {
            best = (c, v);
        }
    }
    best
}

/// One mask video per window: the window of the visual track (no text) is rendered and sent to the matte row.
/// Resumed from the run folder on a rerun.
fn cut_out(
    ff: &Ffmpeg,
    dir: &Path,
    visual: &Visual,
    wins: &[(f64, f64)],
    (w, h): (u32, u32),
    run: &str,
) -> Result<Vec<PathBuf>, crate::genjob::JobErr> {
    use base64::Engine as _;
    let bad = |exit: Exit, message: String, fix: &str| crate::genjob::JobErr {
        exit,
        message,
        fix: fix.into(),
        detail: None,
    };
    let row = crate::generators::all()
        .into_iter()
        .find(|g| g.kind == "matte" && !g.paid() && crate::keys::resolve(&g.key).is_some())
        .ok_or_else(|| {
            bad(
                Exit::NeedsInput,
                "a title card asks for behind, but no free matte row has a key".into(),
                "Drop `behind` from the card, or see `scriptvox providers`.",
            )
        })?;
    let mut out = vec![];
    for (k, (a, b)) in wins.iter().enumerate() {
        let seg = dir.join(format!("behind{}.mp4", k + 1));
        if !seg.is_file() {
            let sep = if visual.graph.is_empty() { "" } else { ";" };
            let mut args = visual.inputs.clone();
            args.extend([
                "-filter_complex".into(),
                format!("{}{sep}{}trim=start={a:.3}:end={b:.3},setpts=PTS-STARTPTS,scale={w}:{h},format=yuv420p[v]", visual.graph, visual.label),
                "-map".into(), "[v]".into(), "-an".into(),
                "-c:v".into(), "libx264".into(), "-crf".into(), "18".into(),
                seg.to_string_lossy().into_owned(),
            ]);
            ff.run(dir, &args, None).map_err(|e| {
                bad(
                    Exit::Error,
                    format!("cannot cut the behind window: {e:?}"),
                    "Run make again.",
                )
            })?;
        }
        let bytes =
            std::fs::read(&seg).map_err(|e| bad(Exit::Error, e.to_string(), "Run make again."))?;
        let req = row.request(&crate::generators::GenInput {
            video: Some(format!(
                "data:video/mp4;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            )),
            ..Default::default()
        });
        let (mask, _) = crate::genjob::run_clip(
            &row,
            &req,
            false,
            Path::new("."),
            dir,
            &format!("behind{}_matte", k + 1),
            run,
            0.0,
            GEN_WAIT_S,
        )?;
        out.push(mask);
    }
    Ok(out)
}

/// The render argv. The run folder is the cwd: captions and fonts are relative.
/// The video graph: visual track, captions, ending in `[v]`. The run folder is
/// the cwd, so captions and fonts are relative names.
pub fn video_graph(visual: &Visual) -> String {
    let sep = if visual.graph.is_empty() { "" } else { ";" };
    format!(
        "{}{sep}{}{},format=yuv420p[v]",
        visual.graph,
        visual.label,
        ass_filter("captions.ass", "fonts")
    )
}

#[cfg(test)]
mod tests {
    /// The run of 13 Nov 2026: the clip of scene 1 was 3.825 s, its audio after the cut 3.229 s, and "us." was from
    /// 3.366 to 3.825 s. Scene 2 started at 3.479 s.
    #[test]
    fn the_words_of_a_part_end_with_its_audio() {
        let w = |t: &str, start, end| Word {
            text: t.into(),
            start,
            end,
        };
        let est = vec![w("for", 2.9, 3.366), w("us.", 3.366, 3.825)];
        let got = super::part_words(est, 1.0, 10.0, 3.229);
        assert!((got[1].end - 13.229).abs() < 1e-9, "{got:?}");
        assert!(got[0].end <= got[1].start && got[1].start < got[1].end);
        // Measured times that end before the cut stay; the tempo scales them.
        let got = super::part_words(vec![w("us.", 2.0, 3.0)], 2.0, 10.0, 1.6);
        assert_eq!((got[0].start, got[0].end), (11.0, 11.5));
    }

    #[test]
    fn second_line_leaves_with_its_main_page() {
        use crate::captions::Page;
        let w = |t: &str, start, end| Word {
            text: t.into(),
            start,
            end,
        };
        let page = |i: usize, j: usize, start, end, said_end| Page {
            lines: vec![(i..j).collect()],
            start,
            end,
            said_end,
        };
        let words = [
            w("One", 0.0, 0.5),
            w("two.", 0.5, 1.0),
            w("Three", 4.0, 4.5),
            w("four.", 4.5, 5.0),
            w("Five", 5.2, 5.6),
            w("six.", 5.6, 6.0),
        ];
        let lines: Vec<Vec<String>> = ["a", "b", "c"]
            .iter()
            .map(|l| vec![l.to_string()])
            .collect();
        // A long pause after the first page; the last two sentences share a page.
        let pages = [page(0, 2, 0.0, 1.0, 1.0), page(2, 6, 4.0, 6.5, 6.0)];
        let cues = super::second_cues(&words, &pages, &lines);
        let times: Vec<(f64, f64)> = cues.iter().map(|c| (c.0, c.1)).collect();
        assert_eq!(times, [(0.0, 1.0), (4.0, 5.2), (5.2, 6.5)]);
    }

    /// Brief `caption_size`: normal is the size of every earlier version, and the pages are measured at the size shown.
    #[test]
    fn caption_size_scales_the_style_and_normal_is_the_old_size() {
        use crate::captions::{size_scale, Measurer};
        let words: Vec<Word> = "Most people think a good video needs a studio and understanding of internationalization today."
            .split(' ')
            .enumerate()
            .map(|(i, t)| Word {
                text: t.into(),
                start: i as f64 * 0.3,
                end: i as f64 * 0.3 + 0.25,
            })
            .collect();
        let size = |lang: &str, frame, style: &str, name: Option<&str>| {
            let (_, _, base, pages, st) =
                super::caption_pages(&words, lang, frame, style, size_scale(name).unwrap());
            (base, st.size, pages)
        };
        let (tall, wide) = ((1080, 1920), (1920, 1080));
        // The sizes before this field, as numbers: a change here changes every old brief.
        for (lang, frame, style, was) in [
            ("en", tall, "karaoke", 84),
            ("en", tall, "pop", 113),
            ("en", tall, "reveal", 100),
            ("en", wide, "karaoke", 84),
            ("ar", wide, "karaoke", 120),
            ("ar", wide, "reveal", 144),
        ] {
            let none = size(lang, frame, style, None);
            assert_eq!(none.1, was, "{lang} {style}");
            assert_eq!(none, size(lang, frame, style, Some("normal")));
            let (small, large) = (
                size(lang, frame, style, Some("small")),
                size(lang, frame, style, Some("large")),
            );
            assert_eq!((small.1, large.1), (was * 4 / 5, was * 5 / 4));
            // The hook, the cards and the second line take the first number: it does not move.
            assert_eq!((small.0, large.0), (none.0, none.0));
        }
        // Each line with two words or more fits the line width, measured at the size on screen. A word wider than
        // the line stands alone on its line, as at any size.
        for (style, max) in [("karaoke", 1080.0 * 0.83), ("pop", 1080.0 * 0.62)] {
            let (_, px, pages) = size("en", tall, style, Some("large"));
            let m = Measurer::new(crate::template::FONTS[0].2.to_vec(), px as f32).unwrap();
            let mut lone_wide = 0;
            for l in pages.iter().flat_map(|p| &p.lines) {
                let text: Vec<&str> = l.iter().map(|&i| words[i].text.as_str()).collect();
                let w = m.width(&text.join(" "));
                if l.len() > 1 {
                    assert!(w <= max, "{style}: {text:?} is {w} of {max}");
                } else if w > max {
                    // Wider than the line, inside the frame with its box (a sixth of the size on each side).
                    assert!(w + px as f32 / 3.0 <= 1080.0, "{text:?} is {w}");
                    lone_wide += 1;
                }
            }
            // At large pop (141 px) "understanding" is 684 px and "internationalization" 926 px: over the 670 px
            // line, inside the 1080 px frame.
            assert_eq!(lone_wide, if style == "pop" { 2 } else { 0 }, "{style}");
        }
        assert!(size_scale(Some("huge"))
            .unwrap_err()
            .contains("small, normal or large"));
    }

    #[test]
    fn a_patch_changes_only_its_fields() {
        let a: super::Args =
            serde_json::from_str(include_str!("../../../fixtures/request-3-scenes.json")).unwrap();
        let p = serde_json::json!({"hook": "Three colours", "music": null, "stock": ["sea", "sky"], "sfx": null});
        let (b, keys) = super::patch_request(&a, &p).unwrap();
        assert_eq!(keys.len(), 4);
        assert_eq!(b.hook.as_deref(), Some("Three colours"));
        assert_eq!(b.music, None);
        assert_eq!(b.stock.as_deref(), Some("sea,sky"));
        assert!(!b.sfx);
        assert_eq!(b.script, a.script);
        assert_eq!(b.scenes, a.scenes);
        // New scenes are the new script.
        let p = serde_json::json!({"scenes": [{"script": "Only one now."}]});
        assert_eq!(
            super::patch_request(&a, &p).unwrap().0.script,
            "Only one now."
        );
        for bad in [
            serde_json::json!({"run_id": "r_x"}),
            serde_json::json!({"brand": "b.json"}),
            serde_json::json!({}),
            serde_json::json!({"fps": "fast"}),
        ] {
            assert!(super::patch_request(&a, &bad).is_err(), "{bad}");
        }
        // `resume --patch {"caption_size": "large"}`: a field a run keeps.
        let p = serde_json::json!({"caption_size": "large"});
        let (b, keys) = super::patch_request(&a, &p).unwrap();
        assert_eq!(keys, ["caption_size"]);
        assert_eq!(b.caption_size.as_deref(), Some("large"));
        // `sources_card` is not a field of a request: the patch puts the card on the last scene, from the patched
        // sources, and `false` takes it off again.
        let p = serde_json::json!({"sources_card": true, "sources": [{"text": "a fact", "publisher": "NASA"}]});
        let (with, keys) = super::patch_request(&a, &p).unwrap();
        assert!(keys.contains(&"sources_card".to_string()), "{keys:?}");
        let last = serde_json::to_value(with.scenes.as_ref().unwrap().last().unwrap()).unwrap();
        assert!(last.to_string().contains(r#""label":"NASA""#), "{last}");
        let (off, _) =
            super::patch_request(&with, &serde_json::json!({"sources_card": false})).unwrap();
        assert_eq!(off.scenes, a.scenes);
        assert!(
            super::patch_request(&a, &serde_json::json!({"sources_card": true}))
                .unwrap_err()
                .contains("Add `sources`")
        );
        assert!(super::patch_request(&a, &serde_json::json!({"sources_card": "yes"})).is_err());
    }

    #[test]
    fn a_square_picture_loses_edges_in_a_tall_clip() {
        let k = shape_keeps(1024, 1024, "9:16").unwrap();
        assert!((k - 0.5625).abs() < 0.001, "{k}");
        assert!((shape_keeps(720, 1280, "9:16").unwrap() - 1.0).abs() < 0.001);
        assert!(shape_keeps(10, 10, "bad").is_none());
    }

    #[test]
    fn no_voice_reads_at_a_pace_and_keeps_pauses() {
        let segs = [
            Seg::Speech("one two".into()),
            Seg::Pause(1.0),
            Seg::Speech("three".into()),
        ];
        let w = reading_times(&segs, "en", None);
        assert_eq!(w.len(), 3);
        assert!((w[1].end - 0.9).abs() < 1e-6, "{w:?}");
        assert!((w[2].start - 1.9).abs() < 1e-6, "{w:?}");
    }

    use super::*;

    #[test]
    fn feed_frame_is_4_5_and_asks_generators_for_9_16() {
        assert_eq!(frame("4:5"), Some((1080, 1350)));
        assert_eq!(gen_aspect("4:5"), "9:16");
        assert_eq!(gen_aspect("2:3"), "9:16");
        assert_eq!(gen_aspect("21:9"), "16:9");
        assert_eq!(frame("21:9"), Some((2520, 1080)));
        assert_eq!(gen_aspect("16:9"), "16:9");
        assert_eq!(frame("5:4"), None);
    }

    #[test]
    fn a_spanish_script_is_seen_and_english_is_not() {
        assert_eq!(foreign_script("La ciudad de Abu Dabi es una de las mas grandes y tiene mucha historia en la costa"), Some("Spanish"));
        assert_eq!(
            foreign_script(
                "The city of Abu Dhabi is one of the largest on the coast, and it is old"
            ),
            None
        );
    }

    /// A script in a language with a voice row gets that language; English stays English (docs/specs/latin-languages.md).
    #[test]
    fn detects_the_latin_languages() {
        for (text, want) in [
            ("El agua de lluvia llega primero a las calles y después a los canales de la ciudad.", "es"),
            ("L'eau de pluie arrive d'abord dans les rues, puis dans les canaux de la ville et du port.", "fr"),
            ("Das Regenwasser fließt zuerst auf die Straßen und dann in die Kanäle der Stadt und ist nicht sauber.", "de"),
            ("A água da chuva chega primeiro às ruas e depois aos canais, e não fica em um só lugar para sempre.", "pt"),
            ("Air hujan pertama-tama mengalir ke jalan, lalu masuk ke saluran yang ada di kota ini dan itu.", "id"),
            ("Rain water goes first to the streets and then to the canals of the city.", "en"),
            ("Il mercato apre prima che il sole sorga e gli amici sono con noi per la festa della città.", "en"),
            ("Hola amigos", "en"),
            (
                "Savez-vous qu'en 2005 la ville avait 1250 fontaines ? Aujourd'hui, 87 % de l'eau de pluie arrive aux canaux en 3,5 heures.",
                "fr",
            ),
            ("¿Sabías que en 2005 la ciudad tenía 1.250 fuentes? Hoy el 87% del agua llega a los canales.", "es"),
            ("Você sabia que em 2005 a cidade tinha 1.250 fontes? Hoje, 87% da água da chuva chega aos canais.", "pt"),
            ("Wussten Sie, dass die Stadt im Jahr 2005 schon 1.250 Brunnen hatte? Große Straßen bleiben trocken.", "de"),
            ("Tahukah kamu bahwa pada tahun 2005 kota ini punya 1.250 air mancur? Itu kerja keras banyak orang.", "id"),
            ("The café is on the left of the piñata shop, and it is open for the people of this town.", "en"),
        ] {
            assert_eq!(detect_lang(text), want, "{text}");
        }
    }

    /// English stays English, also a short line with a word that another language has ("do", "as", "die", "pour",
    /// "den", "con", "pas", "sur", "um"). 13 Nov 2026: "Why do cats purr so much?" got a Portuguese voice.
    #[test]
    fn short_english_lines_stay_english() {
        for line in [
            "Why do cats purr so much?",
            "Do as I say, not as I do.",
            "Why did stars die so young?",
            "Uma and Dan pour water on a den.",
            "Stop scrolling. This changes everything today.",
            "Three habits that changed my mornings forever.",
            "Dates are an old food of the Gulf.",
            "They give fast energy to every person who eats them.",
            "One two three four five six seven eight.",
            "Most floods start with a blocked drain.",
            "Revenue grew by forty two percent in one year.",
            "Rain water goes first to the streets.",
            "Never die wondering what could happen next.",
            "Pour the milk, stir, and wait a minute.",
            "A faux pas at dinner can ruin the night.",
            "Um, as I said, do not do that.",
            "Los Angeles and Las Vegas are six hours apart.",
            "El Paso sits on the border with Mexico.",
            "La Paz has an airport at high altitude.",
            "De facto rules beat written rules every time.",
            "We drove en route to San Jose yesterday.",
            "Yang and Dan met Uma at Como lake.",
            "Coffee beans lose flavor two weeks after roasting.",
            "Sharks existed before trees ever grew on Earth.",
            "Honey never spoils, even after thousands of years.",
            "Speed matters less than direction, most days.",
            "Big news: prices drop again next week.",
            "Come closer and listen. Today Nguyen works at the lab with the team.",
            "I like tomato soup on a cold day.",
            "Khalid is here today with all of us.",
            "ACME makes 2005 parts a day at 87% of plan.",
            "Sleep eight hours, drink water, walk daily, repeat.",
            "Mount Fuji last erupted three centuries ago.",
            "Octopuses have three hearts and blue blood.",
            "Your phone battery hates heat more than cold.",
            "Bananas are berries but strawberries are not.",
            "Why does ice float? Density explains everything.",
            "Sur La Table sells kitchen tools online.",
            "Con artists rely on trust, not force.",
            "The den was dark, quiet, and warm inside.",
            "Los Angeles, Las Vegas, El Paso, La Paz, San Jose: five cities.",
            "Die Hard is a film that people watch in December.",
        ] {
            assert_eq!(detect_lang(line), "en", "{line}");
        }
    }

    #[test]
    fn detects_arabic() {
        assert_eq!(detect_lang("مرحبا بكم في ScriptVox"), "ar");
        assert_eq!(detect_lang("Hello world and all my friends, مرحبا"), "en");
    }

    #[test]
    fn a_behind_title_goes_where_the_subject_crosses_a_fifth_of_it() {
        // A tower: rows 0 to 40 percent empty, then 60 percent wide (the band reads 0.6), a mast 10 percent wide above.
        let mut p = vec![0.0; 100];
        for (i, v) in p.iter_mut().enumerate() {
            *v = if i >= 40 {
                0.6
            } else if i >= 20 {
                0.1
            } else {
                0.0
            };
        }
        let (y, c) = place(&p, 0.06, 1, 0.36, 0.2);
        // The band nearest 20 percent hidden straddles the top of the tower.
        assert!((0.35..=0.42).contains(&y), "{y}");
        assert!((c - 0.2).abs() < 0.12, "{c}");
        // Nothing crosses anywhere: the default height stays.
        assert_eq!(place(&[0.0; 100], 0.06, 1, 0.36, 0.2), (0.36, 0.0));
        // Two lines: the lower line must not sink into the tower, so the pair sits higher than one line would.
        let (y2, c2) = place(&p, 0.06, 2, 0.36, 0.2);
        assert!(y2 < y && c2 <= 0.35, "{y2} {c2}");
    }

    #[test]
    fn the_logo_sits_in_a_top_corner_at_its_share_of_the_width() {
        let v = Visual {
            inputs: vec![],
            graph: String::new(),
            label: "[0:v]".into(),
        };
        let l = crate::brief::Logo {
            file: "l.png".into(),
            corner: None,
            width: None,
            opacity: None,
        };
        assert_eq!(
            logo_graph(&v, &l, 2, (1080, 1920), None),
            "[2:v]scale=172:-2,format=rgba,colorchannelmixer=aa=0.90[lg];[0:v][lg]overlay=W-w-43:96[logo]"
        );
        let l = crate::brief::Logo {
            corner: Some("left".into()),
            width: Some(0.3),
            opacity: Some(0.05),
            ..l
        };
        let g = logo_graph(&v, &l, 2, (1920, 1080), Some(9.5));
        assert!(
            g.contains("scale=576:-2") && g.contains("aa=0.20") && g.contains("overlay=76:54"),
            "{g}"
        );
    }

    #[test]
    fn the_outro_blurs_from_its_start_and_centres_the_logo() {
        let v = Visual {
            inputs: vec![],
            graph: "[0:v]null[bg]".into(),
            label: "[bg]".into(),
        };
        let g = outro_graph(&v, 9.5, Some(3), (1080, 1920));
        assert!(
            g.starts_with(
                "[0:v]null[bg];[bg]split[ob][oe];[oe]boxblur=24:2:enable='gte(t,9.500)',"
            ),
            "{g}"
        );
        assert!(
            g.contains("[3:v]scale=432:-2")
                && g.contains("setpts=PTS+9.500/TB[ol]")
                && g.contains("fade=t=in:st=9.500:d=0.4:alpha=1[oc]")
                && !g.contains("trim=start"),
            "{g}"
        );
        assert!(
            g.ends_with(";[ob][od]overlay=eof_action=pass[outro]"),
            "{g}"
        );
        assert!(!outro_graph(&v, 9.5, None, (1080, 1920)).contains(":v]scale"));
    }

    #[test]
    fn behind_graph_lays_the_subject_over_the_title_in_each_window() {
        let v = Visual {
            inputs: vec![],
            graph: "[0:v]scale=1080:1920[bg]".into(),
            label: "[bg]".into(),
        };
        let g = behind_graph(&v, &[(0.0, 2.5), (6.02, 7.5)], 3, (1080, 1920), 30);
        assert!(g.starts_with("[0:v]scale=1080:1920[bg];[bg]split=3[bgb][fg0][fg1];[bgb]ass=filename=behind.ass:fontsdir=fonts[t0]"), "{g}");
        // The mask has one black frame for each frame of the video before 6.02 s (181 at 30 fps), so the subject
        // branch starts with the video.
        assert!(
            g.contains("[3:v]fps=30,format=gray,scale=1080:1920,tpad=start=0[m0]"),
            "{g}"
        );
        assert!(g.contains("[4:v]fps=30,format=gray,scale=1080:1920,tpad=start=181[m1];[fg1][m1]alphamerge[s1]"), "{g}");
        assert!(
            g.contains("[t1][s1]overlay=eof_action=pass:enable='gte(t,6.020)*lt(t,7.500)'[t2]"),
            "{g}"
        );
        // No branch starts late: a late branch makes the overlay hold every earlier frame (see `outro_graph`).
        assert!(
            !g.contains("trim=start") && !g.contains("setpts=PTS+"),
            "{g}"
        );
        assert!(g.ends_with(";[t2]null[behind]"), "{g}");
    }

    #[test]
    fn render_graph_is_relative() {
        let g = video_graph(&Visual::gradient((1080, 1920), FPS, 3.0));
        assert_eq!(
            g,
            "[0:v]ass=filename=captions.ass:fontsdir=fonts,format=yuv420p[v]"
        );
    }

    #[test]
    fn row_choice_ignores_licence_terms() {
        // Every sound-from-video row is non-commercial, and that alone never stops a plan: without a key it asks for
        // the key, as any row does.
        let rows: Vec<_> = crate::generators::all()
            .into_iter()
            .filter(is_v2a)
            .collect();
        assert!(
            rows.iter().all(|g| g.terms.commercial == Some(false)),
            "{rows:?}"
        );
        if let Err(e) = pick_row("ambience", "sound-from-video", is_v2a) {
            assert_eq!(e.exit, Exit::NeedsInput);
        }
    }
}
