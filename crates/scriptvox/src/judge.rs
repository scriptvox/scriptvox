//! `scriptvox judge <video>`: an open vision model scores evenly spaced frames against the brief and the caption on
//! screen, and says what to fix (P3-008). One request to the OpenAI-compatible service the user set. Advisory: `make`
//! keeps its local gates.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The one vision setting for every caller: an OpenAI-compatible base URL ending in /v1, from `SCRIPTVOX_VISION_URL` or
/// the file `vision-url` in the config folder. There is no default host: frames of a user's video go only where the
/// user said. The optional key is `scriptvox keys set vision`.
pub fn vision_url() -> Option<String> {
    std::env::var("SCRIPTVOX_VISION_URL")
        .ok()
        .or_else(|| std::fs::read_to_string(crate::keys::config_dir().join("vision-url")).ok())
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
}

/// The model name for the vision service: SCRIPTVOX_VISION_MODEL or the file vision-model in the config folder, else the
/// default. A hosted service will not know the default name.
pub fn vision_model() -> String {
    std::env::var("SCRIPTVOX_VISION_MODEL")
        .ok()
        .or_else(|| std::fs::read_to_string(crate::keys::config_dir().join("vision-model")).ok())
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// The refusal when no vision service is set: nothing has been sent.
pub const NO_VISION: &str = "this step sends frames or words of your video to a vision service, and none is set. Nothing was sent. Set SCRIPTVOX_VISION_URL (or the file vision-url in the config folder) to any OpenAI-compatible base URL ending in /v1; if it needs a key, run `scriptvox keys set vision`";

/// The vision URL, or the refusal.
pub fn need_vision() -> Result<String, (Exit, String)> {
    if crate::plan::local_only() {
        return Err((Exit::NeedsInput, crate::plan::LOCAL_ONLY_STOP.to_string()));
    }
    vision_url().ok_or_else(|| (Exit::NeedsInput, NO_VISION.to_string()))
}

pub const DEFAULT_MODEL: &str = "Qwen/Qwen3.8-27B";

pub struct Args {
    pub video: PathBuf,
    /// Each scene card's (start, landed) times, when the video is a run.
    pub cards: Vec<(f64, f64)>,
    /// What the video should show: a brief file's text or the run's request (script, hook, scenes).
    pub brief: Option<String>,
    pub frames: u32,
    /// An OpenAI-compatible base URL; default: the vision setting.
    pub endpoint: Option<String>,
    pub model: String,
    pub min_score: u8,
    pub use_system: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct FrameVerdict {
    pub score: u8,
    #[serde(default)]
    pub matches_brief: bool,
    #[serde(default)]
    pub text_ok: bool,
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub fix: String,
}

impl FrameVerdict {
    fn none() -> Self {
        FrameVerdict {
            score: 0,
            matches_brief: false,
            text_ok: false,
            issues: vec![],
            fix: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Verdict {
    pub frames: Vec<FrameVerdict>,
    pub score: u8,
    #[serde(default)]
    pub fixes: Vec<String>,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("judge", exit, msg.into(), Some(fix.into()))
}

/// The model's reply as a verdict for `n` frames. Takes bare JSON or JSON inside a code fence.
pub fn parse_reply(text: &str, n: usize) -> Result<Verdict, String> {
    let (a, b) = (text.find('{'), text.rfind('}'));
    let body = match (a, b) {
        (Some(a), Some(b)) if a < b => &text[a..=b],
        _ => return Err("the reply holds no JSON object".into()),
    };
    let v: Verdict =
        serde_json::from_str(body).map_err(|e| format!("the reply is not a verdict: {e}"))?;
    if v.frames.len() != n {
        return Err(format!(
            "the reply scores {} frames, not {n}",
            v.frames.len()
        ));
    }
    let bad = |s: u8| !(1..=5).contains(&s);
    if bad(v.score) || v.frames.iter().any(|f| bad(f.score)) {
        return Err("a score is outside 1 to 5".into());
    }
    Ok(v)
}

const RUBRIC: &str = "You check frames from a short narrated video before it is published. Each frame comes with \
its time and the caption shown on screen at that time. Score every frame from 1 to 5. 5: the picture shows what the \
brief asks for at this point, and it looks finished. 3: usable, but generic or weak. 1: wrong subject, a broken or \
smeared picture, or text that is cut off or unreadable. Set text_ok to false when text on screen is cut off, \
overlaps other text, cannot be read, or says other words than the caption. Captions are shown one page at a \
time: a page often ends mid-sentence and the next page goes on, so a sentence that stops at the page end is not \
cut off. Text is cut off only when letters run past the frame edge or a box clips them. Set matches_brief to false when the \
picture does not fit the brief or the caption. For every frame under 4, give one concrete fix in `fix` (a new \
visual for the brief, a shorter caption line, another shot). Give at most 5 fixes for the whole video, most \
important first. A title card with \"behind\": true in the brief is drawn behind the subject on purpose: the \
subject in front of that title is the design, not a fault, so do not ask to move it. For such a title judge only \
whether every word can still be read; when a word cannot be read, set text_ok to false. When the brief asks for \
no footage (no stock, media, generated video or images in it), the plain or gradient background behind the captions \
and cards is the chosen design: score those frames on whether the text and cards are clear, well placed and match \
the voice, not on the lack of footage, and do not ask for stock or new footage. A stat card counts up to its \
number in its first second: a smaller number in the same unit (\"2,783 years\" for \"3,000 years\") is the count \
in progress, not a wrong fact. A plot card marks the x and y range at its edges and the zero crossings on the axis: \
those numbers are its scale, not a fault. A plain or gradient background behind clear text is a choice, also in one \
scene of a brief that has footage elsewhere: it scores 3 at the least, and a score under 3 needs a fault in the text, \
a wrong subject or a broken picture. With `captions: reveal` in the brief a caption shows only the words said so far: \
missing later words are the design. Reply with JSON only.";

fn schema(n: usize) -> Value {
    let score = json!({"type": "integer", "minimum": 1, "maximum": 5});
    json!({
        "type": "object",
        "properties": {
            "frames": {"type": "array", "minItems": n, "maxItems": n, "items": {
                "type": "object",
                "properties": {
                    "score": score,
                    "matches_brief": {"type": "boolean"},
                    "text_ok": {"type": "boolean"},
                    "issues": {"type": "array", "items": {"type": "string"}, "maxItems": 4},
                    "fix": {"type": "string"}
                },
                "required": ["score", "matches_brief", "text_ok", "issues", "fix"]
            }},
            "score": score,
            "fixes": {"type": "array", "items": {"type": "string"}, "maxItems": 5}
        },
        "required": ["frames", "score", "fixes"]
    })
}

/// A brief with no stock, media, generated clip or base video anywhere: its plain background is the design. The
/// rubric alone left this to the model, and it still asked for stock on 3 of 6 frames (4 Nov 2026 eval).
fn no_footage(brief: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(brief) else {
        return false;
    };
    let has = |o: &Value, keys: &[&str]| keys.iter().any(|k| !o[*k].is_null());
    !has(&v, &["media", "stock", "base_video"])
        && v["scenes"]
            .as_array()
            .is_none_or(|s| s.iter().all(|sc| !has(sc, &["media", "stock", "generate"])))
}

/// A frame time moved to the time a card has landed. Before that a chart may show only its title, or a list only the
/// items said so far, and the judge then reports a missing card (5 Nov 2026: a bars card 0.05 s after it started scored
/// 1; a list card 1.4 s in scored 2 on 3 of 3 runs).
pub fn settled(t: f64, cards: &[(f64, f64)]) -> f64 {
    cards
        .iter()
        .find(|(start, landed)| t >= *start && t < *landed)
        .map_or(t, |c| c.1)
}

/// One verdict from several replies for the same frames. A service can answer the same request in different ways (two
/// backends behind one URL gave 555553 and 334452 on 6 Nov), so a frame fails only when every reply has it under the
/// bar. Each frame takes its best reply; `disputed` lists the frames where the replies fall on both sides of the bar,
/// with the lowest reply. Overall score and fixes come from the reply with the best overall score.
pub fn agree(mut replies: Vec<Verdict>, min_score: u8) -> (Verdict, Vec<(usize, FrameVerdict)>) {
    let n = replies[0].frames.len();
    let mut frames = Vec::with_capacity(n);
    let mut disputed = vec![];
    for i in 0..n {
        let mut these: Vec<FrameVerdict> = replies
            .iter_mut()
            .map(|r| std::mem::replace(&mut r.frames[i], FrameVerdict::none()))
            .collect();
        these.sort_by_key(|f| f.score);
        let best = these.pop().expect("one reply at least");
        match these.into_iter().next() {
            Some(low) if low.score < min_score && best.score >= min_score => {
                disputed.push((i, low))
            }
            _ => {}
        }
        frames.push(best);
    }
    let top = replies
        .into_iter()
        .max_by_key(|r| r.score)
        .expect("one reply at least");
    (
        Verdict {
            frames,
            score: top.score,
            fixes: top.fixes,
        },
        disputed,
    )
}

/// The chat request: the rubric and brief, then each frame as a line of text followed by its picture.
pub fn request(
    model: &str,
    brief: Option<&str>,
    frames: &[(f64, Option<String>, Vec<u8>)],
) -> Value {
    let brief = brief
        .map(|b| b.chars().take(6000).collect::<String>())
        .unwrap_or_else(|| "(no brief: judge the picture only)".into());
    let brief = if no_footage(&brief) {
        format!("{brief}\n\nThis brief has no footage on purpose: judge the text and cards on the plain background, never ask for stock, footage or a visual of the subject.")
    } else {
        brief
    };
    let mut content = vec![json!({"type": "text", "text": format!("{RUBRIC}\n\nBrief:\n{brief}")})];
    for (i, (t, cap, jpg)) in frames.iter().enumerate() {
        content.push(json!({"type": "text", "text": format!(
            "Frame {} at {t:.1} s. Caption on screen: {}",
            i + 1,
            cap.as_deref().unwrap_or("(none)")
        )}));
        content.push(json!({"type": "image_url", "image_url": {"url": format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(jpg)
        )}}));
    }
    json!({
        "model": model,
        "temperature": 0,
        "max_tokens": 400 + 200 * frames.len(),
        "messages": [{"role": "user", "content": content}],
        "response_format": {"type": "json_schema", "json_schema": {"name": "verdict", "schema": schema(frames.len())}},
        "chat_template_kwargs": {"enable_thinking": false}
    })
}

/// One frame at `t` as a JPEG `width` pixels wide, written to `out` and returned.
pub fn grab(ff: &Ffmpeg, video: &Path, t: f64, width: u32, out: &Path) -> Result<Vec<u8>, String> {
    // At 0 no seek: `-ss 0` on a JPEG gives no frame and exit 0, so `media_match` could not read any JPEG.
    let seek = format!("{t:.3}");
    let from: &[&str] = if t > 0.0 { &["-ss", &seek] } else { &[] };
    let a: Vec<String> = from
        .iter()
        .copied()
        .chain([
            "-i",
            &video.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={width}:-2"),
            "-q:v",
            "4",
            "-update",
            "1",
            &out.to_string_lossy(),
        ])
        .map(String::from)
        .collect();
    ff.run(Path::new("."), &a, None)?;
    std::fs::read(out).map_err(|e| format!("{}: {e}", out.display()))
}

/// POST a chat request to `<endpoint>/chat/completions` and return the reply text. Exit 5 when a retry is safe.
pub fn ask(endpoint: &str, body: &Value) -> Result<String, (Exit, String)> {
    if crate::plan::local_only() {
        return Err((Exit::NeedsInput, crate::plan::LOCAL_ONLY_STOP.to_string()));
    }
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let agent = crate::http_rows::agent_ca(240, None).map_err(|e| (Exit::Error, e))?;
    let mut req = agent.post(&url).header("Content-Type", "application/json");
    // The stored key belongs to the configured service, over https or on loopback. Any other endpoint gets no key.
    let configured =
        vision_url().is_some_and(|u| u.trim_end_matches('/') == endpoint.trim_end_matches('/'));
    if let Some((key, _)) = crate::keys::resolve("vision")
        .filter(|_| configured && crate::generators::speech_base_ok(endpoint))
    {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let mut resp = req
        .send(body.to_string().as_bytes())
        .map_err(|e| (Exit::ProviderRetryable, format!("{url}: {e}")))?;
    let code = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if code != 200 {
        let exit = if code >= 500 || code == 429 {
            Exit::ProviderRetryable
        } else {
            Exit::Error
        };
        let head: String = text.chars().take(300).collect();
        return Err((exit, format!("{url} answered {code}: {head}")));
    }
    let reply: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(reply["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// Text drawn over the picture of a video (subtitles, a title, a lower third), from 3 frames at 20, 50 and 80
/// percent of `secs`, asked of the free vision model. `Some(what)` when the model sees any; signs and labels that
/// are part of the scene do not count. Used by `make` to warn that a base video's own text stays under the new
/// captions.
pub fn burned_text(
    ff: &Ffmpeg,
    video: &Path,
    secs: f64,
    dir: &Path,
) -> Result<Option<String>, (Exit, String)> {
    let mut content = vec![];
    for (i, f) in [0.2, 0.5, 0.8].iter().enumerate() {
        let jpg = grab(
            ff,
            video,
            secs * f,
            640,
            &dir.join(format!("base_text{}.jpg", i + 1)),
        )
        .map_err(|e| (Exit::Error, e))?;
        content.push(json!({"type": "image_url", "image_url": {"url": format!(
            "data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpg))}}));
    }
    content.push(json!({"type": "text", "text":
        "Three frames of one video. Was text added on top of the picture in editing: subtitles, a title, a caption          bar, a lower third, a logo with words? Signs, screens and labels that are part of the filmed scene do not          count. Reply with JSON: {\"added_text\": true or false, \"what\": \"a few words on what and where, or empty\"}."}));
    let body = json!({
        "model": vision_model(),
        "temperature": 0,
        "max_tokens": 120,
        "messages": [{"role": "user", "content": content}],
        "response_format": {"type": "json_schema", "json_schema": {"name": "text", "schema": {
            "type": "object",
            "properties": {"added_text": {"type": "boolean"}, "what": {"type": "string"}},
            "required": ["added_text", "what"]
        }}},
        "chat_template_kwargs": {"enable_thinking": false}
    });
    let reply = ask(&need_vision()?, &body)?;
    let v: Value = serde_json::from_str(
        reply
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim(),
    )
    .map_err(|e| (Exit::Error, format!("vision reply is not JSON: {e}")))?;
    Ok(v["added_text"]
        .as_bool()
        .unwrap_or(false)
        .then(|| v["what"].as_str().unwrap_or("text").to_string()))
}

pub fn run(a: Args) -> Envelope {
    if crate::plan::local_only() {
        return fail(
            Exit::NeedsInput,
            crate::plan::LOCAL_ONLY_STOP,
            "Run without --local-only, or look at `scriptvox snapshot --sheet 9` yourself.",
        );
    }
    let Some(endpoint) = a.endpoint.clone().or_else(vision_url) else {
        return fail(
            Exit::NeedsInput,
            NO_VISION,
            "Set the vision service, or look at `scriptvox snapshot --sheet 9` yourself.",
        );
    };
    if !(2..=12).contains(&a.frames) {
        return fail(Exit::Usage, "--frames must be 2 to 12", "Use --frames 6.");
    }
    if let Some(env) = crate::media_gate::refuse("judge", [&a.video]) {
        return env;
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("judge", e),
    };
    let total = match ff.duration(&a.video) {
        Ok(d) => d,
        Err(e) => return fail(Exit::Usage, e, "Give a video FFmpeg can read."),
    };
    let stem = a
        .video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let dir = PathBuf::from("scriptvox-out").join(format!("{stem}.judge"));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(
            Exit::Error,
            format!("{}: {e}", dir.display()),
            "Check the folder is writable.",
        );
    }
    let srt = std::fs::read_to_string(a.video.with_extension("srt")).unwrap_or_default();
    let n = a.frames as usize;
    let mut frames = Vec::with_capacity(n);
    for i in 0..n {
        let t = settled(total * (i as f64 + 0.5) / n as f64, &a.cards);
        match grab(
            &ff,
            &a.video,
            t,
            768,
            &dir.join(format!("f{:02}.jpg", i + 1)),
        ) {
            Ok(jpg) => frames.push((t, crate::gates::caption_at(&srt, t), jpg)),
            Err(e) => return fail(Exit::Error, e, "Check the video with `scriptvox check`."),
        }
    }
    // `--model` wins; left at its default, the setting does.
    let model = if a.model == DEFAULT_MODEL {
        vision_model()
    } else {
        a.model.clone()
    };
    let body = request(&model, a.brief.as_deref(), &frames);
    let started = std::time::Instant::now();
    let content = match ask(&endpoint, &body) {
        Ok(c) => c,
        Err((exit, msg)) => return fail(exit, msg, "Retry later, or give --endpoint and --model of another OpenAI-compatible vision model."),
    };
    let v = match parse_reply(&content, n) {
        Ok(v) => v,
        Err(e) => {
            return fail(
                Exit::ProviderRetryable,
                e,
                "Retry; the model gave a reply scriptvox cannot read.",
            )
        }
    };
    // A frame under the bar gets a second and third opinion before it can fail the video. A retry that fails or cannot
    // be read gives no opinion.
    let under = |v: &Verdict| v.frames.iter().any(|f| f.score < a.min_score);
    let mut replies = vec![v];
    if under(&replies[0]) {
        for _ in 0..2 {
            if let Some(r) = ask(&endpoint, &body)
                .ok()
                .and_then(|c| parse_reply(&c, n).ok())
            {
                replies.push(r);
            }
        }
    }
    let asks = replies.len();
    let (v, disputed) = agree(replies, a.min_score);
    let r2 = |x: f64| (x * 100.0).round() / 100.0;
    let list: Vec<Value> = frames
        .iter()
        .zip(&v.frames)
        .enumerate()
        .map(|(i, ((t, cap, _), f))| {
            json!({"frame": i + 1, "t": r2(*t), "caption": cap, "score": f.score, "matches_brief": f.matches_brief,
                   "text_ok": f.text_ok, "issues": f.issues, "fix": f.fix})
        })
        .collect();
    // Findings hold only the frames under the bar; each names its place ("frame 2 of 6") so a reader does not take
    // the findings for the whole list (eval fault of 4 Nov: "listed 3 of 6 frames"). All frames are in `frames`.
    let findings: Vec<Value> = list
        .iter()
        .filter(|f| f["score"].as_u64().unwrap_or(0) < a.min_score as u64)
        .map(|f| {
            json!({"gate": "judge", "status": "fail", "t": f["t"],
                        "frame": format!("frame {} of {n}", f["frame"]), "score": f["score"],
                        "detail": f["issues"], "fix": f["fix"]})
        })
        .collect();
    let mut env = match findings.first() {
        None => Envelope::ok("judge"),
        Some(f) => Envelope::fail(
            "judge",
            Exit::GateFailed,
            format!(
                "{} of {n} frames scored under {} (all {n} are in `frames`)",
                findings.len(),
                a.min_score
            ),
            f["fix"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(String::from),
        ),
    };
    let low = v.frames.iter().map(|f| f.score).min().unwrap_or(0);
    env.findings = findings;
    // The video passed on these frames: a warning, not a failure. Exit 3 needs every reply under the bar.
    env.findings.extend(disputed.iter().map(|(i, low)| {
        json!({"gate": "judge", "status": "warn", "t": r2(frames[*i].0),
               "frame": format!("frame {} of {n}", i + 1), "score": low.score,
               "detail": low.issues, "fix": low.fix,
               "note": format!("{asks} replies disagree on this frame: look at it yourself")})
    }));
    env.metrics.insert("judge_asks".into(), json!(asks));
    env.metrics.insert("judge_score".into(), json!(v.score));
    env.metrics
        .insert("frames_scored".into(), json!(list.len()));
    env.metrics.insert("judge_min".into(), json!(low));
    env.metrics
        .insert("judge_s".into(), json!(r2(started.elapsed().as_secs_f64())));
    env.metrics.insert("model".into(), json!(model));
    env.artifacts.insert(
        "frames".into(),
        json!(std::path::absolute(&dir).unwrap_or(dir)),
    );
    env.next = v.fixes;
    env.with("frames", json!(list))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_frame_waits_until_a_card_has_landed() {
        let cards = [(10.6, 12.5), (20.0, 20.4)];
        assert_eq!(super::settled(10.7, &cards), 12.5);
        assert_eq!(super::settled(12.0, &cards), 12.5);
        assert_eq!(super::settled(12.5, &cards), 12.5);
        assert_eq!(super::settled(5.0, &cards), 5.0);
    }

    #[test]
    fn a_brief_without_footage_says_so() {
        assert!(super::no_footage(
            r#"{"scenes":[{"script":"a","card":{"kind":"title"}}]}"#
        ));
        assert!(!super::no_footage(
            r#"{"scenes":[{"script":"a","stock":"trees"}]}"#
        ));
        assert!(!super::no_footage(
            r#"{"stock":["auto"],"scenes":[{"script":"a"}]}"#
        ));
        assert!(!super::no_footage("not json"));
    }

    use super::*;

    const GOOD: &str = r#"{"frames":[{"score":5,"matches_brief":true,"text_ok":true,"issues":[],"fix":""},
        {"score":2,"matches_brief":false,"text_ok":false,"issues":["caption cut off"],"fix":"shorter line"}],
        "score":3,"fixes":["shorter line"]}"#;

    fn reply(scores: &[u8], overall: u8) -> Verdict {
        Verdict {
            frames: scores
                .iter()
                .map(|s| FrameVerdict {
                    score: *s,
                    ..FrameVerdict::none()
                })
                .collect(),
            score: overall,
            fixes: vec![format!("from {overall}")],
        }
    }

    #[test]
    fn a_frame_fails_only_when_every_reply_has_it_under_the_bar() {
        let (v, disputed) = agree(
            vec![
                reply(&[3, 2, 1], 3),
                reply(&[5, 2, 1], 5),
                reply(&[3, 2, 2], 3),
            ],
            3,
        );
        let scores: Vec<u8> = v.frames.iter().map(|f| f.score).collect();
        assert_eq!(scores, [5, 2, 2]);
        assert_eq!(v.score, 5);
        assert_eq!(v.fixes, ["from 5"]);
        assert!(disputed.is_empty());
        let (v, disputed) = agree(vec![reply(&[2], 2), reply(&[4], 4), reply(&[1], 1)], 3);
        assert_eq!(v.frames[0].score, 4);
        assert_eq!(disputed.len(), 1);
        assert_eq!((disputed[0].0, disputed[0].1.score), (0, 1));
    }

    #[test]
    fn reads_bare_and_fenced_json() {
        let v = parse_reply(GOOD, 2).unwrap();
        assert_eq!(v.frames[1].score, 2);
        assert_eq!(v.fixes, ["shorter line"]);
        assert_eq!(
            parse_reply(&format!("Sure:\n```json\n{GOOD}\n```"), 2).unwrap(),
            v
        );
    }

    #[test]
    fn refuses_wrong_count_and_range() {
        assert!(parse_reply(GOOD, 3).unwrap_err().contains("2 frames"));
        assert!(parse_reply(&GOOD.replace("\"score\":5", "\"score\":9"), 2).is_err());
        assert!(parse_reply("no json here", 2).is_err());
    }

    #[test]
    fn request_puts_each_caption_before_its_picture() {
        let r = request(
            "m",
            Some("A dhow at dawn."),
            &[(1.0, Some("Hello".into()), vec![1, 2])],
        );
        let c = r["messages"][0]["content"].as_array().unwrap();
        assert!(c[0]["text"].as_str().unwrap().contains("A dhow at dawn."));
        assert!(c[1]["text"]
            .as_str()
            .unwrap()
            .contains("Caption on screen: Hello"));
        assert!(c[2]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(
            r["response_format"]["json_schema"]["schema"]["properties"]["frames"]["maxItems"],
            1
        );
    }
}
