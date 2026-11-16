//! Brief `media_match`: each line of the script gets the clip or photo from the user's folder that shows it. A free vision
//! row describes each file once (one frame, kept by the file's hash), and one call picks a file per line. Only `make` sends
//! frames: `plan` shows picks that are already kept and names the host for the rest. To change a pick, write `scenes`.

use crate::brief::Scene;
use crate::engine_ffmpeg::Ffmpeg;
use crate::judge;
use crate::media::{Item, Kind};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A clip longer than this is looked at in parts.
const LONG_CLIP_S: f64 = 12.0;
/// Scene-change level for finding shots in a long clip, as in `track`.
const CUT_THRESHOLD: f64 = 0.25;

/// A scene with no picture of its own.
pub fn is_open(s: &Scene) -> bool {
    s.media.is_none() && s.stock.is_none() && s.generate.is_none()
}

/// One scene per sentence, in the script's own words and order. A sentence under 3 words joins the next, so no shot flashes.
pub fn scenes_from_script(script: &str) -> Vec<Scene> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for w in script.split_whitespace() {
        cur.push(w);
        // The captions' rule, so an initial ("W. H. Auden") does not cut a scene.
        let end = crate::captions::ends_sentence(w) || w.ends_with(['…', '。']);
        if end && cur.len() >= 3 {
            out.push(cur.join(" "));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        match out.last_mut() {
            Some(l) if cur.len() < 3 => {
                l.push(' ');
                l.push_str(&cur.join(" "));
            }
            _ => out.push(cur.join(" ")),
        }
    }
    out.into_iter()
        .map(|script| Scene {
            open_image: None,
            script,
            seconds: None,
            media: None,
            media_from: None,
            media_to: None,
            sound: None,
            stock: None,
            generate: None,
            card: None,
            cards: None,
            camera: None,
            inset: None,
            overlay: None,
            voice: None,
            transition: None,
        })
        .collect()
}

/// The picks from the reply, one per line. A pick out of range or missing takes the file used least so far, so a bad reply
/// still gives a whole video.
pub fn picks(reply: &str, lines: usize, files: usize) -> Vec<usize> {
    let v: Value = serde_json::from_str(
        reply
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim(),
    )
    .unwrap_or(Value::Null);
    let got = v["picks"].as_array().cloned().unwrap_or_default();
    let mut used = vec![0usize; files];
    (0..lines)
        .map(|i| {
            let p = got
                .get(i)
                .and_then(Value::as_u64)
                .map(|p| p as usize)
                .filter(|p| *p < files)
                .unwrap_or_else(|| (0..files).min_by_key(|f| used[*f]).unwrap_or(0));
            used[p] += 1;
            p
        })
        .collect()
}

/// A reply kept in the project, so `plan` and `make` agree and a rerun makes no call. Only a good reply is kept: a bad one
/// kept here would come back on every retry.
pub fn cached(
    name: String,
    good: impl Fn(&str) -> bool,
    make: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let dir = Path::new(".scriptvox/analyze");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(name);
    if let Some(t) = std::fs::read_to_string(&path).ok().filter(|t| good(t)) {
        return Ok(t);
    }
    let t = make()?;
    if good(&t) {
        let _ = std::fs::write(&path, &t);
    }
    Ok(t)
}

/// One call to the vision service. `send` is false at `plan`, which sends nothing: there only a kept reply counts.
fn ask(send: bool, body: &Value) -> Result<String, String> {
    let url = judge::need_vision().map_err(|e| e.1)?;
    if !send {
        return Err("a plan sends nothing".into());
    }
    judge::ask(&url, body).map_err(|e| e.1)
}

/// The parts of a file worth a look: (where it plays from, where it ends, where to look). A still or a short clip is one part. A long clip
/// is its longest shots, up to 4, or three even parts when it has no cuts. Kept by the file's hash: finding cuts decodes the
/// whole clip.
fn parts(ff: &Ffmpeg, item: &Item, hash: &str) -> Vec<(f64, f64, f64)> {
    let total = match item.kind {
        Kind::Video => ff.video_duration(&item.path).unwrap_or(0.0),
        _ => 0.0,
    };
    if total <= LONG_CLIP_S {
        return vec![(0.0, total, total / 2.0)];
    }
    let dir = Path::new(".scriptvox/analyze");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("parts-{}.json", &hash[..16]));
    if let Some(v) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
    {
        return v;
    }
    let cuts = crate::analyze::picture_cuts(ff, &item.path, CUT_THRESHOLD).unwrap_or_default();
    let (mut shots, _) = crate::analyze::shot_stats(&cuts, total);
    if shots.len() < 2 {
        shots = (0..3)
            .map(|i| (total * i as f64 / 3.0, total * (i + 1) as f64 / 3.0))
            .collect();
    }
    shots.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
    shots.truncate(4);
    shots.sort_by(|a, b| a.0.total_cmp(&b.0));
    let v: Vec<(f64, f64, f64)> = shots
        .iter()
        .map(|s| (s.0, s.1, (s.0 + s.1) / 2.0))
        .collect();
    let _ = std::fs::write(&path, json!(v).to_string());
    v
}

/// What one part of a file shows, in a short sentence.
fn describe(ff: &Ffmpeg, item: &Item, hash: &str, at: f64, send: bool) -> Result<String, String> {
    let name = format!("desc-{}-{}", &hash[..16], (at * 10.0).round() as u64);
    cached(
        format!("{name}.txt"),
        |t| !t.trim().is_empty(),
        || {
            let dir = Path::new(".scriptvox/analyze");
            let jpg = judge::grab(ff, &item.path, at, 512, &dir.join(format!("{name}.jpg")))?;
            use base64::Engine;
            let uri = format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&jpg)
            );
            let body = json!({
                "model": judge::vision_model(),
                "temperature": 0,
                "max_tokens": 60,
                "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": uri}},
                    {"type": "text", "text": "One short sentence in English: what this shot shows (subject, place, action). No opinions."}
                ]}],
                "chat_template_kwargs": {"enable_thinking": false}
            });
            ask(send, &body).map(|t| t.trim().to_string())
        },
    )
}

/// One call: a file for each line.
fn assign(lines: &[String], shots: &[String], send: bool) -> Result<Vec<usize>, String> {
    let key = blake3::hash(json!([lines, shots]).to_string().as_bytes()).to_hex();
    let whole = |t: &str| {
        serde_json::from_str::<Value>(t).is_ok_and(|v| {
            v["picks"]
                .as_array()
                .is_some_and(|p| p.len() == lines.len())
        })
    };
    let reply = cached(format!("match-{}.json", &key[..16]), whole, || {
        let list = |v: &[String]| {
            v.iter()
                .enumerate()
                .map(|(i, s)| format!("{i}: {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let body = json!({
            "model": judge::vision_model(),
            "temperature": 0,
            "max_tokens": 40 + lines.len() * 8,
            "messages": [{"role": "user", "content": format!(
                "A video editor picks a shot for each narration line.\n\nNarration lines:\n{}\n\nShots:\n{}\n\nFor each line in order, give the number of the shot that best shows what the line says. Use each shot once before you use any again, unless a line clearly needs one already used.",
                list(lines), list(shots))}],
            "response_format": {"type": "json_schema", "json_schema": {"name": "picks", "schema": {
                "type": "object",
                "properties": {"picks": {"type": "array", "items": {"type": "integer"}}},
                "required": ["picks"]
            }}},
            "chat_template_kwargs": {"enable_thinking": false}
        });
        ask(send, &body)
    })?;
    Ok(picks(&reply, lines.len(), shots.len()))
}

/// Fills `scenes[i].media` for the scenes with no picture of their own. The result names each pick. With an error the scenes
/// stay as they were and the caller falls back to the folder order. With `send` false nothing leaves this machine.
pub fn apply(
    scenes: &mut [Scene],
    ff: &Ffmpeg,
    items: &[Item],
    send: bool,
) -> Result<Value, String> {
    // (file, plays from, ends, what it shows), for every part of every file.
    let mut found: Vec<(usize, f64, f64, String)> = Vec::new();
    for (f, item) in items.iter().enumerate() {
        let hash = crate::store::hash_file(&item.path).map_err(|e| e.to_string())?;
        let whole = parts(ff, item, &hash).len() == 1;
        for (from, to, at) in parts(ff, item, &hash) {
            // One part is the whole file: no end of its own.
            found.push((
                f,
                from,
                if whole { 0.0 } else { to },
                describe(ff, item, &hash, at, send)?,
            ));
        }
    }
    let shots: Vec<String> = found.iter().map(|p| p.3.clone()).collect();
    let open: Vec<usize> = scenes
        .iter()
        .enumerate()
        .filter(|(_, s)| is_open(s))
        .map(|(i, _)| i)
        .collect();
    let lines: Vec<String> = open
        .iter()
        .map(|&i| crate::pauses::strip(&scenes[i].script))
        .collect();
    let chosen = assign(&lines, &shots, send)?;
    let name = |p: &PathBuf| {
        p.file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned())
    };
    let mut report = Vec::new();
    for (&i, &c) in open.iter().zip(&chosen) {
        let (f, from, to, shows) = &found[c];
        scenes[i].media = Some(items[*f].path.clone());
        scenes[i].media_from = (*from > 0.0).then_some(*from);
        scenes[i].media_to = (*to > 0.0).then_some(*to);
        report.push(
            json!({"scene": i + 1, "file": name(&items[*f].path), "from": from, "to": to, "shows": shows}),
        );
    }
    let unused: Vec<String> = items
        .iter()
        .enumerate()
        .filter(|(f, _)| !chosen.iter().any(|&c| found[c].0 == *f))
        .map(|(_, it)| name(&it.path))
        .collect();
    Ok(json!({"scenes": report, "unused": unused}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_keep_every_word_in_order_and_short_ends_join() {
        let s = "A camel walks. Dr. Sami waits at the port! Rain falls hard. Yes.";
        let sc = scenes_from_script(s);
        let texts: Vec<&str> = sc.iter().map(|x| x.script.as_str()).collect();
        assert_eq!(texts.join(" "), s);
        assert_eq!(texts[0], "A camel walks.");
        assert_eq!(texts.last().unwrap(), &"Rain falls hard. Yes.");
        assert_eq!(scenes_from_script("مرحبا بكم هنا؟ نعم نعم نعم.").len(), 2);
    }

    #[test]
    fn a_bad_reply_still_gives_a_file_for_each_line() {
        assert_eq!(picks(r#"{"picks": [2, 0]}"#, 2, 3), vec![2, 0]);
        // Out of range, missing, or no JSON at all: the least used file.
        assert_eq!(picks(r#"{"picks": [9]}"#, 3, 2), vec![0, 1, 0]);
        assert_eq!(picks("sorry", 2, 2), vec![0, 1]);
    }
}
