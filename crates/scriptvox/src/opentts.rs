//! Open voices on self-hosted GPUs inside `make` (P3-002): a `tts` row speaks the text in sentence chunks, FFmpeg joins
//! them, an `stt` row times the joined audio, and the script's own words are laid on those times. Captions always
//! show the script, never what the recogniser heard.

use crate::captions::Word;
use crate::engine_ffmpeg::Ffmpeg;
use crate::generators::{GenInput, Generator};
use crate::rpc::RpcError;
use std::path::{Path, PathBuf};

/// Bumped when the chunk, trim or alignment steps change, so cached voices are made again.
pub const VERSION: &str = "open-v3";
const PROVIDER_REFUSED: i64 = -32010;
const PROVIDER_RETRYABLE: i64 = -32011;
/// Characters per synthesis call: long inputs make the model drift or drop words.
const CHUNK: usize = 220;

fn rpc(e: crate::genjob::JobErr) -> RpcError {
    let code = match e.exit {
        crate::exit::Exit::ProviderRetryable | crate::exit::Exit::StillRunning => {
            PROVIDER_RETRYABLE
        }
        _ => PROVIDER_REFUSED,
    };
    RpcError {
        code,
        message: format!("{} ({})", e.message, e.fix),
    }
}

/// Splits a text into chunks of whole sentences, each at most `CHUNK` characters when the sentences allow it.
pub fn chunks(text: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?' | '؟' | '\n') {
            if !cur.trim().is_empty() {
                sentences.push(cur.trim().to_string());
            }
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        sentences.push(cur.trim().to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for s in sentences {
        match out.last_mut() {
            Some(last) if last.chars().count() + 1 + s.chars().count() <= CHUNK => {
                last.push(' ');
                last.push_str(&s);
            }
            _ => out.push(s),
        }
    }
    out
}

/// Lays the script words on the recogniser's timeline by character position: word k of the script gets the time
/// at the same fraction of characters in the timed words. Exact when the texts match, close when a few words differ.
/// ponytail: proportional alignment; a DTW on letters would fix runs of misheard words.
pub fn align(script: &[&str], timed: &[Word]) -> Vec<Word> {
    let total_t: usize = timed.iter().map(|w| w.text.chars().count().max(1)).sum();
    let total_s: usize = script.iter().map(|w| w.chars().count().max(1)).sum();
    if timed.is_empty() || total_s == 0 {
        return vec![];
    }
    // time at a character fraction f in [0, 1] of the timed words
    let at = |f: f64| -> f64 {
        let target = f * total_t as f64;
        let mut acc = 0.0;
        for w in timed {
            let n = w.text.chars().count().max(1) as f64;
            if acc + n >= target {
                return w.start + (w.end - w.start) * ((target - acc) / n).clamp(0.0, 1.0);
            }
            acc += n;
        }
        timed.last().map_or(0.0, |w| w.end)
    };
    let mut acc = 0usize;
    script
        .iter()
        .map(|s| {
            let n = s.chars().count().max(1);
            let (a, b) = (
                acc as f64 / total_s as f64,
                (acc + n) as f64 / total_s as f64,
            );
            acc += n;
            Word {
                text: s.to_string(),
                start: (at(a) * 1000.0).round() / 1000.0,
                end: (at(b) * 1000.0).round() / 1000.0,
            }
        })
        .collect()
}

/// A row that may be picked with nobody naming it. A row of unknown price needs a cap, and here nobody gave one. A
/// speech row on this machine stays: `base` is http only on loopback, so nothing leaves.
fn pickable(g: &Generator) -> bool {
    !g.unknown_price()
        || (g.gateway == crate::generators::OPENAI_SPEECH
            && g.base.as_deref().is_some_and(|b| b.starts_with("http://")))
}

pub(crate) fn rows(kind: &str) -> Option<Generator> {
    crate::generators::all()
        .into_iter()
        .filter(|g| {
            g.kind == kind
                && pickable(g)
                // No plan prices these rows: a row with a known price is never picked here. Unknown price is a local service.
                && (!g.paid() || g.unknown_price())
                && (g.gateway == "selfhost"
                    || (kind == "tts" && g.gateway == crate::generators::OPENAI_SPEECH))
        })
        // `--local-only`: no service row, so only the local speech to text row is left.
        .find(|g| g.has_key() && !crate::plan::local_only())
        .or_else(|| {
            (kind == "stt" && crate::provision::whisper_ready()).then(crate::generators::local_stt)
        })
}

/// The speech row for `edit`, `clips`, `analyze` and `dub` (G141): a free row first, else the cheapest paid row with a key.
/// A paid one runs only after `stt_gate` passes. `make` keeps to the free rows: its own plan does not price this.
/// With `speakers`, only a row that tells speakers apart (G020).
pub(crate) fn stt_row(speakers: bool) -> Option<Generator> {
    let fits = |g: &Generator| !speakers || g.gives_speakers();
    rows("stt").filter(fits).or_else(|| {
        crate::generators::all()
            .into_iter()
            .filter(|g| {
                !crate::plan::local_only()
                    && g.kind == "stt"
                    && pickable(g)
                    && fits(g)
                    && crate::keys::resolve(&g.key).is_some()
            })
            .min_by(|x, y| x.cost(60.0).total_cmp(&y.cost(60.0)))
    })
}

/// Seconds and price of a 16 kHz mono 16-bit WAV on this row ($0 on a free row).
pub(crate) fn stt_price(g: &Generator, wav: &Path) -> (f64, f64) {
    // ponytail: length from the file size; the header (44 bytes, more with FFmpeg's LIST chunk) is under 0.05 s.
    let bytes = std::fs::metadata(wav).map_or(0, |m| m.len());
    let secs = (bytes as f64 / 32_000.0 - 0.05).ceil().max(0.0);
    (
        secs,
        if g.paid() {
            g.cost_for(secs, 0.0, 0)
        } else {
            0.0
        },
    )
}

/// A paid row hears this WAV only when `--approve` names its plan: the row, the audio's hash and the price.
pub(crate) fn stt_gate(g: &Generator, wav: &Path) -> Result<(), String> {
    let (secs, cost) = stt_price(g, wav);
    let audio = crate::store::hash_file(wav).unwrap_or_default();
    let id = crate::plan::plan_id(
        &serde_json::json!({"command": "stt", "row": g.name, "audio": audio, "usd_per_s": g.usd_per_s}),
    );
    match crate::plan::needs_approval(&id, cost, crate::plan::approved()) {
        None => Ok(()),
        Some(_) => Err(format!(
            "approval needed: {} is a paid speech to text row and would hear {secs:.0} s for ${cost:.2} ({}). It is an external paid API on your own key; the free speech rows (`scriptvox setup speech-local`) go first when they can do the job. plan_id {id}",
            g.name,
            g.cost_words(secs),
        )),
    }
}

/// Speaks `text` with the open voice and returns (audio file, word times). Job files stay in `out_dir`, so a rerun
/// reuses finished chunks.
pub fn speak(
    ff: &Ffmpeg,
    lang: &str,
    text: &str,
    out_dir: &Path,
    run: &str,
    reference: Option<&Path>,
) -> Result<(PathBuf, Vec<Word>), RpcError> {
    let refused = |m: String| RpcError {
        code: PROVIDER_REFUSED,
        message: m,
    };
    let tts = rows("tts").ok_or_else(|| {
        refused(
            "no open voice row with a key on this install: `scriptvox providers` lists what it has"
                .into(),
        )
    })?;
    let stt = rows("stt").ok_or_else(|| refused(crate::analyze::NO_STT.into()))?;
    let reference = match reference {
        Some(p) => Some(data_uri(p, "audio/wav").map_err(refused)?),
        None => None,
    };
    let mut wavs = Vec::new();
    let mut words: Vec<Word> = Vec::new();
    let mut t = 0.0;
    for (i, chunk) in chunks(text).iter().enumerate() {
        let input = tts.request(&GenInput {
            prompt: chunk.clone(),
            lang: Some(lang.to_string()),
            audio: reference.clone(),
            ..Default::default()
        });
        let (wav, _) = crate::genjob::run_clip(
            &tts,
            &input,
            false,
            Path::new("."),
            out_dir,
            &format!("tts_{i:02}"),
            run,
            0.0,
            600,
        )
        .map_err(rpc)?;
        // Time this chunk on its own, then cut anything the voice said after the script (models sometimes add
        // invented words at the end).
        let timed = transcribe(
            &stt,
            &wav,
            Some(lang),
            out_dir,
            &format!("stt_{i:02}"),
            run,
            None,
        )?;
        let script: Vec<&str> = chunk.split_whitespace().collect();
        let keep = spoken_prefix(&script, &timed);
        let total = ff.duration(&wav).map_err(refused)?;
        let cut = keep.last().map_or(total, |w| (w.end + 0.15).min(total));
        let trimmed = out_dir.join(format!("tts_{i:02}.trim.wav"));
        ff.run(
            out_dir,
            &[
                "-i".into(),
                wav.to_string_lossy().into_owned(),
                "-af".into(),
                format!("atrim=0:{cut:.3},asetpts=N/SR/TB"),
                "-ac".into(),
                "1".into(),
                "-ar".into(),
                "24000".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                trimmed.to_string_lossy().into_owned(),
            ],
            None,
        )
        .map_err(refused)?;
        words.extend(align(&script, keep).into_iter().map(|w| Word {
            start: w.start + t,
            end: w.end + t,
            ..w
        }));
        t += cut;
        wavs.push(trimmed);
    }
    // Not "voice.wav": make converts every voice to that name in the same folder.
    let joined = out_dir.join("open_voice.wav");
    let mut args: Vec<String> = Vec::new();
    for w in &wavs {
        args.extend(["-i".into(), w.to_string_lossy().into_owned()]);
    }
    let inputs: String = (0..wavs.len()).map(|i| format!("[{i}:a]")).collect();
    args.extend([
        "-filter_complex".into(),
        format!("{inputs}concat=n={}:v=0:a=1[a]", wavs.len()),
        "-map".into(),
        "[a]".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        joined.to_string_lossy().into_owned(),
    ]);
    ff.run(out_dir, &args, None).map_err(refused)?;
    if words.is_empty() {
        return Err(RpcError {
            code: PROVIDER_RETRYABLE,
            message: "the recogniser found no words in the open voice audio".into(),
        });
    }
    Ok((joined, words))
}

/// Letters only (no punctuation, no Arabic short-vowel marks), to compare script and transcript lengths.
fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphanumeric()).count()
}

/// The timed words that cover the script: words up to the one where the transcript has as many letters as the
/// script (one extra word of slack for spelling). Words after it are speech the script never asked for.
/// A unit or number read out ("km/h" heard as "kilometres per hour") has more letters than the script, so the count
/// ends early: when the script's last word comes a few words later, the cut moves to it (4 Nov 2026, "4 seconds" lost).
pub fn spoken_prefix<'a>(script: &[&str], timed: &'a [Word]) -> &'a [Word] {
    let want: usize = script.iter().map(|w| letters(w)).sum();
    let mut acc = 0;
    for (i, w) in timed.iter().enumerate() {
        acc += letters(&w.text);
        if acc as f64 >= want as f64 * 0.97 {
            let last = script
                .last()
                .map(|w| crate::gates::norm_word(w))
                .unwrap_or_default();
            // ponytail: 8 words of reach; a longer read-out unit keeps the old early cut.
            let end = timed[i..]
                .iter()
                .take(9)
                .position(|t| !last.is_empty() && crate::gates::norm_word(&t.text) == last)
                .map_or(i, |k| i + k);
            return &timed[..=end];
        }
    }
    timed
}

/// Word times of a WAV from the speech to text row; `lang` None lets the row detect the language.
pub(crate) fn transcribe(
    stt: &Generator,
    wav: &Path,
    lang: Option<&str>,
    dir: &Path,
    shot: &str,
    run: &str,
    prompt: Option<&str>,
) -> Result<Vec<Word>, RpcError> {
    let refused = |m: String| RpcError {
        code: PROVIDER_REFUSED,
        message: m,
    };
    let input = stt.request(&GenInput {
        audio: Some(data_uri(wav, "audio/wav").map_err(refused)?),
        lang: lang.map(String::from),
        prompt: prompt.unwrap_or_default().into(),
        ..Default::default()
    });
    // A paid row's cap is its price for this audio; `stt_gate` asked for approval before.
    let cost = stt_price(stt, wav).1;
    let (doc, _) = crate::genjob::run_priced(
        stt,
        &input,
        false,
        Path::new("."),
        dir,
        shot,
        run,
        cost,
        600,
        (cost > 0.0).then_some(cost),
    )
    .map_err(rpc)?;
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&doc).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(format!("transcript: {e}")))?;
    Ok(crate::analyze::stt_words(&v)
        .iter()
        .filter_map(|w| {
            Some(Word {
                text: w["text"].as_str()?.to_string(),
                start: w["start"].as_f64()?,
                end: w["end"].as_f64()?,
            })
        })
        .collect())
}

fn data_uri(p: &Path, mime: &str) -> Result<String, String> {
    use base64::Engine;
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(b)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(t: &str, s: f64, e: f64) -> Word {
        Word {
            text: t.into(),
            start: s,
            end: e,
        }
    }

    #[test]
    fn an_invented_tail_is_cut() {
        let timed = [
            w("صوت", 0.0, 0.4),
            w("مفتوح.", 0.4, 0.9),
            w("ألاك", 1.2, 1.6),
            w("تحير", 1.6, 2.0),
        ];
        let keep = spoken_prefix(&["صوت", "مفتوح."], &timed);
        assert_eq!(keep.len(), 2);
        assert_eq!(
            spoken_prefix(&["صوت", "مفتوح", "جدا"], &timed[..2]).len(),
            2,
            "short transcript: keep all"
        );
        // "km/h" read out as three words: the letter count ends at "gap", the last script word comes later.
        let script = ["speed", "km/h", "so", "the", "gap", "4", "seconds."];
        let timed = [
            w("speed", 0.0, 0.3),
            w("kilometres", 0.3, 0.8),
            w("per", 0.8, 0.9),
            w("hour", 0.9, 1.1),
            w("so", 1.1, 1.2),
            w("the", 1.2, 1.3),
            w("gap", 1.3, 1.5),
            w("4", 1.5, 1.7),
            w("seconds.", 1.7, 2.1),
            w("invented", 2.3, 2.6),
        ];
        assert_eq!(spoken_prefix(&script, &timed).len(), 9);
    }

    #[test]
    fn chunks_keep_sentences_whole() {
        let c = chunks("One. Two! مرحبا؟ Last words");
        assert_eq!(c, vec!["One. Two! مرحبا؟ Last words"]);
        let long = "A sentence of some length here. ".repeat(12);
        let c = chunks(&long);
        assert!(
            c.len() > 1 && c.iter().all(|x| x.chars().count() <= CHUNK),
            "{c:?}"
        );
    }

    #[test]
    fn align_is_exact_when_texts_match_and_close_when_one_word_differs() {
        let timed = [
            w("في", 0.0, 0.2),
            w("أبوظبي", 0.2, 0.8),
            w("اليوم", 0.8, 1.2),
        ];
        let a = align(&["في", "أبوظبي", "اليوم"], &timed);
        assert_eq!((a[1].start, a[1].end), (0.2, 0.8));
        let b = align(&["في", "ابوظبي", "اليوم"], &timed);
        assert_eq!(b[1].text, "ابوظبي", "the script word is kept");
        assert!((b[1].start - 0.2).abs() < 0.05 && (b[2].end - 1.2).abs() < 1e-9);
    }
}
