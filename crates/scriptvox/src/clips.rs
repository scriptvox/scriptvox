//! `scriptvox clips <video>`: a long talk into ranked shorts. The free speech row gives the sentences, the free text row picks
//! the parts that stand alone and writes a hook for each, and `edit` cuts, crops and captions them. Idea: OpenMontage's clip
//! factory, which leaves all of this to the agent; our own code.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::judge;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct Args {
    pub input: PathBuf,
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub aspect: String,
    pub track: bool,
    pub captions: String,
    pub caption_style: crate::caption_style::Style,
    pub lang: Option<String>,
    pub plan: bool,
    pub terms: Option<crate::say::Terms>,
    pub use_system: bool,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("clips", exit, msg.into(), Some(fix.into()))
}

/// A sentence: start and end in seconds.
type Span = (f64, f64);

#[derive(Debug, PartialEq)]
pub struct Pick {
    pub first: usize,
    pub last: usize,
    pub score: u64,
    pub hook: String,
    pub why: String,
}

/// Sentences for one request to the model.
const PICK_CHUNK_S: f64 = 1200.0;

/// Ranges of sentences of about `chunk` seconds each. Each starts `overlap` seconds before the last one ended.
fn pick_chunks(spans: &[Span], chunk: f64, overlap: f64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut lo = 0;
    while lo < spans.len() {
        // At least one sentence, however long.
        let hi = (lo + 1..spans.len())
            .find(|&i| spans[i].1 - spans[lo].0 > chunk)
            .unwrap_or(spans.len());
        out.push((lo, hi));
        if hi == spans.len() {
            break;
        }
        let back = spans[hi].0 - overlap;
        lo = (lo + 1..hi).find(|&i| spans[i].0 >= back).unwrap_or(hi);
    }
    out
}

/// The picks of one reply, with sentence numbers of the whole list.
fn shift_picks(reply: &str, by: usize) -> Vec<Value> {
    let v: Value = serde_json::from_str(
        reply
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim(),
    )
    .unwrap_or(Value::Null);
    v["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let mut c = c.clone();
            c["first"] = json!(c["first"].as_u64()? as usize + by);
            c["last"] = json!(c["last"].as_u64()? as usize + by);
            Some(c)
        })
        .collect()
}

/// The parts from the reply: inside the sentence list, stretched or cut by whole sentences to `min` to `max` seconds,
/// no two sharing a sentence, best first, at most `count`. A part that cannot fit is left out.
pub fn valid_picks(reply: &str, spans: &[Span], min: f64, max: f64, count: usize) -> Vec<Pick> {
    let v: Value = serde_json::from_str(
        reply
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim(),
    )
    .unwrap_or(Value::Null);
    let mut picks: Vec<Pick> = v["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let first = c["first"].as_u64()? as usize;
            let mut last = (c["last"].as_u64()? as usize).min(spans.len().checked_sub(1)?);
            if first > last {
                return None;
            }
            let len = |l: usize| spans[l].1 - spans[first].0;
            while len(last) < min && last + 1 < spans.len() {
                last += 1;
            }
            while len(last) > max && last > first {
                last -= 1;
            }
            (min..=max).contains(&len(last)).then(|| Pick {
                first,
                last,
                score: c["score"].as_u64().unwrap_or(0).min(10),
                hook: c["hook"].as_str().unwrap_or("").trim().to_string(),
                why: c["why"].as_str().unwrap_or("").trim().to_string(),
            })
        })
        .collect();
    picks.sort_by_key(|p| std::cmp::Reverse(p.score));
    let mut kept: Vec<Pick> = Vec::new();
    for p in picks {
        if kept.len() < count && kept.iter().all(|k| p.last < k.first || p.first > k.last) {
            kept.push(p);
        }
    }
    kept
}

/// Ask the free text row for parts; the reply is kept by the sentences, so a rerun picks the same.
fn ask_picks(
    sentences: &[Value],
    count: usize,
    min: f64,
    max: f64,
) -> Result<String, (Exit, String)> {
    let list: String = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{i} [{:.0}s] {}\n",
                s["end"].as_f64().unwrap_or(0.0) - s["start"].as_f64().unwrap_or(0.0),
                s["text"].as_str().unwrap_or("")
            )
        })
        .collect();
    let want = count * 2;
    let prompt = format!(
        "Below are the numbered sentences of a long talk, each with its length. Pick up to {want} parts that work as short videos \
for people who did not see the talk: {min:.0} to {max:.0} seconds, a run of whole sentences that makes sense alone and opens with something \
that makes a viewer stay (a claim, a number, a surprise, a question), not with \"so\", \"and\", or a word that points back. It ends on its own last point, not on a sentence that opens the next topic. They must not overlap.\n\n\
For each part give: first and last (sentence numbers), score (1 to 10, how well it stands alone and holds a viewer; be strict), hook (a headline of at most 6 words, in the language of the talk, \
that adds to the first sentence and does not repeat it), why (under 12 words, in English).\n\n{list}"
    );
    let key = blake3::hash(prompt.as_bytes()).to_hex();
    let body = json!({
        "model": judge::vision_model(),
        "temperature": 0,
        "max_tokens": 300 + want * 80,
        "messages": [{"role": "user", "content": prompt}],
        "response_format": {"type": "json_schema", "json_schema": {"name": "clips", "schema": {
            "type": "object",
            "properties": {"clips": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "first": {"type": "integer"}, "last": {"type": "integer"}, "score": {"type": "integer"},
                    "hook": {"type": "string"}, "why": {"type": "string"}
                },
                "required": ["first", "last", "score", "hook", "why"]
            }}},
            "required": ["clips"]
        }}},
        "chat_template_kwargs": {"enable_thinking": false}
    });
    let url = judge::need_vision()?;
    let mut failed = None;
    // A cut-off reply is not kept, so a retry can help.
    let good = |t: &str| {
        serde_json::from_str::<Value>(t)
            .is_ok_and(|v| v["clips"].as_array().is_some_and(|c| !c.is_empty()))
    };
    let reply = crate::match_media::cached(format!("clips-{}.json", &key[..16]), good, || {
        judge::ask(&url, &body).map_err(|(exit, e)| {
            failed = Some(exit);
            e
        })
    });
    reply.map_err(|e| (failed.unwrap_or(Exit::Error), e))
}

pub fn run(mut a: Args) -> Envelope {
    // Before any call: a bad style would fail each clip after the talk was heard and scored.
    if let Some((e, fix)) = a.caption_style.check(Some(&a.captions)) {
        return fail(Exit::Usage, e, fix);
    }
    if let Some(env) = crate::media_gate::refuse("clips", [&a.input]) {
        return env;
    }
    let hdr = match crate::engine_ffmpeg::sdr_input(&mut a.input, a.use_system) {
        Ok(h) => h,
        Err(e) => {
            return fail(
                Exit::Error,
                e,
                "Check the file, and that the disk has room.",
            )
        }
    };
    let mut env = run_sdr(a);
    env.findings.extend(hdr);
    env
}

fn run_sdr(a: Args) -> Envelope {
    if a.count == 0 || a.min <= 0.0 || a.max <= a.min {
        return fail(
            Exit::Usage,
            "--count must be 1 or more, and --max over --min",
            "Give for example --count 3 --min 20 --max 60.",
        );
    }
    if crate::make::frame(&a.aspect).is_none() {
        return fail(
            Exit::Usage,
            format!("unknown aspect {}", a.aspect),
            "Use --aspect 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9.",
        );
    }
    if !a.input.is_file() {
        return fail(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("clips", e),
    };
    let total = match ff.duration(&a.input) {
        Ok(t) => t,
        Err(e) => return fail(Exit::Usage, e, "Give a video FFmpeg can read."),
    };
    // The same refusal `edit` gives, before the transcript and with --plan too.
    if let Some(made) = crate::edit::crop_cuts_text(&ff, &a.input, &a.aspect) {
        return fail(
            Exit::Usage,
            format!("this ScriptVox video was made in {made}: a {} crop cuts its burned captions and cards", a.aspect),
            &format!("Give --aspect {made}, or use the original footage."),
        );
    }
    if total < a.min {
        return fail(
            Exit::Usage,
            format!(
                "the video runs {total:.0} s, shorter than --min {:.0}",
                a.min
            ),
            "Lower --min, or use `scriptvox edit` on this video.",
        );
    }
    // Before the transcript: without this service the talk's sound would leave for nothing.
    if let Err((exit, e)) = judge::need_vision() {
        return fail(exit, e, "Set a vision service, or pick parts yourself.");
    }
    let (doc, _) = match crate::analyze::transcribe_terms(
        &ff,
        &a.input,
        a.lang.as_deref(),
        total,
        a.terms.as_ref(),
    ) {
        Ok(d) => d,
        Err(mut env) => {
            env.command = "clips".into();
            return *env;
        }
    };
    let sentences: Vec<Value> = doc["sentences"].as_array().cloned().unwrap_or_default();
    let spans: Vec<Span> = sentences
        .iter()
        .map(|s| {
            (
                s["start"].as_f64().unwrap_or(0.0),
                s["end"].as_f64().unwrap_or(0.0),
            )
        })
        .collect();
    // A long talk goes in pieces of about 20 minutes that overlap by a clip, so no list is too long for the model. Each
    // piece gives its best, and the scores decide.
    let mut all = Vec::new();
    for (lo, hi) in pick_chunks(&spans, PICK_CHUNK_S, a.max) {
        let reply =
            match ask_picks(&sentences[lo..hi], a.count, a.min, a.max) {
                Ok(r) => r,
                Err((exit, e)) => return fail(
                    exit,
                    e,
                    "Run it again, or pick parts yourself from `scriptvox analyze --transcript`.",
                ),
            };
        all.extend(shift_picks(&reply, lo));
    }
    let reply = json!({"clips": all}).to_string();
    let picks = valid_picks(&reply, &spans, a.min, a.max, a.count);
    if picks.is_empty() {
        return fail(
            Exit::GateFailed,
            format!(
                "no part of {:.0} to {:.0} s stands alone in {} sentences",
                a.min,
                a.max,
                sentences.len()
            ),
            "Widen --min and --max, or pick parts yourself from `scriptvox analyze --transcript`.",
        );
    }
    let stem = a
        .input
        .file_stem()
        .map_or("video".into(), |s| s.to_string_lossy().into_owned());
    let mut clips = Vec::new();
    let mut failed = None;
    for (n, p) in picks.iter().enumerate() {
        // A little air around the first and last word, as the edit skill says.
        let from = (spans[p.first].0 - 0.15).max(0.0);
        let to = (spans[p.last].1 + 0.2).min(total);
        // Never over an earlier short: another talk or another run may have the same name.
        let out = (0..)
            .map(|k| match k {
                0 => format!("{stem}-clip{}.mp4", n + 1),
                k => format!("{stem}-clip{}-{k}.mp4", n + 1),
            })
            .map(|name| PathBuf::from("scriptvox-out").join(name))
            .find(|p| !p.exists())
            .unwrap();
        let text: Vec<&str> = sentences[p.first..=p.last]
            .iter()
            .filter_map(|s| s["text"].as_str())
            .collect();
        let mut row = json!({
            "rank": n + 1, "score": p.score, "hook": p.hook, "why": p.why,
            "from": (from * 100.0).round() / 100.0, "to": (to * 100.0).round() / 100.0,
            "seconds": ((to - from) * 10.0).round() / 10.0, "text": text.join(" "),
        });
        if !a.plan {
            let env = crate::edit::run(crate::edit::Args {
                terms: None, // the transcript file above has the term fixes already
                label: None,
                keep_words: None,
                bleep: None,
                bleep_with: None,
                bleep_pad: None,
                words: None,
                cut_retakes: false,
                clean_voice: None,
                zoom: vec![],
                text: None,
                fit: None,
                broll: vec![],
                zoom_on_emphasis: false,
                plan: false,
                input: a.input.clone(),
                cut_silence: false,
                min_silence: 0.8,
                pad: 0.15,
                noise_db: -35.0,
                reframe: Some(a.aspect.clone()),
                track: a.track,
                crop_text: false,
                cut_fillers: false,
                subject: None,
                keep: None,
                speed: None,
                duration: None,
                cut_still: false,
                blur: vec![],
                from: Some(from),
                to: Some(to),
                captions: Some(a.captions.clone()),
                // The talk was heard once above: each clip takes its words from there, so a paid row is not paid twice.
                captions_file: doc["file"].as_str().map(PathBuf::from),
                captions_second: None,
                lang: a.lang.clone(),
                // A hook the fonts cannot draw (an emoji from the model) is left out: the short still works.
                hook: Some(p.hook.clone())
                    .filter(|h| !h.is_empty() && crate::textlint::check(h).is_empty()),
                emphasis: None,
                music: None,
                music_level: None,
                sfx: vec![],
                sfx_level: None,
                speed_ranges: None,
                small: Default::default(),
                caption_style: a.caption_style.clone(),
                music_cut: (None, None),
                background: None,
                alpha: false,
                enhance: false,
                max_spend: None,
                lipsync: None,
                logo: None,
                accent: None,
                out: Some(out),
                use_system: a.use_system,
            });
            if let Some(e) = env.error.as_ref().filter(|_| !env.ok) {
                failed = Some((env.exit, n + 1, e.message.clone(), e.fix.clone()));
                row["error"] = json!(e.message);
            }
            row["video"] = env.artifacts.get("video").cloned().unwrap_or(Value::Null);
            row["findings"] = json!(env.findings);
        }
        clips.push(row);
    }
    let mut env = match failed {
        Some((exit, n, m, fix)) => Envelope::fail("clips", exit, format!("clip {n}: {m}"), fix),
        None => Envelope::ok("clips"),
    };
    env.metrics.insert("clips".into(), json!(clips.len()));
    env.metrics
        .insert("sentences".into(), json!(sentences.len()));
    if !a.plan {
        env.artifacts
            .insert("video".into(), clips[0]["video"].clone());
    }
    if env.ok {
        env.next = if a.plan {
            vec![
                "Drop --plan to cut them, or give the `from` and `to` you like to `scriptvox edit`."
                    .into(),
            ]
        } else {
            let first = clips[0]["video"].as_str().unwrap_or("<clip>");
            vec![
                format!("scriptvox snapshot {first} --sheet 9 --json, then the same for each other `clips[].video`."),
                format!("scriptvox export {first} --json"),
            ]
        };
    }
    env.with("clips", json!(clips))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_list_goes_in_chunks_that_overlap_by_a_clip() {
        let spans: Vec<Span> = (0..100)
            .map(|i| (i as f64 * 60.0, i as f64 * 60.0 + 50.0))
            .collect();
        let c = pick_chunks(&spans, 1200.0, 120.0);
        assert_eq!(c[0], (0, 20));
        assert_eq!(c[1].0, 18);
        assert_eq!(c.last().unwrap().1, 100);
        assert_eq!(pick_chunks(&spans[..5], 1200.0, 120.0), [(0, 5)]);
        // A sentence longer than a chunk must not hang.
        assert_eq!(
            pick_chunks(&[(0.0, 5000.0), (5000.0, 5001.0)], 1200.0, 60.0).len(),
            2
        );
        let s = shift_picks(r#"{"clips":[{"first":1,"last":2,"score":7}]}"#, 18);
        assert_eq!(
            (
                s[0]["first"].as_u64(),
                s[0]["last"].as_u64(),
                s[0]["score"].as_u64()
            ),
            (Some(19), Some(20), Some(7))
        );
    }

    fn spans() -> Vec<Span> {
        (0..10)
            .map(|i| (i as f64 * 10.0, i as f64 * 10.0 + 9.5))
            .collect()
    }

    #[test]
    fn parts_fit_the_length_and_do_not_overlap_best_first() {
        let r = r#"{"clips": [
            {"first": 0, "last": 0, "score": 5, "hook": "short", "why": "x"},
            {"first": 1, "last": 2, "score": 9, "hook": "overlap", "why": "x"},
            {"first": 2, "last": 9, "score": 7, "hook": "long", "why": "x"},
            {"first": 7, "last": 40, "score": 3, "hook": "past the end", "why": "x"},
            {"first": 7, "last": 2, "score": 10, "hook": "backward", "why": "x"}
        ]}"#;
        let p = valid_picks(r, &spans(), 20.0, 40.0, 3);
        let got: Vec<(usize, usize, &str)> = p
            .iter()
            .map(|x| (x.first, x.last, x.hook.as_str()))
            .collect();
        // "overlap" 1-2 is 19.5 s, stretched to 1-3. "long" 2-9 is cut to 2-5, which overlaps, so it goes. "short" 0-0
        // stretches into 1-3, so it goes too. "past the end" is held at the last sentence.
        assert_eq!(got, vec![(1, 3, "overlap"), (7, 9, "past the end")]);
    }

    #[test]
    fn a_bad_reply_gives_no_parts() {
        assert!(valid_picks("sorry", &spans(), 20.0, 40.0, 3).is_empty());
        assert!(valid_picks(r#"{"clips": [{"first": 0}]}"#, &spans(), 20.0, 40.0, 3).is_empty());
        assert!(valid_picks(r#"{"clips": []}"#, &[], 20.0, 40.0, 3).is_empty());
    }
}
