//! `make --until voice` (docs/specs/make-until.md) with the stub worker: the voice and the caption files, no video, and
//! `resume` finishes the video with the voice from the cache.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

const SCRIPT: &str = "One two three four five six seven eight.";

fn go(e: &Env, args: &[&str]) -> (i32, Value) {
    let o = e.cmd("ok").args(args).arg("--json").output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    (
        o.status.code().unwrap(),
        serde_json::from_str(text.lines().last().unwrap_or("null")).unwrap_or(Value::Null),
    )
}

/// Every MP4 under `dir`.
fn videos(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() && p.file_name().is_some_and(|n| n != "worker" && n != "data") {
            out.extend(videos(&p));
        } else if p.extension().is_some_and(|x| x == "mp4") {
            out.push(p);
        }
    }
    out
}

/// The stopped run: ok, the voice and the caption files, no video anywhere, and a `next` that names `resume`.
fn stopped(e: &Env, v: &Value) -> String {
    assert_eq!(v["status"], "ok", "{v}");
    assert_eq!(v["metrics"]["until"], "voice", "{v}");
    for k in [
        "audio",
        "words",
        "captions_srt",
        "captions_vtt",
        "captions_ass",
    ] {
        let p = v["artifacts"][k]
            .as_str()
            .unwrap_or_else(|| panic!("no {k}: {v}"));
        assert!(std::fs::metadata(p).is_ok_and(|m| m.len() > 0), "{k}: {p}");
    }
    assert!(v["artifacts"].get("video").is_none(), "{v}");
    assert_eq!(videos(e.dir.path()), Vec::<std::path::PathBuf>::new());
    let id = v["run_id"].as_str().unwrap().to_string();
    assert!(
        v["next"]
            .to_string()
            .contains(&format!("scriptvox resume {id} --wait")),
        "{v}"
    );
    id
}

#[test]
fn until_voice_stops_before_the_render_and_resume_finishes_with_the_voice_reused() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
    // The plan lists the voice alone, under its own id. `--stock` would add a stage and a search.
    let full = ["--script", "s.txt", "--stock", "ocean"];
    let (code, all) = go(&e, &[&["plan"][..], &full].concat());
    assert_eq!(code, 0, "{all}");
    let (code, p) = go(&e, &[&["plan"][..], &full, &["--until", "voice"]].concat());
    assert_eq!(code, 0, "{p}");
    let names = |v: &Value| -> Vec<String> {
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["stage"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&all), ["voice", "stock", "render", "check"]);
    assert_eq!(names(&p), ["voice"], "{p}");
    assert_ne!(all["plan_id"], p["plan_id"]);

    let (code, v) = go(&e, &[&["make"][..], &full, &["--until", "voice"]].concat());
    assert_eq!(code, 0, "{v}");
    let id = stopped(&e, &v);
    let dir = e.dir.path().join(".scriptvox/runs").join(&id);
    assert!(!dir.join("credits.txt").exists(), "no stock was fetched");
    assert!(
        v["metrics"]["voice_s"].as_f64().is_some_and(|s| s > 2.5),
        "{v}"
    );

    // `status` and `wait` tell the same story: a finished run with no video, not a failed one.
    let (code, w) = go(&e, &["wait", &id]);
    assert_eq!(code, 0, "{w}");
    stopped(&e, &w);
    let (_, s) = go(&e, &["status"]);
    let row = s["runs"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["id"] == id.as_str()))
        .unwrap_or_else(|| panic!("no row: {s}"));
    assert_eq!(row["status"], "ok", "{s}");
    assert!(row["video"].is_null(), "{s}");

    // Plain `resume`: the whole video, the voice from the cache. No stock here, so the test stays off the network.
    std::fs::write(e.dir.path().join("p.json"), r#"{"stock": null}"#).unwrap();
    let (code, r) = go(&e, &["resume", &id, "--patch", "p.json", "--wait"]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["metrics"]["voice_reused"], true, "{r}");
    assert!(r["metrics"].get("until").is_none(), "{r}");
    let video = r["artifacts"]["video"].as_str().unwrap();
    assert!(std::fs::metadata(video).is_ok_and(|m| m.len() > 0), "{r}");
}

#[test]
fn plain_resume_of_a_stopped_run_makes_the_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make(
        SCRIPT,
        "ok",
        &[
            "--until",
            "voice",
            "--duration",
            "3.2",
            "--out",
            "final.mp4",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let id = stopped(&e, &v);
    // The fit ran before the stop.
    assert!(
        (v["metrics"]["voice_tempo"].as_f64().unwrap() - 0.938).abs() < 0.01,
        "{v}"
    );
    let (code, r) = go(&e, &["resume", &id, "--wait"]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["metrics"]["voice_reused"], true, "{r}");
    assert_eq!(r["metrics"]["duration_s"], 3.2, "{r}");
    // The stopped run wrote no `--out`, so its child takes that name, not `final.1.mp4`.
    assert_eq!(
        videos(e.dir.path()),
        [e.dir.path().join("final.mp4")],
        "{r}"
    );
}

#[test]
fn another_stage_is_a_usage_error_that_names_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    for cmd in ["make", "plan"] {
        std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
        let (code, v) = go(&e, &[cmd, "--script", "s.txt", "--until", "clips"]);
        assert_eq!(code, 2, "{v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains("clips") && msg.contains("voice"), "{v}");
    }
    assert!(
        !e.dir.path().join(".scriptvox/runs").exists(),
        "nothing ran"
    );
}

#[test]
fn detached_until_voice_ends_ok_and_wait_says_how_to_finish() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), SCRIPT).unwrap();
    let (code, v) = go(
        &e,
        &["make", "--script", "s.txt", "--until", "voice", "--detach"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "running", "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    let (code, w) = go(&e, &["wait", &id]);
    assert_eq!(code, 0, "{w}");
    assert_eq!(stopped(&e, &w), id);
}
