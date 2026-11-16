// Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. Licensed under Apache-2.0.
// Changed for ScriptVox: see docs/provenance/kokoro-rust.md.

//! Kokoro-82M neural voices (feature `kokoro`): pure-Rust inference on candle (CPU).
//!
//! Implemented from the papers the model card cites (StyleTTS 2, iSTFTNet; with ALBERT, HiFi-GAN,
//! the neural source-filter model and the Snake activation they build on), the released
//! `config.json` and the tensor names and shapes in the checkpoint. No reference code was read.
//! Weights (Apache-2.0, hexgrad) are downloaded on first use and never bundled.
//!
//! [`Kokoro::speak`] takes Kokoro phonemes (from the text front end), at most
//! [`MAX_PHONEMES`] per call, and a voice pack.

mod dsp;
mod layers;
mod model;
mod weights;

use std::path::Path;

use crate::TtsError;

pub use model::SAMPLE_RATE as KOKORO_RATE;
pub use weights::{WEIGHTS_SHA256, WEIGHTS_SIZE};

/// Longest phoneme string one forward pass takes (the position table has 512 rows, two of them
/// for the boundary tokens).
pub const MAX_PHONEMES: usize = 510;

/// A loaded Kokoro model.
pub struct Kokoro {
    model: model::Model,
    vocab: std::collections::HashMap<char, u32>,
}

/// A loaded voice pack.
pub struct VoicePack(Vec<f32>);

impl VoicePack {
    pub fn load(path: &Path) -> Result<VoicePack, TtsError> {
        weights::read_voice(path).map(VoicePack)
    }
}

impl Kokoro {
    /// Load the config and the checkpoint (`kokoro-v1_0.pth`).
    pub fn load(config: &Path, checkpoint: &Path) -> Result<Kokoro, TtsError> {
        let cfg = weights::read_config(config)?;
        let w = weights::Weights::load(checkpoint)?;
        Ok(Kokoro {
            model: model::Model::load(&w)?,
            vocab: cfg.vocab,
        })
    }

    /// Phoneme ids of `phonemes`; symbols outside the vocabulary are dropped.
    pub fn ids(&self, phonemes: &str) -> Vec<u32> {
        phonemes
            .chars()
            .filter_map(|c| self.vocab.get(&c).copied())
            .collect()
    }

    /// Speak `phonemes` with `voice` at `speed` (0.5–2). Deterministic for a given `seed`.
    pub fn speak(
        &self,
        phonemes: &str,
        voice: &VoicePack,
        speed: f64,
        seed: u64,
    ) -> Result<Vec<f32>, TtsError> {
        if !(0.5..=2.0).contains(&speed) {
            return Err(TtsError::Invalid("speed must be between 0.5 and 2".into()));
        }
        let ids = self.ids(phonemes);
        if ids.is_empty() {
            return Err(TtsError::Empty);
        }
        if ids.len() > MAX_PHONEMES {
            return Err(TtsError::Invalid(format!(
                "at most {MAX_PHONEMES} phonemes per pass"
            )));
        }
        let style = weights::style_for(&voice.0, ids.len(), self.model.device())?;
        let out = self.model.forward(&ids, &style, speed, seed)?;
        if out.iter().any(|s| !s.is_finite()) {
            return Err(TtsError::Model("the model produced invalid samples".into()));
        }
        Ok(out)
    }
}
