//! The brief: one JSON file that describes a video (`make --brief f`). CLI
//! flags override its fields. `scriptvox schema brief` prints its JSON Schema.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One spoken line of a dub, on the base video's own times.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Seconds from the start of the base video.
    pub start: f64,
    /// When the source line ends. The new line may run past it into the gap before the next line.
    pub end: f64,
    /// The words to say.
    pub text: String,
    /// Another voice for this line: a voice id of the same engine (`scriptvox voices`), for a second speaker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

/// One scene: its narration and, optionally, its own visual.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// The narration of this scene. Leave it out, with `seconds`, for a scene with no words.
    #[serde(default)]
    pub script: String,
    /// For a scene with no `script`: how long its picture shows, in seconds (0.2 to 600). No voice and no caption; any `music` plays on. A scene with a script takes the time of its words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    /// A clip or photo for this scene, inside the project, relative to the brief.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PathBuf>,
    /// Where `media` starts playing, in seconds, when it is a clip. Default: its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_from: Option<f64>,
    /// Where that clip stops, in seconds. Past it the clip slows or holds its last frame, and never runs on into what
    /// comes next in the file. Default: its end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_to: Option<f64>,
    /// The sound of this scene's own `media` clip: `none` (default, the clip is silent), `under` (quiet, about 12 dB
    /// under the voice) or `keep` (at the voice's level, lowered while the voice speaks). It is the same part of the
    /// file as the picture, at the clip's own speed. Not for `stock`, `open_image` or `generate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    /// A stock search term for this scene (Pexels or Pixabay with a key, else Commons and Internet Archive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<String>,
    /// A search term for a free picture for this scene, found with no key through `media search`. Each picture keeps its own
    /// licence in credits.txt and in export; its terms do not stop the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_image: Option<String>,
    /// An AI video clip for this scene, made by a generator row (costs money; `plan` prices it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate: Option<GenScene>,
    /// Motion text over this scene: title, stat, lower_third, list, bars, line, formula, code, diff, terminal, plot, quote, compare, donut,
    /// timeline, table, number_line, diagram, steps, progress or gauge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<crate::cards::Card>,
    /// Or 1 to 4 cards that show one after the other in this scene, never two together, in time order. Each is a
    /// `card` with its own start, `on` (a spoken word) or `at` (seconds from the scene's start), and its own `hold`;
    /// a card ends 0.2 s before the next starts. With no times the cards share the scene evenly. Not with `card`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cards: Option<Vec<crate::cards::Card>>,
    /// A camera move made by scriptvox on this scene's clip or photo, the same every time: `push_in`, `pull_out`,
    /// `pan_left` or `pan_right` (about 12 percent over the scene). Generated clips need it: their own camera words
    /// barely move (measured on Wan 2.2 5B).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// A smaller picture over this scene (a speaker over slides, a detail over a wide shot). Its own sound is not used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset: Option<Inset>,
    /// A full-frame layer with a transparent background over this scene, inside the project, relative to the brief: a
    /// WebM (VP8 or VP9 with alpha), a MOV (ProRes 4444 or Animation) or a PNG, for example a Manim, Remotion or
    /// HyperFrames render. It fits the frame without stretching and starts with the scene; captions stay on top. Its
    /// own sound is not used. A file with no alpha is refused, as it would hide the scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<PathBuf>,
    /// Another speaker for this scene: a voice id of the same engine (`scriptvox voices`), for a dialogue. The open `selfhost`
    /// voice has one speaker and refuses this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// The transition into this scene, instead of the video's `transition`. Same names. Not on the first scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,
}

/// The first of a brief's `script_file` and cutaway files that lies outside the project folder.
pub fn file_outside_project(b: &Brief) -> Option<PathBuf> {
    let root = std::path::Path::new(".").canonicalize().ok()?;
    b.script_file
        .iter()
        .chain(b.cutaways.iter().filter_map(|c| c.file.as_ref()))
        .map(|p| p.canonicalize().unwrap_or_else(|_| lexical(&root.join(p))))
        .find(|p| !p.starts_with(&root))
}

/// A path that does not exist is judged by its words (`..` taken back), not skipped: else `../../etc/hostname` gave
/// "cannot read" on macOS and "outside" on Linux, and the answer told whether a file outside the project exists.
fn lexical(p: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// A picture over the base video for a few seconds (brief `cutaways`). The base video's own sound plays on.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cutaway {
    /// Seconds of the base video where it starts.
    pub at: f64,
    /// How long it shows, 0.5 s or more. A clip shorter than this holds its last frame.
    pub seconds: f64,
    /// A clip or still inside the project, relative to the brief. Or `prompt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Or the shot to make with a generator row (the plan prices it). The brief's `style` goes at its end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// A row from `scriptvox providers`. Default: the cheapest ready row that fits the shape, which can be a paid row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// A still from an image row instead of a clip: far faster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still: Option<bool>,
}

/// A picture in picture.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Inset {
    /// A clip or photo inside the project, relative to the brief.
    pub file: PathBuf,
    /// `top_right` (default), `top_left`, `bottom_right` or `bottom_left`. The bottom ones stay above the captions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner: Option<String>,
    /// Share of the frame width, 0.2 to 0.6. Default 0.4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
}

/// One fact of the script and where it comes from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The fact as the script states it, for example "Revenue grew 42 percent in 2025".
    pub text: String,
    /// Where it comes from: a page (`https://...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The name of the source: a report, a book, a person. Give `url`, `title` or both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The short name of who published it ("Wikipedia", "NASA"): the name that a `sources_card` shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
}

impl Source {
    /// What is wrong with this source, if anything. Nothing is fetched: this checks the form only.
    pub fn problem(&self) -> Option<String> {
        let t = self.text.trim();
        if t.is_empty() || t.chars().count() > 300 {
            return Some("`text` is the fact in 1 to 300 characters".into());
        }
        let url = self.url.as_deref().map(str::trim).filter(|u| !u.is_empty());
        let title = self
            .title
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty());
        if url.is_none() && title.is_none() {
            return Some(format!("\"{t}\" has no `url` and no `title`"));
        }
        if let Some(u) = url {
            if !(u.starts_with("https://") || u.starts_with("http://"))
                || u.contains(char::is_whitespace)
            {
                return Some(format!(
                    "`url` of \"{t}\" is not a web address (https://...)"
                ));
            }
        }
        None
    }

    /// One line for credits.txt.
    pub fn credit(&self) -> String {
        let from = [self.title.as_deref(), self.url.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Source: {} ({from})", self.text.trim())
    }
}

/// A generated clip: what to show, and optionally which row, how long and a first frame.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenScene {
    /// The shot: subject, action, camera, light, style.
    pub prompt: String,
    /// A row from `scriptvox providers` (generators). Default: the cheapest row with a key that fits the aspect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Clip length to ask for. Default: the scene narration length, estimated from its words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    /// A still picture from an image row instead of a clip: far faster (about 10 s against minutes), with a slow push-in
    /// or the scene's `camera`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still: Option<bool>,
    /// A first-frame image inside the project, relative to the brief (image to video).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<PathBuf>,
    /// Or a prompt for a first frame: an image row makes it, then the clip starts from it (image to video).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_frame: Option<String>,
    /// The picture the clip ends on, inside the project, relative to the brief. Needs `image` or `first_frame`, and a
    /// row with `takes_last_frame` in `scriptvox providers`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_frame: Option<PathBuf>,
    /// `false`: this scene leaves out the brief's `character` or `characters` (a skyline, a chart, an empty room).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<bool>,
    /// Which of the brief's `characters` are in this scene, by name (at most 4). Default: all of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub who: Vec<String>,
}

/// The same person or thing in every generated scene.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Character {
    /// The name scenes use in `who` (brief `characters`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Describe them: an image row makes one reference picture, once, and keeps it. Clip prompts carry these words too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// A picture of them inside the project, relative to the brief (PNG, JPEG or WebP). Add `prompt` when the video has
    /// clips: a clip row reads words only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    /// The narration. Pause tags like `[pause 1.5]` are allowed (0.1 to 10 s each; a pause at the end holds the picture and the music), and delivery cues like `[whisper]` for a voice whose row takes them (`takes_tags` in `scriptvox voices`). Use this or `script_file`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Scenes in order: each with its narration and its own clip, photo or stock term.
    /// Cuts fall on scene boundaries. Use this, `script` or `script_file`. A scene with `seconds` and no `script` has no words: its picture shows for that time, with no voice and no caption (a reel with music only: every scene with `seconds`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenes: Option<Vec<Scene>>,
    /// A dub on the times of the base video: each line starts at its own `start` and fits its own time. Needs
    /// `base_video`. Use this in place of `script`, `script_file` and `scenes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<Line>>,
    /// A text file with the narration, relative to the brief.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_file: Option<PathBuf>,
    /// en, ar, es, fr, de, pt or id (`scriptvox voices` lists them). Default: detected from the script.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// TTS engine: edge, elevenlabs, kokoro, kokoro-rust, selfhost (an open voice on your own GPU service), or none: no voice, the script runs
    /// as captions at reading pace over the picture and any music. `reveal` captions suit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<String>,
    /// Voice id from `scriptvox voices`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// Target length in seconds; the narration is fitted before render. A brief with `duration` and no script, script_file, scenes or lines is a video with no words of that length (picture and `music` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9. Generated clips are asked as 9:16 (tall shapes) or 16:9 (wide shapes) and
    /// cropped to the frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<String>,
    /// The short side of the frame in px: 720, 1080 (default), 1440 or 2160, for every `aspect` (16:9 at 2160 is 3840x2160). Text, cards, logo and outro follow it; a smaller clip is scaled up. `plan` gives the render time and disk use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Frames per second of the video: 24 (film; generated clips at 24 play frame for frame), 25, 30 (default) or 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    /// Folder of your own clips and stills, inside the project, relative to the brief.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PathBuf>,
    /// Stock search terms, one per shot, or `auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<Vec<String>>,
    /// A headline shown big for the first 1.6 seconds (the hook).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
    /// Caption style: `karaoke` (default: up to two lines, the spoken word lights up) , `pop` (1 to 3 big words
    /// at a time, each page pops in) or `reveal` (up to 3 lines, no box, each word appears when it is said).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captions: Option<String>,
    /// The transition between shots: `fade` (default, 0.25 s), `slide`, `cut`, `wipe`, `push`, `cover`,
    /// `dissolve`, `blur`, `circle`, `dip` (through black), `flash` (through white), `pixel`, `radial`, `wind`, `zoom`,
    /// `reveal`, `iris`, `split`, `slice` or `rise`.
    /// Sideways moves follow the reading direction. A scene's own `transition` wins for the cut into that scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,
    /// The colour look of the picture: `natural` (default), `warm`, `cool`, `film` (softer colour, more contrast,
    /// a vignette), `punchy` (more contrast and colour), `mono`, or a `.cube` 3D LUT file relative to the brief (a grade
    /// from your colourist). Captions and cards keep their colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    /// Built-in sound effects: a whoosh into each scene cut, a pop when a card appears, ticks while a stat counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<bool>,
    /// `beat`: each scene boundary moves to the nearest beat of the music bed when that beat is at most 0.25 s away;
    /// the picture, the scene's card and the whoosh move, the voice and the captions do not. Needs `scenes` and
    /// `music` as a built-in bed with a beat (`pulse`) or a music file in the project. A boundary with no beat near,
    /// or a bed with no steady beat, stays where it was; the result's `cut_on` lists each boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_on: Option<String>,
    /// Upscale each generated clip with an upscale row (SeedVR2 3B on your own GPU service): a 704x1280 Wan clip comes back
    /// as 1080x1920 with real detail instead of plain scaling. Adds about a minute per clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upscale: Option<bool>,
    /// Smooth motion for each generated clip with an interpolate row (RIFE on your own GPU service): new frames between
    /// the clip's own, 24 to 48 fps. A clip a little short for its scene then plays up to 2 times slower without
    /// a stutter (24 fps clips: 1.43 times at most). Adds about half a minute per clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    /// Sound for each video scene from its own picture (MMAudio on your own GPU service), low under the voice. The
    /// terms of the model are in `providers` and in `metadata.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambience: Option<bool>,
    /// calm, pulse, library:<name> (a song with a free licence, `scriptvox media music` lists them), a music file in the project, an https address, or none. Any song or media file can be used (mp3, m4a, wav, or a video for its sound): the choice and the responsibility are yours. A file shorter than the video starts again, and the music fades out in the last second. An address: `make` downloads its sound one time through `fetch` (run `scriptvox setup fetch` first), and `plan` lists the download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,
    /// The start of the cut of a music file or of music from an address: seconds as a number (`12.5`) or as text (`"12.5"`), or a clock (`"0:12"`).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "seconds_text"
    )]
    #[schemars(with = "Option<SecondsOrClock>")]
    pub music_from: Option<String>,
    /// The end of that cut, in the same forms.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "seconds_text"
    )]
    #[schemars(with = "Option<SecondsOrClock>")]
    pub music_to: Option<String>,
    /// A video to re-voice: it plays from the start under the new narration, and its own sound is dropped
    /// (translate: transcribe it with `scriptvox analyze --transcript`, write the new script, set `lang`). Give the
    /// narration as `script` or `script_file`, not `scenes`: the base video replaces the other visuals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_video: Option<PathBuf>,
    /// The base video's own sound under the new voice: `none` (default), `keep` (all of it, low and ducked) or
    /// `background` (its voice removed by a stem row, so the music and ambience stay: a dub). Replaces the
    /// music bed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sound: Option<String>,
    /// Pictures over `base_video` at its own times, each a file or a generated shot: one plan prices them all, then
    /// they go over the video with its own sound and captions on top. Needs `base_video`, and no script, scenes or lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cutaways: Vec<Cutaway>,
    /// Written forms the voice says another way, `{"ACME": "A C M E"}`, or with IPA between slashes and then the respelled form, `{"Doha": "/ˈdoʊhə/ doh-ha"}` (a voice with `takes_ipa` says the IPA). The captions keep the written form. A brand file
    /// may hold this.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub say: BTreeMap<String, String>,
    /// How far any bed sits under the voice: `quiet`, `normal` (default) or `loud`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_level: Option<String>,
    /// The plain background, when a scene has no picture: `dusk` (default, purple to yellow), `night`, `ocean`, `forest`,
    /// `sand`, `slate`, or 2 or 3 of your colours as `#0b1020,#1b2a4a`. It moves slowly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// `true`: a waveform that follows the video's own sound (voice and music as heard), in the `accent` colour, over
    /// the plain background of the scenes with no picture and no card. For a podcast clip or a voice-only video. It
    /// stays clear of the captions. Needs sound: a voice, a narration or music.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waveform: Option<bool>,
    /// A look added to the end of every `generate` prompt, so the clips of a video or a series look alike. For example
    /// "ink illustration, muted colours, soft morning light". Name the result, not the making.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// A seed for every generated still and clip: the same seed and brief give the same pictures on rows that take one
    /// (`takes_seed` in `scriptvox providers`, today the local image row). The plan says when a row does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// The brand's font for captions, cards and the hook: a TTF or OTF file inside the project, relative to the brief.
    /// It must hold every letter of the script. You answer for its licence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<PathBuf>,
    /// Your own recorded narration (wav, mp3, m4a or flac) instead of a voice: no TTS. Keep `script` as the words said:
    /// captions follow them, exactly with a speech to text row, else at an even pace. The file sets the length, so
    /// no `duration`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration: Option<PathBuf>,
    /// Where captions sit: `bottom` (default), `middle` (lower middle, a common short-form place) or `top`. `middle` and
    /// `top` need a video with no `hook` and no scene `card`, which use that space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_at: Option<String>,
    /// How big the captions are: `small` (four fifths), `normal` (default), `large` (five quarters), or a percent of
    /// normal from `"50%"` to `"200%"`, for example `"115%"`. The lines are measured again at the new size, so they
    /// still fit the frame. The hook, the cards and `captions_second` keep their size.
    /// A bare number is read as a percent: 115 is `"115%"`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "size_text"
    )]
    pub caption_size: Option<String>,
    /// A black outline around the caption letters, in px on a 1080 px short side: 0 to 12 (it follows the frame's
    /// size). Default: none inside the box; with `caption_box` 0, a tenth of the caption size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_outline: Option<f64>,
    /// The black box behind the captions, as its opacity: 0 is no box, 1 is solid black. Default 0.65. Not for
    /// `captions` `reveal`, which has no box. With no box, the gate `contrast` warns when the letters cannot be
    /// read on the picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_box: Option<f64>,
    /// The colours of the cards, the hook and the end card: `dark` (default, light text on dark boxes) or `paper` for a light `background`: dark text, light boxes, thin rules, and the `accent` for headers, marks and numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_style: Option<String>,
    /// `true`: the video is meant to play in a loop. It only asks for the gate `loop_seam`, which compares the last
    /// frame with the first and warns when the jump shows. It changes nothing in the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#loop: Option<bool>,
    /// How fast the voice speaks: 1.0 is the voice's own pace, 0.75 is slower, 1.3 is faster (ElevenLabs: up to 1.2).
    /// For `tts` edge, elevenlabs, kokoro-rust and none. With `duration` the voice keeps this pace, and the length is
    /// reached by a tempo change of at most 16 percent, or the run stops and says how many words to add or cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_rate: Option<f64>,
    /// With `media`: each line of the script gets the file from the folder that shows it, instead of the folder order. A
    /// vision service you set looks at each file once. `plan` sends nothing and names the host; `make` sends the frames and
    /// lists the picks in `media_match`, and a later `plan` shows them. To change one, write `scenes` yourself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_match: Option<bool>,
    /// Key words or phrases of the script that stay in the accent colour on their caption page, for example
    /// `["90 percent", "free"]`. Few of them, or nothing stands out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emphasis: Vec<String>,
    /// A small second caption line under the main one, for example a translation: one entry per sentence of the
    /// script, in order. Each shows while its sentence is said. Not with `caption_at` `top`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captions_second: Vec<String>,
    /// The same person or thing in every generated scene: each still and first frame starts from one reference
    /// picture, and a clip with no first frame gets one. Name them in the scene prompts as "the same woman".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<Character>,
    /// Two or more people in one story, each with a `name`. A scene names who is in it with `generate.who`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<Character>,
    /// A logo on every frame: a PNG (transparent background) in a top corner, under the captions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<Logo>,
    /// The brand colour as `#RRGGBB`: the highlight of the spoken word and the bars of a chart (default yellow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// An end card after the narration: the picture blurs and darkens, the logo (if any) shows large in the middle,
    /// and `text` (a call to action, at most 60 characters) under it. The music runs on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outro: Option<Outro>,
    /// Or a music bed made by a music row from this description (style, instruments, mood, tempo; no vocals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_prompt: Option<String>,
    /// Or a free music bed found with no key through `media search`, from these search words. It keeps its own licence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_search: Option<String>,
    /// The facts the script states and where each comes from (at most 20). ScriptVox fetches nothing and does not judge
    /// them: `export` writes them to `metadata.json` and to the credits, and they travel in the MP4's recipe, so a
    /// viewer or an editor can check a number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    /// `true`: a card "Sources" on the last scene with the names of the publishers of `sources` (each `publisher`, else `title`, else the host of `url`), in English or Arabic. Needs `scenes` and `sources`. Default: no card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources_card: Option<bool>,
    /// A brand file (JSON, path relative to the brief) with the look of a series: lang, tts, voice, aspect, size, fps,
    /// captions, caption_at, caption_size, caption_outline, caption_box, font, style, background, music_level, transition, look, sfx, music, music_prompt, logo, accent, outro, card_style. A field in the brief wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<PathBuf>,
}

/// The schema of a time that is seconds as a number, or text (`"12.5"`, `"0:12"`). The brief keeps it as text.
#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)] // a schema only
enum SecondsOrClock {
    Seconds(f64),
    Text(String),
}

/// `music_from` and `music_to` as text: `12.5` (a number) is `"12.5"`, as `--music-from 12.5` gives it.
fn seconds_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::String(s)) => Some(s),
        None | Some(serde_json::Value::Null) => None,
        Some(v) => {
            return Err(serde::de::Error::custom(format!(
                "seconds as a number, or text like \"0:12\", not {v}"
            )))
        }
    })
}

/// The first field of a JSON object that `T` does not take when it stands alone, with the reason. For an error that
/// does not name its field ("invalid type: integer `0`, expected a string"). Every field of `T` must be optional.
fn bad_field<T: serde::de::DeserializeOwned>(obj: &serde_json::Value) -> Option<(String, String)> {
    obj.as_object()?.iter().find_map(|(k, val)| {
        serde_json::from_value::<T>(serde_json::json!({k: val}))
            .err()
            .map(|e| (k.clone(), e.to_string()))
    })
}

/// `caption_size` as text: a name, a percent, or a bare number that is taken as a percent.
fn size_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        Some(serde_json::Value::Number(n)) => Some(format!("{n}%")),
        Some(serde_json::Value::String(s)) => Some(s),
        None | Some(serde_json::Value::Null) => None,
        Some(v) => {
            return Err(serde::de::Error::custom(format!(
                "caption_size is text or a number, not {v}"
            )))
        }
    })
}

/// The end card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Outro {
    /// The line under the logo, for example a web address or "Follow for more".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Length in seconds, 1 to 6 (default 2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
}

/// A logo in a corner of every frame.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Logo {
    /// A PNG inside the project, relative to the brief. A transparent background reads best.
    pub file: PathBuf,
    /// The top corner: `right` (default) or `left`. Not mirrored for Arabic: a brand keeps its place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner: Option<String>,
    /// Width as a share of the frame width, 0.05 to 0.4 (default 0.16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// 0.2 to 1 (default 0.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

/// A serde error with where it is (`scenes[1].card`) and the closest allowed name for a typo (4 Nov 2026 eval:
/// "unknown variant `bullets`" did not say which scene, "unknown field `captoins`" did not say "captions").
fn explain(v: &serde_json::Value, e: serde_json::Error) -> String {
    let mut msg = e.to_string();
    let scenes = v["scenes"].as_array().into_iter().flatten().enumerate();
    for (i, sc) in scenes {
        if let Err(se) = serde_json::from_value::<Scene>(sc.clone()) {
            let bad = |c: &serde_json::Value| {
                !c.is_null() && serde_json::from_value::<crate::cards::Card>(c.clone()).is_err()
            };
            let in_list = sc["cards"].as_array().and_then(|l| l.iter().position(bad));
            let at = match in_list {
                _ if bad(&sc["card"]) => ".card".to_string(),
                Some(j) => format!(".cards[{j}]"),
                None => String::new(),
            };
            // The field of the scene, when the error does not name one.
            let at = match bad_field::<Scene>(sc) {
                Some((k, _)) if at.is_empty() && !se.to_string().starts_with("unknown field") => {
                    format!(".{k}")
                }
                _ => at,
            };
            msg = format!("scenes[{i}]{at}: {se}");
            break;
        }
    }
    // A top-level field with the wrong type: serde says the type and not the field.
    if msg == e.to_string() && !msg.starts_with("unknown field") {
        if let Some((k, why)) = bad_field::<Brief>(v) {
            msg = format!("{k}: {why}");
        }
    }
    if let Some(s) = did_you_mean(&msg) {
        msg.push_str(&format!(" (did you mean `{s}`?)"));
    }
    msg
}

/// For "unknown field `x`, expected one of `a`, `b`" (or variant): the allowed name closest to `x`.
fn did_you_mean(msg: &str) -> Option<String> {
    let rest = msg.split_once("unknown ")?.1;
    let mut ticks = rest.split('`').skip(1).step_by(2);
    let bad = ticks.next()?;
    let names: Vec<&str> = ticks.collect();
    // Letters do not say that a pie is a donut ("pie" gave `line`).
    let same = [
        ("pie", "donut"),
        ("doughnut", "donut"),
        ("bar", "bars"),
        ("chart", "bars"),
        ("graph", "line"),
    ];
    if let Some((_, c)) = same.iter().find(|(b, c)| *b == bad && names.contains(c)) {
        return Some((*c).to_string());
    }
    names
        .into_iter()
        .map(|c| (strsim::jaro_winkler(bad, c), c))
        .filter(|(s, _)| *s > 0.7)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c.to_string())
}

/// The fix line for a brief that did not load.
pub fn load_fix(err: &str, path: &Path) -> String {
    if err.starts_with("cannot read") {
        format!(
            "Check the path: {} does not exist or cannot be read.",
            std::path::absolute(path)
                .unwrap_or(path.to_path_buf())
                .display()
        )
    } else {
        "Run `scriptvox schema brief` to see the format.".into()
    }
}

/// The narration text: the script file (`--script` or `script_file`), else `script`, `lines` or `scenes`. None: there is none.
pub fn script_text(file: Option<&Path>, b: &Brief) -> Result<Option<String>, String> {
    let join = |parts: Vec<&str>| Some(parts.join(" "));
    Ok(match (file, &b.script) {
        (Some(p), _) => {
            let s = std::fs::read_to_string(p)
                .map_err(|e| format!("cannot read {}: {e}", p.display()))?;
            // A voice would read the marks of a Markdown file aloud, and captions would show them.
            Some(if p.extension().is_some_and(|e| e == "md") {
                plain_markdown(&s)
            } else {
                s
            })
        }
        (None, Some(s)) => Some(s.clone()),
        (None, None) if b.lines.is_some() => {
            join(b.lines.iter().flatten().map(|l| l.text.trim()).collect())
        }
        (None, None) if b.scenes.is_some() => {
            join(b.scenes.iter().flatten().map(|s| s.script.trim()).collect())
        }
        (None, None) => None,
    })
}

impl Brief {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        let mut v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a valid brief: {e}", path.display()))?;
        if let Some(bp) = v
            .get("brand")
            .and_then(|x| x.as_str())
            .map(|x| base.join(x))
        {
            merge_brand(&mut v, &bp)?;
        }
        let mut b: Brief = serde_json::from_value(v.clone()).map_err(|e| {
            format!(
                "{} is not a valid brief: {}",
                path.display(),
                explain(&v, e)
            )
        })?;
        // Before the checks of the cards: the Sources card is a card as the others.
        crate::sources_card::apply(&mut b)?;
        // Paths in a brief are relative to the brief.
        b.script_file = b.script_file.map(|p| base.join(p));
        b.media = b.media.map(|p| base.join(p));
        b.base_video = b.base_video.map(|p| base.join(p));
        for c in &mut b.cutaways {
            c.file = c.file.take().map(|p| base.join(p));
        }
        b.narration = b.narration.map(|p| base.join(p));
        if let Some(o) = &b.outro {
            if !(1.0..=6.0).contains(&o.seconds.unwrap_or(2.5)) {
                return Err("outro seconds is 1 to 6".into());
            }
            if o.text.as_ref().is_some_and(|t| t.chars().count() > 60) {
                return Err("outro text is at most 60 characters".into());
            }
            if o.text.is_none() && b.logo.is_none() {
                return Err("an outro needs text, a logo, or both".into());
            }
        }
        if let Some(c) = &b.accent {
            if crate::captions::ass_colour(c).is_none() {
                return Err(format!("accent {c} is not a colour like #1F6FEB"));
            }
            let k = crate::captions::contrast_on_black(c).unwrap_or(0.0);
            if k < 3.0 {
                return Err(format!("accent {c} is too dark on the black caption box (contrast {k:.1}, needs 3); pick a lighter shade"));
            }
        }
        crate::audio::music_gap_db(b.music_level.as_deref())?;
        if let Some(bg) = &b.background {
            crate::template::background_colours(bg)?;
        }
        b.font = b.font.map(|p| base.join(p));
        b.look = b.look.map(|l| {
            if l.to_ascii_lowercase().ends_with(".cube") {
                base.join(l).to_string_lossy().into_owned()
            } else {
                l
            }
        });
        if let Some(f) = &b.font {
            crate::template::read_font(f)?;
        }
        if !matches!(
            b.caption_at.as_deref(),
            None | Some("bottom" | "middle" | "top")
        ) {
            return Err("caption_at is bottom, middle or top".into());
        }
        crate::captions::size_scale(b.caption_size.as_deref())?;
        if let Some(e) = crate::card_style::problem(
            b.card_style.as_deref(),
            b.background.as_deref(),
            b.accent.as_deref(),
        ) {
            return Err(e);
        }
        crate::size::check(b.size)?;
        if let Some(e) =
            crate::caption_style::problem(b.caption_outline, b.caption_box, b.captions.as_deref())
        {
            return Err(e);
        }
        // The range only: `make` checks it against the engine it picks.
        crate::fit::voice_rate_pct(b.voice_rate, "edge")?;
        if !b.captions_second.is_empty() {
            if b.caption_at.as_deref() == Some("top") {
                return Err(
                    "captions_second sits under the main captions: not with caption_at top".into(),
                );
            }
            if b.captions_second.iter().any(|t| t.trim().is_empty()) {
                return Err(
                    "captions_second has an empty entry: give one line per sentence".into(),
                );
            }
        }
        let placed = matches!(b.caption_at.as_deref(), Some("middle" | "top"));
        let uses_space = b.hook.is_some()
            || b.scenes
                .iter()
                .flatten()
                .any(|s| crate::scene_cards::of(s).next().is_some());
        if placed && uses_space {
            return Err("caption_at middle or top needs a video with no hook and no scene card: they use that space".into());
        }
        for c in b.character.iter_mut().chain(b.characters.iter_mut()) {
            c.file = c.file.take().map(|f| base.join(f));
            let prompt = c.prompt.as_deref().is_some_and(|t| !t.trim().is_empty());
            if !prompt && c.file.is_none() {
                return Err("a character needs a prompt, a file, or both".into());
            }
            if let Some(f) = c.file.as_ref().filter(|f| !f.is_file()) {
                return Err(format!("character {} does not exist", f.display()));
            }
        }
        let names: Vec<&str> = b
            .characters
            .iter()
            .filter_map(|c| c.name.as_deref())
            .collect();
        if names.len() != b.characters.len()
            || (1..names.len()).any(|i| names[..i].contains(&names[i]))
        {
            return Err("each of `characters` needs its own `name`".into());
        }
        if b.character.is_some() && !b.characters.is_empty() {
            return Err("give `character` or `characters`, not both".into());
        }
        if let Some(l) = b.logo.as_mut() {
            l.file = base.join(&l.file);
            if !l.file.is_file() {
                return Err(format!("logo {} does not exist", l.file.display()));
            }
            if !matches!(l.corner.as_deref(), None | Some("right") | Some("left")) {
                return Err("logo corner is right or left".into());
            }
            if !(0.05..=0.4).contains(&l.width.unwrap_or(0.16)) {
                return Err("logo width is a share of the frame width, 0.05 to 0.4".into());
            }
        }
        if let Some(sc) = b.scenes.as_mut() {
            for s in sc.iter_mut() {
                s.media = s.media.take().map(|p| base.join(p));
                if let Some(o) = s.overlay.take() {
                    let o = base.join(o);
                    if !o.is_file() || crate::media::kind_of(&o).is_none() {
                        return Err(format!(
                            "overlay {} is not a clip or picture in the project",
                            o.display()
                        ));
                    }
                    s.overlay = Some(o);
                }
                if let Some(i) = s.inset.as_mut() {
                    i.file = base.join(&i.file);
                    if !i.file.is_file() || crate::media::kind_of(&i.file).is_none() {
                        return Err(format!(
                            "inset {} is not a clip or photo in the project",
                            i.file.display()
                        ));
                    }
                    if !matches!(
                        i.corner.as_deref(),
                        None | Some("top_right" | "top_left" | "bottom_right" | "bottom_left")
                    ) {
                        return Err(
                            "inset corner is top_right, top_left, bottom_right or bottom_left"
                                .into(),
                        );
                    }
                    if !(0.2..=0.6).contains(&i.width.unwrap_or(0.4)) {
                        return Err("inset width is a share of the frame width, 0.2 to 0.6".into());
                    }
                }
                if let Some(g) = s.generate.as_mut() {
                    g.image = g.image.take().map(|p| base.join(p));
                    g.last_frame = g.last_frame.take().map(|p| base.join(p));
                }
            }
        }
        let given = [
            b.script.is_some(),
            b.script_file.is_some(),
            b.scenes.as_ref().is_some_and(|s| !s.is_empty()),
            b.lines.is_some(),
        ]
        .iter()
        .filter(|x| **x)
        .count();
        for (i, sc) in b.scenes.iter().flatten().enumerate() {
            if let Some(p) = crate::scene_cards::problem(sc) {
                return Err(format!("{}: scene {}: {p}", path.display(), i + 1));
            }
        }
        if !b.cutaways.is_empty() {
            if given != 0 || b.base_video.is_none() {
                return Err(format!(
                    "{}: cutaways go over a base_video with its own sound, and no script, script_file, scenes or lines",
                    path.display()
                ));
            }
        } else if given != 1 && !(given == 0 && b.duration.is_some()) {
            return Err(format!(
                "{}: give exactly one of `script`, `script_file`, `scenes` and `lines` (or, for a video with no words, `duration` alone)",
                path.display()
            ));
        }
        Ok(b)
    }
}

/// JSON Schema documents, by name.
pub fn schema(name: &str) -> Option<serde_json::Value> {
    Some(match name {
        "brief" => serde_json::to_value(schemars::schema_for!(Brief)).ok()?,
        "result" => serde_json::to_value(schemars::schema_for!(crate::envelope::Envelope)).ok()?,
        "event" => serde_json::from_str(include_str!("../../../schemas/event.json")).ok()?,
        "openapi" => crate::openapi::spec(),
        _ => return None,
    })
}

pub const SCHEMAS: [&str; 4] = ["brief", "result", "event", "openapi"];

/// The brief fields the main skill lists (`skill.rs`). Every other field is listed in the second skill,
/// `scriptvox-reference`, so a new field goes there until its name is added here. Keep this list short: the main
/// skill has a length test.
pub const COMMON_FIELDS: [&str; 25] = [
    "accent",
    "aspect",
    "background",
    "caption_at",
    "caption_size",
    "captions",
    "duration",
    "hook",
    "lang",
    "logo",
    "media",
    "music",
    "music_level",
    "narration",
    "outro",
    "say",
    "scenes",
    "script",
    "script_file",
    "sfx",
    "sources",
    "stock",
    "transition",
    "tts",
    "voice",
];

/// The brief fields a brand file may hold: the look and sound of a series, nothing about one video's content.
pub const BRAND_KEYS: [&str; 25] = [
    "lang",
    "tts",
    "voice",
    "aspect",
    "size",
    "fps",
    "captions",
    "transition",
    "look",
    "sfx",
    "music",
    "music_prompt",
    "logo",
    "accent",
    "outro",
    "font",
    "caption_at",
    "caption_size",
    "caption_outline",
    "caption_box",
    "style",
    "background",
    "music_level",
    "say",
    "card_style",
];

/// Fill the brief's missing fields from the brand file; a field the brief sets wins. A logo path in the brand file is
/// relative to the brand file.
fn merge_brand(v: &mut serde_json::Value, path: &Path) -> Result<(), String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("brand {}: {e}", path.display()))?;
    let mut brand: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)
        .map_err(|e| format!("brand {} is not a JSON object: {e}", path.display()))?;
    if let Some(k) = brand.keys().find(|k| !BRAND_KEYS.contains(&k.as_str())) {
        return Err(format!(
            "brand {}: {k} is not a brand field (use {})",
            path.display(),
            BRAND_KEYS.join(", ")
        ));
    }
    for key in ["font", "look"] {
        if let Some(f) = brand.get(key).and_then(|f| f.as_str()).map(String::from) {
            if key == "look" && !f.to_ascii_lowercase().ends_with(".cube") {
                continue;
            }
            let dir = path.parent().unwrap_or(Path::new("."));
            let full = std::path::absolute(dir.join(&f)).unwrap_or(dir.join(&f));
            brand.insert(key.into(), full.display().to_string().into());
        }
    }
    if let Some(f) = brand.get_mut("logo").and_then(|l| l.get_mut("file")) {
        if let Some(rel) = f.as_str() {
            let dir = path.parent().unwrap_or(Path::new("."));
            *f = serde_json::Value::String(
                std::path::absolute(dir.join(rel))
                    .unwrap_or(dir.join(rel))
                    .display()
                    .to_string(),
            );
        }
    }
    let obj = v.as_object_mut().ok_or("a brief is a JSON object")?;
    for (k, x) in brand {
        if obj.get(&k).is_none_or(|y| y.is_null()) {
            obj.insert(k, x);
        }
    }
    Ok(())
}

/// The words of a Markdown script: no heading, list or quote marks, no emphasis marks, no code fence lines.
pub fn plain_markdown(md: &str) -> String {
    md.lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .map(|l| {
            let l = l.trim_start();
            let l = l
                .trim_start_matches('#')
                .trim_start_matches('>')
                .trim_start();
            let l = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("+ "))
                .unwrap_or(l);
            let digits = l.chars().take_while(char::is_ascii_digit).count();
            let l = if digits > 0 {
                l[digits..].strip_prefix(". ").unwrap_or(l)
            } else {
                l
            };
            l.replace("**", "").replace('`', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {

    #[test]
    fn markdown_marks_are_not_read_aloud() {
        assert_eq!(
            super::plain_markdown(
                "# Title here\n- A first **point**.\n2. Next `one`.\n> Said\n2024 was good."
            ),
            "Title here\nA first point.\nNext one.\nSaid\n2024 was good."
        );
    }

    #[test]
    fn a_pie_offers_the_donut() {
        let m = "unknown variant `pie`, expected one of `line`, `donut`";
        assert_eq!(did_you_mean(m).as_deref(), Some("donut"));
        let m = "unknown variant `pie`, expected one of `line`";
        assert_eq!(did_you_mean(m).as_deref(), Some("line"));
    }

    #[test]
    fn a_bad_brief_says_where_and_what_was_meant() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.json");
        std::fs::write(&p, r#"{"captoins":"pop","scenes":[{"script":"a"}]}"#).unwrap();
        let e = super::Brief::load(&p).unwrap_err();
        assert!(e.contains("did you mean `captions`"), "{e}");
        std::fs::write(
            &p,
            r#"{"scenes":[{"script":"a"},{"script":"b","card":{"kind":"lsit","items":["x"]}}]}"#,
        )
        .unwrap();
        let e = super::Brief::load(&p).unwrap_err();
        assert!(
            e.contains("scenes[1].card:") && e.contains("did you mean `list`"),
            "{e}"
        );
        let e = super::Brief::load(&d.path().join("none.json")).unwrap_err();
        assert!(super::load_fix(&e, &d.path().join("none.json")).starts_with("Check the path"));
    }

    use super::*;

    #[test]
    fn a_brand_file_fills_missing_fields_and_the_brief_wins() {
        let d = std::env::temp_dir().join(format!("rvx-brand-{}", std::process::id()));
        std::fs::create_dir_all(d.join("kit")).unwrap();
        std::fs::write(d.join("kit/logo.png"), b"png").unwrap();
        std::fs::write(
            d.join("kit/brand.json"),
            r##"{"accent": "#FFD040", "look": "warm", "aspect": "4:5", "logo": {"file": "logo.png"}}"##,
        )
        .unwrap();
        let p = d.join("b.json");
        std::fs::write(
            &p,
            r#"{"script": "Hi.", "brand": "kit/brand.json", "look": "cool"}"#,
        )
        .unwrap();
        let b = Brief::load(&p).unwrap();
        assert_eq!(b.accent.as_deref(), Some("#FFD040"));
        assert_eq!(b.aspect.as_deref(), Some("4:5"));
        assert_eq!(b.look.as_deref(), Some("cool"), "the brief wins");
        assert!(
            b.logo.unwrap().file.ends_with("kit/logo.png"),
            "logo path is relative to the brand file"
        );
        std::fs::write(d.join("kit/brand.json"), r#"{"script": "no"}"#).unwrap();
        assert!(Brief::load(&p).unwrap_err().contains("not a brand field"));
    }

    #[test]
    fn a_scene_card_is_checked_when_the_brief_loads() {
        let d = std::env::temp_dir().join(format!("rvx-brief-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("b.json");
        let write = |card: &str| {
            std::fs::write(
                &p,
                format!(r#"{{"scenes": [{{"script": "Hi.", "card": {card}}}]}}"#),
            )
            .unwrap()
        };
        write(r#"{"kind": "stat", "text": "90%", "label": "of water"}"#);
        assert_eq!(
            Brief::load(&p).unwrap().scenes.unwrap()[0]
                .card
                .as_ref()
                .unwrap()
                .kind,
            crate::cards::Kind::Stat
        );
        write(r#"{"kind": "list", "items": []}"#);
        assert!(Brief::load(&p)
            .unwrap_err()
            .contains("scene 1: a list card needs 1 to 5 items"));
        write(r#"{"kind": "wipe"}"#);
        assert!(Brief::load(&p).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn caption_place_is_refused_where_a_hook_or_card_uses_the_space() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.json");
        let load = |body: &str| {
            std::fs::write(&p, body).unwrap();
            Brief::load(&p)
        };
        assert!(load(r#"{"script": "Hi there.", "caption_at": "top"}"#).is_ok());
        assert!(load(r#"{"script": "Hi there.", "caption_at": "side"}"#).is_err());
        assert!(load(r#"{"script": "Hi there.", "captions_second": ["Salut."]}"#).is_ok());
        assert!(load(
            r#"{"script": "Hi there.", "captions_second": ["Salut."], "caption_at": "top"}"#
        )
        .is_err());
        assert!(load(r#"{"script": "Hi there.", "captions_second": [" "]}"#).is_err());
        assert!(load(r#"{"script": "Hi there.", "caption_at": "middle", "hook": "X"}"#).is_err());
        assert!(load(r#"{"script": "Hi there.", "caption_at": "bottom", "hook": "X"}"#).is_ok());
    }

    #[test]
    fn a_logo_is_checked_when_the_brief_loads() {
        let d = std::env::temp_dir().join(format!("rvx-logo-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("l.png"), b"png").unwrap();
        let p = d.join("b.json");
        let load = |logo: &str| {
            std::fs::write(&p, format!(r#"{{"script": "Hi.", "logo": {logo}}}"#)).unwrap();
            Brief::load(&p)
        };
        assert_eq!(
            load(r#"{"file": "l.png"}"#).unwrap().logo.unwrap().file,
            d.join("l.png")
        );
        assert!(load(r#"{"file": "none.png"}"#)
            .unwrap_err()
            .contains("does not exist"));
        assert!(load(r#"{"file": "l.png", "corner": "bottom"}"#)
            .unwrap_err()
            .contains("right or left"));
        assert!(load(r#"{"file": "l.png", "width": 0.9}"#)
            .unwrap_err()
            .contains("0.05 to 0.4"));
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod outside_tests {
    #[test]
    fn a_missing_path_is_judged_by_its_words() {
        let root = std::path::Path::new("/p/q");
        assert_eq!(
            super::lexical(&root.join("../../etc/x")),
            std::path::Path::new("/etc/x")
        );
        assert!(super::lexical(&root.join("a/../b")).starts_with(root));
    }
}
