//! `scriptvox analyze <video>`: read any video the way an editor would before copying its style (P3-003). Shots and
//! pace from FFmpeg scene detection, loudness and silence, a strip of one frame per shot, and with `--transcript` the
//! words and speaking rate from a speech to text row. Local and free except the transcript (a free selfhost row today).

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Args {
    pub input: PathBuf,
    pub threshold: f64,
    pub transcript: bool,
    pub lang: Option<String>,
    pub like: Option<PathBuf>,
    pub terms: Option<crate::say::Terms>,
    /// With `transcript`: who speaks, as `S1`, `S2` on each word and sentence (a row that tells speakers apart).
    pub speakers: bool,
    /// Also find the beat grid of the sound (docs/specs/beats.md).
    pub beats: bool,
    pub use_system: bool,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("analyze", exit, msg.into(), Some(fix.into()))
}

/// Cut times from FFmpeg `showinfo` lines after a scene `select`. Only showinfo prints `pts_time:` in these graphs, so
/// the `[Parsed_showinfo` prefix is not required: the audio and video filters log from two threads, and a line that
/// lands inside another filter's unfinished line loses its prefix (a cut missed once in about 5 CI runs under load).
fn parse_cuts(log: &str) -> Vec<f64> {
    log.lines()
        .filter_map(|l| {
            l.split("pts_time:")
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .collect()
}

/// The exact frame of a cut found at 4 fps near `near`: the same scene test on every frame of the second around it.
/// None when it is not found there.
pub fn exact_cut(ff: &Ffmpeg, video: &Path, near: f64, threshold: f64) -> Option<f64> {
    let from = (near - 0.5).max(0.0);
    let o = Command::new(&ff.ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-ss",
            &format!("{from:.3}"),
            "-t",
            "1.0",
        ])
        .args(crate::media_gate::input(video).ok()?)
        .args([
            "-an",
            "-vf",
            &format!("select='gt(scene,{threshold})',showinfo"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .ok()?;
    parse_cuts(&String::from_utf8_lossy(&o.stderr))
        .first()
        .map(|t| from + t)
}

/// Cut times of the picture only (the same 4 fps scene test as `analyze`), for `edit --track`.
pub fn picture_cuts(ff: &Ffmpeg, video: &Path, threshold: f64) -> Result<Vec<f64>, String> {
    let o = Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin"])
        .args(crate::media_gate::input(video)?)
        .args([
            "-an",
            "-vf",
            &format!("fps=4,select='gt(scene,{threshold})',showinfo"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
    Ok(parse_cuts(&String::from_utf8_lossy(&o.stderr)))
}

/// Shot lengths from cuts, and summary numbers: count, mean, shortest, longest, cuts per minute.
pub fn shot_stats(cuts: &[f64], total: f64) -> (Vec<(f64, f64)>, serde_json::Value) {
    let mut edges = vec![0.0];
    edges.extend(
        cuts.iter()
            .copied()
            .filter(|t| *t > 0.05 && *t < total - 0.05),
    );
    edges.push(total);
    let shots: Vec<(f64, f64)> = edges.windows(2).map(|w| (w[0], w[1])).collect();
    let lens: Vec<f64> = shots.iter().map(|(a, b)| b - a).collect();
    let n = lens.len().max(1) as f64;
    let r = |x: f64| (x * 100.0).round() / 100.0;
    let stats = json!({
        "shots": lens.len(),
        "mean_shot_s": r(lens.iter().sum::<f64>() / n),
        "shortest_s": r(lens.iter().cloned().fold(f64::INFINITY, f64::min)),
        "longest_s": r(lens.iter().cloned().fold(0.0, f64::max)),
        "cuts_per_min": r((lens.len() as f64 - 1.0).max(0.0) / total.max(0.001) * 60.0),
    });
    (shots, stats)
}

pub fn run(a: Args) -> Envelope {
    if !a.input.is_file() {
        return fail(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    if let Some(env) = crate::media_gate::refuse("analyze", [&a.input]) {
        return env;
    }
    if let Some(p) = a.like.as_ref().filter(|p| p.exists()) {
        return fail(
            Exit::Usage,
            format!("{} exists", p.display()),
            "Give a new file name: --like does not write over a brief.",
        );
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("analyze", e),
    };
    let total = match ff.duration(&a.input) {
        Ok(d) => d,
        Err(e) => return fail(Exit::Usage, e, "Give a video FFmpeg can read."),
    };
    let rb = ff.readback(&a.input).ok();
    let hdr = ff.hdr_kind(&a.input);
    // One pass: scene cuts on the picture, loudness and silence on the sound.
    let o = Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin"])
        // Refused files stopped at `duration` above; with no input FFmpeg opens nothing.
        .args(crate::media_gate::input(&a.input).unwrap_or_default())
        .args([
            "-vf",
            // Sampled at 4 fps: a crossfade spreads a cut over many frames (0.09 per frame on a scriptvox fade),
            // but between two samples 0.25 s apart it scores about 0.4; a moving shot with no cut stays near 0.08.
            &format!("fps=4,select='gt(scene,{})',showinfo", a.threshold),
            "-af",
            // framelog=quiet: only the summary, not a line per 100 ms written in pieces between the showinfo lines.
            "silencedetect=n=-40dB:d=0.5,ebur128=framelog=quiet",
            "-f",
            "null",
            "-",
        ])
        .output();
    let log = match o {
        Ok(o) => String::from_utf8_lossy(&o.stderr).into_owned(),
        Err(e) => {
            return fail(
                Exit::Error,
                format!("cannot start FFmpeg: {e}"),
                "Check FFmpeg.",
            )
        }
    };
    let (shots, stats) = shot_stats(&parse_cuts(&log), total);
    let silence: f64 = log
        .split("silence_duration:")
        .skip(1)
        .filter_map(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .sum();
    let lufs = log.rfind("Summary:").and_then(|i| {
        log[i..]
            .split("I:")
            .nth(1)?
            .split_whitespace()
            .next()?
            .parse::<f64>()
            .ok()
    });
    let has_audio = ff.has_stream(&a.input, "audio");
    let mut env = Envelope::ok("analyze");
    if let Some(kind) = hdr {
        env.metrics.insert("hdr".into(), json!(kind));
        env.findings.push(json!({"gate": "hdr", "status": "warn",
            "detail": format!("the source is HDR ({kind}). The frames of this report look washed out, and that is not the picture"),
            "fix": "Nothing to do: `edit`, `clips`, `join` and `make` tone map it to SDR first."}));
    }
    env.metrics
        .insert("duration_s".into(), json!((total * 100.0).round() / 100.0));
    if let Some(r) = &rb {
        env.metrics
            .insert("size".into(), json!(format!("{}x{}", r.width, r.height)));
        // The real average: a variable-rate clip's nominal rate may be its highest rate or its time base.
        let avg = ff.frame_rate(&a.input).unwrap_or(r.fps);
        env.metrics
            .insert("fps".into(), json!((avg * 100.0).round() / 100.0));
        if crate::engine_ffmpeg::variable_rate(r.fps, avg) {
            env.metrics.insert("variable_rate".into(), json!(true));
        }
    }
    for (k, v) in stats.as_object().unwrap() {
        env.metrics.insert(k.clone(), v.clone());
    }
    if has_audio {
        env.metrics.insert(
            "silence_share".into(),
            json!((silence / total.max(0.001) * 100.0).round() / 100.0),
        );
        if let Some(i) = lufs {
            env.metrics.insert("loudness_lufs".into(), json!(i));
        }
    }
    // A strip of one frame per shot (at most 12, evenly picked), so an agent can see the look.
    let pick: Vec<f64> = {
        let mids: Vec<f64> = shots.iter().map(|(a, b)| (a + b) / 2.0).collect();
        let k = mids.len().min(12);
        (0..k).map(|i| mids[i * mids.len() / k]).collect()
    };
    let stem = a
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let strip = PathBuf::from("scriptvox-out").join(format!("{stem}.shots.png"));
    let _ = std::fs::create_dir_all("scriptvox-out");
    if crate::gates::snapshot(&ff, &a.input, &pick, None, &strip).is_ok() {
        env.artifacts.insert(
            "shots_image".into(),
            json!(std::path::absolute(&strip).unwrap_or(strip)),
        );
    }
    let shot_list: Vec<_> = shots
        .iter()
        .map(|(s, e)| json!({"start": (s * 100.0).round() / 100.0, "end": (e * 100.0).round() / 100.0}))
        .collect();
    env = env.with("shot_list", json!(shot_list));
    if a.beats {
        if !has_audio {
            return fail(Exit::Usage, "the video has no audio track", "Drop --beats.");
        }
        // 10 minutes is enough for a tempo, and keeps a long talk from filling memory (11025 samples a second).
        let pcm = match crate::beats::read(&ff, &a.input, 600.0) {
            Ok(p) => p,
            Err(e) => {
                return fail(
                    Exit::Error,
                    format!("cannot read the sound: {e}"),
                    "Give a file FFmpeg can read.",
                )
            }
        };
        let beats = match crate::beats::of(&pcm) {
            Some(b) => {
                let steady = b.confidence >= crate::beats::STEADY;
                env.metrics.insert("bpm".into(), json!(b.bpm));
                json!({"bpm": b.bpm, "confidence": b.confidence, "steady": steady, "count": b.times.len(),
                "interval_s": ((60.0 / b.bpm) * 1000.0).round() / 1000.0, "times": b.times,
                "note": if steady {
                    "One steady tempo is assumed. Cut on a beat only where it does not move a spoken word."
                } else {
                    "No steady beat: speech alone, a free tempo, or a tempo that changes. Do not cut on these times."
                }})
            }
            None => {
                json!({"bpm": null, "confidence": 0.0, "steady": false, "count": 0, "times": [],
                "note": "Under 4 s of sound, or silence: no beat grid."})
            }
        };
        env = env.with("beats", beats);
    }
    if let Some(path) = &a.like {
        return like(env, &ff, &a, path, &shots, total, has_audio);
    }
    if a.transcript {
        if !has_audio {
            return fail(
                Exit::Usage,
                "the video has no audio track",
                "Drop --transcript.",
            );
        }
        match transcribe_speakers(
            &ff,
            &a.input,
            a.lang.as_deref(),
            total,
            a.terms.as_ref(),
            a.speakers,
        ) {
            Ok((doc, pace)) => {
                env.metrics.insert("words_per_min".into(), json!(pace));
                env = env.with("transcript", doc);
            }
            Err(e) => return *e,
        }
    }
    env
}

/// `--like`: the starter brief. The free rows add the words and the look; without them the measured part still stands.
fn like(
    mut env: Envelope,
    ff: &Ffmpeg,
    a: &Args,
    path: &Path,
    shots: &[(f64, f64)],
    total: f64,
    has_audio: bool,
) -> Envelope {
    use crate::like::{Said, Seen};
    let mut warn = |detail: String, fix: &str| {
        env.findings
            .push(json!({"gate": "like", "status": "warn", "detail": detail, "fix": fix}));
    };
    let mut said: Vec<Said> = vec![];
    if has_audio {
        match transcribe_terms(ff, &a.input, a.lang.as_deref(), total, a.terms.as_ref()) {
            Ok((doc, _)) => {
                said = doc["sentences"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| Said {
                        at: (s["start"].as_f64().unwrap_or(0.0) + s["end"].as_f64().unwrap_or(0.0))
                            / 2.0,
                        text: s["text"].as_str().unwrap_or("").to_string(),
                    })
                    .filter(|s| !s.text.is_empty())
                    .collect();
            }
            Err(e) => warn(
                format!(
                    "no words from the reference: {}",
                    e.error
                        .as_ref()
                        .map_or("speech to text failed", |x| x.message.as_str())
                ),
                "The scripts are placeholders with a word count. Write them.",
            ),
        }
    }
    let spans = crate::like::scenes(shots, &said);
    // One frame per scene, a third of the way in (past a crossfade).
    let dir = Path::new(".scriptvox/analyze");
    let _ = std::fs::create_dir_all(dir);
    // Every scene or none: the prompts come back by scene number.
    let frames: Result<Vec<Vec<u8>>, String> = spans
        .iter()
        .enumerate()
        .map(|(i, (s, e))| {
            let f = dir.join(format!("like{i}.jpg"));
            crate::judge::grab(ff, &a.input, s + (e - s) / 3.0, 512, &f)
        })
        .collect();
    let seen = frames
        .and_then(|frames| {
            let body = crate::like::request(&crate::judge::vision_model(), &frames);
            crate::judge::need_vision()
                .and_then(|u| crate::judge::ask(&u, &body))
                .map_err(|e| e.1)
        })
        .and_then(|t| crate::like::parse_seen(&t))
        .unwrap_or_else(|e| {
            warn(
                format!("no style from the vision row: {e}"),
                "Look at shots_image and set look, captions and the scene visuals yourself.",
            );
            Seen::default()
        });
    let size = ff
        .shown_size(&a.input)
        .map_or((1080, 1920), |(w, h)| (u64::from(w), u64::from(h)));
    let real = ff.frame_rate(&a.input).unwrap_or(30.0);
    let brief = crate::like::brief(size, real, &spans, &said, &seen);
    if !said.is_empty() {
        warn(
            "the scripts hold the reference's own words, as a guide to length and tone".into(),
            "Write your own words before you make the video. Do not publish the reference's script.",
        );
    }
    let text = serde_json::to_string_pretty(&brief).unwrap_or_default();
    if let Err(e) = std::fs::write(path, text + "\n") {
        return fail(
            Exit::Error,
            format!("{}: {e}", path.display()),
            "Give a path you can write.",
        );
    }
    env.artifacts.insert(
        "brief".into(),
        json!(std::path::absolute(path).unwrap_or(path.to_path_buf())),
    );
    env.metrics.insert("scenes".into(), json!(spans.len()));
    env.next = vec![format!("scriptvox plan --brief {} --json", path.display())];
    env
}

/// No way to hear on this install.
pub const NO_STT: &str = "no speech to text: install the local plugin with `scriptvox setup speech-local` (about 490 MB to download, about 880 MB on disk, free, on this machine), or give your own GPU service a key";

pub fn transcribe(
    ff: &Ffmpeg,
    input: &Path,
    lang: Option<&str>,
    total: f64,
) -> Result<(serde_json::Value, f64), Box<Envelope>> {
    transcribe_terms(ff, input, lang, total, None)
}

/// [`transcribe`] with a term file: a hint to the row, then the listed fix-up in `term_fixes`.
pub fn transcribe_terms(
    ff: &Ffmpeg,
    input: &Path,
    lang: Option<&str>,
    total: f64,
    terms: Option<&crate::say::Terms>,
) -> Result<(serde_json::Value, f64), Box<Envelope>> {
    transcribe_speakers(ff, input, lang, total, terms, false)
}

/// [`transcribe_terms`], and with `speakers` each word and sentence says who speaks (`S1`, `S2`).
pub fn transcribe_speakers(
    ff: &Ffmpeg,
    input: &Path,
    lang: Option<&str>,
    total: f64,
    terms: Option<&crate::say::Terms>,
    speakers: bool,
) -> Result<(serde_json::Value, f64), Box<Envelope>> {
    if speakers && crate::opentts::stt_row(true).is_none() {
        return Err(Box::new(fail(
            Exit::NeedsInput,
            "no speech to text row here tells speakers apart",
            "`scribe-fal` does (paid, a fal key: `scriptvox keys set fal`, then `--approve` of its plan). A free local route is not built yet.",
        )));
    }
    // A paid row only with `--approve` of its own plan (G141), checked once the audio is known.
    let Some(row) = crate::opentts::stt_row(speakers) else {
        return Err(Box::new(fail(
            Exit::NeedsInput,
            NO_STT,
            "Run `scriptvox setup speech-local`, or drop this step. `scriptvox providers` lists what this install has.",
        )));
    };
    // 16 kHz mono WAV keeps the upload small (about 1.9 MB a minute).
    let dir = Path::new(".scriptvox/analyze");
    let _ = std::fs::create_dir_all(dir);
    // Keyed on the content, not the path: a video made again at the same path must be heard again.
    let key = crate::store::hash_file(input).unwrap_or_else(|_| {
        blake3::hash(input.to_string_lossy().as_bytes())
            .to_hex()
            .to_string()
    })[..12]
        .to_string();
    let wav = dir.join(format!("{key}.wav"));
    let hint = terms.map(|t| t.hint()).unwrap_or_default();
    // The hint, the speakers and the language change what the row gives, so each has its own job record. With none of
    // them the old name stays valid.
    let tag = lang.map(|l| format!("lang={l}")).unwrap_or_default();
    let key = if hint.is_empty() && !speakers && tag.is_empty() {
        key
    } else {
        blake3::hash(format!("{key}{hint}{}{tag}", if speakers { "spk" } else { "" }).as_bytes())
            .to_hex()[..12]
            .to_string()
    };
    let args: Vec<String> = vec![
        "-i".into(),
        input.display().to_string(),
        "-vn".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        "16000".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        wav.display().to_string(),
    ];
    ff.run(Path::new("."), &args, None)
        .map_err(|e| Box::new(fail(Exit::Error, e, "Check the video's audio.")))?;
    crate::opentts::stt_gate(&row, &wav)
        .map_err(|m| Box::new(crate::plan::approval_fail("analyze", m)))?;
    let long = std::fs::metadata(&wav).is_ok_and(|m| m.len() > WINDOW_BYTES);
    let hear = |file: &Path, job: &str| hear_file(&row, file, lang, &hint, dir, job, speakers);
    let mut doc = if long {
        // Too big for one request: windows that overlap, each its own job, so a rerun picks up where one broke.
        let secs = ff.duration(&wav).unwrap_or(total);
        let spans = windows(secs);
        let mut parts = Vec::with_capacity(spans.len());
        for (i, &(start, len)) in spans.iter().enumerate() {
            let piece = dir.join(format!("{key}-w{i}.wav"));
            let args: Vec<String> = vec![
                "-ss".into(),
                format!("{start:.3}"),
                "-t".into(),
                format!("{len:.3}"),
                "-i".into(),
                wav.display().to_string(),
                "-c".into(),
                "copy".into(),
                piece.display().to_string(),
            ];
            ff.run(Path::new("."), &args, None)
                .map_err(|e| Box::new(fail(Exit::Error, e, "Check the disk has room.")))?;
            let file = hear(&piece, &format!("{key}-w{i}"))?;
            let _ = std::fs::remove_file(&piece);
            parts.push((start, read_json(&file)));
        }
        let joined = dir.join(format!("{key}-long.json"));
        std::fs::write(&joined, merge_windows(&parts).to_string())
            .map_err(|e| Box::new(fail(Exit::Error, e.to_string(), "Check the folder.")))?;
        joined
    } else {
        hear(&wav, &key)?
    };
    let mut v = read_json(&doc);
    let mut all = stt_words(&v);
    let mut term_fixes = vec![];
    if let Some(t) = terms {
        let mut words: Vec<crate::captions::Word> =
            serde_json::from_value(json!(all)).unwrap_or_default();
        term_fixes = t.fix_words(&mut words);
        if !term_fixes.is_empty() {
            all = words.iter().map(|w| json!(w)).collect();
            let text: Vec<&str> = words.iter().map(|w| w.text.trim()).collect();
            v["text"] = text.join(" ").into();
            v["words"] = json!(words);
            // `timed_words` reads this file, so captions and cuts get the fixed words too.
            doc = dir.join(format!("{key}-terms.json"));
            std::fs::write(&doc, v.to_string())
                .map_err(|e| Box::new(fail(Exit::Error, e.to_string(), "Check the folder.")))?;
        }
    }
    let words = all.len();
    let speech: f64 = v["segments"].as_array().map_or(total, |s| {
        s.iter()
            .map(|x| x["end"].as_f64().unwrap_or(0.0) - x["start"].as_f64().unwrap_or(0.0))
            .sum()
    });
    let pace = (words as f64 / speech.max(1.0) * 60.0).round();
    Ok((
        json!({"language": v["language"], "text": v["text"], "words": words, "sentences": sentences(&all), "file": doc, "term_fixes": term_fixes}),
        pace,
    ))
}

/// Above this a WAV goes in windows. A request holds it as text, a third larger.
pub(crate) const WINDOW_BYTES: u64 = 38_000_000;
/// 15 minutes of 16 kHz mono is 28.8 MB.
const WINDOW_S: f64 = 900.0;
/// A word cut by a window's edge is whole in the next one.
const OVERLAP_S: f64 = 10.0;

/// (start, length) of each window.
pub(crate) fn windows(secs: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut start = 0.0;
    loop {
        out.push((start, WINDOW_S.min(secs - start)));
        if start + WINDOW_S >= secs {
            return out;
        }
        start += WINDOW_S - OVERLAP_S;
    }
}

fn read_json(file: &Path) -> serde_json::Value {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// One speech to text job on one WAV. The reply's file.
fn hear_file(
    row: &crate::generators::Generator,
    file: &Path,
    lang: Option<&str>,
    hint: &str,
    dir: &Path,
    key: &str,
    speakers: bool,
) -> Result<PathBuf, Box<Envelope>> {
    use base64::Engine;
    let bytes = std::fs::read(file)
        .map_err(|e| Box::new(fail(Exit::Error, e.to_string(), "Check the folder.")))?;
    let input_json = row.request(&crate::generators::GenInput {
        audio: Some(format!(
            "data:audio/wav;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )),
        lang: lang.map(String::from),
        prompt: hint.to_string(),
        ..Default::default()
    });
    let mut input_json = input_json;
    if speakers {
        input_json["diarize"] = json!(true);
    }
    let cost = crate::opentts::stt_price(row, file).1;
    let (doc, _) = crate::genjob::run_priced(
        row,
        &input_json,
        false,
        Path::new("."),
        dir,
        key,
        &format!("analyze_{key}"),
        cost,
        900,
        (cost > 0.0).then_some(cost),
    )
    .map_err(|e| Box::new(fail(e.exit, e.message, &e.fix)))?;
    Ok(doc)
}

/// One reply from the replies of the windows, on the source's times. In an overlap each window keeps the words before the
/// middle, so none comes twice. A word sits on one side by its own middle.
pub(crate) fn merge_windows(parts: &[(f64, serde_json::Value)]) -> serde_json::Value {
    let take = |list: &str, i: usize, start: f64| -> Vec<serde_json::Value> {
        let lo = if i == 0 {
            f64::MIN
        } else {
            start + OVERLAP_S / 2.0
        };
        let hi = parts.get(i + 1).map_or(f64::MAX, |n| n.0 + OVERLAP_S / 2.0);
        parts[i].1[list]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let (s, e) = (w["start"].as_f64()? + start, w["end"].as_f64()? + start);
                let mid = (s + e) / 2.0;
                let mut w = w.clone();
                w["start"] = json!(s);
                w["end"] = json!(e);
                (mid >= lo && mid < hi).then_some(w)
            })
            .collect()
    };
    let mut words = vec![];
    let mut segments = vec![];
    for (i, (start, _)) in parts.iter().enumerate() {
        words.extend(take("words", i, *start));
        segments.extend(take("segments", i, *start));
    }
    let text: Vec<&str> = words
        .iter()
        .filter_map(|w| w["text"].as_str())
        .map(str::trim)
        .collect();
    json!({"language": parts.first().map(|p| p.1["language"].clone()), "text": text.join(" "),
        "words": words, "segments": segments})
}

/// The timed words of a [`transcribe`] result: its `words` field is only a count, the words are in its `file`.
pub fn timed_words(doc: &serde_json::Value) -> Option<Vec<crate::captions::Word>> {
    let text = std::fs::read_to_string(doc["file"].as_str()?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(serde_json::Value::Array(stt_words(&v))).ok()
}

/// The words of a speech to text reply, with a number whole again: Whisper can give "3" then ",000", or "1" then ".4".
/// The segment text is right, only the word list splits them, and that list reaches the screen as "3 ,000".
pub fn stt_words(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    // A row's own speaker ids ("speaker_0") become S1, S2 in the order they first speak.
    let mut ids: Vec<String> = Vec::new();
    for w in doc["words"].as_array().into_iter().flatten() {
        // ElevenLabs lists the spaces and sound events between words too.
        if w["type"].as_str().is_some_and(|t| t != "word") {
            continue;
        }
        let mut w = w.clone();
        if let Some(id) = w["speaker_id"].as_str().map(String::from) {
            let n = ids.iter().position(|x| *x == id).unwrap_or_else(|| {
                ids.push(id);
                ids.len() - 1
            });
            w["speaker"] = json!(format!("S{}", n + 1));
        }
        let w = &w;
        let t = w["text"].as_str().unwrap_or("").trim();
        let mut c = t.chars();
        let tail = match (c.next(), c.next()) {
            (Some(',' | '.' | '\u{066B}' | '\u{066C}'), Some(d)) => d.is_numeric(),
            (Some('%' | '\u{066A}'), _) => true,
            _ => false,
        };
        let head = out.last_mut().filter(|p| {
            tail && p["text"]
                .as_str()
                .and_then(|p| p.trim().chars().last())
                .is_some_and(char::is_numeric)
        });
        match head {
            Some(p) => {
                let joined = format!("{}{t}", p["text"].as_str().unwrap_or("").trim());
                p["text"] = joined.into();
                p["end"] = w["end"].clone();
            }
            None => out.push(w.clone()),
        }
    }
    out
}

/// Sentences with times from the timed words, so an agent picks a 20 to 60 s part without the word list. A sentence
/// ends at a word that ends in . ? ! or the Arabic question mark, or before a pause over 1.2 s (speech to text
/// without punctuation).
pub fn sentences(words: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let r = |t: f64| (t * 100.0).round() / 100.0;
    let mut out = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let (mut start, mut end) = (0.0, 0.0);
    for (i, w) in words.iter().enumerate() {
        let (t, s, e) = (
            w["text"].as_str().unwrap_or("").trim(),
            w["start"].as_f64().unwrap_or(end),
            w["end"].as_f64().unwrap_or(end),
        );
        if cur.is_empty() {
            start = s;
        }
        if !t.is_empty() {
            cur.push(t);
        }
        end = e;
        let pause = words
            .get(i + 1)
            .and_then(|n| n["start"].as_f64())
            .is_some_and(|n| n - e > 1.2);
        // Another speaker starts a new sentence.
        let turn = words
            .get(i + 1)
            .is_some_and(|n| !w["speaker"].is_null() && n["speaker"] != w["speaker"]);
        if !cur.is_empty()
            && (t.ends_with(['.', '?', '!', '\u{061F}']) || pause || turn || i + 1 == words.len())
        {
            let mut s = json!({"start": r(start), "end": r(end), "text": cur.join(" ")});
            if !w["speaker"].is_null() {
                s["speaker"] = w["speaker"].clone();
            }
            out.push(s);
            cur.clear();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_number_is_whole_again() {
        let doc = json!({"words": [
            {"text": "are", "start": 0.0, "end": 0.5},
            {"text": "3", "start": 0.5, "end": 0.7},
            {"text": ",000", "start": 0.7, "end": 1.2},
            {"text": "or", "start": 1.3, "end": 1.4},
            {"text": "1", "start": 1.4, "end": 1.5},
            {"text": ".4,", "start": 1.5, "end": 1.8},
            {"text": "90", "start": 2.0, "end": 2.2},
            {"text": "%", "start": 2.2, "end": 2.4},
            {"text": "Hi", "start": 3.0, "end": 3.1},
            {"text": ".5", "start": 3.1, "end": 3.2},
        ]});
        let got = stt_words(&doc);
        let text: Vec<_> = got.iter().map(|w| w["text"].as_str().unwrap()).collect();
        assert_eq!(text, ["are", "3,000", "or", "1.4,", "90%", "Hi", ".5"]);
        assert_eq!(got[1]["start"], 0.5);
        assert_eq!(got[1]["end"], 1.2);
    }

    #[test]
    fn long_audio_goes_in_overlapping_windows_and_no_word_comes_twice() {
        assert_eq!(windows(100.0), [(0.0, 100.0)]);
        let w = windows(2000.0);
        assert_eq!(w.len(), 3);
        assert_eq!(w[1], (890.0, 900.0));
        assert_eq!(w[2].0, 1780.0);
        assert!((w[2].0 + w[2].1 - 2000.0).abs() < 1e-9);
        // The word at 893 of the source is in both windows; the word at 900 too.
        let word = |t: &str, s: f64| json!({"text": t, "start": s, "end": s + 0.5});
        let first = json!({"language": "en", "words": [word("a", 10.0), word("b", 893.0), word("c", 899.0)],
            "segments": [{"start": 0.0, "end": 899.5}]});
        let then = json!({"words": [word("b", 3.0), word("c", 9.0), word("d", 20.0)],
            "segments": [{"start": 5.0, "end": 21.0}]});
        let v = merge_windows(&[(0.0, first), (890.0, then)]);
        assert_eq!(v["text"], "a b c d");
        let at: Vec<f64> = v["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["start"].as_f64().unwrap())
            .collect();
        assert_eq!(at, [10.0, 893.0, 899.0, 910.0]);
        assert_eq!(v["language"], "en");
    }

    #[test]
    fn timed_words_come_from_the_file_not_the_count() {
        let f = std::env::temp_dir().join(format!("scriptvox-timed-{}.json", std::process::id()));
        std::fs::write(
            &f,
            r#"{"words": [{"text": "hi", "start": 0.5, "end": 0.9}]}"#,
        )
        .unwrap();
        let w = timed_words(&json!({"words": 1, "file": f})).unwrap();
        std::fs::remove_file(&f).unwrap();
        assert_eq!((w.len(), w[0].start), (1, 0.5));
    }

    #[test]
    fn speakers_are_named_in_order_and_split_sentences() {
        let doc = json!({"words": [
            {"text": "Hi", "start": 0.0, "end": 0.3, "type": "word", "speaker_id": "speaker_7"},
            {"text": " ", "start": 0.3, "end": 0.35, "type": "spacing", "speaker_id": "speaker_7"},
            {"text": "there", "start": 0.35, "end": 0.6, "type": "word", "speaker_id": "speaker_7"},
            {"text": "hello", "start": 0.7, "end": 1.0, "type": "word", "speaker_id": "speaker_2"},
            {"text": "back.", "start": 1.0, "end": 1.3, "type": "word", "speaker_id": "speaker_2"},
        ]});
        let w = stt_words(&doc);
        assert_eq!(w.len(), 4);
        assert_eq!(w[0]["speaker"], "S1");
        assert_eq!(w[2]["speaker"], "S2");
        let s = sentences(&w);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(
            (s[0]["text"].as_str(), s[0]["speaker"].as_str()),
            (Some("Hi there"), Some("S1"))
        );
        assert_eq!(
            (s[1]["text"].as_str(), s[1]["speaker"].as_str()),
            (Some("hello back."), Some("S2"))
        );
    }

    #[test]
    fn sentences_from_words() {
        let w = |t: &str, s: f64, e: f64| json!({"text": t, "start": s, "end": e});
        let s = sentences(&[
            w("Hello", 0.0, 0.4),
            w("there.", 0.4, 0.9),
            w("Where", 1.0, 1.3),
            w("next", 1.3, 1.6),
            w("then", 3.0, 3.3),
            w("هل؟", 3.3, 3.8),
            w("end", 4.0, 4.2),
        ]);
        let t: Vec<&str> = s.iter().map(|x| x["text"].as_str().unwrap()).collect();
        assert_eq!(t, ["Hello there.", "Where next", "then هل؟", "end"]);
        assert_eq!(
            (s[1]["start"].as_f64(), s[1]["end"].as_f64()),
            (Some(1.0), Some(1.6))
        );
        assert!(sentences(&[]).is_empty());
    }

    #[test]
    fn shots_from_cuts() {
        let (shots, s) = shot_stats(&[2.0, 5.0, 9.98], 10.0);
        assert_eq!(
            shots,
            vec![(0.0, 2.0), (2.0, 5.0), (5.0, 10.0)],
            "a cut at the very end is ignored"
        );
        assert_eq!(s["shots"], 3);
        assert_eq!(s["cuts_per_min"], 12.0);
        let log = "[Parsed_showinfo_1 @ 0x1] n:0 pts:48 pts_time:2.002 duration:1\n[Parsed_showinfo_1 @ 0x1] n:1 pts_time:4.5 x";
        assert_eq!(parse_cuts(log), vec![2.002, 4.5]);
        // A showinfo line that lost its prefix inside an unfinished line of another filter still counts.
        let mixed = "[Parsed_ebur128_3 @ 0x2] t: 2.1  M: -20 n:   0 pts:  48 pts_time:3.0 duration:1\nsilence_start: 1";
        assert_eq!(parse_cuts(mixed), vec![3.0]);
    }
}
