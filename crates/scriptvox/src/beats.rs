//! `analyze --beats`: the beat grid of a file's sound (docs/specs/beats.md). FFmpeg decodes; the rest is here.
//! Method: energy rise per 10 ms, tempo by autocorrelation of that curve, then the phase with the most rise on the
//! grid. One steady tempo is assumed: a track that changes tempo gets a low `confidence`, not a wrong grid sold as right.
use crate::engine_ffmpeg::Ffmpeg;
use std::path::Path;
use std::process::Command;

/// Samples a second of the mono signal read for beats. Low on purpose: a beat is in the envelope, not in the treble.
pub const RATE: u32 = 11025;
const HOP: usize = 110; // 10 ms at RATE
const HOP_S: f64 = HOP as f64 / RATE as f64;
/// Tempo range searched, beats a minute.
const BPM: (f64, f64) = (60.0, 200.0);
/// How late the rise curve shows the start of a sound, in seconds.
const LATE_S: f64 = 0.035;
/// Under this the sound has no steady beat we would cut on (speech alone measures 0.05 to 0.15).
pub const STEADY: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct Beats {
    pub bpm: f64,
    /// 0 to 1: how much of the rise in the sound repeats at this tempo.
    pub confidence: f64,
    /// Beat times in seconds, on one grid.
    pub times: Vec<f64>,
}

/// Mono f32 samples of the first audio stream at `RATE`. At most `max_s` seconds are read.
pub fn read(ff: &Ffmpeg, input: &Path, max_s: f64) -> Result<Vec<f32>, String> {
    let out = Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error"])
        .args(crate::media_gate::input(input)?)
        .args(["-vn", "-ac", "1", "-ar", &RATE.to_string(), "-t"])
        .arg(format!("{max_s:.3}"))
        .args(["-f", "f32le", "-"])
        .output()
        .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect())
}

/// The rise of the energy per hop, above its local mean: one value per 10 ms, never negative.
///
/// The power is smoothed over about 30 ms sample by sample, starting at the level of the first 50 ms, and each hop
/// takes the mean of that curve, not one reading of it. A rise is measured against the value 5 hops (50 ms) before.
/// Each part answers a fault measured on the built-in beds on 10 Nov 2026:
/// - plain 10 ms windows: a pad of three sines "beat" at 113 BPM with confidence 0.91, because its tones interfere
///   faster than that;
/// - a smoother that starts at zero: one false rise at the start, larger than every real one;
/// - one reading per hop: the 100 BPM swell of `pulse` read as 97, because the ripple of its 55 Hz tone (110 Hz in
///   the power) and the 100 Hz hop made a slow pattern of their own.
fn rise(pcm: &[f32]) -> Vec<f64> {
    let sq = |s: &f32| (*s as f64) * (*s as f64);
    let alpha = 1.0 - (-1.0 / (RATE as f64 * 0.03)).exp();
    let head = &pcm[..pcm.len().min(HOP * 5)];
    let mut y = head.iter().map(sq).sum::<f64>() / head.len().max(1) as f64;
    let mut acc = 0.0f64;
    let mut energy = Vec::with_capacity(pcm.len() / HOP + 1);
    for (i, s) in pcm.iter().enumerate() {
        y += alpha * (sq(s) - y);
        acc += y;
        if (i + 1) % HOP == 0 {
            energy.push((acc / HOP as f64 + 1e-9).ln());
            acc = 0.0;
        }
    }
    const BACK: usize = 5;
    // A rise counts from 0.08 in log energy (about 8 percent louder in 50 ms). Less is the wobble of a steady tone, or
    // a slow swell of several seconds, which is not a beat.
    let flux: Vec<f64> = (0..energy.len())
        .map(|i| {
            if i < BACK {
                0.0
            } else {
                (energy[i] - energy[i - BACK] - 0.08).max(0.0)
            }
        })
        .collect();
    // Local mean over 1 s, so a loud part and a quiet part count alike.
    let half = 50usize;
    (0..flux.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(half), (i + half + 1).min(flux.len()));
            let mean = flux[a..b].iter().sum::<f64>() / (b - a) as f64;
            (flux[i] - mean).max(0.0)
        })
        .collect()
}

/// The beat grid of `pcm` (mono, at `RATE`). None: under 4 s of sound, or no rise at all (silence).
pub fn of(pcm: &[f32]) -> Option<Beats> {
    let r = rise(pcm);
    if r.len() < 400 {
        return None;
    }
    let zero: f64 = r.iter().map(|x| x * x).sum();
    if zero <= 1e-9 {
        return None;
    }
    let lag_of = |bpm: f64| 60.0 / bpm / HOP_S;
    let (lo, hi) = (
        lag_of(BPM.1).floor() as usize,
        lag_of(BPM.0).ceil() as usize,
    );
    let ac =
        |lag: usize| -> f64 { r.iter().zip(&r[lag..]).map(|(a, b)| a * b).sum::<f64>() / zero };
    // A tempo and its half and double all repeat: a mild pull to 120 picks the one a person would tap.
    let pull = |lag: usize| {
        let bpm = 60.0 / (lag as f64 * HOP_S);
        (-((bpm / 120.0).log2().powi(2)) / (2.0 * 0.9f64.powi(2))).exp()
    };
    let (lag, _) = (lo..=hi.min(r.len() / 2))
        .map(|l| (l, ac(l) * pull(l)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    // Finer than one hop: the mean of the three best neighbours, weighted.
    let (a, b, c) = (ac(lag - 1).max(0.0), ac(lag).max(0.0), ac(lag + 1).max(0.0));
    let fine = if a + b + c > 0.0 {
        ((lag - 1) as f64 * a + lag as f64 * b + (lag + 1) as f64 * c) / (a + b + c)
    } else {
        lag as f64
    };
    // The mean over the lag and its two neighbours: a real beat repeats at lags 10 ms apart too, and a lone spike
    // at one lag (measured 0.58 on the `calm` pad, with 0.05 and 0.02 beside it) is a pattern of the tones.
    let confidence = ((ac(lag - 1) + ac(lag) + ac(lag + 1)) / 3.0).clamp(0.0, 1.0);
    // Phase: the offset whose grid collects the most rise.
    let score = |off: f64| -> f64 {
        let mut s = 0.0;
        let mut t = off;
        while (t as usize) < r.len() {
            s += r[t as usize];
            t += fine;
        }
        s
    };
    let off = (0..lag)
        .map(|o| o as f64)
        .max_by(|x, y| score(*x).total_cmp(&score(*y)))?;
    let mut times = vec![];
    let mut t = off;
    while (t as usize) < r.len() {
        // The smoothing and the 50 ms step show a start about 35 ms late (measured on click tracks): take it off.
        let at = (t * HOP_S - LATE_S).max(0.0);
        times.push((at * 1000.0).round() / 1000.0);
        t += fine;
    }
    Some(Beats {
        bpm: ((60.0 / (fine * HOP_S)) * 10.0).round() / 10.0,
        confidence: (confidence * 100.0).round() / 100.0,
        times,
    })
}

/// `cut_on: beat` (`cut-on-beat.md`): a scene boundary moves to a beat at most this far away.
pub const SNAP_LIMIT_S: f64 = 0.25;
/// No move makes a scene shorter than this.
pub const SNAP_MIN_SCENE_S: f64 = 1.0;

/// The beat grid of a music bed for `cut_on: beat`. `times` is empty when the bed has no steady beat.
#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub bpm: Option<f64>,
    pub confidence: Option<f64>,
    pub times: Vec<f64>,
    /// The length of a music file that loops under a longer video: the grid starts again at each loop.
    pub loop_s: Option<f64>,
}

impl Grid {
    /// The grid of a built-in bed (`expr`, made for `len` seconds) or of a music file.
    pub fn of_bed(ff: &Ffmpeg, dir: &Path, music: &crate::audio::Music, len: f64) -> Grid {
        use crate::audio::Music;
        let built = dir.join("bed-beats.wav");
        let (file, loop_s) = match music {
            Music::None => return Grid::default(),
            Music::Bed(expr) => {
                let lavfi = format!("aevalsrc='{expr}':s=48000:d={:.3}", len.clamp(8.0, 600.0));
                let args = ["-f", "lavfi", "-i", &lavfi].map(String::from);
                let mut args = args.to_vec();
                args.push(built.to_string_lossy().into_owned());
                if ff.run(dir, &args, None).is_err() {
                    return Grid::default();
                }
                (built.as_path(), None)
            }
            Music::File(f) | Music::Base(f) => (
                f.as_path(),
                // A base video's sound plays once; a music file loops.
                ff.duration(f)
                    .ok()
                    .filter(|d| matches!(music, Music::File(_)) && *d < len),
            ),
        };
        let beats = read(ff, file, 600.0).ok().and_then(|pcm| of(&pcm));
        let _ = std::fs::remove_file(&built);
        match beats {
            Some(b) => Grid {
                bpm: Some(b.bpm),
                confidence: Some(b.confidence),
                times: if b.confidence >= STEADY {
                    b.times
                } else {
                    vec![]
                },
                loop_s,
            },
            None => Grid::default(),
        }
    }

    /// The beat nearest to video time `t`.
    fn nearest(&self, t: f64) -> Option<f64> {
        let (base, local) = match self.loop_s {
            Some(l) if l > 0.0 => ((t / l).floor() * l, t % l),
            _ => (0.0, t),
        };
        self.times
            .iter()
            .min_by(|a, b| (*a - local).abs().total_cmp(&(*b - local).abs()))
            .map(|b| base + b)
    }

    /// Scene starts moved to the grid, and the `cut_on` part of the result. `starts[0]` is the start of the video
    /// and never moves; `end` is where the last scene ends.
    pub fn snap(&self, starts: &[f64], end: f64) -> (Vec<f64>, serde_json::Value) {
        use serde_json::json;
        let r = |v: f64| (v * 1000.0).round() / 1000.0;
        let mut out = starts.to_vec();
        let mut rows = Vec::new();
        for i in 1..starts.len() {
            let t = starts[i];
            let next = starts.get(i + 1).copied().unwrap_or(end);
            let why = match self.nearest(t) {
                None => Some("the music has no steady beat"),
                Some(b) if (b - t).abs() > SNAP_LIMIT_S => Some("no beat within 0.25 s"),
                Some(b) if b - out[i - 1] < SNAP_MIN_SCENE_S || next - b < SNAP_MIN_SCENE_S => {
                    Some("a scene would be shorter than 1 s")
                }
                Some(b) => {
                    out[i] = b;
                    None
                }
            };
            rows.push(match why {
                Some(w) => json!({"scene": i + 1, "from": r(t), "stayed": w}),
                None => {
                    json!({"scene": i + 1, "from": r(t), "to": r(out[i]), "moved_s": r(out[i] - t)})
                }
            });
        }
        let moved = rows.iter().filter(|x| x.get("to").is_some()).count();
        let report = json!({
            "to": "beat", "bpm": self.bpm.map(|b| (b * 10.0).round() / 10.0),
            "confidence": self.confidence.map(|c| (c * 100.0).round() / 100.0),
            "limit_s": SNAP_LIMIT_S, "moved": moved, "stayed": rows.len() - moved, "boundaries": rows,
        });
        (out, report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Short clicks every `60 / bpm` s, the first at `first` s, with a quiet hum under them.
    fn clicks(bpm: f64, first: f64, secs: f64) -> Vec<f32> {
        let n = (secs * RATE as f64) as usize;
        let step = 60.0 / bpm;
        (0..n)
            .map(|i| {
                let t = i as f64 / RATE as f64;
                let since = (t - first).rem_euclid(step);
                let click = if t >= first && since < 0.03 {
                    (since * 2.0 * std::f64::consts::PI * 900.0).sin() * (1.0 - since / 0.03)
                } else {
                    0.0
                };
                (click * 0.8 + (t * 2.0 * std::f64::consts::PI * 110.0).sin() * 0.02) as f32
            })
            .collect()
    }

    #[test]
    fn a_click_track_gives_its_tempo_and_its_grid() {
        for (bpm, first) in [(120.0, 0.25), (90.0, 0.4), (150.0, 0.1)] {
            let b = of(&clicks(bpm, first, 16.0)).unwrap();
            assert!((b.bpm - bpm).abs() < 1.5, "{bpm}: {b:?}");
            assert!(b.confidence >= STEADY, "{bpm}: {}", b.confidence);
            // Every beat of the first 10 s is within 40 ms of a click: the tempo is known to about 0.3 BPM, so the grid drifts.
            let step = 60.0 / bpm;
            for t in b.times.iter().filter(|t| **t < 10.0) {
                let d = (t - first).rem_euclid(step);
                assert!(d.min(step - d) < 0.04, "{bpm}: beat at {t}");
            }
        }
    }

    /// The built-in `pulse` bed: a soft swell at 100 BPM, with no sharp start to any beat.
    #[test]
    fn a_soft_swell_gives_its_tempo() {
        let pi2 = 2.0 * std::f64::consts::PI;
        let pcm: Vec<f32> = (0..RATE as usize * 14)
            .map(|i| {
                let t = i as f64 / RATE as f64;
                (0.12 * (pi2 * 55.0 * t).sin() * (0.5 + 0.5 * (pi2 * 1.6667 * t).cos())
                    + 0.06 * ((pi2 * 220.0 * t).sin() + (pi2 * 329.63 * t).sin()))
                    as f32
            })
            .collect();
        let b = of(&pcm).unwrap();
        assert!((b.bpm - 100.0).abs() < 2.0, "{b:?}");
        assert!(b.confidence >= STEADY, "{}", b.confidence);
    }

    /// The built-in `calm` bed: three tones that breathe every 4 s. It has no beat.
    #[test]
    fn a_pad_that_breathes_has_no_steady_beat() {
        let pi2 = 2.0 * std::f64::consts::PI;
        let pcm: Vec<f32> = (0..RATE as usize * 20)
            .map(|i| {
                let t = i as f64 / RATE as f64;
                (0.10
                    * ((pi2 * 220.0 * t).sin()
                        + 0.8 * (pi2 * 261.63 * t).sin()
                        + 0.7 * (pi2 * 329.63 * t).sin())
                    * (0.65 + 0.35 * (pi2 * 0.25 * t).sin())) as f32
            })
            .collect();
        assert!(
            of(&pcm).is_none_or(|b| b.confidence < STEADY),
            "{:?}",
            of(&pcm).map(|b| (b.bpm, b.confidence))
        );
    }

    #[test]
    fn silence_and_a_short_sound_give_no_grid() {
        assert_eq!(of(&vec![0.0; RATE as usize * 8]), None);
        assert_eq!(of(&clicks(120.0, 0.0, 2.0)), None);
    }

    #[test]
    fn a_steady_tone_has_no_steady_beat() {
        let tone: Vec<f32> = (0..RATE as usize * 10)
            .map(|i| {
                ((i as f64 / RATE as f64) * 2.0 * std::f64::consts::PI * 220.0).sin() as f32 * 0.5
            })
            .collect();
        assert!(of(&tone).is_none_or(|b| b.confidence < STEADY));
    }

    fn grid(times: &[f64], loop_s: Option<f64>) -> Grid {
        Grid {
            bpm: Some(100.0),
            confidence: Some(0.6),
            times: times.to_vec(),
            loop_s,
        }
    }

    #[test]
    fn a_boundary_moves_to_a_beat_inside_the_limit_and_stays_outside_it() {
        let beats: Vec<f64> = (0..40).map(|k| f64::from(k) * 0.6).collect();
        // 2.3 is 0.1 from 2.4; 4.5 is 0.3 from both 4.2 and 4.8; 7.1 is 0.1 from 7.2.
        let (out, rep) = grid(&beats, None).snap(&[0.0, 2.3, 4.5, 7.1], 10.0);
        assert_eq!(out[0], 0.0);
        assert!(
            (out[1] - 2.4).abs() < 1e-9 && out[2] == 4.5 && (out[3] - 7.2).abs() < 1e-9,
            "{out:?}"
        );
        assert_eq!(rep["moved"], 2);
        assert_eq!(rep["stayed"], 1);
        assert_eq!(rep["boundaries"][0]["scene"], 2);
        assert_eq!(rep["boundaries"][0]["moved_s"], 0.1);
        assert_eq!(rep["boundaries"][1]["stayed"], "no beat within 0.25 s");
        assert_eq!(rep["limit_s"], 0.25);
        // No move is longer than the limit, and the order holds.
        for (a, b) in out.iter().zip([0.0, 2.3, 4.5, 7.1]) {
            assert!((a - b).abs() <= SNAP_LIMIT_S);
        }
        assert!(out.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn no_move_makes_a_short_scene_or_puts_two_boundaries_on_one_beat() {
        let beats: Vec<f64> = (0..40).map(|k| f64::from(k) * 0.6).collect();
        // 2.2 and 2.6 both look at 2.4: a move of one leaves 0.2 s to the other, so both stay.
        let (out, rep) = grid(&beats, None).snap(&[0.0, 2.2, 2.6, 6.1], 10.0);
        assert_eq!(out, vec![0.0, 2.2, 2.6, 6.0]);
        for i in [0, 1] {
            assert_eq!(
                rep["boundaries"][i]["stayed"],
                "a scene would be shorter than 1 s"
            );
        }
        // A move may use the room that the move before it left: 2.3 goes to 2.4, then 3.5 to 3.6 (1.2 s later).
        let (out, _) = grid(&beats, None).snap(&[0.0, 2.3, 3.5], 10.0);
        assert!(
            (out[1] - 2.4).abs() < 1e-9 && (out[2] - 3.6).abs() < 1e-9,
            "{out:?}"
        );
        // The last scene keeps 1 s too: 9.5 would go to 9.6, 0.4 s before the end.
        let (out, _) = grid(&beats, None).snap(&[0.0, 9.5], 10.0);
        assert_eq!(out[1], 9.5);
    }

    #[test]
    fn no_steady_beat_moves_nothing_and_a_looped_file_starts_its_grid_again() {
        let (out, rep) = Grid::default().snap(&[0.0, 2.3, 4.5], 8.0);
        assert_eq!(out, vec![0.0, 2.3, 4.5]);
        assert_eq!(rep["moved"], 0);
        assert_eq!(
            rep["boundaries"][0]["stayed"],
            "the music has no steady beat"
        );
        assert!(rep["bpm"].is_null());
        // A 5 s file with beats at 0.5, 1.5, ...: at 11.4 s of the video the file is at 1.4 s.
        let (out, _) = grid(&[0.5, 1.5, 2.5, 3.5, 4.5], Some(5.0)).snap(&[0.0, 11.4], 20.0);
        assert!((out[1] - 11.5).abs() < 1e-9, "{out:?}");
    }
}
