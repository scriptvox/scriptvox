//! `scriptvox edit --sfx NAME@SECONDS`: a built-in or own sound effect lands at its time, at its level, and the
//! picture and the length stay. No network: FFmpeg makes every sound.
mod common;
use serde_json::Value;
use std::path::Path;

fn ffmpeg(ff: &str, dir: &Path, args: &[&str]) {
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "{args:?}");
}

/// 6 s of picture with a 440 Hz "voice", the same picture with no sound, and an own effect: 0.2 s at 2 kHz.
fn files(ff: &str, dir: &Path) {
    let picture = ["-f", "lavfi", "-i", "testsrc2=s=320x180:r=25:d=6"];
    let enc = ["-c:v", "libx264", "-pix_fmt", "yuv420p"];
    let mut talk = picture.to_vec();
    talk.extend([
        "-f",
        "lavfi",
        "-i",
        "sine=f=440:d=6",
        "-c:a",
        "aac",
        "-shortest",
    ]);
    talk.extend(enc);
    talk.push("talk.mp4");
    ffmpeg(ff, dir, &talk);
    let mut mute = picture.to_vec();
    mute.extend(enc);
    mute.push("mute.mp4");
    ffmpeg(ff, dir, &mute);
    ffmpeg(
        ff,
        dir,
        &["-f", "lavfi", "-i", "sine=f=2000:d=0.2", "ding.wav"],
    );
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

/// The sound of a video as mono 48 kHz samples.
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

/// The picture packets of a video, hashed one by one: the same text means the same frames.
fn picture(ff: &str, video: &Path) -> String {
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
        .collect::<Vec<_>>()
        .join("\n")
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

fn rms(s: &[f64], from: f64, to: f64) -> f64 {
    let part = &s[(from * 48000.0) as usize..(to * 48000.0) as usize];
    (part.iter().map(|x| x * x).sum::<f64>() / part.len() as f64).sqrt()
}

#[test]
fn an_own_effect_lands_at_its_time_12_db_under_the_sound_and_the_picture_stays() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "talk.mp4",
            "--sfx",
            "ding.wav@4.5",
            "--sfx",
            "ding.wav@1",
            "--sfx",
            "pop@3",
            "--out",
            "s.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["picture_copied"], true, "{v}");
    assert_eq!(
        v["metrics"]["seconds_after"], v["metrics"]["seconds_before"],
        "{v}"
    );
    assert_eq!(v["sfx"]["level"], "normal");
    assert_eq!(v["sfx"]["cues"].as_array().unwrap().len(), 3, "{v}");
    assert_eq!(v["sfx"]["cues"][0]["starts_at"], 4.5);
    let out = d.path().join("s.mp4");
    assert_eq!(picture(&ff, &d.path().join("talk.mp4")), picture(&ff, &out));
    let s = samples(&ff, &out);
    assert!((s.len() as f64 / 48000.0 - 6.0).abs() < 0.1, "{}", s.len());
    // The 2 kHz effect is at 1.0 and 4.5 s and nowhere else.
    let voice = level(&s, 2.0, 2.5, 440.0);
    let (first, second, none) = (
        level(&s, 1.02, 1.18, 2000.0),
        level(&s, 4.52, 4.68, 2000.0),
        level(&s, 2.0, 2.5, 2000.0),
    );
    assert!(none < voice / 500.0, "{none} {voice}");
    // 12 dB under the voice's peak: two sines, so the amplitudes say it.
    for got in [first, second] {
        let db = 20.0 * (got / voice).log10();
        assert!((db + 12.0).abs() < 1.5, "{db} dB");
    }
    // The pop (500 to 1,200 Hz) adds sound at 3.0 s that is not there at 2.0 s: what is left without the 440 Hz.
    let rest = |from: f64| {
        let to = from + 0.05;
        (rms(&s, from, to).powi(2) - level(&s, from, to, 440.0).powi(2) / 2.0).max(0.0)
    };
    assert!(rest(3.0) > 1e-5, "{}", rest(3.0));
    assert!(rest(3.0) > 20.0 * rest(2.0), "{} {}", rest(3.0), rest(2.0));

    // Loud is 6 dB under.
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "talk.mp4",
            "--sfx",
            "ding.wav@4.5",
            "--sfx-level",
            "loud",
            "--out",
            "l.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    let s = samples(&ff, &d.path().join("l.mp4"));
    let db = 20.0 * (level(&s, 4.52, 4.68, 2000.0) / level(&s, 2.0, 2.5, 440.0)).log10();
    assert!((db + 6.0).abs() < 1.5, "{db} dB");
}

#[test]
fn a_whoosh_peaks_at_its_time_on_a_video_with_no_sound() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &["edit", "mute.mp4", "--sfx", "whoosh@3", "--out", "w.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["sfx"]["cues"][0]["starts_at"], 2.6);
    let out = d.path().join("w.mp4");
    assert_eq!(picture(&ff, &d.path().join("mute.mp4")), picture(&ff, &out));
    let s = samples(&ff, &out);
    assert!((s.len() as f64 / 48000.0 - 6.0).abs() < 0.1, "{}", s.len());
    // Loudest in the 0.1 s before its time; silent before it starts and after it ends.
    let peak = rms(&s, 2.9, 3.0);
    assert!(peak > 0.01, "{peak}");
    assert!(rms(&s, 2.6, 2.7) < peak / 4.0, "the swell");
    assert!(rms(&s, 1.0, 2.5) < peak / 1000.0, "before");
    assert!(rms(&s, 3.3, 5.5) < peak / 1000.0, "after");
}

#[test]
fn a_bad_effect_is_refused_with_a_fix_and_no_file() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let outside = tempfile::tempdir().unwrap();
    std::fs::copy(d.path().join("ding.wav"), outside.path().join("o.wav")).unwrap();
    let far = format!("{}@1", outside.path().join("o.wav").display());
    for (args, says, fix) in [
        (vec!["--sfx", "whoosh"], "no time", "NAME@SECONDS"),
        (
            vec!["--sfx", "boom@1"],
            "no sound effect boom",
            "whoosh, pop or tick",
        ),
        (
            vec!["--sfx", "pop@99"],
            "after the end of the video (6.0",
            "from 0 to 6.0",
        ),
        (
            vec!["--sfx", "pop@1", "--sfx-level", "huge"],
            "unknown --sfx-level huge",
            "quiet, normal or loud",
        ),
        (
            vec!["--sfx", &far],
            "outside this project folder",
            "Copy the sound file",
        ),
    ] {
        let mut a = vec!["edit", "talk.mp4", "--out", "bad.mp4"];
        a.extend(args);
        let (code, v) = rv(&ff, d.path(), &a);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(says),
            "{v}"
        );
        assert!(v["error"]["fix"].as_str().unwrap().contains(fix), "{v}");
        assert!(!d.path().join("bad.mp4").exists(), "{a:?} left a file");
    }
}
