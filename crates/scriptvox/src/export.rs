//! `scriptvox export <run_id>`: one folder to publish from. The video, its captions as SRT, a cover image from the hook
//! frame, the credits, and `metadata.json` with a title, a description and the terms of every part used (voice,
//! generator rows, stock), so an agent or a person can see at once whether the video may go to a client.
//! `--cover-prompt` makes the cover with an image row instead (docs/specs/export-cover.md): the only part of `export`
//! that can reach a network or cost money, and it does so through `generate`.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Args {
    pub run: String,
    pub dir: Option<PathBuf>,
    pub gif: bool,
    pub title: Option<String>,
    /// Hashtags for the platform texts, without '#'.
    pub tags: Vec<String>,
    /// More copies of the video: webm, av1, hevc, prores, hls.
    pub formats: Vec<String>,
    /// Also write the captions and cards alone on a transparent frame.
    pub overlay: bool,
    /// Also write the video as an editor's timeline: fcpxml, otio, or both.
    pub timeline: Vec<String>,
    /// Also write the sound parts as WAVs: narration.wav and music.wav (a run only).
    pub stems: bool,
    /// Also write a copy of at most this many MB (1 MB = 1,000,000 bytes), by two-pass bitrate (G071).
    pub max_mb: Option<f64>,
    /// Soft caption tracks in a copy with no re-encode: `en` (the run's own captions) or `ar=ar.srt` (G072).
    pub soft_subs: Vec<String>,
    pub use_system: bool,
    /// `--cover-prompt`: a cover made by an image row (G116). None: the export is what it was.
    pub cover: Option<Cover>,
}

/// A cover picture to make, with the money flags of the command line: the same ones `generate` takes.
pub struct Cover {
    pub prompt: String,
    /// A row name, or `free`, `cheapest` or `best`. Default `cheapest`, as `generate --model`.
    pub model: Option<String>,
    pub approve: Option<String>,
    pub max_spend: Option<f64>,
    pub plan_only: bool,
    pub idempotency_key: Option<String>,
}

/// What `made_cover` gives: the picture as the row made it, and its `covers` entry.
struct MadeCover {
    src: PathBuf,
    row: String,
    cost: f64,
    entry: Value,
}

/// The cover picture (G116): one image from an image row, through `generate` itself, so the row choice, the plan, the
/// approval, the cap and the ledger are the ones of `scriptvox generate`. The store keeps the picture by row, text and
/// shape: the same cover again makes no call. Runs before the export folder is written, so a stop writes nothing.
/// The prompt goes only into the row's JSON request.
fn made_cover(c: &Cover, aspect: &str, out: &Path) -> Result<MadeCover, Box<Envelope>> {
    let stop = |exit, msg: String, fix: &str| Box::new(fail(exit, msg, fix));
    if c.prompt.trim().is_empty() {
        return Err(stop(
            Exit::Usage,
            "the cover prompt is empty".into(),
            "Give --cover-prompt: what the cover shows.",
        ));
    }
    let word = c.model.as_deref().unwrap_or("cheapest");
    let (row, why) = match crate::generators::choose_by_word(word, "image") {
        Some(Ok((g, why))) => (g.name, Some(why)),
        Some(Err(why)) => {
            return Err(stop(
                Exit::NeedsInput,
                why,
                "Name a row with `--cover-model`, or run `scriptvox providers --kind image`.",
            ))
        }
        None => (word.to_string(), None),
    };
    let rows = crate::generators::all();
    let g = rows.iter().find(|g| g.name == row);
    if let Some(g) = g.filter(|g| g.kind != "image.gen") {
        return Err(stop(
            Exit::Usage,
            format!("{} makes no picture (its kind is {})", g.name, g.kind),
            "Give --cover-model an image row: `scriptvox providers --kind image`.",
        ));
    }
    let name = format!("cover-src.{}", g.map_or("png", |g| g.ext()));
    let key = crate::store::key(
        "gen",
        "1",
        &json!({"model": row, "prompt": c.prompt, "aspect": aspect, "cover": true}),
    );
    let cas = crate::store::Cas::open();
    let mut entry = json!({"file": "cover.jpg", "kind": "generated", "row": row, "prompt": c.prompt,
        "aspect": aspect, "cost_usd": 0.0, "reused": true});
    if let Some(why) = why {
        entry["row_choice"] = json!({"word": word, "why": why});
    }
    if c.plan_only && cas.has(&key) {
        let mut env = Envelope::ok("plan");
        env.metrics.insert("cost_usd".into(), json!(0.0));
        env.next = vec!["Run the same command without --plan.".into()];
        return Err(Box::new(env.with(
            "stages",
            json!([{"stage": "cover", "tool": format!("image.gen.{row}"), "cost_usd": 0.0, "network": false,
                "explain": "already made: no call and no charge"}]),
        )));
    }
    let stored = (!c.plan_only)
        .then(|| cas.get(&key, out))
        .flatten()
        .and_then(|f| f.into_iter().next());
    if let Some((_, src)) = stored {
        return Ok(MadeCover {
            src,
            row,
            cost: 0.0,
            entry,
        });
    }
    let src = out.join(&name);
    let mut env = crate::generate::run(crate::generate::Args {
        model: row.clone(),
        kind: Some("image".into()),
        prompt: c.prompt.clone(),
        audio: None,
        video: None,
        voice: None,
        lang: None,
        image: None,
        seconds: None,
        aspect: aspect.to_string(),
        out: Some(src.clone()),
        wait_s: 900,
        approve: c.approve.clone(),
        max_spend: c.max_spend,
        plan_only: c.plan_only,
        idempotency_key: c.idempotency_key.clone(),
        seed: None,
        last_image: None,
    });
    if c.plan_only || !env.ok {
        // The envelope is `generate`'s: its words name this command, and no line repeats the prompt.
        if env.command == "generate" {
            env.command = "export".into();
        }
        let id = env.plan_id.clone().unwrap_or_default();
        let had = env.next.len();
        env.next.retain(|n| !n.starts_with("scriptvox generate"));
        if env.next.len() < had && env.status == "approval_needed" {
            env.next.insert(
                0,
                if env.error.as_ref().is_some_and(|e| e.code == "SVX_SPEND_CAP") {
                    "Another row: --cover-model <row> from `scriptvox providers --kind image --ready`.".into()
                } else {
                    format!("Run the same export command with --approve {id}")
                },
            );
        } else if env.next.len() < had {
            env.next.insert(
                0,
                format!("Run the same export command without --plan, with --approve {id}"),
            );
        }
        if let Some(e) = &mut env.error {
            e.fix = e.fix.take().map(|f| f.replace("--model", "--cover-model"));
        }
        return Err(Box::new(env));
    }
    let cost = env
        .metrics
        .get("cost_usd")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let terms = g.map_or(Value::Null, |g| json!(g.terms));
    let _ = cas.put(&key, &row, "1", &[(name.as_str(), &src)], terms);
    entry["cost_usd"] = json!(cost);
    entry["reused"] = json!(false);
    entry["plan_id"] = json!(env.plan_id);
    Ok(MadeCover {
        src,
        row,
        cost,
        entry,
    })
}

pub const FORMATS: [&str; 5] = ["webm", "av1", "hevc", "prores", "hls"];

/// One more copy of the video: the file (from the export folder) and the FFmpeg arguments after the input. The video
/// keeps its size, rate and sound level.
fn format_args(name: &str) -> Option<(&'static str, Vec<&'static str>)> {
    Some(match name {
        "webm" => (
            "video.webm",
            vec![
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "32",
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-cpu-used",
                "4",
                "-c:a",
                "libopus",
                "-b:a",
                "128k",
            ],
        ),
        "av1" => (
            "video.av1.mp4",
            vec![
                "-c:v",
                "libsvtav1",
                "-crf",
                "34",
                "-preset",
                "8",
                "-c:a",
                "copy",
                "-movflags",
                "+faststart",
            ],
        ),
        "hevc" => (
            "video.hevc.mp4",
            vec![
                "-c:v",
                "libx265",
                "-crf",
                "24",
                "-preset",
                "medium",
                "-x265-params",
                "log-level=error",
                "-tag:v",
                "hvc1",
                "-c:a",
                "copy",
                "-movflags",
                "+faststart",
            ],
        ),
        "prores" => (
            "video.mov",
            vec![
                "-c:v",
                "prores_ks",
                "-profile:v",
                "3",
                "-pix_fmt",
                "yuv422p10le",
                "-c:a",
                "pcm_s16le",
            ],
        ),
        "hls" => (
            "hls/index.m3u8",
            // A second encode: stream copy cuts only on keyframes, about 10 s apart in our MP4, and 6 s parts need
            // a keyframe every 6 s.
            vec![
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
                "-force_key_frames",
                "expr:gte(t,n_forced*6)",
                "-c:a",
                "copy",
                "-f",
                "hls",
                "-hls_time",
                "6",
                "-hls_playlist_type",
                "vod",
                "-hls_segment_filename",
            ],
        ),
        _ => return None,
    })
}

/// The expected codec of a copy, read back from the file. HLS holds the source's own codecs.
fn format_codec(name: &str) -> Option<&'static str> {
    match name {
        "webm" => Some("vp9"),
        "av1" => Some("av1"),
        "hevc" => Some("hevc"),
        "prores" => Some("prores"),
        _ => None,
    }
}

/// FFmpeg arguments for the captions and cards alone on a transparent frame. Run from the run's folder. ProRes 4444
/// for an editor, VP9 with alpha for the web.
fn overlay_args(w: u32, h: u32, fps: f64, secs: f64, out: &Path, webm: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!("color=c=black@0:s={w}x{h}:r={fps}:d={secs:.3},format=rgba"),
        "-vf".into(),
        format!(
            "{}:alpha=1",
            crate::filterpath::ass_filter("captions.ass", "fonts")
        ),
    ];
    let codec: &[&str] = if webm {
        &[
            "-c:v",
            "libvpx-vp9",
            "-pix_fmt",
            "yuva420p",
            "-crf",
            "30",
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-cpu-used",
            "4",
        ]
    } else {
        // Quality, not a bit rate: the fixed rate spent 14 MB a second on a mostly empty frame (2 MB at this setting).
        &[
            "-c:v",
            "prores_ks",
            "-profile:v",
            "4",
            "-pix_fmt",
            "yuva444p10le",
            "-qscale:v",
            "9",
        ]
    };
    a.extend(codec.iter().map(|s| s.to_string()));
    a.push(out.to_string_lossy().into_owned());
    a
}

/// Chapters from the scenes: each scene's first word time and a short name (its card, else its first words). The
/// first chapter starts at 0. None for a one-scene video.
pub fn chapters(req: &Value, words: &[crate::captions::Word]) -> Vec<(f64, String)> {
    let Some(scenes) = req["scenes"].as_array().filter(|s| s.len() > 1) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut idx = 0usize;
    for (i, sc) in scenes.iter().enumerate() {
        let script = crate::pauses::strip(sc["script"].as_str().unwrap_or(""));
        let t = if i == 0 {
            0.0
        } else {
            words.get(idx).map_or(0.0, |w| w.start)
        };
        idx += script.split_whitespace().count();
        // The first card of a `cards` list names the chapter, as a single `card` does.
        let card = match &sc["card"] {
            serde_json::Value::Null => &sc["cards"][0],
            c => c,
        };
        let short = |x: &&str| !x.trim().is_empty() && x.chars().count() <= 40;
        let (text, label) = (
            card["text"].as_str().filter(short),
            card["label"].as_str().filter(short),
        );
        // A card names the chapter ("400+ km of coastline"); else the scene's first clause, cut at a comma or stop.
        let items = card["items"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|x| short(&x.as_str()));
        let name = match (text, label) {
            (Some(t), Some(l)) if t.chars().count() + l.chars().count() < 40 => format!("{t} {l}"),
            (_, Some(l)) => l.to_string(),
            (Some(t), None) => t.to_string(),
            (None, None) if items.is_some() => items.unwrap_or_default(),
            (None, None) => {
                let clause = script
                    .split_inclusive([',', '.', '!', '?', '\u{060C}', '\u{061F}', '\u{061B}'])
                    .next()
                    .unwrap_or(&script)
                    .trim_end_matches([',', '\u{060C}', '\u{061B}'])
                    .trim();
                // A cut clause must not end on a small word: "So cities plant trees along their" (4 Nov 2026 eval).
                let mut name = title(clause, 40);
                if name != clause {
                    const SMALL: [&str; 16] = [
                        "a", "an", "the", "of", "to", "and", "or", "with", "along", "in", "on",
                        "for", "their", "our", "your", "its",
                    ];
                    while let Some((head, last)) = name.rsplit_once(' ') {
                        if !SMALL.contains(&last.to_lowercase().as_str()) {
                            break;
                        }
                        name = head.to_string();
                    }
                }
                name
            }
        };
        out.push(((t * 10.0).round() / 10.0, name));
    }
    out
}

/// YouTube makes chapters from a description only when the first is at 0:00, there are at least 3 and each lasts at
/// least 10 s.
pub fn youtube_chapters_ok(ch: &[(f64, String)], duration: f64) -> bool {
    ch.len() >= 3
        && ch[0].0 == 0.0
        && ch.windows(2).all(|w| w[1].0 - w[0].0 >= 10.0)
        && duration - ch.last().map_or(0.0, |c| c.0) >= 10.0
}

fn clock(t: f64) -> String {
    let s = t.floor() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Text for each platform, inside its limits (as of late 2025): YouTube title 100, description 5000, at most 15
/// hashtags (more and YouTube ignores them all); TikTok caption 2200; Instagram caption 2200 and 5 hashtags; X 280.
/// Nothing is uploaded.
pub fn platforms(
    title_text: &str,
    desc: &str,
    tags: &[String],
    ch: &[(f64, String)],
    duration: f64,
) -> Value {
    let hash = |n: usize| {
        tags.iter()
            .take(n)
            .map(|t| {
                format!(
                    "#{}",
                    t.trim_start_matches('#')
                        .split_whitespace()
                        .collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let join = |a: &str, b: &str| {
        if b.is_empty() {
            a.to_string()
        } else {
            format!("{a}\n\n{b}")
        }
    };
    let chapter_text = if youtube_chapters_ok(ch, duration) {
        ch.iter()
            .map(|(t, n)| format!("{} {n}", clock(*t)))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    };
    let yt_desc = join(&join(&title(desc, 4000), &chapter_text), &hash(15));
    let tik = join(&title(desc, 2000), &hash(20));
    let ig = join(&title(desc, 2000), &hash(5));
    let x_tags = hash(3);
    let x = join(
        &title(title_text, 280 - x_tags.chars().count() - 2),
        &x_tags,
    );
    json!({
        "youtube": {"title": title(title_text, 100), "description": yt_desc, "chapters_in_description": !chapter_text.is_empty()},
        "tiktok": {"caption": tik},
        "instagram": {"caption": ig},
        "x": {"text": title(&x, 280)},
    })
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("export", exit, msg.into(), Some(fix.into()))
}

fn read(p: &Path) -> Value {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The text if it fits, else its first clause that fits, else a cut on a word.
pub fn first_clause(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut end = 0;
    for (n, (i, c)) in text.char_indices().enumerate() {
        if n >= max {
            break;
        }
        // A mark inside a number is not a clause end: "1,000", "2.5".
        let in_number = i > 0
            && text[..i].ends_with(|p: char| p.is_ascii_digit())
            && text[i + c.len_utf8()..].starts_with(|n: char| n.is_ascii_digit());
        if !in_number && matches!(c, ',' | ';' | ':' | '.' | '\u{060C}' | '\u{061B}') {
            end = i;
        }
    }
    // Too short to stand as a title ("However"): cut on a word instead.
    if text[..end].chars().count() >= 20 {
        text[..end].to_string()
    } else {
        title(text, max)
    }
}

/// A title of at most `max` characters, cut on a word.
pub fn title(text: &str, max: usize) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        return t;
    }
    let mut out = String::new();
    for w in t.split(' ') {
        if out.chars().count() + w.chars().count() + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

/// FFmpeg arguments for a looping preview GIF: the first 6 s at 12 fps, 360 px wide, with one palette made from the
/// clip itself (changed pixels weigh most) and ordered dithering, so gradients do not band or crawl.
pub fn gif_args(video: &Path, out: &Path) -> Vec<String> {
    [
        "-t",
        "6",
        "-i",
        &video.to_string_lossy(),
        "-filter_complex",
        "fps=12,scale=360:-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=192:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle",
        "-loop",
        "0",
        &out.to_string_lossy(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn run(a: Args) -> Envelope {
    if let Some(bad) = a.formats.iter().find(|f| !FORMATS.contains(&f.as_str())) {
        return fail(
            Exit::Usage,
            format!("unknown format \"{bad}\""),
            &format!("Use one or more of: {}.", FORMATS.join(", ")),
        );
    }
    // Only a run id as ScriptVox writes one becomes a path under `.scriptvox/runs`.
    let dir = Path::new(".scriptvox/runs").join(if crate::run::safe_id(&a.run) {
        a.run.as_str()
    } else {
        "-"
    });
    let result = read(&dir.join("result.json"));
    // A video file works too (from `edit` or `join`): no run, so no script, chapters or known terms.
    let file = Path::new(&a.run).is_file().then(|| PathBuf::from(&a.run));
    if let Some(env) = crate::media_gate::refuse("export", file.iter()) {
        return env;
    }
    let stem = file
        .as_ref()
        .and_then(|f| f.file_stem())
        .map_or(a.run.clone(), |s| s.to_string_lossy().into_owned());
    let found = file
        .clone()
        .or_else(|| result["artifacts"]["video"].as_str().map(PathBuf::from));
    let Some(video) = found else {
        return fail(
            Exit::Usage,
            format!("{} is not a finished run or a video file", a.run),
            "Run `scriptvox status` to see the runs.",
        );
    };
    let req = read(&dir.join("request.json"));
    // A draft by its run, or by its file (the recipe in the MP4, which an `edit` of it keeps).
    let by_file = Ffmpeg::locate(a.use_system, false)
        .ok()
        .and_then(|ff| ff.recipe(&video).ok())
        .is_some_and(|r| r["request"]["draft"] == true);
    if req["draft"] == true || by_file {
        let fix = if req["draft"] == true {
            format!(
                "Make the full video first: scriptvox resume {} --final --json",
                a.run
            )
        } else {
            "Make the full video first: scriptvox resume <run_id> --final --json".to_string()
        };
        return fail(
            Exit::Usage,
            format!("{} is a draft (half size, 15 fps)", a.run),
            &fix,
        );
    }
    if let Some(bad) = a
        .timeline
        .iter()
        .find(|t| !matches!(t.as_str(), "fcpxml" | "otio"))
    {
        return fail(
            Exit::Usage,
            format!("unknown timeline \"{bad}\""),
            "Use fcpxml (Final Cut Pro, DaVinci Resolve), otio (OpenTimelineIO: Kdenlive, Resolve), or both.",
        );
    }
    if a.stems && !dir.join("voice.wav").is_file() {
        return fail(
            Exit::Usage,
            format!("{} has no narration file to give as a stem", a.run),
            "Give a run id of `make`: a video file holds its sound only mixed.",
        );
    }
    if a.overlay && !dir.join("captions.ass").is_file() {
        return fail(
            Exit::Usage,
            format!("{} has no captions or cards to put on a transparent frame", a.run),
            "Give a run id: --overlay needs the run's captions, a video file holds them only as pixels.",
        );
    }
    let receipt = read(&dir.join("receipt.json"));
    let out = a
        .dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("scriptvox-out").join(format!("{stem}-export")));
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("export", e),
    };
    if video.is_file() && !ff.has_stream(&video, "video") {
        return fail(
            Exit::Usage,
            format!("{} has no video stream FFmpeg can read", video.display()),
            "Give a video file that plays.",
        );
    }
    // A made cover comes first: a paid row that is not approved stops here, before the folder exists.
    let made = match &a.cover {
        Some(c) => {
            // The video's shape, as a row offers it: the run's own aspect, else the shape of the picture.
            let aspect = match (req["aspect"].as_str(), ff.shown_size(&video)) {
                (Some(asp), _) => crate::make::gen_aspect(asp),
                (None, Some((w, h))) if w > h => "16:9",
                (None, Some((w, h))) if w == h => "1:1",
                _ => "9:16",
            };
            match made_cover(c, aspect, &out) {
                Ok(m) => Some(m),
                Err(env) => return *env,
            }
        }
        None => None,
    };
    if let Err(e) = std::fs::create_dir_all(&out) {
        return fail(
            Exit::Error,
            format!("{}: {e}", out.display()),
            "Check the folder.",
        );
    }
    let copy = |from: &Path, name: &str| -> Option<PathBuf> {
        let to = out.join(name);
        // A copy onto itself empties the file.
        if crate::engine_ffmpeg::same_file(&to, from) {
            return Some(to);
        }
        std::fs::copy(from, &to).ok().map(|_| to)
    };
    let Some(mp4) = copy(&video, "video.mp4") else {
        return fail(
            Exit::Error,
            format!("cannot copy {}", video.display()),
            "Make the video again.",
        );
    };
    let srt = copy(&dir.join("captions.srt"), "captions.srt");
    let vtt = copy(&dir.join("captions.vtt"), "captions.vtt");
    let mut credits = copy(&video.with_extension("credits.txt"), "credits.txt");
    // The cover: the hook frame (0.6 s) when there is a hook, else the middle of the first second.
    let secs = ff.duration(&mp4).unwrap_or(1.0);
    let at: f64 = if req["hook"].as_str().is_some_and(|h| !h.trim().is_empty()) {
        0.6
    } else {
        0.5
    };
    let at = at.min(secs / 2.0);
    // A card on the first scene: the cover waits until the whole card is on screen (test 3: a quote with no speaker).
    let at = read(&dir.join("cards.json"))
        .as_array()
        .and_then(|c| c.first().cloned())
        .filter(|c| c["scene"] == 1 && req["hook"].as_str().is_none_or(|h| h.trim().is_empty()))
        .and_then(|c| c["landed"].as_f64())
        .map_or(at, |t| t.min(secs - 0.05));
    let grab = |at: f64, name: &str| -> Option<PathBuf> {
        let p = out.join(name);
        let args: Vec<String> = [
            "-ss".to_string(),
            format!("{at:.2}"),
            "-i".into(),
            mp4.to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            // Without it the image muxer reads a % in the folder as a frame number.
            "-update".into(),
            "1".into(),
            "-q:v".into(),
            "3".into(),
            p.to_string_lossy().into_owned(),
        ]
        .to_vec();
        ff.run(Path::new("."), &args, None).ok().map(|_| p)
    };
    let mut cover = grab(
        at,
        if made.is_some() {
            "cover-frame.jpg"
        } else {
            "cover.jpg"
        },
    );
    let cover_frame = cover.clone().filter(|_| made.is_some());
    if let Some(m) = &made {
        // The row's picture at the video's size (the middle of it, when the shapes differ). Numbers only in the filter.
        let p = out.join("cover.jpg");
        let mut args = vec!["-i".to_string(), m.src.to_string_lossy().into_owned()];
        if let Some((w, h)) = ff.shown_size(&mp4) {
            args.extend([
                "-vf".to_string(),
                format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"),
            ]);
        }
        args.extend(["-frames:v", "1", "-update", "1", "-q:v", "2"].map(String::from));
        args.push(p.to_string_lossy().into_owned());
        let done = ff.run(Path::new("."), &args, None);
        let _ = std::fs::remove_file(&m.src);
        if let Err(e) = done {
            return fail(
                Exit::Error,
                format!("the cover picture could not be written as cover.jpg: {e}"),
                "Run export again; the picture is in the store, so it is not made again.",
            );
        }
        cover = Some(p);
    }
    // More covers to choose from: each scene card once it has landed (a title, a number, a graph often makes a
    // better thumbnail than the first second). At most 3.
    let cards: Vec<Value> = read(&dir.join("cards.json"))
        .as_array()
        .cloned()
        .unwrap_or_default();
    let covers: Vec<Value> = cards
        .iter()
        .filter_map(|c| Some((c["landed"].as_f64()?, c)))
        .take(3)
        .enumerate()
        .filter_map(|(i, (t, c))| {
            let p = grab(t.min(secs - 0.05), &format!("cover-{}.jpg", i + 1))?;
            Some(json!({"file": p.file_name()?.to_string_lossy(), "at": t, "kind": c["kind"], "scene": c["scene"]}))
        })
        .collect();
    // With a made cover the list starts with it and the frame cover, so a reader knows which is which.
    let covers: Vec<Value> = match &made {
        Some(m) => [
            m.entry.clone(),
            json!({"file": "cover-frame.jpg", "kind": "frame", "at": at}),
        ]
        .into_iter()
        .chain(covers)
        .collect(),
        None => covers,
    };
    let gif = if a.gif {
        let p = out.join("preview.gif");
        match ff.run(Path::new("."), &gif_args(&mp4, &p), None) {
            Ok(_) => Some(p),
            Err(e) => {
                return fail(
                    Exit::Error,
                    format!("preview.gif: {e}"),
                    "Run export again without --gif.",
                )
            }
        }
    } else {
        None
    };
    let abs = |p: &Path| std::path::absolute(p).unwrap_or(p.to_path_buf());
    let size_of = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
    let mut copies: Vec<Value> = Vec::new();
    for name in &a.formats {
        let Some((file, args)) = format_args(name) else {
            continue;
        };
        let p = out.join(file);
        if crate::engine_ffmpeg::same_file(&p, &video) {
            return fail(
                Exit::Usage,
                format!(
                    "format {name} would write over the input {}",
                    video.display()
                ),
                "Give another --dir.",
            );
        }
        // HLS names its parts by a pattern, which would read a % in the folder: FFmpeg runs inside the folder.
        let hls = name == "hls";
        let cwd = if hls {
            out.join("hls")
        } else {
            PathBuf::from(".")
        };
        if let Err(e) = std::fs::create_dir_all(&cwd) {
            return fail(Exit::Error, format!("{name}: {e}"), "Check the folder.");
        }
        let mut full = vec!["-i".to_string(), abs(&mp4).to_string_lossy().into_owned()];
        full.extend(args.iter().map(|s| s.to_string()));
        if hls {
            full.extend(["%03d.ts".to_string(), "index.m3u8".to_string()]);
        } else {
            full.push(p.to_string_lossy().into_owned());
        }
        if let Err(e) = ff.run(&cwd, &full, None) {
            return fail(
                Exit::Error,
                format!("format {name}: {e}"),
                "Run export again without this format.",
            );
        }
        // A part longer than promised is a fault too.
        if name == "hls" {
            let list = std::fs::read_to_string(&p).unwrap_or_default();
            let longest = list
                .lines()
                .filter_map(|l| l.strip_prefix("#EXTINF:"))
                .filter_map(|l| l.trim_end_matches(',').parse::<f64>().ok())
                .fold(0.0, f64::max);
            if longest > 6.5 {
                return fail(
                    Exit::Error,
                    format!("format hls: a part runs {longest:.1} s, over the 6 s promised"),
                    "Run export again without this format.",
                );
            }
        }
        // Read back: the file holds the codec it should and the whole video.
        // A playlist lists its stream once per part.
        let (got, length) = if name == "hls" {
            own_hls(&ff, &p)
        } else {
            (
                ff.video_codec(&p)
                    .and_then(|c| c.lines().next().map(str::to_string)),
                ff.duration(&p).unwrap_or(0.0),
            )
        };
        if format_codec(name).is_some_and(|c| got.as_deref() != Some(c))
            || (length - secs).abs() > 0.5
        {
            return fail(
                Exit::Error,
                format!("format {name}: the file holds {got:?}, {length:.2} s of {secs:.2} s"),
                "Run export again without this format.",
            );
        }
        let bytes = if name == "hls" {
            std::fs::read_dir(out.join("hls"))
                .map(|d| d.flatten().map(|e| size_of(&e.path())).sum())
                .unwrap_or(0)
        } else {
            size_of(&p)
        };
        copies.push(json!({"format": name, "file": file, "codec": got, "bytes": bytes}));
    }
    if let Some(mb) = a.max_mb {
        match max_mb_copy(&ff, &mp4, &out, mb, secs) {
            Ok((p, bytes, kbps)) => copies.push(json!({"format": format!("max_{mb}mb"), "file": p.file_name().map(|f| f.to_string_lossy().into_owned()), "bytes": bytes, "video_kbps": kbps})),
            Err(e) => return fail(Exit::Usage, e, "Give a bigger --max-mb, or make a shorter video."),
        }
    }
    if !a.soft_subs.is_empty() {
        let own = dir.join("captions.srt");
        match soft_subs_copy(&ff, &mp4, own.is_file().then_some(own.as_path()), &a.soft_subs, &out) {
            Ok(p) => copies.push(json!({"format": "subs", "file": p.file_name().map(|f| f.to_string_lossy().into_owned()), "bytes": size_of(&p), "tracks": a.soft_subs})),
            Err(e) => return fail(Exit::Usage, e, "Give --soft-subs like en,ar=ar.srt: a bare code is this run's own captions."),
        }
    }
    let mut overlays: Vec<Value> = Vec::new();
    if a.overlay {
        let (w, h) = ff.shown_size(&mp4).unwrap_or((1080, 1920));
        let fps = ff.frame_rate(&mp4).unwrap_or(30.0);
        let fontconfig = abs(&dir.join("fonts.conf"));
        for (file, webm) in [("overlay.mov", false), ("overlay.webm", true)] {
            let p = abs(&out.join(file));
            let args = overlay_args(w, h, fps, secs, &p, webm);
            if let Err(e) = ff.run(&dir, &args, Some(&fontconfig)) {
                return fail(
                    Exit::Error,
                    format!("{file}: {e}"),
                    "Run export again without --overlay.",
                );
            }
            overlays.push(json!({"file": file, "bytes": size_of(&p), "alpha": true}));
        }
    }

    let (stems, timelines) = match timeline_files(&ff, &a, &dir, &req, &out, &mp4, secs) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                Exit::Error,
                e,
                "Run export again without --timeline and --stems.",
            )
        }
    };

    // Every part with its terms: the voice, each generator row the run used, and the stock clips.
    let mut parts = Vec::new();
    if let Some(id) = receipt["voice"]["id"].as_str() {
        let v = crate::voices::all().into_iter().find(|v| v.id == id);
        parts.push(
            json!({"kind": "voice", "name": id, "commercial": v.as_ref().map(|v| v.commercial),
                          "terms": receipt["voice"]["terms"]}),
        );
    }
    let rows = crate::generators::all();
    let mut used: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".job.json"))
        .filter_map(|e| read(&e.path())["generator"].as_str().map(String::from))
        .collect();
    // A clip that came from the cache has no job file here, but its terms still count.
    used.extend(
        receipt["generators"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| g.as_str().map(String::from)),
    );
    // The cover's row is a part like the row of a generated scene: its terms count in the same way.
    used.extend(made.iter().map(|m| m.row.clone()));
    used.sort();
    used.dedup();
    for name in used {
        let g = rows.iter().find(|g| g.name == name);
        parts.push(
            json!({"kind": "generator", "name": name, "commercial": g.map(|g| g.terms.commercial),
                          "attribution_required": g.map(|g| g.terms.attribution_required),
                          "terms": g.map(|g| g.terms.note.clone())}),
        );
    }
    // Terms per file (Commons, Internet Archive) count; a row's general stock terms do not, as before.
    for s in receipt["stock"].as_array().into_iter().flatten() {
        let mut part = json!({"kind": "stock", "item": s});
        if s["terms"]["commercial"].is_boolean() {
            part["commercial"] = s["terms"]["commercial"].clone();
        }
        parts.push(part);
    }
    // A built-in bed is ours. A music file is the user's own choice: the entry says only that the user supplied it.
    match req["music"].as_str() {
        Some("calm" | "pulse") => parts.push(
            json!({"kind": "music", "name": req["music"], "commercial": true, "terms": "built-in bed, made by scriptvox"}),
        ),
        Some("none") | None => {}
        // Music from an address is a stock item of the receipt, with its own terms.
        Some(f) if crate::fetch::is_address(f) => {}
        Some(f) => parts.push(
            json!({"kind": "music", "name": f, "commercial": null, "terms": "supplied by the user"}),
        ),
    }
    // A generator row that asks for attribution reaches the credits file too (L6).
    let mut due = attribution_lines(&parts);
    // The brief's facts and their sources follow the video: one line each in the credits, the list in metadata.json.
    let sources: Vec<crate::brief::Source> =
        serde_json::from_value(req["sources"].clone()).unwrap_or_default();
    due.extend(sources.iter().map(crate::brief::Source::credit));
    if !due.is_empty() {
        let file = out.join("credits.txt");
        let mut body = std::fs::read_to_string(&file).unwrap_or_default();
        body.push_str(&format!("{}\n", due.join("\n")));
        if std::fs::write(&file, body).is_ok() {
            credits = Some(file);
        }
    }
    let commercial_ok = commercial_ok(&parts, receipt["voice"]["id"].is_string());
    let ai_parts = ai_parts(&parts);
    let script = req["script"].as_str().map(String::from).unwrap_or_else(|| {
        req["scenes"].as_array().map_or(String::new(), |sc| {
            sc.iter()
                .filter_map(|s| s["script"].as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
    });
    let script = crate::pauses::strip(&script);
    let first = script
        .split_inclusive(['.', '!', '?', '\u{061F}'])
        .next()
        .unwrap_or(&script);
    let words: Vec<crate::captions::Word> = std::fs::read_to_string(dir.join("words.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let ch = chapters(&req, &words);
    // The first clause when the sentence is long: a title never stops in mid sentence.
    let title_text = match a.title.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(t) => title(t, 100),
        None => match req["hook"].as_str().filter(|h| !h.trim().is_empty()) {
            Some(h) => title(h, 70),
            None if file.is_some() && first.trim().is_empty() => title(&stem, 70),
            None => first_clause(first, 70),
        },
    };
    let description = script.split_whitespace().collect::<Vec<_>>().join(" ");
    // Chapter marks inside the MP4 too (players and editors show them); the video stream is copied, not re-encoded.
    let mut chapters_in_mp4 = false;
    if ch.len() >= 2 {
        let fm = out.join("chapters.ffmeta");
        let mut t = String::from(";FFMETADATA1\n");
        for (i, (s, n)) in ch.iter().enumerate() {
            let e = ch.get(i + 1).map_or(secs, |c| c.0);
            let clean = n.replace(['=', ';', '#', '\\', '\n'], " ");
            t += &format!(
                "[CHAPTER]\nTIMEBASE=1/1000\nSTART={}\nEND={}\ntitle={clean}\n",
                (s * 1000.0) as i64,
                (e * 1000.0) as i64
            );
        }
        let tmp = out.join("video.chapters.mp4");
        let args: Vec<String> = [
            "-i",
            &mp4.to_string_lossy(),
            "-f",
            "ffmetadata",
            "-i",
            &fm.to_string_lossy(),
            "-map",
            "0",
            "-map_metadata",
            "1",
            "-map_chapters",
            "1",
            "-c",
            "copy",
            &tmp.to_string_lossy(),
        ]
        .iter()
        .map(|x| x.to_string())
        .collect();
        if std::fs::write(&fm, t).is_ok()
            && ff.run(Path::new("."), &args, None).is_ok()
            && std::fs::rename(&tmp, &mp4).is_ok()
        {
            chapters_in_mp4 = true;
        }
        let _ = std::fs::remove_file(&fm);
        let _ = std::fs::remove_file(&tmp);
    }
    let meta = json!({
        "title": title_text,
        "description": description,
        "chapters": ch.iter().map(|(t, n)| json!({"t": t, "title": n})).collect::<Vec<_>>(),
        "chapters_in_mp4": chapters_in_mp4,
        "platforms": platforms(&title_text, &description, &a.tags, &ch, secs),
        "tags": a.tags,
        "lang": result["lang"],
        "aspect": req["aspect"],
        "duration_s": if file.is_some() { json!((secs * 100.0).round() / 100.0) } else { result["metrics"]["duration_s"].clone() },
        "cost_usd": result["metrics"]["cost_usd"],
        "commercial_ok": commercial_ok,
        "ai_parts": ai_parts,
        "parts": parts,
        "covers": covers,
        "formats": copies,
        "overlays": overlays,
        "run_id": if file.is_some() { Value::Null } else { json!(a.run) },
        "made_with": format!("scriptvox {}", env!("CARGO_PKG_VERSION")),
        "sources": sources,
    });
    let meta_path = out.join("metadata.json");
    if let Err(e) = std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    ) {
        return fail(Exit::Error, e.to_string(), "Check the folder.");
    }
    let mut env = Envelope::ok("export");
    for (k, p) in [
        ("video", Some(mp4)),
        ("captions_srt", srt),
        ("captions_vtt", vtt),
        ("cover", cover),
        ("cover_frame", cover_frame),
        ("gif", gif),
        ("credits", credits),
        ("metadata", Some(meta_path)),
    ] {
        if let Some(p) = p {
            env.artifacts
                .insert(k.into(), json!(std::path::absolute(&p).unwrap_or(p)));
        }
    }
    for (k, p) in stems.iter().chain(&timelines) {
        env.artifacts.insert(k.clone(), json!(abs(p)));
    }
    for c in copies.iter().chain(&overlays) {
        if let Some(f) = c["file"].as_str() {
            let key = c["format"]
                .as_str()
                .map_or_else(|| f.replace('.', "_"), |n| format!("video_{n}"));
            env.artifacts.insert(key, json!(abs(&out.join(f))));
        }
    }
    if !covers.is_empty() {
        let paths: Vec<Value> = covers
            .iter()
            .filter_map(|c| c["file"].as_str())
            .map(|f| json!(std::path::absolute(out.join(f)).unwrap_or(out.join(f))))
            .collect();
        env.artifacts.insert("covers".into(), json!(paths));
    }
    env.metrics
        .insert("commercial_ok".into(), json!(commercial_ok));
    if let Some(m) = &made {
        env.metrics.insert("cover_cost_usd".into(), json!(m.cost));
    }
    // No note about the terms here (owner, 13 Nov 2026): `commercial_ok` and the parts in metadata.json are the data.
    env
}

/// false: a part has a known limit. true: every part is cleared. null: not checked (a part with unknown terms, or a run
/// with no receipt, so no voice part). Only false gives the note (audit M1). A stock file counts when it has terms of its
/// own, so a licence that could not be read gives null; an old receipt's stock item has no terms and does not count.
fn commercial_ok(parts: &[Value], has_voice: bool) -> Value {
    let counted: Vec<&Value> = parts
        .iter()
        .filter(|p| p["kind"] != "stock" || p["item"]["terms"].is_object())
        .collect();
    if counted.iter().any(|p| p["commercial"] == false) {
        json!(false)
    } else if has_voice && counted.iter().all(|p| p["commercial"] == true) {
        json!(true)
    } else {
        Value::Null
    }
}

/// One credit line per generator part whose terms ask for attribution.
fn attribution_lines(parts: &[Value]) -> Vec<String> {
    parts
        .iter()
        .filter(|p| p["kind"] == "generator" && p["attribution_required"] == true)
        .map(|p| {
            format!(
                "Made with {}: {}",
                p["name"].as_str().unwrap_or("?"),
                p["terms"].as_str().unwrap_or("see its licence")
            )
        })
        .collect()
}

/// Which parts a model made: data for the user's own disclosure. No claim that any rule is met.
/// A row that only times words or removes a background from what exists still touched the file, so it is listed with its kind.
fn ai_parts(parts: &[Value]) -> Vec<Value> {
    let rows = crate::generators::all();
    parts
        .iter()
        .filter_map(|p| match p["kind"].as_str()? {
            "voice" if p["name"] != "none" => Some(json!({"kind": "voice", "name": p["name"]})),
            "generator" => {
                let name = p["name"].as_str()?;
                let kind = rows.iter().find(|g| g.name == name).map(|g| g.kind.clone());
                Some(json!({"kind": kind, "name": name}))
            }
            _ => None,
        })
        .collect()
}

/// `--stems` and `--timeline` (G053): the narration and music as 48 kHz WAVs of the video's length, and the timelines
/// that point at them, the picture cut at its shot changes.
#[allow(clippy::type_complexity)]
fn timeline_files(
    ff: &Ffmpeg,
    a: &Args,
    dir: &Path,
    req: &Value,
    out: &Path,
    mp4: &Path,
    secs: f64,
) -> Result<(Vec<(String, PathBuf)>, Vec<(String, PathBuf)>), String> {
    let mut stems: Vec<(String, PathBuf)> = Vec::new();
    if a.stems {
        // The bed as it went into the mix, before its level and ducking: the editor sets those.
        let bed = Some(dir.join("bed.wav"))
            .filter(|p| p.is_file())
            .or_else(|| {
                req["music"]
                    .as_str()
                    .filter(|m| !matches!(*m, "none" | "calm" | "pulse"))
                    .map(PathBuf::from)
                    .filter(|p| p.is_file())
            });
        for (name, src, looped) in [
            ("narration", Some(dir.join("voice.wav")), false),
            ("music", bed, true),
        ] {
            let Some(src) = src else { continue };
            let to = out.join(format!("{name}.wav"));
            let mut args: Vec<String> = vec![];
            if looped {
                args.extend(["-stream_loop".into(), "-1".into()]);
            }
            args.extend([
                "-i".into(),
                std::path::absolute(&src)
                    .unwrap_or(src.clone())
                    .display()
                    .to_string(),
                "-af".into(),
                "aresample=48000,apad".into(),
                "-t".into(),
                format!("{secs:.3}"),
                "-ac".into(),
                "2".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                std::path::absolute(&to)
                    .unwrap_or(to.clone())
                    .display()
                    .to_string(),
            ]);
            ff.run(Path::new("."), &args, None)
                .map_err(|e| format!("{name}.wav: {e}"))?;
            stems.push((format!("stem_{name}"), to));
        }
    }
    let mut files = Vec::new();
    if a.timeline.is_empty() {
        return Ok((stems, files));
    }
    let fps = ff.frame_rate(mp4).unwrap_or(30.0).round() as u32;
    let total = (secs * f64::from(fps)).round() as u64;
    // Found at 4 fps, then placed on the exact frame: a quarter second off would cut into the next shot.
    let cuts: Vec<f64> = crate::analyze::picture_cuts(ff, mp4, 0.25)?
        .into_iter()
        .map(|c| crate::analyze::exact_cut(ff, mp4, c, 0.25).unwrap_or(c))
        .collect();
    let shots = crate::timeline::shots(&cuts, fps, total);
    let sounds: Vec<crate::timeline::Sound> = stems
        .iter()
        .map(|(k, p)| crate::timeline::Sound {
            name: k.trim_start_matches("stem_"),
            file: p,
            frames: total,
        })
        .collect();
    let name = &a.run;
    for t in &a.timeline {
        let (file, text) = if t == "fcpxml" {
            let size = ff.shown_size(mp4).unwrap_or((1080, 1920));
            (
                "timeline.fcpxml",
                crate::timeline::fcpxml(name, mp4, size, fps, &shots, &sounds),
            )
        } else {
            let v = crate::timeline::otio(name, mp4, fps, &shots, &sounds);
            (
                "timeline.otio",
                serde_json::to_string_pretty(&v).unwrap_or_default(),
            )
        };
        let p = out.join(file);
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        files.push((format!("timeline_{t}"), p));
    }
    Ok((stems, files))
}

/// Video bitrate in kbit/s for a file of at most `mb` MB over `secs` seconds, with 128 kbit/s of sound and 3 percent
/// left for the container. None when the picture would get under 150 kbit/s.
pub fn max_mb_kbps(mb: f64, secs: f64) -> Option<u32> {
    let total = mb * 1_000_000.0 * 8.0 / 1000.0 / secs.max(0.1) * 0.97;
    let video = total - 128.0;
    (video >= 150.0).then_some(video as u32)
}

/// `--max-mb` (G071): two passes of libx264 at the bitrate that fits, then the size is measured; one more try at 90
/// percent if it is still over.
fn max_mb_copy(
    ff: &Ffmpeg,
    mp4: &Path,
    out: &Path,
    mb: f64,
    secs: f64,
) -> Result<(PathBuf, u64, u32), String> {
    let Some(mut kbps) = max_mb_kbps(mb, secs) else {
        return Err(format!(
            "{secs:.1} s does not fit in {mb} MB: the picture would get under 150 kbit/s"
        ));
    };
    let name = format!("video-{}mb.mp4", mb);
    let to = out.join(&name);
    let log = out.join("max-mb-pass");
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let limit = (mb * 1_000_000.0) as u64;
    let clean = || {
        for f in std::fs::read_dir(out).into_iter().flatten().flatten() {
            if f.file_name().to_string_lossy().starts_with("max-mb-pass") {
                let _ = std::fs::remove_file(f.path());
            }
        }
    };
    for _ in 0..2 {
        for pass in ["1", "2"] {
            let mut a: Vec<String> = vec!["-i".into(), abs(mp4)];
            // A short clip overshoots the average; the cap holds it near the target.
            a.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "medium".into(),
                "-b:v".into(),
                format!("{kbps}k"),
                "-maxrate".into(),
                format!("{kbps}k"),
                "-bufsize".into(),
                format!("{}k", kbps * 2),
                "-pass".into(),
                pass.into(),
                "-passlogfile".into(),
                abs(&log),
            ]);
            if pass == "1" {
                a.extend(["-an", "-f", "null", "-"].map(String::from));
            } else {
                a.extend(
                    ["-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart"].map(String::from),
                );
                a.push(abs(&to));
            }
            if let Err(e) = ff.run(Path::new("."), &a, None) {
                clean();
                return Err(e);
            }
        }
        let got = std::fs::metadata(&to).map_or(u64::MAX, |m| m.len());
        if got <= limit {
            clean();
            return Ok((to, got, kbps));
        }
        kbps = kbps * 85 / 100;
    }
    clean();
    let _ = std::fs::remove_file(&to);
    Err(format!("{name} is still over {mb} MB after two tries"))
}

/// ISO 639-2 for an MP4 language tag: two-letter codes scriptvox speaks, else a three-letter code as given.
fn lang3(l: &str) -> Option<&str> {
    match l {
        "ar" => Some("ara"),
        "en" => Some("eng"),
        "fr" => Some("fra"),
        "es" => Some("spa"),
        "de" => Some("deu"),
        "ur" => Some("urd"),
        "hi" => Some("hin"),
        l if l.len() == 3 && l.chars().all(|c| c.is_ascii_lowercase()) => Some(l),
        _ => None,
    }
}

/// The codec and the length of the playlist that this export wrote a moment ago. It is ScriptVox's own file, so it is
/// read as HLS by name (`media-inputs.md`, what is trusted); a list from anywhere else never gets here.
fn own_hls(ff: &Ffmpeg, list: &Path) -> (Option<String>, f64) {
    let o = std::process::Command::new(&ff.ffprobe)
        .args(["-v", "error", "-f", "hls", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=codec_name:format=duration"])
        .args(["-of", "json", "-i"])
        .arg(list)
        .output();
    let v: Value = o
        .ok()
        .and_then(|o| serde_json::from_slice(&o.stdout).ok())
        .unwrap_or_default();
    (
        v["streams"][0]["codec_name"].as_str().map(str::to_string),
        v["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0),
    )
}

/// `--soft-subs en,ar=ar.srt` (G072): `video-subs.mp4`, the picture and sound copied as they are, one `mov_text` track
/// per entry with its language. A bare code is the run's own SRT.
fn soft_subs_copy(
    ff: &Ffmpeg,
    mp4: &Path,
    own: Option<&Path>,
    specs: &[String],
    out: &Path,
) -> Result<PathBuf, String> {
    let mut files = vec![];
    for s in specs {
        let (lang, file) = match s.split_once('=') {
            Some((l, f)) => (l.trim(), PathBuf::from(f.trim())),
            None => (
                s.trim(),
                own.map(Path::to_path_buf).ok_or_else(|| {
                    format!("{s}: this video has no captions of its own; give {s}=file.srt")
                })?,
            ),
        };
        let code = lang3(lang)
            .ok_or_else(|| format!("{lang} is not a language code (ar, en, or three letters)"))?;
        if !file.is_file() {
            return Err(format!("{} does not exist", file.display()));
        }
        // FFmpeg reads subtitles as UTF-8: hand it this file decoded, so an Arabic file in Windows-1256 stays Arabic.
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let text =
            crate::edit::decode_srt(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        let utf8 = out.join(format!("subs-{}-{}.srt", files.len(), code));
        std::fs::write(&utf8, text).map_err(|e| e.to_string())?;
        files.push((code.to_string(), utf8));
    }
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let to = out.join("video-subs.mp4");
    let mut a: Vec<String> = vec!["-i".into(), abs(mp4)];
    for (_, f) in &files {
        // The copy holds the user's text: read as SRT and nothing else, so FFmpeg guesses no format from it.
        a.extend(["-f".into(), "srt".into(), "-i".into(), abs(f)]);
    }
    a.extend(["-map".into(), "0:v".into(), "-map".into(), "0:a?".into()]);
    for i in 0..files.len() {
        a.extend(["-map".into(), format!("{}:s", i + 1)]);
    }
    a.extend(["-c:v", "copy", "-c:a", "copy", "-c:s", "mov_text"].map(String::from));
    for (i, (code, _)) in files.iter().enumerate() {
        a.extend([format!("-metadata:s:s:{i}"), format!("language={code}")]);
        a.extend([
            format!("-disposition:s:{i}"),
            if i == 0 { "default" } else { "0" }.into(),
        ]);
    }
    a.extend(["-movflags".into(), "+faststart".into(), abs(&to)]);
    let done = ff.run(Path::new("."), &a, None);
    for (_, f) in &files {
        let _ = std::fs::remove_file(f);
    }
    done?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    #[test]
    fn language_codes_for_mp4() {
        assert_eq!(super::lang3("ar"), Some("ara"));
        assert_eq!(super::lang3("en"), Some("eng"));
        assert_eq!(super::lang3("tur"), Some("tur"));
        assert_eq!(super::lang3("xx"), None);
        assert_eq!(super::lang3("ENG"), None);
    }

    #[test]
    fn a_row_asking_attribution_gets_a_credit_line() {
        let parts = [
            serde_json::json!({"kind": "generator", "name": "a", "attribution_required": true, "terms": "CC BY, by X"}),
            serde_json::json!({"kind": "generator", "name": "b", "attribution_required": false, "terms": "MIT"}),
            serde_json::json!({"kind": "voice", "name": "v", "attribution_required": true}),
        ];
        assert_eq!(
            super::attribution_lines(&parts),
            ["Made with a: CC BY, by X"]
        );
    }

    #[test]
    fn max_mb_bitrate_fits_or_refuses() {
        // 16 MB over 60 s: 2069 kbit/s in all, 128 of it sound.
        assert_eq!(super::max_mb_kbps(16.0, 60.0), Some(1941));
        assert_eq!(super::max_mb_kbps(0.1, 9.5), None);
        assert_eq!(super::max_mb_kbps(-1.0, 9.5), None);
    }

    #[test]
    fn an_unread_stock_licence_is_not_cleared() {
        let voice = serde_json::json!({"kind": "voice", "name": "v", "commercial": true});
        let unread = serde_json::json!({"kind": "stock", "item": {"terms": {"commercial": null, "note": "licence not read"}}});
        let old = serde_json::json!({"kind": "stock", "item": {"url": "x"}});
        assert_eq!(
            super::commercial_ok(&[voice.clone(), unread], true),
            serde_json::Value::Null
        );
        assert_eq!(super::commercial_ok(&[voice, old], true), true);
    }

    #[test]
    fn ai_parts_list_the_voice_and_rows_only() {
        let parts = vec![
            serde_json::json!({"kind": "voice", "name": "a"}),
            serde_json::json!({"kind": "generator", "name": "no-such-row"}),
            serde_json::json!({"kind": "stock", "item": {}}),
            serde_json::json!({"kind": "music", "name": "calm"}),
        ];
        let got = super::ai_parts(&parts);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0]["kind"], "voice");
        assert_eq!(got[1]["name"], "no-such-row");
    }

    use super::*;

    #[test]
    fn every_format_has_arguments_and_overlay_keeps_alpha() {
        for f in FORMATS {
            assert!(format_args(f).is_some(), "{f}");
        }
        assert!(format_args("mkv").is_none());
        let a = overlay_args(1080, 1920, 30.0, 4.5, Path::new("o.webm"), true).join(" ");
        assert!(
            a.contains("black@0:s=1080x1920:r=30:d=4.500")
                && a.contains("alpha=1")
                && a.contains("yuva420p"),
            "{a}"
        );
    }

    #[test]
    fn gif_args_make_one_palette() {
        let a = gif_args(Path::new("v.mp4"), Path::new("p.gif")).join(" ");
        assert!(
            a.starts_with("-t 6 -i v.mp4 -filter_complex fps=12,scale=360:-2"),
            "{a}"
        );
        assert!(
            a.contains("palettegen")
                && a.contains("paletteuse=dither=bayer")
                && a.ends_with("-loop 0 p.gif")
        );
    }

    #[test]
    fn chapters_follow_scenes_and_youtube_rules_hold() {
        let w = |t: &str, s: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: s + 0.3,
        };
        let words = vec![
            w("One", 0.2),
            w("two.", 0.6),
            w("Three", 12.0),
            w("four.", 12.4),
            w("Five", 25.0),
        ];
        let req = json!({"scenes": [
            {"script": "One two."},
            {"script": "Three four.", "card": {"kind": "stat", "text": "90%", "label": "of water"}},
            {"script": "Five [pause 1] six seven eight nine ten eleven twelve thirteen fourteen fifteen."}]});
        let ch = chapters(&req, &words);
        assert_eq!(ch[0], (0.0, "One two.".into()));
        let r2 = json!({"scenes": [{"script": "It counts over two hundred islands, from Saadiyat to Yas."}, {"script": "x"}]});
        assert_eq!(
            chapters(&r2, &words)[0].1,
            "It counts over two hundred islands"
        );
        assert_eq!(ch[1], (12.0, "90% of water".into()));
        let r3 = json!({"scenes": [
            {"script": "So cities plant trees along their streets, because trees give shade."},
            {"script": "x", "card": {"kind": "list", "items": ["Shade", "Cool air"]}}]});
        let c3 = chapters(&r3, &words);
        assert_eq!(c3[0].1, "So cities plant trees");
        assert_eq!(c3[1].1, "Shade, Cool air");
        assert_eq!(ch[2].0, 25.0);
        assert!(youtube_chapters_ok(&ch, 40.0));
        assert!(
            !youtube_chapters_ok(&ch, 30.0),
            "the last chapter is under 10 s"
        );
        assert!(chapters(&json!({"script": "x"}), &words).is_empty());
        let tags: Vec<String> = (1..=40).map(|i| format!("tag{i}")).collect();
        let p = platforms("Water", "A story about water.", &tags, &ch, 40.0);
        assert_eq!(
            p["instagram"]["caption"]
                .as_str()
                .unwrap()
                .matches('#')
                .count(),
            5
        );
        assert_eq!(
            p["youtube"]["description"]
                .as_str()
                .unwrap()
                .matches('#')
                .count(),
            15
        );
        assert!(p["youtube"]["description"]
            .as_str()
            .unwrap()
            .contains("0:12 90% of water"));
        assert!(p["x"]["text"].as_str().unwrap().chars().count() <= 280);
    }

    #[test]
    fn a_long_sentence_gives_its_first_clause() {
        assert_eq!(
            first_clause("A city is built on hard ground, so when heavy rain falls, the water stands in the low streets", 70),
            "A city is built on hard ground, so when heavy rain falls"
        );
        assert_eq!(first_clause("Short one.", 70), "Short one.");
        let t = first_clause("More than 1,000 people were moved from the low streets of the city when heavy rain fell.", 70);
        assert!(t.starts_with("More than 1,000 people"), "{t}");
        let t = first_clause("However the long sentence runs on and on with no other mark in it at all until the end", 70);
        assert!(t.chars().count() > 40, "{t}");
    }

    #[test]
    fn titles_cut_on_a_word() {
        assert_eq!(
            title("Water older than the city", 70),
            "Water older than the city"
        );
        assert_eq!(title("one two three four", 9), "one two");
    }
}
