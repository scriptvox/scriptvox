//! Brief `voice_rate` (docs/specs/voice-rate.md) with the stub worker: the rate reaches the worker, the voice key, the
//! plan id and `metrics.voice_rate_pct`; with `duration` the rate stays and only the tempo moves. The stub's audio is
//! always 3 s, so these tests prove the wiring, not the sound: the spec has the real runs.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};

const SCRIPT: &str = "One two three four five six seven eight.";

fn go(e: &Env, args: &[&str]) -> (i32, Value) {
    let o = e.cmd("ok").args(args).arg("--json").output().unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

/// Writes b.json: the script, 16:9 and `extra`.
fn brief(e: &Env, extra: Value) {
    let mut b = json!({"script": SCRIPT, "aspect": "16:9"});
    b.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
}

fn plan(e: &Env, extra: Value) -> Value {
    brief(e, extra);
    let (code, v) = go(e, &["plan", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    v
}

fn make(e: &Env, extra: Value) -> (i32, Value) {
    brief(e, extra);
    go(e, &["make", "--brief", "b.json"])
}

/// The rate the stub worker got in the run's one voice call.
fn got_rate(e: &Env, v: &Value) -> String {
    let run = v["run_id"].as_str().unwrap();
    std::fs::read_to_string(
        e.dir
            .path()
            .join(".scriptvox/runs")
            .join(run)
            .join("rate.txt"),
    )
    .unwrap_or_else(|_| "no call".into())
}

#[test]
fn the_rate_reaches_the_worker_the_voice_key_the_plan_id_and_the_metric() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let none = plan(&e, json!({}))["plan_id"].clone();
    assert_eq!(
        plan(&e, json!({"voice_rate": 1.0}))["plan_id"],
        none,
        "1.0 is no field"
    );
    let slow = plan(&e, json!({"voice_rate": 0.85}));
    assert_ne!(slow["plan_id"], none);
    assert!(
        slow["stages"][0]["explain"]
            .as_str()
            .unwrap()
            .contains("-15 percent"),
        "the plan says the pace: {slow}"
    );

    let (code, v) = make(&e, json!({}));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["voice_rate_pct"], 0, "{v}");
    assert_eq!(got_rate(&e, &v), "", "no rate goes to the worker");

    let (code, v) = make(&e, json!({"voice_rate": 0.85}));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["voice_rate_pct"], -15, "{v}");
    assert!(
        v["metrics"]["voice_reused"].is_null(),
        "a new rate is a new voice: {v}"
    );
    assert_eq!(got_rate(&e, &v), "-15%");
    assert!(
        !v["findings"].to_string().contains("to fill the duration"),
        "no fit warning for a pace the user chose: {v}"
    );

    let (_, v) = make(&e, json!({"voice_rate": 0.85}));
    assert_eq!(
        v["metrics"]["voice_reused"], true,
        "the same rate is the same voice: {v}"
    );
    let (_, v) = make(&e, json!({"voice_rate": 1.0}));
    assert_eq!(
        v["metrics"]["voice_reused"], true,
        "1.0 is the voice with no field: {v}"
    );
    assert_eq!(v["metrics"]["voice_rate_pct"], 0, "{v}");
}

#[test]
fn a_rate_outside_the_range_is_a_usage_error() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    for bad in [json!(0.5), json!(1.31), json!(0), json!(-1)] {
        let (code, v) = make(&e, json!({"voice_rate": bad}));
        assert_eq!(code, 2, "{v}");
        let msg = v["error"].to_string();
        assert!(
            msg.contains("voice_rate") && msg.contains("0.75") && msg.contains("1.3"),
            "{v}"
        );
    }
    let (code, v) = make(&e, json!({"voice_rate": "fast"}));
    assert_eq!(code, 2, "{v}");
    assert!(
        !e.dir.path().join(".scriptvox/runs").exists(),
        "nothing ran"
    );
}

#[test]
fn with_a_duration_the_rate_stays_and_only_the_tempo_moves() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    // The plan says the rule whenever both are set.
    let p = plan(&e, json!({"voice_rate": 0.85, "duration": 3.2}));
    let said = p["stages"][0]["explain"].as_str().unwrap();
    assert!(
        said.contains("voice_rate") && said.contains("16 percent"),
        "{p}"
    );
    // 3.0 s of voice for 3.2 s: inside the tempo step. One read, at the asked rate.
    let (code, v) = make(&e, json!({"voice_rate": 0.85, "duration": 3.2}));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["voice_rate_pct"], -15, "{v}");
    let tempo = v["metrics"]["voice_tempo"].as_f64().unwrap();
    assert!((tempo - 0.938).abs() < 0.01, "{v}");
    assert_eq!(got_rate(&e, &v), "-15%", "no second read at another rate");
    // 3.0 s for 4.0 s is past the tempo step: the run stops and names the field. Without voice_rate the fit would
    // read again at -25 percent.
    let (code, v) = make(&e, json!({"voice_rate": 0.85, "duration": 4.0}));
    assert_eq!(code, 3, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("voice_rate"),
        "{v}"
    );
    assert_eq!(
        got_rate(&e, &v),
        "no call",
        "the one read came from the cache: {v}"
    );
    // The plan warns for the same pair: 8 words at 0.85 are estimated near 2.9 s.
    let p = plan(&e, json!({"voice_rate": 0.85, "duration": 4.0}));
    let fit = p["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "fit")
        .unwrap_or_else(|| panic!("no fit warning: {p}"));
    assert!(
        fit["detail"].as_str().unwrap().contains("voice_rate"),
        "{p}"
    );
}

#[test]
fn resume_patch_sets_the_rate_and_replay_keeps_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("p.json"), r#"{"voice_rate": 0.9}"#).unwrap();
    for extra in [json!({}), json!({"duration": 3.0})] {
        let (code, v) = make(&e, extra);
        assert_eq!(code, 0, "{v}");
        let run = v["run_id"].as_str().unwrap().to_string();
        let (code, v) = go(&e, &["resume", &run, "--patch", "p.json", "--wait"]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["patched"], json!(["voice_rate"]), "{v}");
        assert_eq!(v["metrics"]["voice_rate_pct"], -10, "{v}");
        if v["run_id"].as_str().unwrap().matches('.').count() == 1 {
            std::fs::write(e.dir.path().join("bad.json"), r#"{"voice_rate": 2}"#).unwrap();
            let (code, v) = go(&e, &["resume", &run, "--patch", "bad.json", "--wait"]);
            assert_eq!(code, 2, "{v}");
        }
    }
    // The recipe in the MP4 holds the rate: a replay finds the voice in the cache at that rate.
    let (code, v) = {
        brief(&e, json!({"voice_rate": 0.85}));
        go(&e, &["make", "--brief", "b.json", "--out", "v.mp4"])
    };
    assert_eq!(code, 0, "{v}");
    let (code, v) = go(&e, &["replay", "v.mp4", "--out", "r.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["voice_rate_pct"], -15, "{v}");
    assert_eq!(v["metrics"]["voice_reused"], true, "{v}");
}
