//! `scriptvox edit`: cut pauses and crop, on a generated test video.
mod common;
use serde_json::Value;

#[test]
fn cut_silence_removes_the_pause_and_reframe_makes_it_tall() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    // 6 s of picture; sound: 2 s tone, 2 s silence, 2 s tone.
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:d=6",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=2",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono:d=2",
        ])
        .args(["-f", "lavfi", "-i", "sine=f=440:d=2"])
        .args([
            "-filter_complex",
            "[1:a][2:a][3:a]concat=n=3:v=0:a=1[a]",
            "-map",
            "0:v",
            "-map",
            "[a]",
        ])
        .args([
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "in.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "edit",
            "in.mp4",
            "--cut-silence",
            "--reframe",
            "9:16",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((4.0..4.6).contains(&after), "{v}");
    assert_eq!(v["metrics"]["cuts"], 1);
    assert_eq!(v["metrics"]["size"], "1080x1920");
    let out = std::path::Path::new(v["artifacts"]["video"].as_str().unwrap());
    let probe = std::process::Command::new(std::path::Path::new(&ff).with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(out)
        .output()
        .unwrap();
    let real: f64 = String::from_utf8_lossy(&probe.stdout)
        .trim()
        .parse()
        .unwrap();
    assert!(
        (real - after).abs() < 0.25,
        "file {real} s, reported {after} s"
    );

    // A clip out of the middle; captions need a speech to text key and say so.
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_DATA", d.path().join("no-data")) // A machine with `setup speech-local` must not answer here.
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env_remove("SCRIPTVOX_SELFHOST_KEY")
            .args(["edit", "in.mp4"])
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run(&["--from", "1", "--to", "4", "--out", "clip.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["seconds_before"], 3.0, "{v}");
    let (code, v) = run(&["--from", "1", "--to", "1.2"]);
    assert_eq!(code, Some(2), "{v}");
    // A logo alone is an edit; a missing logo or a dark accent is refused.
    let png = d.path().join("logo.png");
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=300x100",
            "-frames:v",
            "1",
            "-y",
        ])
        .arg(&png)
        .status()
        .unwrap();
    assert!(st.success());
    let (code, v) = run(&["--logo", "logo.png", "--out", "logo.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["size"], "640x360", "{v}");
    let (code, _) = run(&["--logo", "none.png"]);
    assert_eq!(code, Some(2));
    let (code, _) = run(&["--logo", "logo.png", "--accent", "#000080"]);
    assert_eq!(code, Some(2));
    // Music: the picture alone stays as it is; a missing file or an unknown level is refused.
    let (code, v) = run(&["--music", "calm", "--out", "bed.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(v["findings"][0]["status"] == "pass", "{v}");
    let (code, _) = run(&["--music", "none.wav"]);
    assert_eq!(code, Some(2));
    let (code, _) = run(&["--music", "calm", "--music-level", "huge"]);
    assert_eq!(code, Some(2));
    // A hook alone needs no speech and no key; emphasis without captions names what it needs.
    let (code, v) = run(&["--hook", "The short version", "--out", "hook.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    let (code, v) = run(&["--emphasis", "word"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--captions"),
        "{v}"
    );
    let (code, v) = run(&["--captions", "fancy"]);
    assert_eq!(code, Some(2), "{v}");
    let (code, v) = run(&["--captions"]);
    assert_eq!(
        code,
        Some(10),
        "no speech to text key is a needs-input exit: {v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("scriptvox providers"),
        "{v}"
    );
}

/// A test video: 30 fps picture of `video_s`, a tone of `audio_s` at `db`.
fn av_source(ff: &str, out: &std::path::Path, video_s: f64, audio_s: f64, db: i32) {
    let st = std::process::Command::new(ff)
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=s=320x240:r=30:d={video_s}"))
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=f=300:d={audio_s}"))
        .args(["-filter:a", &format!("volume={db}dB")])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
        .arg(out)
        .status()
        .unwrap();
    assert!(st.success());
}

#[test]
fn edit_exits_3_and_keeps_the_file_when_sound_and_picture_drift() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    av_source(&ff, &d.path().join("short.mp4"), 4.0, 2.0, -6);
    av_source(&ff, &d.path().join("even.mp4"), 4.0, 4.0, -6);
    let run = |f: &str| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["edit", f, "--keep", "0-3", "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run("short.mp4");
    assert_eq!(code, Some(3), "{v}");
    assert_eq!(v["ok"], false, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("av_drift"),
        "{v}"
    );
    let kept = v["artifacts"]["video"].as_str().unwrap();
    assert!(std::path::Path::new(kept).is_file(), "the file stays: {v}");
    let (code, v) = run("even.mp4");
    assert_eq!(code, Some(0), "a clean cut passes: {v}");
}

/// G176: a label too wide for the frame is cut off by it. The gate of `make` says so here too, and the file stays.
#[test]
fn a_label_that_leaves_the_frame_exits_3_and_keeps_the_file() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    av_source(&ff, &d.path().join("in.mp4"), 2.0, 2.0, -6);
    let run = |label: &str, out: &str| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["edit", "in.mp4", "--label", label, "--out", out, "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run(
        "A label with far too many words to stay inside so small a frame",
        "wide.mp4",
    );
    assert_eq!(code, Some(3), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("text_bounds"),
        "{v}"
    );
    assert!(
        !v["error"]["fix"].as_str().unwrap().contains("resume"),
        "{v}"
    );
    assert!(d.path().join("wide.mp4").is_file(), "the file stays: {v}");
    let (code, v) = run("Short", "short.mp4");
    assert_eq!(code, Some(0), "{v}");
    let found = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "text_bounds");
    assert_eq!(found.unwrap()["status"], "pass", "{v}");
}

/// `master()` writes the sound again under a copied picture: every frame stays, and the sound stops at the last one.
/// Fails with `-shortest` in place of the `atrim`, which is the fault of 5 Nov.
#[test]
fn master_keeps_the_last_frames_of_a_copied_picture() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("cut.mp4");
    // Quiet, so it is mastered at all. The sound ends just before the picture, as a cut on frames can leave it: with
    // `-shortest` this exact source lost 3 frames (video 1.90 s, sound 2.00 s).
    av_source(&ff, &d.path().join("src.mp4"), 2.0, 1.98, -30);
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-i"])
        .arg(d.path().join("src.mp4"))
        .args(["-c", "copy"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    let f = scriptvox::engine_ffmpeg::Ffmpeg::locate(false, false).unwrap();
    let before = f.video_duration(&out).unwrap();
    scriptvox::edit::master(&f, &out).expect("the file is mastered");
    let after = f.video_duration(&out).unwrap();
    assert!(
        (before - after).abs() < 0.001,
        "frames lost: {before} to {after}"
    );
    let found = scriptvox::gates::check(&f, &out).unwrap();
    let drift = found.iter().find(|g| g.gate == "av_drift").unwrap();
    assert_eq!(drift.status, "pass", "{}", drift.detail);
}

#[test]
fn edit_with_nothing_to_do_is_a_usage_error() {
    let o = common::bin()
        .args(["edit", "x.mp4", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn track_without_reframe_is_a_usage_error() {
    let o = common::bin()
        .args(["edit", "x.mp4", "--cut-silence", "--track", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stdout).contains("--track needs --reframe"));
}

#[test]
fn a_base_video_plays_under_a_new_voice() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    let st = std::process::Command::new(&ff)
        .current_dir(p)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:d=8",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=500:d=8",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "src.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(
        p.join("b.json"),
        r#"{"script": "A new voice speaks over the old picture.", "base_video": "src.mp4"}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["shots"], 1);
    std::fs::write(
        p.join("c.json"),
        r#"{"script": "x y", "base_video": "src.mp4", "stock": ["sea"]}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "c.json", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "base_video with stock is refused");
    // The base video's own sound under the voice: keep works offline; a bed with it, or an unknown mode, is refused.
    for (brief, code) in [
        (
            r#"{"script": "The old tone stays under the voice.", "base_video": "src.mp4", "base_sound": "keep"}"#,
            0,
        ),
        (
            r#"{"script": "x y", "base_video": "src.mp4", "base_sound": "keep", "music": "calm"}"#,
            2,
        ),
        (
            r#"{"script": "x y", "base_video": "src.mp4", "base_sound": "loud"}"#,
            2,
        ),
        (r#"{"script": "x y", "base_sound": "keep"}"#, 2),
    ] {
        std::fs::write(p.join("d.json"), brief).unwrap();
        let o = e
            .cmd("ok")
            .args(["make", "--brief", "d.json", "--json"])
            .output()
            .unwrap();
        assert_eq!(
            o.status.code(),
            Some(code),
            "{brief}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
    }
    // Sending the base video's sound to a model is refused offline, before any voice is made.
    std::fs::write(
        p.join("d.json"),
        r#"{"script": "x y", "base_video": "src.mp4", "base_sound": "background"}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "d.json", "--local-only", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "background is refused with --local-only"
    );
}

#[test]
fn reframe_refuses_a_scriptvox_video_made_in_another_frame() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let recipe = r#"{"scriptvox":"0.0.1","request":{"aspect":"16:9"}}"#;
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:d=1",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=1",
            "-pix_fmt",
            "yuv420p",
            "-shortest",
        ])
        .args(["-metadata", &format!("comment={recipe}"), "-y", "made.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    let run = |extra: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["edit", "made.mp4", "--reframe", "9:16", "--json"])
            .args(extra)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code(), v)
    };
    let (code, v) = run(&[]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("\"aspect\": \"9:16\""),
        "{v}"
    );
    let (code, v) = run(&["--crop-text"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["size"], "1080x1920");
}

#[test]
fn a_variable_rate_clip_keeps_its_length_after_a_cut() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    // 8 s: 10 fps, then 60 fps, then 20 fps (nominal rate 60), with a pause in the sound. A cut used to squeeze the
    // picture to frames/60 s and `-shortest` the sound with it: 8 s became 2.3 s.
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=s=180x320:r=60:d=8"])
        .args(["-f", "lavfi", "-i", "sine=f=300:d=8"])
        .args([
            "-filter_complex",
            "[0:v]select='if(lt(t,3),not(mod(n,6)),if(lt(t,5),1,not(mod(n,3))))'[v];[1:a]volume='if(between(t,3,4.5),0,1)':eval=frame[a]",
            "-map", "[v]", "-map", "[a]", "-fps_mode", "vfr", "-pix_fmt", "yuv420p", "-c:a", "aac", "-y", "in.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args(["edit", "in.mp4", "--cut-silence", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((6.0..7.5).contains(&after), "{v}");
    // No warning: the file really runs that long.
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["gate"] != "length"),
        "{v}"
    );
    let a = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args(["analyze", "in.mp4", "--json"])
        .output()
        .unwrap();
    let a: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(a["metrics"]["variable_rate"], true, "{a}");
    assert!(a["metrics"]["fps"].as_f64().unwrap() < 40.0, "{a}");
}

#[test]
fn like_refuses_to_write_over_a_file() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("in.mp4"), b"x").unwrap();
    std::fs::write(d.path().join("b.json"), b"{}").unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .args(["analyze", "in.mp4", "--like", "b.json", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(std::fs::read(d.path().join("b.json")).unwrap(), b"{}");
}

#[test]
fn a_scene_voice_must_share_the_engine_and_the_language() {
    let d = tempfile::tempdir().unwrap();
    let plan = |brief: &str| {
        std::fs::write(d.path().join("b.json"), brief).unwrap();
        let o = common::bin()
            .current_dir(d.path())
            .args(["plan", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        serde_json::from_slice::<Value>(&o.stdout).unwrap()
    };
    let two = |v: &str| {
        format!(
            r#"{{"tts":"edge","scenes":[{{"script":"One two three."}},{{"script":"Four five six.","voice":"{v}"}}]}}"#
        )
    };
    assert_eq!(plan(&two("en-US-AvaNeural"))["ok"], true);
    assert!(plan(&two("nobody"))["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown voice"));
    assert!(plan(&two("af_heart"))["error"]["message"]
        .as_str()
        .unwrap()
        .contains("one engine"));
    assert!(plan(&two("ar-AE-HamdanNeural"))["error"]["message"]
        .as_str()
        .unwrap()
        .contains("does not speak"));
}

#[test]
fn join_keeps_length_and_sound_over_mixed_clips() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let make = |name: &str, size: &str, sound: bool| {
        let mut c = std::process::Command::new(&ff);
        c.current_dir(d.path()).args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=s={size}:r=25:d=2"),
        ]);
        if sound {
            c.args(["-f", "lavfi", "-i", "sine=f=300:d=2", "-c:a", "aac"]);
        }
        assert!(c
            .args(["-pix_fmt", "yuv420p", "-y", name])
            .status()
            .unwrap()
            .success());
    };
    make("a.mp4", "180x320", true);
    make("b.mp4", "320x180", false);
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "join",
            "a.mp4",
            "b.mp4",
            "--transition",
            "wipe",
            "--out",
            "j.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["size"], "180x320", "{v}");
    let secs = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((3.5..3.8).contains(&secs), "{v}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["status"] == "pass"),
        "{v}"
    );
}

/// A small clip with a tone: `af` shapes the sound (a quiet level, or a hole in its times).
fn tone_clip(ff: &str, dir: &std::path::Path, name: &str, secs: u32, af: &str) {
    assert!(std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=s=180x320:r=25:d={secs}"))
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=f=300:d={secs}"))
        .args(["-af", af, "-c:a", "aac", "-pix_fmt", "yuv420p", "-y", name])
        .status()
        .unwrap()
        .success());
}

/// The seconds of sound a file really holds: its decoded samples, not the length its header states.
fn sound_secs(ff: &str, file: &std::path::Path) -> f64 {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
        .output()
        .unwrap();
    o.stdout.len() as f64 / 96000.0
}

#[test]
fn join_fills_a_hole_in_the_sound_of_a_clip() {
    // 12 Nov 2026: a clip from `edit` had a hole in its sound times; `join` gave video 24.000 s, sound 23.837 s, exit 3.
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    tone_clip(
        &ff,
        d.path(),
        "hole.mp4",
        4,
        "aselect='not(between(t,1,1.3))'",
    );
    tone_clip(&ff, d.path(), "b.mp4", 3, "anull");
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "join",
            "hole.mp4",
            "b.mp4",
            "--transition",
            "cut",
            "--out",
            "j.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let secs = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!(
        (sound_secs(&ff, &d.path().join("j.mp4")) - secs).abs() < 0.05,
        "{v}"
    );
}

#[test]
fn a_cut_of_a_quiet_clip_keeps_all_of_its_sound() {
    // 12 Nov 2026: the loudness step left a hole in the sound at 4 s: the header said 6.88 s, the samples were 6.85 s.
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    tone_clip(&ff, d.path(), "q.mp4", 10, "volume=0.05");
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "edit",
            "q.mp4",
            "--keep",
            "0.5-4.4,6-9",
            "--out",
            "k.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let got = sound_secs(&ff, &d.path().join("k.mp4"));
    assert!((6.875..6.905).contains(&got), "sound {got} s: {v}");
    // 5 cuts, each placed where a whole decoded frame (21 ms) falls out of the part: on whole frames the
    // sound came out about 0.12 s short of the picture; on frames of 64 samples each cut is exact to 1.3 ms. The
    // limit is 30 ms because the pinned FFmpeg decodes the last AAC frame whole (up to 21 ms more than the header).
    tone_clip(
        &ff,
        d.path(),
        "q48.mp4",
        10,
        "volume=0.05,aformat=sample_rates=48000:channel_layouts=mono",
    );
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "edit",
            "q48.mp4",
            "--keep",
            "0.44-1.32,1.88-2.92,3.32-4.36,4.92-5.8,6.36-7.24,7.8-8.68",
            "--out",
            "k6.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let secs = v["metrics"]["seconds_after"].as_f64().unwrap();
    let got = sound_secs(&ff, &d.path().join("k6.mp4"));
    assert!(
        (got - secs).abs() < 0.03,
        "sound {got} s, picture {secs} s: {v}"
    );
    // The lengths the file states: whole frames gave "32.0 ms" on this 48 kHz mono clip, under the gate's limit of one frame but not exact.
    let drift = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "av_drift")
        .and_then(|f| {
            f["detail"]
                .as_str()?
                .strip_suffix(" ms")?
                .parse::<f64>()
                .ok()
        })
        .unwrap_or_else(|| panic!("no av_drift in ms: {v}"));
    assert!(drift <= 5.0, "{v}");
}

#[test]
fn detach_refuses_a_bad_brief_at_once() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("b.json"),
        r#"{"scenes":[{"script":"One two.","transition":"wipe"},{"script":"Three."}]}"#,
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .args(["make", "--brief", "b.json", "--detach", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("first scene"),
        "{v}"
    );
    assert!(
        !d.path().join(".scriptvox/runs").exists()
            || std::fs::read_dir(d.path().join(".scriptvox/runs"))
                .unwrap()
                .count()
                == 0
    );
}

#[test]
fn a_still_asks_for_an_image_row() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("b.json"),
        r#"{"scenes":[{"script":"One two three.","generate":{"prompt":"a boat","still":true}}]}"#,
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_CONFIG", d.path())
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args(["plan", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let text = v.to_string();
    assert!(text.contains("asks for a still"), "{text}");
}

#[test]
fn export_takes_a_file_and_says_its_terms_are_unknown() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=180x320:r=25:d=2",
        ])
        .args(["-pix_fmt", "yuv420p", "-y", "j.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args(["export", "j.mp4", "--title", "My title", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let text = std::fs::read_to_string(v["artifacts"]["metadata"].as_str().unwrap()).unwrap();
    let meta: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(meta["title"], "My title");
    // No receipt: terms not known, so null and no note (audit M1).
    assert_eq!(meta["commercial_ok"], serde_json::Value::Null);
    assert!(v["terms_note"].is_null(), "{v}");
}

#[test]
fn join_takes_the_shown_shape_of_a_turned_clip_and_keeps_earlier_results() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        assert!(std::process::Command::new(&ff)
            .current_dir(d.path())
            .args(["-v", "error"])
            .args(args)
            .status()
            .unwrap()
            .success())
    };
    run(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=25:d=2",
        "-pix_fmt",
        "yuv420p",
        "-y",
        "raw.mp4",
    ]);
    run(&[
        "-display_rotation",
        "90",
        "-i",
        "raw.mp4",
        "-c",
        "copy",
        "-y",
        "rot.mp4",
    ]);
    let join = || {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["join", "rot.mp4", "rot.mp4", "--json"])
            .output()
            .unwrap();
        serde_json::from_slice::<Value>(&o.stdout).unwrap()
    };
    let a = join();
    assert_eq!(a["metrics"]["size"], "180x320", "{a}");
    let b = join();
    assert_ne!(a["artifacts"]["video"], b["artifacts"]["video"], "{b}");
}

#[test]
fn clips_refuses_bad_ranges_before_any_call() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("talk.mp4"), b"x").unwrap();
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .args(["clips"])
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    assert_eq!(run(&["none.mp4"]).0, Some(2));
    assert_eq!(run(&["talk.mp4", "--min", "60", "--max", "20"]).0, Some(2));
    assert_eq!(run(&["talk.mp4", "--count", "0"]).0, Some(2));
    assert_eq!(run(&["talk.mp4", "--aspect", "3:2"]).0, Some(2));
    // The caption style flags go through the one check (`caption_style.rs`), before the talk is heard.
    let (code, v) = run(&["talk.mp4", "--caption-outline", "40"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("--caption-outline"),
        "{v}"
    );
    let (code, v) = run(&["talk.mp4", "--captions", "reveal", "--caption-box", "1"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("reveal has no box"),
        "{v}"
    );
}

#[test]
fn a_hook_alone_works_in_a_new_folder_and_a_letter_no_font_has_is_refused() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:d=2",
            "-c:v",
            "libx264",
        ])
        .arg(d.path().join("a.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let run = |hook: &str| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["edit", "a.mp4", "--hook", hook, "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run("\u{1F525}\u{1F525}");
    assert_eq!(code, Some(2), "{v}");
    let (code, v) = run("The short version");
    assert_eq!(code, Some(0), "{v}");
}

#[test]
fn dub_lines_start_on_their_own_times() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    let st = std::process::Command::new(&ff)
        .current_dir(p)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:d=12",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=500:d=12",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "src.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(
        p.join("b.json"),
        r#"{"base_video": "src.mp4", "lines": [
            {"start": 1.0, "end": 5.0, "text": "The first line of the new voice."},
            {"start": 7.0, "end": 11.0, "text": "The second line waits for its own time."}]}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["lines"][1]["start"], 7.0, "{v}");
    assert!(
        (v["metrics"]["duration_s"].as_f64().unwrap() - 12.0).abs() < 0.2,
        "{v}"
    );
    for brief in [
        r#"{"lines": [{"start": 1, "end": 2, "text": "no video"}]}"#,
        r#"{"base_video": "src.mp4", "script": "x", "lines": [{"start": 1, "end": 2, "text": "both"}]}"#,
        r#"{"base_video": "src.mp4", "lines": [{"start": 1, "end": 3, "text": "a"}, {"start": 2, "end": 4, "text": "b"}]}"#,
        r#"{"base_video": "src.mp4", "lines": [{"start": 11, "end": 20, "text": "past the end"}]}"#,
    ] {
        std::fs::write(p.join("d.json"), brief).unwrap();
        let o = e
            .cmd("ok")
            .args(["make", "--brief", "d.json", "--json"])
            .output()
            .unwrap();
        assert_eq!(
            o.status.code(),
            Some(2),
            "{brief}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
    }
}

#[test]
fn dub_of_a_scriptvox_video_warns_about_its_burned_captions() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    std::fs::write(p.join("s.txt"), "One short line for a small video.").unwrap();
    let run = |args: &[&str]| {
        let o = e.cmd("ok").args(args).output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code(), v)
    };
    let (code, v) = run(&["make", "--script", "s.txt", "--out", "made.mp4", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    std::fs::write(
        p.join("l.json"),
        r#"{"lines": [{"start": 0.2, "end": 2.0, "text": "A new line."}]}"#,
    )
    .unwrap();
    let (code, v) = run(&[
        "dub", "made.mp4", "--lines", "l.json", "--out", "d.mp4", "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["gate"] == "base_text" && f["status"] == "warn"),
        "{v}"
    );
}

#[test]
fn dub_runs_lines_as_one_make_and_keeps_the_source_size() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    let st = std::process::Command::new(&ff)
        .current_dir(p)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:d=8",
            "-f",
            "lavfi",
            "-i",
            "sine=f=500:d=8",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "src.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    // The sheet form: `room_s` is ignored, and the source has no words here, so only its form is tested.
    std::fs::write(
        p.join("l.json"),
        r#"{"lines": [{"start": 1.0, "end": 6.0, "room_s": 7.0, "text": "A new line on its own time."}]}"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        let o = e.cmd("ok").args(args).output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code(), v)
    };
    let (code, v) = run(&["dub", "src.mp4", "--json"]);
    assert_eq!(code, Some(2), "neither --plan nor --lines: {v}");
    std::fs::write(p.join("none.json"), "[]").unwrap();
    let (code, v) = run(&["dub", "src.mp4", "--lines", "none.json", "--json"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("no lines"),
        "{v}"
    );
    // A second caption line: one entry per sentence of the lines, checked before any voice.
    std::fs::write(p.join("two.txt"), "Une.\nDeux.\n").unwrap();
    let (code, v) = run(&[
        "dub",
        "src.mp4",
        "--lines",
        "l.json",
        "--captions-second",
        "two.txt",
        "--json",
    ]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v.to_string()
            .contains("has 2 entries, the script has 1 sentences"),
        "{v}"
    );
    std::fs::write(p.join("one.txt"), "Une nouvelle ligne.\n").unwrap();
    let (code, v) = run(&[
        "dub",
        "src.mp4",
        "--lines",
        "l.json",
        "--captions-second",
        "one.txt",
        "--plan",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    // The caption style flags become the three brief fields; a bad one stops with the flag in its fix line.
    let (code, v) = run(&[
        "dub",
        "src.mp4",
        "--lines",
        "l.json",
        "--caption-size",
        "huge",
        "--plan",
        "--json",
    ]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("--caption-size"),
        "{v}"
    );
    let (code, v) = run(&[
        "dub",
        "src.mp4",
        "--lines",
        "l.json",
        "--caption-size",
        "120%",
        "--caption-box",
        "0",
        "--caption-outline",
        "4",
        "--plan",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    let brief: Value = serde_json::from_str(
        &std::fs::read_to_string(p.join(".scriptvox/dub/src-brief.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (
            &brief["caption_size"],
            &brief["caption_box"],
            &brief["caption_outline"]
        ),
        (
            &serde_json::json!("120%"),
            &serde_json::json!(0.0),
            &serde_json::json!(4.0)
        ),
        "{brief}"
    );
    let (code, v) = run(&["dub", "src.mp4", "--lines", "l.json", "--plan", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["command"], "dub");
    assert!(v["plan_id"].is_string(), "{v}");
    let (code, v) = run(&[
        "dub",
        "src.mp4",
        "--lines",
        "l.json",
        "--base-sound",
        "none",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    let gate = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "source_size")
        .unwrap();
    assert_eq!(gate["status"], "pass", "{v}");
    assert!(
        gate["detail"].as_str().unwrap().contains("(640, 360)"),
        "{v}"
    );
    assert_eq!(v["metrics"]["lines"][0]["start"], 1.0, "{v}");
}

/// G069: `--duration` sets the speed from the kept length, and refuses outside 0.5 to 2.
#[test]
fn duration_sets_the_speed_within_its_limits() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    av_source(&ff, &d.path().join("t.mp4"), 4.0, 4.0, -6);
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["edit", "t.mp4", "--json"])
            .args(args)
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run(&["--duration", "3"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        (v["metrics"]["seconds_after"].as_f64().unwrap() - 3.0).abs() < 0.05,
        "{v}"
    );
    let (code, v) = run(&["--duration", "10"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("0.40x"),
        "{v}"
    );
    let (code, _) = run(&["--duration", "3", "--speed", "1.2"]);
    assert_eq!(code, Some(2));
}

/// G081: captions with `--speed` are heard on the sped-up sound, so their times are the result's; a VTT sits beside the SRT.
#[test]
fn speed_with_captions_is_allowed_now() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    av_source(&ff, &d.path().join("t.mp4"), 2.0, 2.0, -6);
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "edit",
            "t.mp4",
            "--speed",
            "1.5",
            "--captions",
            "karaoke",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    // No speech row in the test data folder: the refusal is about the row now, not about --speed.
    assert!(
        !v.to_string().contains("does not mix with --captions"),
        "{v}"
    );
}

#[test]
fn captions_second_goes_under_each_sentence_and_a_wrong_count_is_refused() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=s=640x360:d=4"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono:d=4",
            "-shortest",
        ])
        .args(["-c:v", "libx264", "-c:a", "aac"])
        .arg(d.path().join("a.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let srt = "1\n00:00:00,200 --> 00:00:01,800\nRain falls.\n\n2\n00:00:02,000 --> 00:00:03,600\nDrains work!\n";
    std::fs::write(d.path().join("a.srt"), srt).unwrap();
    let run = |second: &str| {
        std::fs::write(d.path().join("s.txt"), second).unwrap();
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args([
                "edit",
                "a.mp4",
                "--captions-file",
                "a.srt",
                "--captions-second",
                "s.txt",
            ])
            .args(["--out", "b.mp4", "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run("يسقط المطر.\n");
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v.to_string()
            .contains("1 lines, the captions have 2 sentences"),
        "{v}"
    );
    let (code, v) = run("يسقط المطر.\n\nالمصارف تعمل!\n");
    assert_eq!(code, Some(2), "{v}");
    assert!(!d.path().join("b.mp4").exists());
    let (code, v) = run("يسقط المطر.\nالمصارف تعمل!\n");
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["captions"]["second_lines"], 2, "{v}");
    let ass = std::fs::read_to_string(d.path().join(".scriptvox/edit/a/captions.ass"))
        .unwrap_or_default();
    assert!(
        ass.is_empty() || ass.matches(",Cap2,").count() == 2,
        "{ass}"
    );
}

fn clip(ff: &str, dir: &std::path::Path, name: &str) {
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=3",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .args(["-c:a", "aac", "-shortest", "-y", name])
        .status()
        .unwrap();
    assert!(st.success());
}

fn rv(ff: &str, dir: &std::path::Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
}

#[cfg(unix)]
#[test]
fn an_out_that_reaches_the_input_is_refused_and_the_input_stays() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    clip(&ff, d.path(), "a.mp4");
    clip(&ff, d.path(), "b.mp4");
    std::os::unix::fs::symlink("a.mp4", d.path().join("link.mp4")).unwrap();
    let before = std::fs::read(d.path().join("a.mp4")).unwrap();
    for args in [
        vec![
            "edit",
            "a.mp4",
            "--from",
            "1",
            "--to",
            "2",
            "--out",
            "sub/../a.mp4",
        ],
        vec![
            "edit", "a.mp4", "--from", "1", "--to", "2", "--out", "link.mp4",
        ],
        vec![
            "edit", "a.mp4", "--from", "1", "--to", "2", "--out", "a.mp4",
        ],
        vec!["join", "b.mp4", "a.mp4", "--out", "sub/../a.mp4"],
        vec!["join", "b.mp4", "a.mp4", "--out", "link.mp4"],
        vec!["dub", "a.mp4", "--lines", "l.txt", "--out", "a.mp4"],
        vec!["dub", "a.mp4", "--lines", "l.txt", "--out", "link.mp4"],
    ] {
        let (code, v) = rv(&ff, d.path(), &args);
        assert_eq!(code, Some(2), "{args:?}: {v}");
        assert!(v.to_string().contains("--out is"), "{args:?}: {v}");
        assert!(
            std::fs::read(d.path().join("a.mp4")).unwrap() == before,
            "{args:?}"
        );
    }
}

#[test]
fn export_of_the_video_in_its_own_folder_keeps_it() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let x = d.path().join("x");
    std::fs::create_dir(&x).unwrap();
    clip(&ff, &x, "video.mp4");
    let before = std::fs::read(x.join("video.mp4")).unwrap();
    let (code, v) = rv(&ff, d.path(), &["export", "x/video.mp4", "--dir", "x"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(std::fs::read(x.join("video.mp4")).unwrap() == before);
    // A converted copy that would land on the input is refused too.
    std::fs::rename(x.join("video.mp4"), x.join("video.webm")).unwrap();
    let (code, v) = rv(
        &ff,
        d.path(),
        &["export", "x/video.webm", "--dir", "x", "--format", "webm"],
    );
    assert_eq!(code, Some(2), "{v}");
    assert!(std::fs::read(x.join("video.webm")).unwrap() == before);
}

#[test]
fn export_refuses_a_file_that_is_not_a_video() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("fake.mp4"), "hello\n").unwrap();
    let (code, v) = rv(&ff, d.path(), &["export", "fake.mp4"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(!d.path().join("scriptvox-out/fake-export").exists(), "{v}");
}

#[test]
fn edit_keeps_the_caption_file_it_read_beside_its_out() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    clip(&ff, d.path(), "v.mp4");
    let srt = "1\n00:00:00,200 --> 00:00:02,800\n<i>Rain</i> falls on the road, and the drains work well today.\n";
    std::fs::write(d.path().join("x.srt"), srt).unwrap();
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "v.mp4",
            "--captions-file",
            "x.srt",
            "--out",
            "x.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(
        std::fs::read_to_string(d.path().join("x.srt")).unwrap(),
        srt
    );
    assert!(
        v["artifacts"]["sidecar_srt"]
            .as_str()
            .unwrap()
            .ends_with("x-2.srt"),
        "{v}"
    );
    assert!(d.path().join("x-2.srt").is_file());
}

#[test]
fn snapshot_at_names_a_value_it_cannot_read() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    clip(&ff, d.path(), "v.mp4");
    for at in ["abc", "1,abc"] {
        let (code, v) = rv(
            &ff,
            d.path(),
            &["snapshot", "v.mp4", "--at", at, "--out", "s.png"],
        );
        assert_eq!(code, Some(2), "{at}: {v}");
        assert!(v.to_string().contains("\\\"abc\\\""), "{at}: {v}");
        assert!(!d.path().join("s.png").exists(), "{at}");
    }
    let (code, v) = rv(
        &ff,
        d.path(),
        &["snapshot", "v.mp4", "--at", "1,2.5", "--out", "s.png"],
    );
    assert_eq!(code, Some(0), "{v}");
}

#[test]
fn a_hard_link_to_the_input_is_the_input() {
    let d = tempfile::tempdir().unwrap();
    let (a, b, c) = (d.path().join("a"), d.path().join("b"), d.path().join("c"));
    std::fs::write(&a, "x").unwrap();
    std::fs::write(&c, "x").unwrap();
    std::fs::hard_link(&a, &b).unwrap();
    assert!(scriptvox::engine_ffmpeg::same_file(&b, &a));
    assert!(!scriptvox::engine_ffmpeg::same_file(&c, &a));
}

fn small_clip(ff: &str, out: &std::path::Path) {
    let st = std::process::Command::new(ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30:d=3",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-y",
        ])
        .arg(out)
        .status()
        .unwrap();
    assert!(st.success());
}

#[test]
fn edit_never_writes_over_its_music_file() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    small_clip(&ff, &d.path().join("a.mp4"));
    let m = d.path().join("m.m4a");
    std::fs::write(&m, "my song").unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .args([
            "edit", "a.mp4", "--music", "m.m4a", "--out", "m.m4a", "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(2), "{v}");
    assert_eq!(std::fs::read_to_string(&m).unwrap(), "my song", "{v}");
}

/// The gates cannot read the result: edit, join and make say so, exit 3.
#[test]
#[cfg(unix)]
fn a_failed_gate_read_is_not_a_clean_pass() {
    use std::os::unix::fs::PermissionsExt;
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    small_clip(&ff, &p.join("a.mp4"));
    small_clip(&ff, &p.join("b.mp4"));
    let bin = p.join("fakebin");
    std::fs::create_dir(&bin).unwrap();
    let wrap = bin.join("ffmpeg");
    std::fs::write(
        &wrap,
        format!("#!/bin/sh\ncase \"$*\" in *blackdetect*) echo boom >&2; exit 1;; esac\nexec \"{ff}\" \"$@\"\n"),
    )
    .unwrap();
    std::fs::set_permissions(&wrap, std::fs::Permissions::from_mode(0o755)).unwrap();
    let probe = std::path::Path::new(&ff).with_file_name("ffprobe");
    std::os::unix::fs::symlink(probe, bin.join("ffprobe")).unwrap();
    std::fs::write(p.join("s.txt"), "A short line for the gate.").unwrap();
    let cases: [&[&str]; 3] = [
        &[
            "edit", "a.mp4", "--from", "0", "--to", "2", "--out", "c.mp4", "--json",
        ],
        &["join", "a.mp4", "b.mp4", "--out", "d.mp4", "--json"],
        &["make", "--script", "s.txt", "--out", "f.mp4", "--json"],
    ];
    for args in cases {
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_FFMPEG", &wrap)
            .args(args)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(o.status.code(), Some(3), "{args:?}: {v}");
    }
}
