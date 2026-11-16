#![cfg(unix)]
//! Chunked render: kill and resume, and the watchdog (RVX-017).
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

fn json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

const SCRIPT: &str = "First shot here. Second shot now. Third one comes. And the last.";

#[test]
fn killed_render_resumes_and_matches_a_clean_render() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
    let make = |fault: Option<&str>, out: &str| {
        let mut c = e.cmd("ok");
        c.env("SCRIPTVOX_CHUNK_S", "1").args([
            "make", "--script", "s.txt", "--media", "media", "--json", "--out", out,
        ]);
        if let Some(n) = fault {
            c.env("SCRIPTVOX_FAULT_AFTER_CHUNKS", n);
        }
        c.output().unwrap()
    };
    // 3.6 s at 1 s chunks = 4 chunks. Stop after 2, as a kill would.
    let out = make(Some("2"), "a.mp4");
    assert_eq!(out.status.code(), Some(1), "{}", json(&out));
    let out = make(None, "a.mp4");
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(
        (
            v["metrics"]["chunks"].as_u64(),
            v["metrics"]["resumed_from_chunk"].as_u64()
        ),
        (Some(4), Some(2)),
        "{v}"
    );
    // A clean render of the same job (the chunks were removed after success).
    let out = make(None, "b.mp4");
    assert_eq!(json(&out)["metrics"]["resumed_from_chunk"], 0);
    let psnr = std::process::Command::new(&ff)
        .current_dir(e.dir.path())
        .args([
            "-hide_banner",
            "-i",
            "a.mp4",
            "-i",
            "b.mp4",
            "-lavfi",
            "[0:v][1:v]psnr",
            "-f",
            "null",
            "-",
        ])
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

/// A kept chunk with the wrong length: the readback refuses the video, the fix line says what to do, and doing it works.
#[test]
fn a_bad_kept_chunk_fails_the_readback_with_a_fix_that_works() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
    let make = |fault: Option<&str>| {
        let mut c = e.cmd("ok");
        c.env("SCRIPTVOX_CHUNK_S", "1").args([
            "make", "--script", "s.txt", "--media", "media", "--json", "--out", "a.mp4",
        ]);
        if let Some(n) = fault {
            c.env("SCRIPTVOX_FAULT_AFTER_CHUNKS", n);
        }
        let o = c.output().unwrap();
        (o.status.code(), json(&o))
    };
    assert_eq!(make(Some("2")).0, Some(1));
    // The second kept chunk loses most of its frames, as a file cut short would.
    let chunks = e.dir.path().join(".scriptvox/chunks");
    let key = std::fs::read_dir(&chunks)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-i"])
        .arg(key.join("chunk_0001.mp4"))
        .args(["-frames:v", "3", "-y"])
        .arg(key.join("short.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::rename(key.join("short.mp4"), key.join("chunk_0001.mp4")).unwrap();
    let (code, v) = make(None);
    assert_eq!(code, Some(3), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("readback: "),
        "{v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("run the same command again"),
        "{v}"
    );
    // The fix: the same command again.
    let (code, v) = make(None);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["resumed_from_chunk"], 0, "{v}");
}

#[test]
fn watchdog_kills_a_stalled_render_with_exit_7() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    // A fake ffmpeg: the render pass (the only one with -progress) hangs.
    let fake = e.dir.path().join("fakebin");
    std::fs::create_dir_all(&fake).unwrap();
    let script = format!("#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = \"-progress\" ] && exec sleep 60; done\nexec '{ff}' \"$@\"\n");
    std::fs::write(fake.join("ffmpeg"), script).unwrap();
    std::os::unix::fs::symlink(
        Path::new(&ff).with_file_name("ffprobe"),
        fake.join("ffprobe"),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(fake.join("ffmpeg"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(e.dir.path().join("s.txt"), "Hello there.").unwrap();
    let t0 = std::time::Instant::now();
    let out = e
        .cmd("ok")
        .env("SCRIPTVOX_FFMPEG", fake.join("ffmpeg"))
        .env("SCRIPTVOX_WATCHDOG_S", "2")
        .args(["make", "--script", "s.txt", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(7), "{v}");
    assert!(v["error"]["fix"].as_str().unwrap().contains("resumes"));
    assert!(
        t0.elapsed().as_secs() < 30,
        "the watchdog acted, not the 60 s sleep"
    );
}

#[test]
fn only_scene_cuts_one_scene_from_the_cache_with_no_new_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let brief = serde_json::json!({"tts": "none", "aspect": "9:16", "scenes": [
        {"script": "One short scene to start with here.", "card": {"kind": "title", "text": "First"}},
        {"script": "A second scene that the agent fixes now.", "card": {"kind": "title", "text": "Second"}},
        {"script": "And a third one to end the short video.", "card": {"kind": "title", "text": "Third"}},
    ]});
    std::fs::write(p.join("b.json"), brief.to_string()).unwrap();
    let v = json(
        &e.cmd("ok")
            .args(["make", "--brief", "b.json", "--out", "a.mp4", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(v["status"], "ok", "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let runs = || {
        std::fs::read_dir(p.join(".scriptvox/runs"))
            .unwrap()
            .count()
    };
    let before = runs();
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--only-scene", "2", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert_eq!(runs(), before, "no new run: nothing was planned or called");
    let clip = v["artifacts"]["video"].as_str().unwrap().to_string();
    let dur = |f: &str| -> f64 {
        let o = std::process::Command::new(Path::new(&ff).with_file_name("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                f,
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).trim().parse().unwrap()
    };
    let (start, end) = (
        v["metrics"]["start_s"].as_f64().unwrap(),
        v["metrics"]["end_s"].as_f64().unwrap(),
    );
    assert!(start > 0.0 && end > start, "{v}");
    assert!(
        (dur(&clip) - (end - start)).abs() < 0.2,
        "{} {start} {end}",
        dur(&clip)
    );
    assert!(dur(&clip) < dur(p.join("a.mp4").to_str().unwrap()) - 1.0);
    // Out of range and a run with no scenes say so.
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--only-scene", "9", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", json(&out));
    assert!(json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("3 scenes"));
    // A changed title makes a new run through the normal path, and cuts from that.
    std::fs::write(
        p.join("p.json"),
        serde_json::json!({"hook": "New hook"}).to_string(),
    )
    .unwrap();
    let out = e
        .cmd("ok")
        .args([
            "resume",
            &run,
            "--only-scene",
            "1",
            "--patch",
            "p.json",
            "--json",
        ])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(runs(), before + 1, "{v}");
    assert!(
        std::path::Path::new(v["artifacts"]["scene_video"].as_str().unwrap()).is_file(),
        "{v}"
    );
    // `--plan` shows the plan of the child and starts no run (skills test, 11 Nov 2026: it started one).
    let now = runs();
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--patch", "p.json", "--plan", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert!(v["plan_id"].is_string() && v["stages"].is_array(), "{v}");
    assert_eq!(v["patched"][0], "hook", "{v}");
    assert_eq!(runs(), now, "{v}");
    // later-1: a cut that fails after the run stays a finding. The run id and the video are not lost.
    let out = e
        .cmd("ok")
        .args([
            "resume",
            &run,
            "--only-scene",
            "9",
            "--patch",
            "p.json",
            "--json",
        ])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert!(v["run_id"].as_str().unwrap().contains(&run), "{v}");
    assert!(
        std::path::Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file(),
        "{v}"
    );
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["gate"] == "only_scene" && f["status"] == "warn"),
        "{v}"
    );
}

/// make-8: a resume of a run that a live supervisor still works on is refused.
#[test]
fn resume_refuses_a_parent_that_is_still_running() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let (code, v) = e.make("A short video to resume.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    // The state of a live supervisor: no result yet, and a live pid (this test) on the run.
    std::fs::remove_file(p.join(".scriptvox/runs").join(&run).join("result.json")).unwrap();
    let db = rusqlite::Connection::open(p.join(".scriptvox/state.db")).unwrap();
    db.execute(
        "UPDATE runs SET pid = ?1 WHERE id = ?2",
        rusqlite::params![std::process::id(), run],
    )
    .unwrap();
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--wait", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("still running"),
        "{v}"
    );
}

/// make-2: new words drop the parent's scenes and cards, in a brief and in a patch. The same words keep them.
#[test]
fn new_words_drop_the_parents_scenes_and_the_same_words_keep_them() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let write = |n: &str, v: serde_json::Value| std::fs::write(p.join(n), v.to_string()).unwrap();
    write(
        "b.json",
        serde_json::json!({"tts": "none", "aspect": "9:16", "scenes": [
            {"script": "One short scene to start with here.", "card": {"kind": "title", "text": "OLD ONE"}},
            {"script": "A second scene that ends it all now.", "card": {"kind": "title", "text": "OLD TWO"}},
        ]}),
    );
    let v = json(
        &e.cmd("ok")
            .args(["make", "--brief", "b.json", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(v["status"], "ok", "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let scenes = |how: &str, file: &str| {
        let v = json(
            &e.cmd("ok")
                .args(["resume", &run, how, file, "--wait", "--json"])
                .output()
                .unwrap(),
        );
        assert_eq!(v["status"], "ok", "{v}");
        let req = p
            .join(".scriptvox/runs")
            .join(v["run_id"].as_str().unwrap())
            .join("request.json");
        let req: Value = serde_json::from_str(&std::fs::read_to_string(req).unwrap()).unwrap();
        req["scenes"].as_array().map_or(0, Vec::len)
    };
    let new = "A totally new script that has other words in it, and it runs for a while.";
    write(
        "new.json",
        serde_json::json!({"tts": "none", "aspect": "9:16", "script": new}),
    );
    assert_eq!(scenes("--brief", "new.json"), 0, "a brief with new words");
    write("p.json", serde_json::json!({"script": new}));
    assert_eq!(scenes("--patch", "p.json"), 0, "a patch with new words");
    let same = "One short scene to start with here. A second scene that ends it all now.";
    write("same.json", serde_json::json!({"script": same}));
    assert_eq!(
        scenes("--patch", "same.json"),
        2,
        "the same words keep the scenes"
    );
}

/// make-3: a file changed in place after a failed render does not reuse the chunks of the old file.
#[test]
fn a_media_file_changed_in_place_starts_the_render_over() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
    let make = |fault: Option<&str>| {
        let mut c = e.cmd("ok");
        c.env("SCRIPTVOX_CHUNK_S", "1")
            .args(["make", "--script", "s.txt", "--media", "media", "--json"]);
        if let Some(n) = fault {
            c.env("SCRIPTVOX_FAULT_AFTER_CHUNKS", n);
        }
        json(&c.output().unwrap())
    };
    let v = make(Some("2"));
    assert_eq!(v["ok"], false, "{v}");
    // Same path, new bytes (a JPEG decoder stops at its end marker, so the picture still opens).
    let photo = e.dir.path().join("media/02-fractal.jpg");
    let mut bytes = std::fs::read(&photo).unwrap();
    bytes.push(0);
    std::fs::write(&photo, bytes).unwrap();
    let v = make(None);
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(v["metrics"]["resumed_from_chunk"], 0, "{v}");
}

/// make-8: a running child blocks a sister resume, which would share the parent's folders and jobs.
#[test]
fn resume_refuses_while_a_child_of_the_run_is_running() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let (code, v) = e.make("A short video to resume.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--wait", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    let child = v["run_id"].as_str().unwrap().to_string();
    std::fs::remove_file(p.join(".scriptvox/runs").join(&child).join("result.json")).unwrap();
    let db = rusqlite::Connection::open(p.join(".scriptvox/state.db")).unwrap();
    db.execute(
        "UPDATE runs SET pid = ?1 WHERE id = ?2",
        rusqlite::params![std::process::id(), child],
    )
    .unwrap();
    let out = e
        .cmd("ok")
        .args(["resume", &run, "--wait", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains(&child),
        "{v}"
    );
}

/// docs/specs/resume-scene-voice.md: `resume --patch` with new words in scene 2 voices scene 2 alone. Scenes 1 and 3
/// come from the voice cache at the parent's pace, with the same samples, and scene 1 keeps its start time.
#[test]
fn a_change_in_one_scene_voices_that_scene_alone_and_keeps_the_pace() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let scenes = |second: &str| {
        serde_json::json!([
            {"script": "One short scene to start with here."},
            {"script": second},
            {"script": "And a third one to end the short video."},
        ])
    };
    // Three reads of 3 s and two breaths are about 9.6 s: 9 s needs a tempo change.
    let brief = serde_json::json!({"aspect": "9:16", "duration": 9, "scenes": scenes("A second scene that the agent fixes now.")});
    std::fs::write(p.join("b.json"), brief.to_string()).unwrap();
    let parent = json(
        &e.cmd("ok")
            .args(["make", "--brief", "b.json", "--out", "a.mp4", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(parent["status"], "ok", "{parent}");
    assert_ne!(parent["metrics"]["voice_tempo"], 1.0, "{parent}");
    let run = parent["run_id"].as_str().unwrap().to_string();
    let patch = serde_json::json!({"scenes": scenes("Other words for the scene in the middle.")});
    std::fs::write(p.join("p.json"), patch.to_string()).unwrap();
    let plan = json(
        &e.cmd("ok")
            .args(["resume", &run, "--patch", "p.json", "--plan", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(plan["voiced_again"], serde_json::json!([2]), "{plan}");
    let explain = plan["stages"][0]["explain"].as_str().unwrap_or_default();
    assert!(
        explain.contains("voiced again: scene 2; voice kept: scene 1, 3"),
        "{plan}"
    );
    let child = json(
        &e.cmd("ok")
            .args(["resume", &run, "--patch", "p.json", "--wait", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(child["status"], "ok", "{child}");
    assert_eq!(
        child["metrics"]["voice_tempo"], parent["metrics"]["voice_tempo"],
        "{child}"
    );
    let dir = |id: &str| p.join(".scriptvox/runs").join(id);
    let (old, new) = (dir(&run), dir(child["run_id"].as_str().unwrap()));
    // Parts 00, 02 and 04 are the scenes; 01 and 03 are the breaths between them.
    for part in ["seg_00", "seg_04"] {
        assert_eq!(
            std::fs::read(old.join(part).join("trimmed.wav")).unwrap(),
            std::fs::read(new.join(part).join("trimmed.wav")).unwrap(),
            "{part}: the same samples"
        );
        // The stub writes rate.txt on each call: none here, the part came from the cache.
        assert!(
            !new.join(part).join("rate.txt").exists(),
            "{part} was voiced again"
        );
    }
    assert!(
        new.join("seg_02/rate.txt").exists(),
        "scene 2 was not voiced"
    );
    let starts = |d: &Path| -> Vec<f64> {
        serde_json::from_str(&std::fs::read_to_string(d.join("scenes.json")).unwrap()).unwrap()
    };
    assert_eq!(
        starts(&old)[..2],
        starts(&new)[..2],
        "scenes 1 and 2 start where they did"
    );
    // The child's captions are beside the child's video, and the result says where.
    for ext in ["srt", "vtt"] {
        let at = child["artifacts"][format!("sidecar_{ext}")]
            .as_str()
            .unwrap_or_default();
        assert!(
            at.ends_with(&format!("a.1.{ext}")) && Path::new(at).is_file(),
            "{child}"
        );
    }
}
