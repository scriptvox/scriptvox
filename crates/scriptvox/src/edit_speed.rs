//! `edit --speed "12-15@0.5"`: speed over time ranges with an ease, pitch kept (`edit-speed-range.md`). One time
//! map, a staircase of constant speeds, drives the picture (`setpts`), the sound (`atempo` per step, each step set
//! to its exact sample count) and the caption files.

use serde::Serialize;

pub const MAX: usize = 8;
pub const MIN_S: f64 = 0.5;
/// The ease at each end of a range: seconds of the source, at most a quarter of the range.
pub const EASE_S: f64 = 0.4;
pub const EASE_STEPS: usize = 4;
const RATE: f64 = 48000.0;
/// Samples of the source that a step reads past its end, 0.25 s.
const MORE: u64 = 12000;
pub const FIX: &str =
    "Give seconds and a speed: --speed \"12-15@0.5\" (commas for more), or a plain number for the whole video.";

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Range {
    pub from: f64,
    pub to: f64,
    pub speed: f64,
}

/// A stretch of the source at one constant speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub from: f64,
    pub to: f64,
    pub speed: f64,
}

/// `"12-15@0.5,20-24@2"` to ranges. `total`: the length they must fit in (infinite: only the form is checked).
pub fn parse(spec: &str, total: f64) -> Result<Vec<Range>, String> {
    let mut out: Vec<Range> = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || format!("--speed {part}: not FROM-TO@SPEED");
        let (span, speed) = part.split_once('@').ok_or_else(bad)?;
        let (from, to) = span.split_once('-').ok_or_else(bad)?;
        let num = |t: &str| t.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let (Some(from), Some(to), Some(speed)) = (num(from), num(to), num(speed)) else {
            return Err(bad());
        };
        if !(0.5..=2.0).contains(&speed) {
            return Err(format!("--speed {part}: the speed is 0.5 to 2"));
        }
        if from < 0.0 || to - from < MIN_S {
            return Err(format!(
                "--speed {part}: a range starts at 0 or later and is at least {MIN_S} s long"
            ));
        }
        if out.last().is_some_and(|l| from < l.to) {
            return Err(format!(
                "--speed {part}: the ranges are in time order and do not overlap"
            ));
        }
        // A frame of slack: a length read from a file is not exact.
        if to > total + 0.05 {
            return Err(format!(
                "--speed {part}: the range ends after the video ({total:.2} s)"
            ));
        }
        out.push(Range {
            from,
            to: to.min(total),
            speed,
        });
    }
    if out.is_empty() {
        return Err(format!("--speed {spec}: not FROM-TO@SPEED"));
    }
    if out.len() > MAX {
        return Err(format!(
            "{} speed ranges, and one call takes at most {MAX}",
            out.len()
        ));
    }
    Ok(out)
}

/// The staircase: for each range, `EASE_STEPS` steps up to its speed, the body, and the same steps back.
pub fn steps(ranges: &[Range]) -> Vec<Step> {
    let mut out = Vec::new();
    for r in ranges {
        let ease = EASE_S.min((r.to - r.from) / 4.0);
        let d = ease / EASE_STEPS as f64;
        let at = |j: usize| r.speed.powf(j as f64 / (EASE_STEPS + 1) as f64);
        for j in 1..=EASE_STEPS {
            let from = r.from + d * (j - 1) as f64;
            out.push(Step {
                from,
                to: from + d,
                speed: at(j),
            });
        }
        out.push(Step {
            from: r.from + ease,
            to: r.to - ease,
            speed: r.speed,
        });
        for j in (1..=EASE_STEPS).rev() {
            let to = r.to - d * (j - 1) as f64;
            out.push(Step {
                from: to - d,
                to,
                speed: at(j),
            });
        }
    }
    out
}

/// The time in the result of source time `t`.
pub fn map(steps: &[Step], t: f64) -> f64 {
    t + steps
        .iter()
        .map(|s| (1.0 / s.speed - 1.0) * (t - s.from).clamp(0.0, s.to - s.from))
        .sum::<f64>()
}

/// The picture filter: the same map on each frame's time. The caller puts `fps` after it.
pub fn setpts(steps: &[Step]) -> String {
    let terms: String = steps
        .iter()
        .map(|s| {
            format!(
                "{:+.6}*clip(T-{:.6},0,{:.6})",
                1.0 / s.speed - 1.0,
                s.from,
                s.to - s.from
            )
        })
        .collect();
    format!("setpts='(T{terms})/TB'")
}

/// The sound filter, one input and one output, for an `-af` chain: the sound at 48 kHz is cut at the step edges by
/// sample number, each step goes through `atempo` and is padded or cut to its exact length, and the parts are joined.
pub fn sound(steps: &[Step]) -> String {
    let n = |t: f64| (t * RATE).round() as u64;
    // (first sample, end sample or the end of the sound, speed)
    let mut parts: Vec<(u64, Option<u64>, f64)> = Vec::new();
    let mut at = 0u64;
    for s in steps {
        let (from, to) = (n(s.from), n(s.to));
        if from > at {
            parts.push((at, Some(from), 1.0));
        }
        parts.push((from, Some(to), s.speed));
        at = to;
    }
    parts.push((at, None, 1.0));
    let k = parts.len();
    let ins: String = (0..k).map(|i| format!("[sp{i}]")).collect();
    let outs: String = (0..k).map(|i| format!("[sq{i}]")).collect();
    let mut g = vec![format!("aresample={RATE},asplit={k}{ins}")];
    for (i, (from, to, speed)) in parts.iter().enumerate() {
        // A step with a new speed reads `MORE` samples past its end: `atempo` gives its last sound late or not at
        // all, and the cut to the exact length must cut real sound, not the padding (13 Nov 2026: at 0.5 each step
        // ended in a dip of 5 to 10 dB, and two of them in 10 ms of silence).
        let cut = match to {
            Some(to) if *speed != 1.0 => {
                format!("atrim=start_sample={from}:end_sample={}", to + MORE)
            }
            Some(to) => format!("atrim=start_sample={from}:end_sample={to}"),
            None => format!("atrim=start_sample={from}"),
        };
        let tempo = match to {
            Some(to) if *speed != 1.0 => {
                let len = ((to - from) as f64 / speed).round() as u64;
                format!(",atempo={speed:.6},asetpts=N/SR/TB,apad,atrim=end_sample={len},asetpts=N/SR/TB")
            }
            _ => String::new(),
        };
        g.push(format!("[sp{i}]{cut},asetpts=N/SR/TB{tempo}[sq{i}]"));
    }
    g.push(format!("{outs}concat=n={k}:v=0:a=1"));
    g.join(";")
}

fn clock(t: f64, sep: char) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{sep}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

fn seconds(t: &str) -> Option<f64> {
    let mut p: Vec<&str> = t.trim().split(':').collect();
    let s: f64 = p.pop()?.replace(',', ".").parse().ok()?;
    let m: f64 = p.pop()?.parse().ok()?;
    let h: f64 = p.pop().map_or(Some(0.0), |h| h.parse().ok())?;
    p.is_empty().then_some(h * 3600.0 + m * 60.0 + s)
}

/// An SRT or VTT text with every cue time moved through the map. Other lines stay as they are.
pub fn retime(text: &str, steps: &[Step]) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let moved = body.split_once(" --> ").and_then(|(a, rest)| {
            // VTT may have cue settings after the end time.
            let (b, tail) = rest.split_once(' ').map_or((rest, ""), |(b, t)| (b, t));
            let sep = if a.contains(',') { ',' } else { '.' };
            let (a, b) = (seconds(a)?, seconds(b)?);
            let tail = if tail.is_empty() {
                String::new()
            } else {
                format!(" {tail}")
            };
            Some(format!(
                "{} --> {}{tail}",
                clock(map(steps, a), sep),
                clock(map(steps, b), sep)
            ))
        });
        match moved {
            Some(m) => {
                out.push_str(&m);
                out.push_str(&line[body.len()..]);
            }
            None => out.push_str(line),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_parses_and_a_bad_one_names_itself() {
        let r = parse("12-15@0.5, 20-24@2", 30.0).unwrap();
        assert_eq!(
            r,
            vec![
                Range {
                    from: 12.0,
                    to: 15.0,
                    speed: 0.5
                },
                Range {
                    from: 20.0,
                    to: 24.0,
                    speed: 2.0
                }
            ]
        );
        for (bad, says) in [
            ("abc", "not FROM-TO@SPEED"),
            ("12-15", "not FROM-TO@SPEED"),
            ("12-15@3", "the speed is 0.5 to 2"),
            ("12-12.2@0.5", "at least 0.5 s"),
            ("12-15@0.5,14-18@2", "do not overlap"),
            ("12-40@0.5", "ends after the video (30.00 s)"),
        ] {
            let e = parse(bad, 30.0).unwrap_err();
            assert!(e.contains(says), "{bad}: {e}");
        }
        assert!(parse(&["1-2@2"; 9].join(","), f64::INFINITY).is_err());
    }

    #[test]
    fn the_map_is_the_sum_of_the_steps_and_never_goes_back() {
        let st = steps(&[Range {
            from: 2.0,
            to: 5.0,
            speed: 0.5,
        }]);
        assert_eq!(st.len(), 2 * EASE_STEPS + 1);
        // The steps tile the range.
        assert_eq!(st[0].from, 2.0);
        assert!((st.last().unwrap().to - 5.0).abs() < 1e-9);
        for w in st.windows(2) {
            assert!((w[0].to - w[1].from).abs() < 1e-9);
        }
        // Before the range nothing moves; after it everything moves by the same amount.
        assert_eq!(map(&st, 1.5), 1.5);
        let grown: f64 = st
            .iter()
            .map(|s| (s.to - s.from) * (1.0 / s.speed - 1.0))
            .sum();
        assert!((map(&st, 5.0) - 5.0 - grown).abs() < 1e-9);
        assert!((map(&st, 9.0) - 9.0 - grown).abs() < 1e-9);
        // 2.2 s at half speed and 0.8 s of ease: between the plain 3 s and the full 6 s.
        assert!(grown > 2.2 && grown < 3.0, "{grown}");
        let mut last = -1.0;
        for i in 0..100 {
            let t = map(&st, i as f64 * 0.1);
            assert!(t > last);
            last = t;
        }
        // A short range: the ease is a quarter of it at each end.
        let short = steps(&[Range {
            from: 0.0,
            to: 0.8,
            speed: 2.0,
        }]);
        assert!(
            (short[EASE_STEPS].from - 0.2).abs() < 1e-9
                && (short[EASE_STEPS].to - 0.6).abs() < 1e-9
        );
    }

    #[test]
    fn the_filters_hold_every_step() {
        let st = steps(&[Range {
            from: 1.0,
            to: 3.0,
            speed: 2.0,
        }]);
        let v = setpts(&st);
        assert!(v.starts_with("setpts='(T-") && v.ends_with(")/TB'"), "{v}");
        assert_eq!(v.matches("clip(").count(), 9);
        let a = sound(&st);
        // The part before, 9 steps, the part after.
        assert!(a.starts_with("aresample=48000,asplit=11[sp0]"), "{a}");
        assert!(
            a.contains("[sp0]atrim=start_sample=0:end_sample=48000,asetpts=N/SR/TB[sq0]"),
            "{a}"
        );
        assert_eq!(a.matches("atempo=").count(), 9);
        // The body: 1.2 s at 2x is 0.6 s, 28,800 samples.
        assert!(
            a.contains("atempo=2.000000,asetpts=N/SR/TB,apad,atrim=end_sample=28800"),
            "{a}"
        );
        assert!(
            a.contains("[sp10]atrim=start_sample=144000,asetpts=N/SR/TB[sq10]"),
            "{a}"
        );
        assert!(a.ends_with("concat=n=11:v=0:a=1"), "{a}");
        // A range from 0: no empty part in front.
        let a = sound(&steps(&[Range {
            from: 0.0,
            to: 2.0,
            speed: 0.5,
        }]));
        assert!(a.starts_with("aresample=48000,asplit=10[sp0]"), "{a}");
    }

    #[test]
    fn caption_times_move_through_the_map_and_the_text_stays() {
        let st = [Step {
            from: 2.0,
            to: 4.0,
            speed: 0.5,
        }];
        let srt = "1\n00:00:01,000 --> 00:00:03,000\nOne --> two\n\n2\n00:00:04,000 --> 00:00:05,500\nThree\n";
        assert_eq!(
            retime(srt, &st),
            "1\n00:00:01,000 --> 00:00:04,000\nOne --> two\n\n2\n00:00:06,000 --> 00:00:07,500\nThree\n"
        );
        let vtt = "WEBVTT\r\n\r\n00:01.000 --> 00:03.000 line:80%\r\nOne\r\n";
        assert_eq!(
            retime(vtt, &st),
            "WEBVTT\r\n\r\n00:00:01.000 --> 00:00:04.000 line:80%\r\nOne\r\n"
        );
    }
}
