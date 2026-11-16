//! `tts.kokoro-rust`: the English voice in pure Rust, beside the Python plugin. CPU only, in this process.
//! The model files come through the pinned setup route (`scriptvox setup voice-rust`).

use std::path::{Path, PathBuf};

use scriptvox_kokoro::{Engine, VoicePack, KOKORO_RATE};

use crate::captions::Word;
use crate::make::reading_times;
use crate::pauses::Seg;
use crate::provision::{install, installed, pin};

pub const ENGINE: &str = "kokoro-rust";
pub const PINS: [&str; 5] = [
    "kokoro-rust-weights",
    "kokoro-rust-config",
    "kokoro-rust-dict",
    "kokoro-rust-voice-af_heart",
    "kokoro-rust-voice-am_michael",
];

fn file(name: &str) -> Option<PathBuf> {
    Some(installed(&pin(name)?)?.remove(0))
}

/// True when `setup voice-rust` has run.
pub fn ready() -> bool {
    PINS.iter().all(|n| file(n).is_some())
}

/// `setup voice-rust`: the pinned model files, each checked by its sha256.
pub fn install_files(progress: &dyn Fn(&str)) -> Result<(), String> {
    for n in PINS {
        install(&pin(n).ok_or(format!("no pin for {n}"))?, progress)?;
    }
    Ok(())
}

pub fn load() -> Result<Engine, String> {
    let f = |n: &str| {
        file(n).ok_or(format!(
            "{n} is not installed: run `scriptvox setup voice-rust`"
        ))
    };
    Engine::load(
        &f("kokoro-rust-config")?,
        &f("kokoro-rust-weights")?,
        &f("kokoro-rust-dict")?,
    )
    .map_err(|e| e.to_string())
}

/// An IPA form as the sounds this voice has: the American English set of the model. A sound it does not have is an
/// error that names it, so a term is never read with a part missing.
pub fn ipa(form: &str) -> Result<String, String> {
    // Two letters first: the model writes each of these sounds as one symbol.
    const PAIRS: [(&str, &str); 22] = [
        ("eɪ", "A"),
        ("aɪ", "I"),
        ("oʊ", "O"),
        ("əʊ", "O"),
        ("aʊ", "W"),
        ("ɔɪ", "Y"),
        ("tʃ", "ʧ"),
        ("dʒ", "ʤ"),
        ("ɝ", "ɜɹ"),
        ("g", "ɡ"),
        ("r", "ɹ"),
        ("ɒ", "ɑ"),
        ("a", "ɑ"),
        ("e", "ɛ"),
        ("o", "ɔ"),
        ("ɐ", "ə"),
        ("ɫ", "l"),
        ("ɾ", "d"),
        ("ː", ""),
        (".", ""),
        ("'", "ˈ"),
        ("͡", ""),
    ];
    let mut s = form.trim().to_string();
    for (from, to) in PAIRS {
        s = s.replace(from, to);
    }
    let known = scriptvox_kokoro::SYMBOLS;
    let bad: Vec<String> = s
        .chars()
        .filter(|c| !known.contains(*c) || ",.!?;:—…()\"".contains(*c))
        .map(|c| c.to_string())
        .collect();
    if !bad.is_empty() || s.trim().is_empty() {
        return Err(format!(
            "the IPA form /{form}/ has {} that the local voice cannot say: it has the sounds of American English only (ɑ æ ə ʌ ɔ ɛ ɚ ɜ ɪ ʊ i u, eɪ aɪ oʊ aʊ ɔɪ, b tʃ d ð f ɡ h dʒ k l m n ŋ p ɹ s ʃ t θ v w j z ʒ, and the stress marks ˈ ˌ)",
            if bad.is_empty() { "no sound".to_string() } else { bad.join(" ") }
        ));
    }
    Ok(s)
}

/// Speaks `text` to `voice.wav` in `out_dir`; the word times are the reading spread over the voiced part of the
/// audio (`timing: estimated`, as the model gives no word events). `sounds` is [`crate::say::ipa_words`]: a word of
/// the text and its IPA form.
pub fn speak(
    engine: &Engine,
    text: &str,
    id: &str,
    rate: Option<i32>,
    out_dir: &Path,
    sounds: &std::collections::BTreeMap<String, String>,
) -> Result<(PathBuf, Vec<Word>), String> {
    let name = format!("kokoro-rust-voice-{id}");
    let pack = file(&name).ok_or(format!(
        "voice {id} is not installed: run `scriptvox setup voice-rust`"
    ))?;
    let pack = VoicePack::load(&pack).map_err(|e| e.to_string())?;
    let pace = 1.0 + f64::from(rate.unwrap_or(0)) / 100.0;
    let mut said = scriptvox_kokoro::Overrides::default();
    for (word, form) in sounds {
        said.0.insert(word.clone(), format!("/{}/", ipa(form)?));
    }
    let samples = engine
        .speak_with(text, &pack, pace, &said)
        .map_err(|e| e.to_string())?;
    let audio = out_dir.join("voice.wav");
    std::fs::write(&audio, wav_mono16(&samples, KOKORO_RATE)).map_err(|e| e.to_string())?;
    let (from, to) = voiced(&samples, KOKORO_RATE);
    Ok((audio, spread_words(text, from, to)))
}

/// A sample under this level is silence: -50 dB, the level at which `make::synthesize` cuts the end of a part.
const QUIET: f32 = 0.00316;

/// The seconds from the first to the last sample that is not silence. The model gives about 0.3 s of silence
/// before the words and 0.6 s after them; a word time in that silence is after the cut end of the part, in the
/// next scene (13 Nov 2026: two caption pages on screen together).
fn voiced(samples: &[f32], rate: u32) -> (f64, f64) {
    let loud = |s: &f32| s.abs() > QUIET;
    let from = samples.iter().position(loud).unwrap_or(0);
    let to = samples
        .iter()
        .rposition(loud)
        .map_or(samples.len(), |i| i + 1);
    (from as f64 / f64::from(rate), to as f64 / f64::from(rate))
}

/// The script's relative word lengths, scaled to the span `from` to `to` in seconds.
fn spread_words(text: &str, from: f64, to: f64) -> Vec<Word> {
    let mut words = reading_times(&[Seg::Speech(text.to_string())], "en", None);
    let first = words.first().map_or(0.0, |w| w.start);
    let scale = (to - from) / (words.last().map_or(1.0, |w| w.end) - first).max(1e-3);
    for w in &mut words {
        w.start = from + (w.start - first) * scale;
        w.end = from + (w.end - first) * scale;
    }
    words
}

/// 16-bit mono PCM WAV.
fn wav_mono16(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipa_becomes_the_sounds_of_the_voice_or_names_what_it_cannot_say() {
        assert_eq!(ipa("ˈdoʊhə").unwrap(), "ˈdOhə");
        assert_eq!(ipa("tʃɑːrt").unwrap(), "ʧɑɹt");
        assert_eq!(ipa("ŋwiən").unwrap(), "ŋwiən");
        let e = ipa("xɑlid").unwrap_err();
        assert!(e.contains(" x ") && e.contains("American English"), "{e}");
        assert!(ipa("  ").is_err());
        assert!(ipa("ʁy").is_err());
    }

    #[test]
    fn words_fill_the_real_audio_length_in_order() {
        let w = spread_words("one two three", 0.0, 3.0);
        assert_eq!(w.len(), 3);
        assert!(w[0].start.abs() < 1e-9);
        assert!((w[2].end - 3.0).abs() < 1e-6, "{w:?}");
        assert!(w[0].end <= w[1].start + 1e-9);
    }

    /// The silence before and after the words holds no word: 1 s of sound in 2 s of samples.
    #[test]
    fn words_stay_inside_the_voiced_part() {
        let mut s = vec![0.0f32; 2000];
        s[500..1500].fill(0.5);
        let (from, to) = voiced(&s, 1000);
        assert_eq!((from, to), (0.5, 1.5));
        let w = spread_words("one two three", from, to);
        assert!(
            (w[0].start - 0.5).abs() < 1e-9 && (w[2].end - 1.5).abs() < 1e-6,
            "{w:?}"
        );
        // No sound at all: the whole clip, as before.
        assert_eq!(voiced(&[0.0; 1000], 1000), (0.0, 1.0));
    }

    #[test]
    fn wav_has_a_correct_header_length() {
        let b = wav_mono16(&[0.0, 1.0, -1.0], 24000);
        assert_eq!(b.len(), 44 + 6);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 42);
    }

    /// The weights are checked before their pickle is read: the crate's size and hash must equal the pin.
    #[test]
    fn crate_hash_matches_the_pin() {
        let p = pin("kokoro-rust-weights").unwrap();
        assert_eq!(p.sha256, scriptvox_kokoro::WEIGHTS_SHA256);
        assert_eq!(p.size, scriptvox_kokoro::WEIGHTS_SIZE);
    }
}
