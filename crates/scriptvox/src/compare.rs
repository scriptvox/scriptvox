//! `scriptvox compare a.mp4 b.mp4`: how alike 2 videos look, frame by frame, and the times that differ. Own code, FFmpeg's
//! `ssim` filter. Spec: `docs/specs/compare.md`.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

pub struct Args {
    pub a: PathBuf,
    pub b: PathBuf,
    pub threshold: f64,
    pub use_system: bool,
}

/// At most this many stretches in a result; `stretches` gives the count.
const MAX_LISTED: usize = 50;
/// Differing frames closer than this are one stretch.
const JOIN_S: f64 = 0.5;

/// The difference of each frame, 0 to 1, from the `All:` value of each line of the filter's stats.
fn parse_stats(text: &str) -> Vec<f64> {
    text.lines()
        .filter_map(|l| {
            l.split("All:")
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse::<f64>()
                .ok()
        })
        .map(|same| (1.0 - same).max(0.0))
        .collect()
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Stretches of frames above the threshold: start, end and the largest difference.
fn stretches(diffs: &[f64], fps: f64, threshold: f64) -> Vec<Value> {
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    for (i, d) in diffs.iter().enumerate().filter(|(_, d)| **d > threshold) {
        let (start, end) = (i as f64 / fps, (i + 1) as f64 / fps);
        match out.last_mut() {
            Some(last) if start - last.1 < JOIN_S => {
                last.1 = end;
                last.2 = last.2.max(*d);
            }
            _ => out.push((start, end, *d)),
        }
    }
    out.into_iter()
        .map(|(s, e, p)| json!({"start": r3(s), "end": r3(e), "peak": r3(p)}))
        .collect()
}

pub fn run(a: Args) -> Envelope {
    let fail =
        |code, msg: String, fix: &str| Envelope::fail("compare", code, msg, Some(fix.to_string()));
    if !(0.0..1.0).contains(&a.threshold) {
        return fail(
            Exit::Usage,
            format!("--threshold {} is outside 0 to 1", a.threshold),
            "Use a value such as 0.02.",
        );
    }
    if let Some(env) = crate::media_gate::refuse("compare", [&a.a, &a.b]) {
        return env;
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("compare", e),
    };
    let mut read = Vec::new();
    for p in [&a.a, &a.b] {
        match ff.readback(p) {
            Ok(r) => read.push(r),
            Err(_) => {
                return fail(
                    Exit::Usage,
                    format!("{} is not a video FFmpeg can read", p.display()),
                    "Give 2 video files.",
                )
            }
        }
    }
    let (ra, rb) = (&read[0], &read[1]);
    let fps = ra.fps;
    // Each to the first video's size and rate, each from its own zero, so a re-encode lines up frame by frame.
    let graph = format!(
        "[0:v]fps={fps},setpts=PTS-STARTPTS[x];[1:v]scale={}:{},fps={fps},setpts=PTS-STARTPTS[y];[x][y]ssim=stats_file=-:shortest=1",
        ra.width, ra.height
    );
    let done = Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error"])
        // Both were read by `readback` above, which refuses a file that is not plain media.
        .args(crate::media_gate::input(&a.a).unwrap_or_default())
        .args(crate::media_gate::input(&a.b).unwrap_or_default())
        .args(["-lavfi", &graph, "-an", "-f", "null", "-"])
        .output();
    let out = match done {
        Ok(o) => o,
        Err(e) => {
            return fail(
                Exit::Error,
                format!("cannot start FFmpeg: {e}"),
                "Run `scriptvox doctor`.",
            )
        }
    };
    let diffs = parse_stats(&String::from_utf8_lossy(&out.stdout));
    if !out.status.success() || diffs.is_empty() {
        return fail(
            Exit::Error,
            format!(
                "FFmpeg gave no frame values: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            "Run `scriptvox check` on each file.",
        );
    }
    let (secs_a, secs_b) = (
        ff.video_duration(&a.a).unwrap_or(0.0),
        ff.video_duration(&a.b).unwrap_or(0.0),
    );
    let same_length = (secs_a - secs_b).abs() <= 1.0 / fps + 1e-3;
    let all = stretches(&diffs, fps, a.threshold);
    let peak = diffs.iter().cloned().fold(0.0, f64::max);
    let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let same_size = (ra.width, ra.height) == (rb.width, rb.height);
    let mut env = Envelope::ok("compare")
        .with("same", json!(all.is_empty() && same_length))
        .with("threshold", json!(a.threshold))
        .with("stretches", json!(all.len()))
        .with(
            "differs",
            json!(all.iter().take(MAX_LISTED).collect::<Vec<_>>()),
        )
        .with(
            "not_checked",
            json!([
                "sound: only the picture is compared",
                "person: a number says that frames differ, it does not say which video looks better"
            ]),
        );
    for (k, v) in [
        ("frames_compared", json!(diffs.len())),
        ("seconds_a", json!(r3(secs_a))),
        ("seconds_b", json!(r3(secs_b))),
        ("mean_difference", json!(r3(mean))),
        ("peak_difference", json!(r3(peak))),
    ] {
        env.metrics.insert(k.into(), v);
    }
    if !same_length {
        env.findings.push(json!({"gate": "length", "status": "warn",
            "detail": format!("{secs_a:.2} s and {secs_b:.2} s: only the first {:.2} s are compared, frame for frame", diffs.len() as f64 / fps),
            "fix": "A cut or a speed change moves all later frames, so they all differ. Compare the parts that line up."}));
    }
    if !same_size {
        env.findings.push(json!({"gate": "size", "status": "warn",
            "detail": format!("{}x{} and {}x{}: the second was scaled to the first", ra.width, ra.height, rb.width, rb.height),
            "fix": "Scaling alone adds a small difference."}));
    }
    if let Some(first) = all.first() {
        env = env.with(
            "next",
            json!([format!(
                "Look at both: `scriptvox snapshot <file> --at {}`.",
                first["start"]
            )]),
        );
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_lines_become_differences() {
        let t = "n:1 Y:0.9 U:0.9 V:0.9 All:0.900000 (10.0)\nn:2 Y:1 U:1 V:1 All:1.000000 (inf)\nnoise\n";
        let d = parse_stats(t);
        assert_eq!(d.len(), 2);
        assert!((d[0] - 0.1).abs() < 1e-9 && d[1] == 0.0);
    }

    #[test]
    fn near_frames_join_and_far_frames_do_not() {
        let mut d = vec![0.0; 100];
        (10..12).for_each(|i| d[i] = 0.2);
        d[14] = 0.5; // 0.2 s later at 10 fps: the same stretch
        d[50] = 0.3; // far: a new one
        let s = stretches(&d, 10.0, 0.05);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0], json!({"start": 1.0, "end": 1.5, "peak": 0.5}));
        assert_eq!(s[1]["start"], 5.0);
        assert!(stretches(&d, 10.0, 0.9).is_empty());
    }
}
