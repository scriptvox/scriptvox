//! Mains hum and low rumble under a voice: found from the samples, removed by `edit --clean-voice`
//! (`edit-hum-rumble.md`). No dependency: a single-frequency measure per 1 s window.

use crate::engine_ffmpeg::Ffmpeg;
use serde_json::{json, Value};
use std::path::Path;

const RATE: f64 = crate::beats::RATE as f64;
/// Seconds of sound that are measured.
const MAX_S: f64 = 60.0;
const HARMONICS: usize = 8;
/// A harmonic stands this far above its neighbours, and above this level.
const OVER_DB: f64 = 12.0;
const MIN_DB: f64 = -75.0;
/// Once the mains frequency is known, a harmonic this far above its neighbours goes too: a voice around it hides
/// part of its height (13 Nov 2026: 150 Hz under a real voice stood 9 dB up and stayed in the result).
const ALSO_DB: f64 = 6.0;
/// Rumble: the band under both mains tones and under every voice, and the level from which it counts.
const RUMBLE_HZ: [f64; 8] = [18.0, 22.0, 26.0, 30.0, 34.0, 38.0, 42.0, 46.0];
const RUMBLE_DB: f64 = -50.0;
const NOTCH_HZ: f64 = 4.0;
const HIGHPASS_HZ: f64 = 60.0;

/// Tones as (hz, level in dB).
type Tones = Vec<(f64, f64)>;

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// 50 or 60 when hum was found, with the harmonics that stand out: (hz, level in dB).
    pub hum_hz: Option<f64>,
    pub harmonics: Tones,
    pub rumble: bool,
    pub rumble_db: f64,
}

/// The mean power of the `hz` sine over 1 s Hann windows: 0.5 for a full-scale sine.
fn power(pcm: &[f32], hz: f64) -> f64 {
    let n = RATE as usize;
    let w = std::f64::consts::TAU * hz / RATE;
    let (mut sum, mut count) = (0.0, 0);
    for seg in pcm.chunks_exact(n) {
        let (mut re, mut im, mut ws) = (0.0, 0.0, 0.0);
        for (i, x) in seg.iter().enumerate() {
            let win = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            let v = f64::from(*x) * win;
            re += v * (w * i as f64).cos();
            im += v * (w * i as f64).sin();
            ws += win;
        }
        sum += 2.0 * (re * re + im * im) / (ws * ws);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

fn db(power: f64) -> f64 {
    10.0 * power.max(1e-12).log10()
}

/// The level of the `hz` tone in dB (a full-scale sine is -3).
pub fn tone_db(pcm: &[f32], hz: f64) -> f64 {
    db(power(pcm, hz))
}

/// The level of the rumble band in dB. Each reading sees 1.5 Hz of noise (a Hann window of 1 s) and stands for
/// the 4 Hz to the next one.
pub fn rumble_db(pcm: &[f32]) -> f64 {
    db(RUMBLE_HZ.iter().map(|f| power(pcm, *f)).sum::<f64>() * 4.0 / 1.5)
}

/// How far the tone stands above the median of its neighbours, 4 to 8 Hz away on each side.
fn height(pcm: &[f32], hz: f64) -> (f64, f64) {
    let level = tone_db(pcm, hz);
    let mut near: Vec<f64> = (4..=8)
        .flat_map(|d| [hz - f64::from(d), hz + f64::from(d)])
        .map(|f| tone_db(pcm, f))
        .collect();
    near.sort_by(f64::total_cmp);
    (level, level - (near[4] + near[5]) / 2.0)
}

/// What stands in the sound. Under 2 s: nothing is found.
pub fn find(pcm: &[f32]) -> Report {
    // (mains frequency, score, harmonics to remove)
    let mut best: Option<(f64, f64, Tones)> = None;
    if pcm.len() >= 2 * RATE as usize {
        for base in [50.0, 60.0] {
            let all: Vec<(usize, f64, f64)> = (1..=HARMONICS)
                .map(|k| {
                    let (level, over) = height(pcm, base * k as f64);
                    (k, level, over)
                })
                .filter(|h| h.1 > MIN_DB)
                .collect();
            // 300 Hz is a harmonic of both.
            let own: Vec<_> = all
                .iter()
                .filter(|h| h.2 >= OVER_DB && (base * h.0 as f64 - 300.0).abs() > 0.5)
                .collect();
            if !(own.iter().any(|h| h.0 == 1) || own.len() >= 2) {
                continue;
            }
            let score: f64 = own.iter().map(|h| h.2).sum();
            if best.as_ref().is_none_or(|b| score > b.1) {
                let hs = all
                    .iter()
                    .filter(|h| h.2 >= ALSO_DB)
                    .map(|h| (base * h.0 as f64, h.1))
                    .collect();
                best = Some((base, score, hs));
            }
        }
    }
    let rumble_db = rumble_db(pcm);
    let (hum_hz, harmonics) = best.map_or((None, vec![]), |b| (Some(b.0), b.2));
    Report {
        hum_hz,
        harmonics,
        rumble: pcm.len() >= 2 * RATE as usize && rumble_db >= RUMBLE_DB,
        rumble_db,
    }
}

/// The samples that `find` reads. None: no sound FFmpeg can read.
pub fn read(ff: &Ffmpeg, file: &Path) -> Option<Vec<f32>> {
    crate::beats::read(ff, file, MAX_S).ok()
}

impl Report {
    /// The filters that remove what was found, for the front of the sound path.
    pub fn filters(&self) -> Vec<String> {
        let mut f: Vec<String> = Vec::new();
        if self.rumble {
            // Two passes of 12 dB an octave.
            f.extend(vec![format!("highpass=f={HIGHPASS_HZ}"); 2]);
        }
        f.extend(
            self.harmonics
                .iter()
                .map(|h| format!("bandreject=f={}:width_type=h:w={NOTCH_HZ}", h.0)),
        );
        f
    }

    /// The `hum` and `rumble` parts of the result. `after`: the samples of the result.
    pub fn result(&self, after: Option<&[f32]>) -> (Value, Value) {
        let r = |v: f64| (v * 10.0).round() / 10.0;
        let hum = match self.hum_hz {
            None => json!({"found": false}),
            Some(hz) => json!({
                "found": true, "hz": hz,
                "removed_hz": self.harmonics.iter().map(|h| h.0).collect::<Vec<_>>(),
                "db_before": self.harmonics.iter().map(|h| r(h.1)).collect::<Vec<_>>(),
                "db_after": after.map(|p| self.harmonics.iter().map(|h| r(tone_db(p, h.0))).collect::<Vec<_>>()),
            }),
        };
        let rumble = json!({"found": self.rumble, "db_before": r(self.rumble_db),
            "db_after": after.map(|p| r(rumble_db(p)))});
        (hum, rumble)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic noise, -1 to 1.
    fn noise(n: usize) -> Vec<f64> {
        let mut s = 0x2545_f491_4f6c_dd1du64;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 11) as f64 / (1u64 << 52) as f64 - 1.0
            })
            .collect()
    }

    /// 6 s: a "voice" (a tone that moves between 110 and 140 Hz, and 880 Hz), soft noise, and the given tones.
    fn sound(tones: &[(f64, f64)]) -> Vec<f32> {
        let n = 6 * RATE as usize;
        let ns = noise(n);
        let mut phase = 0.0f64;
        (0..n)
            .map(|i| {
                let t = i as f64 / RATE;
                phase += std::f64::consts::TAU * (125.0 + 15.0 * (t * 2.3).sin()) / RATE;
                let voice = 0.2 * phase.sin() + 0.1 * (std::f64::consts::TAU * 880.0 * t).sin();
                let hum: f64 = tones
                    .iter()
                    .map(|(hz, a)| a * (std::f64::consts::TAU * hz * t).sin())
                    .sum();
                (voice + 0.003 * ns[i] + hum) as f32
            })
            .collect()
    }

    #[test]
    fn hum_at_50_is_found_with_its_harmonics_and_each_gets_a_notch() {
        let r = find(&sound(&[(50.0, 0.05), (100.0, 0.02), (150.0, 0.01)]));
        assert_eq!(r.hum_hz, Some(50.0), "{r:?}");
        let hz: Vec<f64> = r.harmonics.iter().map(|h| h.0).collect();
        assert_eq!(hz, vec![50.0, 100.0, 150.0], "{r:?}");
        // 0.05 of full scale is -26 dB as a peak, -29 as a level.
        assert!((r.harmonics[0].1 + 29.0).abs() < 1.0, "{r:?}");
        assert!(!r.rumble, "{r:?}");
        assert_eq!(
            r.filters(),
            vec![
                "bandreject=f=50:width_type=h:w=4",
                "bandreject=f=100:width_type=h:w=4",
                "bandreject=f=150:width_type=h:w=4"
            ]
        );
        let (hum, rumble) = r.result(None);
        assert_eq!(hum["hz"], 50.0);
        assert_eq!(rumble["found"], false);
    }

    #[test]
    fn hum_at_60_is_60_and_300_alone_decides_nothing() {
        let r = find(&sound(&[(60.0, 0.03), (180.0, 0.02), (300.0, 0.02)]));
        assert_eq!(r.hum_hz, Some(60.0), "{r:?}");
        let hz: Vec<f64> = r.harmonics.iter().map(|h| h.0).collect();
        assert_eq!(hz, vec![60.0, 180.0, 300.0], "{r:?}");
        // 300 Hz does not count: alone, or with one tone more, it is not hum. Two tones of one mains are.
        assert_eq!(find(&sound(&[(300.0, 0.05)])).hum_hz, None);
        assert_eq!(find(&sound(&[(300.0, 0.05), (240.0, 0.05)])).hum_hz, None);
        assert_eq!(
            find(&sound(&[(180.0, 0.05), (240.0, 0.05)])).hum_hz,
            Some(60.0)
        );
        assert_eq!(
            find(&sound(&[(150.0, 0.05), (250.0, 0.05)])).hum_hz,
            Some(50.0)
        );
    }

    #[test]
    fn a_clean_voice_has_no_hum_and_no_rumble_and_no_filter() {
        let r = find(&sound(&[]));
        assert_eq!(r.hum_hz, None, "{r:?}");
        assert!(!r.rumble && r.rumble_db < -60.0, "{r:?}");
        assert!(r.filters().is_empty());
        // A weak harmonic goes with a hum that is there, and is nothing alone.
        let weak = (150.0, 0.0012);
        assert_eq!(find(&sound(&[weak])).hum_hz, None);
        let r = find(&sound(&[(50.0, 0.05), weak]));
        assert_eq!(
            r.harmonics.iter().map(|h| h.0).collect::<Vec<_>>(),
            vec![50.0, 150.0],
            "{r:?}"
        );
        // Too short to say.
        assert_eq!(find(&sound(&[(50.0, 0.1)])[..11025]).hum_hz, None);
    }

    #[test]
    fn rumble_is_found_from_its_level_and_gets_a_high_pass() {
        // Two slow tones at 23 and 37 Hz, as a hand on the microphone.
        let r = find(&sound(&[(23.0, 0.02), (37.0, 0.02)]));
        assert!(r.rumble, "{r:?}");
        assert!(r.rumble_db > -40.0 && r.rumble_db < -25.0, "{r:?}");
        assert_eq!(r.hum_hz, None, "{r:?}");
        assert_eq!(r.filters(), vec!["highpass=f=60", "highpass=f=60"]);
    }
}
