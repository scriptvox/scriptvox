//! `scriptvox dub`: a timed line sheet from a video, then one `make` on those lines (docs/specs/dub.md). The agent
//! writes the text: there is no text model here.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn fail(exit: Exit, msg: impl Into<String>, fix: impl Into<String>) -> Envelope {
    Envelope::fail("dub", exit, msg.into(), Some(fix.into()))
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map_or("video".into(), |s| s.to_string_lossy().into_owned())
}

/// The nearest frame for a source shape, within 5 percent: video models give shapes such as 1280x704. `make` renders that
/// frame, and `finish` brings it back to the source size, so up to 5 percent of the edges can go.
pub fn shape_of((w, h): (u32, u32)) -> Option<&'static str> {
    let ratio = f64::from(w) / f64::from(h.max(1));
    let off = |a: &str| {
        crate::make::frame(a).map_or(f64::MAX, |(fw, fh)| {
            (f64::from(fw) / f64::from(fh) / ratio - 1.0).abs()
        })
    };
    ["9:16", "16:9", "1:1", "4:5", "3:4", "4:3", "2:3", "21:9"]
        .into_iter()
        .min_by(|a, b| off(a).total_cmp(&off(b)))
        .filter(|a| off(a) < 0.05)
}

/// `dub <video> --plan`: the source's sentences on their times, in the form `--lines` reads.
pub fn sheet(
    input: &Path,
    source_lang: Option<&str>,
    terms: Option<&crate::say::Terms>,
    speakers: bool,
    use_system: bool,
) -> Envelope {
    if let Some(env) = crate::media_gate::refuse("dub", [input]) {
        return env;
    }
    let ff = match Ffmpeg::locate(use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("dub", e),
    };
    if !input.is_file() {
        return fail(
            Exit::Usage,
            format!("{} does not exist", input.display()),
            "Give a video file.",
        );
    }
    if !ff.has_stream(input, "audio") {
        return fail(
            Exit::Usage,
            "this video has no sound, so it has no lines to time",
            "Write `lines` yourself and pass --lines.",
        );
    }
    let total = ff.duration(input).unwrap_or(0.0);
    let doc = match crate::analyze::transcribe_speakers(
        &ff,
        input,
        source_lang,
        total,
        terms,
        speakers,
    ) {
        Ok((doc, _)) => doc,
        Err(e) => {
            let mut e = *e;
            e.command = "dub".into();
            return e;
        }
    };
    let said = doc["sentences"].as_array().cloned().unwrap_or_default();
    let lines: Vec<Value> = said
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (start, end) = (
                s["start"].as_f64().unwrap_or(0.0),
                s["end"].as_f64().unwrap_or(0.0).min(total),
            );
            let next = said
                .get(i + 1)
                .and_then(|n| n["start"].as_f64())
                .unwrap_or(total);
            let mut l = json!({"start": start, "end": end, "text": s["text"],
                "room_s": ((next.max(end) - start) * 100.0).round() / 100.0});
            if !s["speaker"].is_null() {
                l["speaker"] = s["speaker"].clone();
            }
            l
        })
        .collect();
    let file = Path::new(".scriptvox/dub").join(format!("{}-lines.json", stem(input)));
    let _ = std::fs::create_dir_all(".scriptvox/dub");
    if let Err(e) = std::fs::write(
        &file,
        serde_json::to_string_pretty(&json!({"lines": lines})).unwrap_or_default(),
    ) {
        return fail(Exit::Error, e.to_string(), "Check the folder is writable.");
    }
    let keep = keep_terms(terms);
    let mut env = Envelope::ok("dub")
        .with("language", doc["language"].clone())
        .with("lines", json!(lines))
        .with("term_fixes", doc["term_fixes"].clone())
        .with("keep_terms", json!(keep));
    env.artifacts
        .insert("lines".into(), json!(file.display().to_string()));
    let keep_note = if keep.is_empty() {
        String::new()
    } else {
        format!(" Do not translate: {}.", keep.join(", "))
    };
    env.next = vec![format!(
        "Copy {} and put the new language in each `text`, then: scriptvox dub {} --lines <copy> --lang <code> --plan.{keep_note}",
        file.display(),
        input.display()
    )];
    env
}

fn keep_terms(terms: Option<&crate::say::Terms>) -> Vec<String> {
    let mut keep: Vec<String> = terms.map_or(vec![], |t| {
        t.terms.iter().chain(t.fix.values()).cloned().collect()
    });
    keep.sort();
    keep.dedup();
    keep
}

/// A warning for each term not in the new lines: a term was to stay as it is.
pub fn check_terms(env: &mut Envelope, lines: &Path, terms: Option<&crate::say::Terms>) {
    let text = std::fs::read_to_string(lines).unwrap_or_default();
    let missing: Vec<String> = keep_terms(terms)
        .into_iter()
        .filter(|t| !text.contains(t.as_str()))
        .collect();
    if !missing.is_empty() {
        env.findings.push(json!({"gate": "terms", "status": "warn",
            "detail": format!("not in the new lines: {}", missing.join(", ")),
            "fix": "Keep these words as they are in the lines, unless the change is on purpose."}));
    }
}

/// The first free lip sync row with a key: `dub` has one plan, and a paid step after it would need a second.
pub fn lipsync_row() -> Option<crate::generators::Generator> {
    crate::generators::all()
        .into_iter()
        .find(|g| g.kind == "lipsync" && !g.paid() && crate::keys::resolve(&g.key).is_some())
}

/// `S1=af_heart,S2=am_michael` as a map. Empty: no map.
fn speaker_map(s: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) if !k.trim().is_empty() && !v.trim().is_empty() => {
                Ok((k.trim().to_string(), v.trim().to_string()))
            }
            _ => Err(format!("{p} is not SPEAKER=voice")),
        })
        .collect()
}

pub struct Brief<'a> {
    pub input: &'a Path,
    pub lines: &'a Path,
    pub lang: Option<&'a str>,
    pub tts: Option<&'a str>,
    pub voice: Option<&'a str>,
    /// `S1=af_heart,S2=am_michael`: the voice of each line's `speaker`.
    pub voices: Option<&'a str>,
    /// A text file, one line per sentence of the new lines: brief `captions_second`.
    pub captions_second: Option<&'a Path>,
    /// `--caption-size`, `--caption-outline`, `--caption-box`: they go into the brief as its three fields.
    pub caption_style: &'a crate::caption_style::Style,
    pub base_sound: Option<&'a str>,
    pub local_only: bool,
    pub use_system: bool,
}

/// Writes the brief for `make`: same lines and settings, same file, same plan.
pub fn brief(b: &Brief) -> Result<PathBuf, Box<Envelope>> {
    let stop = |exit, msg: String, fix: &str| Box::new(fail(exit, msg, fix));
    if let Some((e, fix)) = b.caption_style.check(None) {
        return Err(stop(Exit::Usage, e, fix));
    }
    let ff = Ffmpeg::locate(b.use_system, true)
        .map_err(|e| Box::new(crate::engine_ffmpeg::locate_failure("dub", e)))?;
    let video = std::fs::canonicalize(b.input).map_err(|e| {
        stop(
            Exit::Usage,
            format!("{}: {e}", b.input.display()),
            "Give a video file.",
        )
    })?;
    let text = std::fs::read_to_string(b.lines).map_err(|e| {
        stop(
            Exit::Usage,
            format!("{}: {e}", b.lines.display()),
            "Run `scriptvox dub <video> --plan` for a sheet to copy.",
        )
    })?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| {
        stop(
            Exit::Usage,
            format!("{}: {e}", b.lines.display()),
            "Give a JSON list of lines, or the sheet from `--plan`.",
        )
    })?;
    let list = doc.get("lines").cloned().unwrap_or(doc);
    let map = match speaker_map(b.voices.unwrap_or("")) {
        Ok(m) => m,
        Err(e) => {
            return Err(stop(
                Exit::Usage,
                e,
                "Give --voices like S1=af_heart,S2=am_michael.",
            ))
        }
    };
    // The fields a line has: `room_s` of the sheet is not part of the brief, and `speaker` becomes its voice.
    let lines: Vec<crate::brief::Line> = serde_json::from_value(json!(list
        .as_array()
        .into_iter()
        .flatten()
        .map(|l| json!({
            "start": l["start"], "end": l["end"], "text": l["text"],
            "voice": l["voice"].as_str().or_else(|| l["speaker"].as_str().and_then(|s| map.get(s)).map(String::as_str)),
        }))
        .collect::<Vec<_>>()))
    .map_err(|e| {
        stop(
            Exit::Usage,
            format!("{}: {e}", b.lines.display()),
            "Each line needs `start`, `end` and `text`.",
        )
    })?;
    if lines.is_empty() {
        return Err(stop(
            Exit::Usage,
            format!("{} has no lines", b.lines.display()),
            "Give a JSON list of lines, or the sheet from `--plan`.",
        ));
    }
    let second = match b.captions_second.map(crate::make::read_second).transpose() {
        Ok(s) => s,
        Err(e) => {
            return Err(stop(
                Exit::Usage,
                e,
                "Give a text file with one line per sentence of the new lines.",
            ))
        }
    };
    let size = ff.shown_size(&video).ok_or_else(|| {
        stop(
            Exit::Usage,
            format!("{} has no picture", b.input.display()),
            "Give a video file.",
        )
    })?;
    let aspect = shape_of(size).ok_or_else(|| {
        stop(
            Exit::Usage,
            format!(
                "the source is {}x{}, which is not a frame shape of scriptvox",
                size.0, size.1
            ),
            "Run `scriptvox edit --reframe` first, then dub the result.",
        )
    })?;
    let stems = crate::generators::all()
        .iter()
        .any(|g| g.kind == "stems" && !g.paid() && crate::keys::resolve(&g.key).is_some());
    // The old voice goes and its music stays when a free stem row can do it. Else the source's sound is dropped.
    let sound = b.base_sound.unwrap_or(if stems && !b.local_only {
        "background"
    } else {
        "none"
    });
    let mut body = json!({"base_video": video, "aspect": aspect, "base_sound": sound, "lines": lines,
        "lang": b.lang, "tts": b.tts, "voice": b.voice});
    if let Some(s) = second {
        body["captions_second"] = json!(s);
    }
    let cs = b.caption_style;
    for (k, v) in [
        ("caption_size", json!(cs.size)),
        ("caption_outline", json!(cs.outline)),
        ("caption_box", json!(cs.box_)),
    ] {
        if !v.is_null() {
            body[k] = v;
        }
    }
    let text = serde_json::to_string_pretty(&body).unwrap_or_default();
    let file = Path::new(".scriptvox/dub").join(format!("{}-brief.json", stem(b.input)));
    std::fs::create_dir_all(".scriptvox/dub")
        .and_then(|()| std::fs::write(&file, text))
        .map_err(|e| stop(Exit::Error, e.to_string(), "Check the folder is writable."))?;
    Ok(file)
}

/// After `make`: lip sync on request, then the gate that the dub keeps the source's size.
pub fn finish(mut env: Envelope, input: &Path, lipsync: bool, use_system: bool) -> Envelope {
    env.command = "dub".into();
    let video = env
        .artifacts
        .get("video")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let Some(video) = video.filter(|_| env.ok) else {
        return env;
    };
    let ff = match Ffmpeg::locate(use_system, false) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("dub", e),
    };
    // A video made by ScriptVox has its captions burned in, and the dub draws its own on top: two layers of text, one
    // over the other (seen on a real dub, 10 Nov 2026). The recipe in the file tells, with no vision row.
    if ff.recipe(input).is_ok() {
        env.findings.push(json!({"gate": "base_text", "status": "warn",
            "detail": "the source is a ScriptVox video with its own burned captions: the new captions land on them",
            "fix": "Dub the footage it was made from. Or make the video again in the new language: `scriptvox resume <run> --brief <file>` with the new script."}));
    }
    let text = |k: &str| {
        env.artifacts
            .get(k)
            .and_then(Value::as_str)
            .map(PathBuf::from)
    };
    if let (true, Some(row), Some(voice), Some(master)) =
        (lipsync, lipsync_row(), text("audio"), text("master"))
    {
        let synced = video.with_extension("lipsync.mp4");
        // The row stops at the end of its sound: without this pad a voice that ends early cuts the picture (28.76 s of 30 s).
        let padded = video.with_extension("voice.wav");
        let total = ff.duration(&video).unwrap_or(0.0);
        let pad: Vec<String> = [
            "-i".to_string(),
            voice.display().to_string(),
            "-af".into(),
            "apad".into(),
            "-t".into(),
            format!("{total:.3}"),
            padded.display().to_string(),
        ]
        .to_vec();
        if let Err(e) = ff.run(Path::new("."), &pad, None) {
            return fail(Exit::Error, e, "Check the disk and the output folder.");
        }
        let got = crate::generate::run(crate::generate::Args {
            kind: None,
            last_image: None,
            model: row.name,
            prompt: String::new(),
            audio: Some(padded.clone()),
            video: Some(video.clone()),
            voice: None,
            lang: None,
            image: None,
            seconds: None,
            aspect: "9:16".into(),
            out: Some(synced.clone()),
            wait_s: 1800,
            approve: None,
            max_spend: None,
            plan_only: false,
            idempotency_key: None,
            seed: None,
        });
        let _ = std::fs::remove_file(&padded);
        if !got.ok {
            let mut got = got;
            got.command = "dub".into();
            got.artifacts.insert(
                "video_no_lipsync".into(),
                json!(video.display().to_string()),
            );
            return got;
        }
        // The row returns the voice alone: the full mix goes back over its picture.
        let args: Vec<String> = [
            "-i".to_string(),
            synced.display().to_string(),
            "-i".into(),
            master.display().to_string(),
            "-map".into(),
            "0:v".into(),
            "-map".into(),
            "1:a".into(),
            "-c:v".into(),
            "copy".into(),
            "-c:a".into(),
            "aac".into(),
            "-shortest".into(),
            video.display().to_string(),
        ]
        .to_vec();
        if let Err(e) = ff.run(Path::new("."), &args, None) {
            return fail(Exit::Error, e, "The lip sync clip is next to the video.");
        }
        let _ = std::fs::remove_file(&synced);
        env.metrics.insert("lipsync".into(), json!(true));
    }
    let want = ff.shown_size(input);
    if let Some((w, h)) = want.filter(|s| ff.shown_size(&video) != Some(*s)) {
        let scaled = crate::engine_ffmpeg::part_path(&video);
        let mut args: Vec<String> = [
            "-i".to_string(),
            video.display().to_string(),
            "-vf".into(),
            format!(
                "scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h}"
            ),
        ]
        .to_vec();
        match ff.pick_encoder() {
            Ok((enc, _)) => args.extend(enc.args()),
            Err(e) => return fail(Exit::Error, e, "Run `scriptvox doctor`."),
        }
        args.extend([
            "-c:a".to_string(),
            "copy".into(),
            scaled.display().to_string(),
        ]);
        if let Err(e) = ff
            .run(Path::new("."), &args, None)
            .and_then(|()| std::fs::rename(&scaled, &video).map_err(|e| e.to_string()))
        {
            return fail(Exit::Error, e, "Check the disk and the output folder.");
        }
    }
    let got = ff.shown_size(&video);
    let (from, to) = (
        ff.duration(input).unwrap_or(0.0),
        ff.duration(&video).unwrap_or(0.0),
    );
    let same = got == want && (from - to).abs() < 0.3;
    env.findings.push(
        json!({"gate": "source_size", "status": if same { "pass" } else { "fail" },
        "detail": format!("source {want:?} {from:.2} s, dub {got:?} {to:.2} s")}),
    );
    if !same {
        env.ok = false;
        env.status = "error";
        env.exit = Exit::GateFailed;
    }
    env
}

#[cfg(test)]
mod tests {
    use super::{shape_of, speaker_map};

    #[test]
    fn speaker_map_reads_pairs() {
        let m = speaker_map("S1=af_heart, S2=am_michael").unwrap();
        assert_eq!(m["S2"], "am_michael");
        assert!(speaker_map("").unwrap().is_empty());
        assert!(speaker_map("S1").is_err());
        assert!(speaker_map("S1=").is_err());
    }

    #[test]
    fn a_source_takes_its_frame_shape() {
        assert_eq!(shape_of((1280, 720)), Some("16:9"));
        assert_eq!(shape_of((1280, 704)), Some("16:9"));
        assert_eq!(shape_of((720, 1280)), Some("9:16"));
        assert_eq!(shape_of((100, 300)), None);
    }
}
