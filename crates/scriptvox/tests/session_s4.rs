//! The plan's §4 agent session, step by step, with the stub worker (RVX-027).
//! The stub voice is always 3 s. A 6-word script for 1.9 s passes the estimate
//! before TTS but measures 1.58x after it (a fitter failure). A 4-word script
//! for 3.05 s fits.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;

fn run(e: &Env, mode: &str, args: &[&str]) -> (i32, Value) {
    let o = e.cmd(mode).args(args).arg("--json").output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    (
        o.status.code().unwrap(),
        serde_json::from_str(s.lines().last().unwrap_or("null")).unwrap(),
    )
}

#[test]
fn section_4_session() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    for f in ["dates-ar.json", "dates-ar-short.json"] {
        std::fs::copy(root().join("fixtures/briefs").join(f), e.dir.path().join(f)).unwrap();
    }
    // 1. doctor
    let (code, v) = run(&e, "ok", &["doctor", "--offline"]);
    assert_eq!(code, 0, "doctor: {v}");
    // 2. plan: cost 0
    let (code, v) = run(
        &e,
        "ok",
        &[
            "plan",
            "--brief",
            "dates-ar.json",
            "--duration",
            "1.9",
            "--max-spend",
            "0",
        ],
    );
    assert_eq!(
        (code, v["metrics"]["cost_usd"].as_f64()),
        (0, Some(0.0)),
        "plan: {v}"
    );
    // 3. make --detach
    let (code, v) = run(
        &e,
        "ok",
        &[
            "make",
            "--brief",
            "dates-ar.json",
            "--duration",
            "1.9",
            "--detach",
        ],
    );
    assert_eq!(code, 0, "detach: {v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    // 4. wait: exit 3 with the fitter's finding, before any render
    let (code, v) = run(&e, "ok", &["wait", &id]);
    assert_eq!(code, 3, "wait: {v}");
    assert_eq!(v["error"]["code"], "SVX_NEEDS_SHORTER_SCRIPT");
    // 5. resume with the revised brief and a new length: child .1 (media and
    // language come from the parent's saved request).
    let (code, v) = run(
        &e,
        "ok",
        &[
            "resume",
            &id,
            "--brief",
            "dates-ar-short.json",
            "--duration",
            "3.05",
        ],
    );
    assert_eq!(code, 0, "resume: {v}");
    let child = v["run_id"].as_str().unwrap().to_string();
    assert_eq!(child, format!("{id}.1"));
    // 6. wait on the child: exit 0
    let (code, v) = run(&e, "ok", &["wait", &child]);
    assert_eq!(code, 0, "child: {v}");
    assert_eq!(v["parent_run"], id.as_str());
    assert!(v["artifacts"]["video"]
        .as_str()
        .unwrap()
        .contains("scriptvox-out"));
    // 7. snapshot --sheet 9
    let (code, v) = run(&e, "ok", &["snapshot", &child, "--sheet", "9"]);
    assert_eq!(code, 0, "snapshot: {v}");
    assert!(std::path::Path::new(v["artifacts"]["image"].as_str().unwrap()).is_file());
}

/// A detached run that stops at the estimate before TTS (far too long): `wait` replays that error, not a dead
/// supervisor, and `resume` finds the saved request (4 Nov 2026, found recording the session GIF).
#[test]
fn detached_early_stop_is_replayed_and_resumable() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    for f in ["dates-ar.json", "dates-ar-short.json"] {
        std::fs::copy(root().join("fixtures/briefs").join(f), e.dir.path().join(f)).unwrap();
    }
    let (code, v) = run(
        &e,
        "ok",
        &[
            "make",
            "--brief",
            "dates-ar.json",
            "--duration",
            "0.3",
            "--detach",
        ],
    );
    assert_eq!(code, 0, "detach: {v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    let (code, v) = run(&e, "ok", &["wait", &id]);
    assert_eq!(
        (code, v["error"]["code"].as_str()),
        (3, Some("SVX_NEEDS_SHORTER_SCRIPT")),
        "wait: {v}"
    );
    let (code, v) = run(
        &e,
        "ok",
        &[
            "resume",
            &id,
            "--brief",
            "dates-ar-short.json",
            "--duration",
            "3.05",
        ],
    );
    assert_eq!(code, 0, "resume: {v}");
}
