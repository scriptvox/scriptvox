//! An English voice in pure Rust: text normalizer, CMUdict plus letter-to-sound rules, and Kokoro-82M on candle (CPU).
//! From a pinned Apache-2.0 source, see `docs/provenance/kokoro-rust.md`. The weights are downloaded by `scriptvox setup voice-rust`.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

mod kokoro;
mod normalize;
mod phonemize;

use std::path::Path;

pub use kokoro::{KOKORO_RATE, Kokoro, VoicePack, WEIGHTS_SHA256, WEIGHTS_SIZE};
pub use normalize::Lang;
use phonemize::Chunk;
pub use phonemize::{Lexicon, Overrides, SYMBOLS};

/// Silence between sentences, ms.
const SENTENCE_GAP_MS: u32 = 120;
/// Longest narration, in samples at [`KOKORO_RATE`] (30 minutes).
const MAX_SAMPLES: usize = 30 * 60 * KOKORO_RATE as usize;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TtsError {
    #[error("the script has nothing to say")]
    Empty,
    #[error("the narration would be longer than 30 minutes")]
    AudioTooLong,
    #[error("{0}")]
    Invalid(String),
    #[error("voice model: {0}")]
    Model(String),
}

/// The model and its pronunciation dictionary, loaded once for a run.
pub struct Engine {
    model: Kokoro,
    lexicon: Lexicon,
}

impl Engine {
    pub fn load(config: &Path, weights: &Path, dictionary: &Path) -> Result<Engine, TtsError> {
        let model = Kokoro::load(config, weights)?;
        let bytes = std::fs::read(dictionary)
            .map_err(|e| TtsError::Model(format!("{}: {e}", dictionary.display())))?;
        let lexicon = Lexicon::parse_cmudict(&bytes)
            .map_err(|e| TtsError::Model(format!("{}: {e}", dictionary.display())))?;
        Ok(Engine { model, lexicon })
    }

    /// Mono samples at [`KOKORO_RATE`]. `pace` is 0.5 to 2 (1 is normal). The same input gives the same samples.
    pub fn speak(&self, text: &str, voice: &VoicePack, pace: f64) -> Result<Vec<f32>, TtsError> {
        self.speak_with(text, voice, pace, &Overrides::default())
    }

    /// [`Engine::speak`] with the user's pronunciations: a word and its respelling, or its phonemes between slashes.
    pub fn speak_with(
        &self,
        text: &str,
        voice: &VoicePack,
        pace: f64,
        overrides: &Overrides,
    ) -> Result<Vec<f32>, TtsError> {
        let chunks = phonemize::phonemize(text, Lang::EnUs, &self.lexicon, overrides)
            .map_err(|e| TtsError::Invalid(e.to_string()))?;
        let rate = KOKORO_RATE;
        let mut out: Vec<f32> = Vec::new();
        let mut spoke = false;
        for (i, c) in chunks.iter().enumerate() {
            match c {
                Chunk::Pause { millis } => {
                    let n = (u64::from(*millis) * u64::from(rate) / 1000) as usize;
                    out.resize(out.len().saturating_add(n), 0.0);
                }
                Chunk::Phonemes(p) => {
                    if self.model.ids(p).is_empty() {
                        continue;
                    }
                    if spoke {
                        out.resize(
                            out.len()
                                .saturating_add((SENTENCE_GAP_MS * rate / 1000) as usize),
                            0.0,
                        );
                    }
                    out.extend_from_slice(&self.model.speak(p, voice, pace, i as u64 + 1)?);
                    spoke = true;
                }
            }
            if out.len() > MAX_SAMPLES {
                return Err(TtsError::AudioTooLong);
            }
        }
        if !spoke {
            return Err(TtsError::Empty);
        }
        // Headroom: never clip.
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        if peak > 0.98 {
            let g = 0.98 / peak;
            out.iter_mut().for_each(|x| *x *= g);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dictionary comes first, unknown words go to letter-to-sound rules, and numbers are read as words.
    #[test]
    fn text_becomes_phonemes_without_any_model() {
        let lex = Lexicon::parse_cmudict(b"HELLO  HH AH0 L OW1\nWORLD  W ER1 L D\n").unwrap();
        let c = phonemize::phonemize(
            "Hello world. 3 zzyx",
            Lang::EnUs,
            &lex,
            &Overrides::default(),
        )
        .unwrap();
        let Chunk::Phonemes(a) = &c[0] else {
            panic!("{c:?}")
        };
        assert!(a.contains("wˈɜɹld") && a.ends_with('.'), "{a}");
        let all: String = c
            .iter()
            .filter_map(|x| {
                if let Chunk::Phonemes(p) = x {
                    Some(p.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            all.len() > a.len(),
            "the number and the unknown word are spoken: {all}"
        );
    }
}
