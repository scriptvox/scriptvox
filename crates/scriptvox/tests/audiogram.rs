#![cfg(unix)]
//! The waveform on the plain background (brief `waveform`, G107).
mod common;
use common::*;
use serde_json::Value;

const SCRIPT: &str = "First shot here. Second shot now. Third one comes. And the last.";

/// Pixels in the wave band (the middle of the frame) that have the default accent colour, at 1 s.
fn accent_pixels(ff: &str, video: &std::path::Path) -> usize {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-ss", "1", "-i"])
        .arg(video)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "crop=iw:ih/4:0:ih*11/40",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .unwrap();
    assert!(!o.stdout.is_empty());
    // The default accent is #FFE500. No named background holds it.
    o.stdout
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|p| p[0] > 215 && p[1] > 195 && p[2] < 70)
        .count()
}

fn make(e: &Env, brief: &str, out: &str, fault: Option<&str>) -> (Option<i32>, Value) {
    std::fs::write(e.dir.path().join("b.json"), brief).unwrap();
    let mut c = e.cmd("ok");
    c.env("SCRIPTVOX_CHUNK_S", "1")
        .args(["make", "--brief", "b.json", "--json", "--out", out]);
    if let Some(n) = fault {
        c.env("SCRIPTVOX_FAULT_AFTER_CHUNKS", n);
    }
    let o = c.output().unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

#[test]
fn a_waveform_shows_on_the_plain_background_and_passes_the_gates() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |wave: bool| {
        let mut b = serde_json::json!({"script": SCRIPT, "aspect": "16:9"});
        if wave {
            b["waveform"] = true.into();
        }
        b.to_string()
    };
    let (code, v) = make(&e, &brief(true), "wave.mp4", None);
    assert_eq!(code, Some(0), "{v}");
    let gates = v["findings"].as_array().unwrap();
    assert!(gates.iter().all(|g| g["status"] != "fail"), "{v}");
    let (code, plain) = make(&e, &brief(false), "plain.mp4", None);
    assert_eq!(code, Some(0), "{plain}");
    let p = e.dir.path();
    let (with, without) = (
        accent_pixels(&ff, &p.join("wave.mp4")),
        accent_pixels(&ff, &p.join("plain.mp4")),
    );
    assert!(with > 2000 && without == 0, "wave {with}, plain {without}");
    // The caption and text gates read the same with and without the wave.
    for name in ["text_bounds", "captions"] {
        let of = |v: &Value| {
            v["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["gate"] == name)
                .map(|g| g["status"].clone())
        };
        assert_eq!(of(&v), of(&plain), "{name}");
    }
}

#[test]
fn a_killed_waveform_render_resumes_and_matches_a_clean_render() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = serde_json::json!({"script": SCRIPT, "waveform": true}).to_string();
    // 3.6 s at 1 s chunks = 4 chunks. Stop after 2, as a kill would. No idempotency key: the run folder changes.
    let (code, v) = make(&e, &brief, "a.mp4", Some("2"));
    assert_eq!(code, Some(1), "{v}");
    let (code, v) = make(&e, &brief, "a.mp4", None);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["resumed_from_chunk"], 2, "{v}");
    let (_, v) = make(&e, &brief, "b.mp4", None);
    assert_eq!(v["metrics"]["resumed_from_chunk"], 0, "{v}");
    let psnr = std::process::Command::new(&ff)
        .current_dir(e.dir.path())
        .args(["-hide_banner", "-i", "a.mp4", "-i", "b.mp4", "-lavfi"])
        .args(["[0:v][1:v]psnr", "-f", "null", "-"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&psnr.stderr);
    let avg = err
        .split("average:")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap();
    assert!(
        avg == "inf" || avg.parse::<f64>().unwrap() >= 50.0,
        "PSNR {avg}"
    );
}

#[test]
fn a_waveform_with_no_sound_is_refused_at_plan_time() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let plan = |brief: &str| {
        std::fs::write(e.dir.path().join("b.json"), brief).unwrap();
        let o = e
            .cmd("ok")
            .args(["plan", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = plan(r#"{"script": "Hi there.", "tts": "none", "waveform": true}"#);
    assert_eq!(code, Some(2), "{v}");
    assert!(v.to_string().contains("waveform needs sound"), "{v}");
    // Music alone is sound.
    let (code, v) =
        plan(r#"{"script": "Hi there.", "tts": "none", "music": "calm", "waveform": true}"#);
    assert_eq!(code, Some(0), "{v}");
}

/// 10 Nov 2026: with the wave drawn inside the render graph, FFmpeg queued full frames in front of the overlay with
/// no limit and two renders reached 19 GB each. A 30 s render must stay small: here FFmpeg has 2.5 GB of address space.
#[test]
fn a_30_s_waveform_render_stays_inside_a_memory_cap() {
    use std::os::unix::fs::PermissionsExt;
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let bin = p.join("capbin");
    std::fs::create_dir_all(&bin).unwrap();
    let capped = bin.join("ffmpeg");
    std::fs::write(
        &capped,
        format!("#!/bin/sh\nulimit -v 2500000\nexec '{ff}' \"$@\"\n"),
    )
    .unwrap();
    std::fs::set_permissions(&capped, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(
        std::path::Path::new(&ff).with_file_name("ffprobe"),
        bin.join("ffprobe"),
    )
    .unwrap();
    // A 30 s voice in place of the stub's 3 s tone.
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .args(["sine=f=330:d=30", "-c:a", "libmp3lame"])
        .arg(p.join("voice.mp3"))
        .status()
        .unwrap();
    assert!(st.success());
    let script = "One more word here. ".repeat(20);
    let brief = serde_json::json!({"script": script, "aspect": "16:9", "waveform": true});
    std::fs::write(p.join("b.json"), brief.to_string()).unwrap();
    let o = e
        .cmd("ok")
        .env("SCRIPTVOX_FFMPEG", &capped)
        .args(["make", "--brief", "b.json", "--json", "--out", "long.mp4"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert!(v["metrics"]["duration_s"].as_f64().unwrap() >= 29.0, "{v}");
    assert!(accent_pixels(&ff, &p.join("long.mp4")) > 2000);
}
