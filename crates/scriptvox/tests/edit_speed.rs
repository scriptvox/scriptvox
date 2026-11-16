//! `scriptvox edit --speed "2-4@0.5"`: a part of the video plays slower, the rest does not move, the pitch stays,
//! picture and sound end together, and the caption file beside the input gets the new times. No network.
mod common;
use serde_json::Value;
use std::path::Path;

/// 6 s at 25 fps with a 440 Hz tone and a 2 ms burst of 3 kHz at 1.0, 3.0 and 5.0 s; and its caption file.
fn talk(ff: &str, dir: &Path) {
    let sound = "aevalsrc='0.3*sin(2*PI*440*t)+0.6*sin(2*PI*3000*t)*(between(t,1,1.002)+between(t,3,3.002)+between(t,5,5.002))':s=48000:d=6";
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=320x180:r=25:d=6")
        .args(["-f", "lavfi", "-i", sound])
        .args([
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k",
        ])
        .args(["-shortest", "talk.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(
        dir.join("talk.srt"),
        "1\n00:00:01,000 --> 00:00:02,000\nBefore the range.\n\n2\n00:00:05,000 --> 00:00:05,500\nAfter it.\n",
    )
    .unwrap();
}

fn rv(ff: &str, dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
}

fn samples(ff: &str, video: &Path) -> Vec<f64> {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(video)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
        .output()
        .unwrap();
    assert!(o.status.success());
    o.stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f64::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

fn frames(ff: &str, video: &Path) -> usize {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(video)
        .args(["-map", "0:v", "-c", "copy", "-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(o.status.success());
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .count()
}

/// The amplitude of the `hz` sine from `from` to `to` seconds.
fn level(s: &[f64], from: f64, to: f64, hz: f64) -> f64 {
    let part = &s[(from * 48000.0) as usize..(to * 48000.0) as usize];
    let w = std::f64::consts::TAU * hz / 48000.0;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, x) in part.iter().enumerate() {
        re += x * (w * i as f64).cos();
        im += x * (w * i as f64).sin();
    }
    2.0 * re.hypot(im) / part.len() as f64
}

/// Where the 3 kHz burst is loudest between `from` and `to`, in seconds (5 ms windows).
fn burst(s: &[f64], from: f64, to: f64) -> f64 {
    let mut best = (0.0, from);
    let mut t = from;
    while t + 0.005 <= to {
        let l = level(s, t, t + 0.005, 3000.0);
        if l > best.0 {
            best = (l, t);
        }
        t += 0.001;
    }
    best.1
}

#[test]
fn a_range_plays_slower_with_the_same_pitch_and_the_rest_keeps_its_place() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &["edit", "talk.mp4", "--speed", "2-4@0.5", "--out", "s.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    // The map by hand: 4 steps of 0.1 s at 0.5^(j/5) on each side, and 1.2 s at 0.5.
    let ease: f64 = (1..=4).map(|j| 0.1 / 0.5f64.powf(j as f64 / 5.0)).sum();
    let end = 2.0 + 2.0 * ease + 1.2 / 0.5;
    let grown = end - 4.0;
    let r = &v["speed"]["ranges"][0];
    assert_eq!(r["result_from"], 2.0, "{v}");
    assert!(
        (r["result_to"].as_f64().unwrap() - end).abs() < 0.002,
        "{v}"
    );
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((after - (6.0 + grown)).abs() < 0.06, "{after} {grown}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["gate"] == "av_drift" && f["status"] == "pass"),
        "{v}"
    );

    let out = d.path().join("s.mp4");
    // Frames: the new length on the 25 fps grid.
    let n = frames(&ff, &out) as f64;
    assert!((n - (6.0 + grown) * 25.0).abs() <= 1.5, "{n} frames");
    let s = samples(&ff, &out);
    assert!(
        (s.len() as f64 / 48000.0 - (6.0 + grown)).abs() < 0.06,
        "{}",
        s.len()
    );
    // The burst before the range has not moved; the one after it moved by the whole growth; the one inside is
    // where the map says (atempo smears a 2 ms burst, so 60 ms of room).
    assert!(
        (burst(&s, 0.5, 1.5) - 1.0).abs() < 0.01,
        "{}",
        burst(&s, 0.5, 1.5)
    );
    let last = burst(&s, 4.5 + grown, 5.5 + grown);
    assert!(
        (last - (5.0 + grown)).abs() < 0.01,
        "{last} {}",
        5.0 + grown
    );
    let inside = 2.0 + ease + (3.0 - 2.4) / 0.5;
    let got = burst(&s, 2.5, end - 0.2);
    assert!((got - inside).abs() < 0.06, "{got} {inside}");
    // The pitch: 440 Hz is as strong in the slow part as outside it, and no 220 Hz came.
    let (outside, slow) = (level(&s, 0.2, 0.9, 440.0), level(&s, 4.0, 5.0, 440.0));
    assert!((slow / outside - 1.0).abs() < 0.2, "{slow} {outside}");
    assert!(level(&s, 4.0, 5.0, 220.0) < outside / 20.0);

    // The caption file of the run: the cue before stays, the cue after moves by the growth.
    assert_eq!(
        v["speed"]["captions_retimed"],
        serde_json::json!(["srt"]),
        "{v}"
    );
    let srt = std::fs::read_to_string(d.path().join("s.srt")).unwrap();
    assert!(
        srt.contains("00:00:01,000 --> 00:00:02,000\nBefore the range."),
        "{srt}"
    );
    let ms = ((5.0 + grown) * 1000.0).round() as u64;
    let want = format!("00:00:{:02},{:03} --> ", ms / 1000, ms % 1000);
    assert!(srt.contains(&want), "{srt} {want}");

    // A plain number is still the whole video.
    let (code, v) = rv(
        &ff,
        d.path(),
        &["edit", "talk.mp4", "--speed", "2", "--out", "fast.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert!(
        (v["metrics"]["seconds_after"].as_f64().unwrap() - 3.0).abs() < 0.1,
        "{v}"
    );
    // It has no ease and no ranges; the caption file of the run moves with it (the test below).
    assert_eq!(v["speed"]["speed"], 2.0, "{v}");
    assert!(v["speed"].get("ranges").is_none(), "{v}");
}

/// One speed for the whole video moves the caption files too, as a range does (they stayed at the old times).
#[test]
fn a_plain_speed_moves_the_caption_files_with_the_video() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    std::fs::write(
        d.path().join("talk.vtt"),
        "WEBVTT\n\n00:05.000 --> 00:05.500\nAfter it.\n",
    )
    .unwrap();
    let (code, v) = rv(
        &ff,
        d.path(),
        &["edit", "talk.mp4", "--speed", "1.25", "--out", "f.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["speed"]["speed"], 1.25, "{v}");
    assert_eq!(
        v["speed"]["captions_retimed"],
        serde_json::json!(["srt", "vtt"]),
        "{v}"
    );
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((after - 4.8).abs() < 0.06, "{after}");
    // 1 s, 2 s, 5 s and 5.5 s at 1.25x.
    let srt = std::fs::read_to_string(d.path().join("f.srt")).unwrap();
    assert!(
        srt.contains("00:00:00,800 --> 00:00:01,600\nBefore the range."),
        "{srt}"
    );
    assert!(
        srt.contains("00:00:04,000 --> 00:00:04,400\nAfter it."),
        "{srt}"
    );
    let vtt = std::fs::read_to_string(d.path().join("f.vtt")).unwrap();
    assert!(vtt.contains("00:04.000 --> 00:00:04.400"), "{vtt}");
    // The burst of the sound at 5 s is at 4 s: the cue is on its sound.
    let s = samples(&ff, &d.path().join("f.mp4"));
    assert!((burst(&s, 3.5, 4.5) - 4.0).abs() < 0.06);
    // With a cut the old times are wrong, so no file moves (the rule of a range).
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit", "talk.mp4", "--from", "1", "--speed", "1.25", "--out", "g.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["speed"]["captions_retimed"], serde_json::json!([]), "{v}");
    assert!(!d.path().join("g.srt").exists());
}

#[test]
fn a_bad_range_is_refused_with_a_fix_and_no_file() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    for (spec, says, fix) in [
        ("abc", "not FROM-TO@SPEED", "--speed \"12-15@0.5\""),
        ("2-4@3", "the speed is 0.5 to 2", "--speed \"12-15@0.5\""),
        ("2-2.1@0.5", "at least 0.5 s", "--speed \"12-15@0.5\""),
        ("2-4@0.5,3-5@2", "do not overlap", "--speed \"12-15@0.5\""),
        (
            "2-9@0.5",
            "ends after the video (6.0",
            "Give times from 0 to 6.0",
        ),
    ] {
        let (code, v) = rv(
            &ff,
            d.path(),
            &["edit", "talk.mp4", "--speed", spec, "--out", "bad.mp4"],
        );
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(says),
            "{v}"
        );
        assert!(v["error"]["fix"].as_str().unwrap().contains(fix), "{v}");
        assert!(!d.path().join("bad.mp4").exists(), "{spec} left a file");
    }
}

#[test]
fn no_step_of_the_ease_ends_in_a_dip_or_a_hole() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=320x180:r=25:d=6")
        .args(["-f", "lavfi", "-i", "sine=f=440:d=6"])
        .args([
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k",
        ])
        .args(["-shortest", "tone.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    // A long and the shortest range, slow and fast: the steady tone stays steady in every 10 ms of the result.
    // Before each step read past its own end, 0.5 gave dips of 5 to 10 dB and 10 ms of silence at step ends.
    for spec in ["2-4@0.5", "2-2.5@0.5", "2-2.5@2"] {
        let (code, v) = rv(
            &ff,
            d.path(),
            &["edit", "tone.mp4", "--speed", spec, "--out", "o.mp4"],
        );
        assert_eq!(code, Some(0), "{v}");
        let s = samples(&ff, &d.path().join("o.mp4"));
        let rms: Vec<f64> = s[4800..s.len() - 4800]
            .chunks(480)
            .map(|c| (c.iter().map(|x| x * x).sum::<f64>() / c.len() as f64).sqrt())
            .collect();
        let mut sorted = rms.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let (low, at) = rms
            .iter()
            .enumerate()
            .map(|(i, r)| (*r, 0.1 + i as f64 * 0.01))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        assert!(
            low > median * 0.89,
            "{spec}: {low} at {at} s, median {median}"
        );
    }
}
