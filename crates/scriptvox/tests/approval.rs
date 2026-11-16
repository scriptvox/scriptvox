//! Plans, approval, ledger and idempotency (RVX-026).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;

fn out(o: std::process::Output) -> (i32, Value) {
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

#[test]
fn same_inputs_give_the_same_plan_id_and_free_plans_need_no_token() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "Plan me twice.").unwrap();
    let plan = || {
        out(e
            .cmd("ok")
            .args(["make", "--script", "s.txt", "--plan", "--json"])
            .output()
            .unwrap())
    };
    let ((c1, a), (_, b)) = (plan(), plan());
    assert_eq!(c1, 0, "{a}");
    assert_eq!(a["plan_id"], b["plan_id"]);
    // `scriptvox plan` is the same as `make --plan`, as the skill and README write it.
    let (c3, p) = out(e
        .cmd("ok")
        .args(["plan", "--script", "s.txt", "--json"])
        .output()
        .unwrap());
    assert_eq!(c3, 0, "{p}");
    assert_eq!(p["plan_id"], a["plan_id"]);
    assert!(p["run_id"].is_null(), "plan runs nothing: {p}");
    assert_eq!(
        a["plan_id"].as_str().unwrap().len(),
        18,
        "p_ plus 64 bits of hex"
    );
    assert_eq!(a["metrics"]["cost_usd"], 0.0);
    // A $0 plan runs with no --approve.
    let (code, v) = e.make("Plan me twice.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn paid_plan_needs_its_own_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "This voice costs money.").unwrap();
    let run = |extra: &[&str]| {
        out(e
            .cmd("ok")
            .env("ELEVENLABS_API_KEY", "fake-eleven-key-123456")
            .args(["make", "--script", "s.txt", "--tts", "elevenlabs", "--json"])
            .args(extra)
            .output()
            .unwrap())
    };
    let (code, v) = run(&[]);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    // The stop asks for the person first, and gives the command only after that (13 Nov 2026).
    let asks = |v: &Value| {
        assert_eq!(v["needs_input"][0]["kind"], "approval", "{v}");
        let action = v["needs_input"][0]["human_action"].as_str().unwrap();
        assert!(
            action.contains("ask") && action.contains("must not approve"),
            "{v}"
        );
        let first = v["next"][0].as_str().unwrap();
        assert!(
            first.starts_with("Ask the person") && !first.contains("--approve"),
            "{v}"
        );
        assert!(
            v["next"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n.as_str().unwrap().contains(&id)),
            "{v}"
        );
    };
    asks(&v);
    // The plan of the same paid video says the same, with the price, at exit 0.
    let (code, plan) = run(&["--plan"]);
    assert_eq!(code, 0, "{plan}");
    assert!(
        plan["metrics"]["cost_usd"].as_f64().unwrap() > 0.0,
        "{plan}"
    );
    asks(&plan);
    assert!(
        plan["needs_input"][0]["human_action"]
            .as_str()
            .unwrap()
            .contains('$'),
        "{plan}"
    );
    let (code, v) = run(&["--approve", "p_000000"]);
    assert_eq!(code, 4, "a stale approval is not this plan: {v}");
    assert!(v["error"]["message"].as_str().unwrap().contains("changed"));
    // The right token passes approval (the paid engine itself arrives in RVX-028).
    let (code, v) = run(&["--approve", &id]);
    assert_ne!(code, 4, "{v}");
}

#[test]
fn parallel_reserves_over_the_cap_refuse_exactly_one() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("state.db");
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        scriptvox::plan::ledger_init(&c).unwrap();
    }
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let db = db.clone();
            std::thread::spawn(move || {
                let mut c = rusqlite::Connection::open(&db).unwrap();
                c.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
                scriptvox::plan::reserve(&mut c, "r_abc123", "elevenlabs", 0.6, 1.0).is_ok()
            })
        })
        .collect();
    let ok: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(ok.iter().filter(|x| **x).count(), 1, "{ok:?}");
}

/// A free step after the paid steps have spent the whole cap still runs: it asks for $0 with its own cap of $0.
#[test]
fn a_step_that_costs_nothing_passes_a_cap_that_is_spent() {
    let d = tempfile::tempdir().unwrap();
    let mut c = rusqlite::Connection::open(d.path().join("state.db")).unwrap();
    assert!(scriptvox::plan::reserve(&mut c, "r_abc123", "kling", 0.35, 0.35).is_ok());
    assert!(scriptvox::plan::reserve(&mut c, "r_abc123", "music-self", 0.0, 0.0).is_ok());
    assert!(scriptvox::plan::reserve(&mut c, "r_abc123", "kling", 0.01, 0.35).is_err());
}

#[test]
fn terms_never_stop_a_run_and_the_plan_gives_one_note() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "For a paying client.").unwrap();
    let (code, v) = out(e
        .cmd("ok")
        .args(["plan", "--script", "s.txt", "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{v}");
    // No line about the terms in the output (owner, 13 Nov 2026): they are data in credits.txt and metadata.json.
    assert!(v.get("terms_note").is_none(), "{v}");
    assert_eq!(v["needs_input"], serde_json::json!([]), "{v}");
    let (code, v) = e.make("For a paying client.", "ok", &[]);
    assert_eq!(code, 0, "the note stops nothing: {v}");
    // The flag is gone.
    let (code, _) = e.make("For a paying client.", "ok", &["--commercial"]);
    assert_eq!(code, 2);
}

#[test]
fn same_idempotency_key_returns_the_same_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, a) = e.make("Only once, please.", "ok", &["--idempotency-key", "job-42"]);
    assert_eq!(code, 0, "{a}");
    // STUB_MODE=flaky would fail any new work: exit 0 proves the run was returned.
    let (code, b) = e.make(
        "Only once, please.",
        "flaky",
        &["--idempotency-key", "job-42"],
    );
    assert_eq!(code, 0, "{b}");
    assert_eq!(a["run_id"], b["run_id"]);
    assert_eq!(b["idempotent"], true);
}

/// make-4: the same key answers from the old run only while its video is on disk.
#[test]
fn same_key_with_the_video_gone_makes_it_again() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let key = ["--idempotency-key", "job-43", "--out", "k.mp4"];
    let (code, a) = e.make("Only once, please.", "ok", &key);
    assert_eq!(code, 0, "{a}");
    let video = e.dir.path().join("k.mp4");
    std::fs::remove_file(&video).unwrap();
    let (code, b) = e.make("Only once, please.", "ok", &key);
    assert_eq!(code, 0, "{b}");
    assert_eq!(a["run_id"], b["run_id"]);
    assert!(b.get("idempotent").is_none(), "{b}");
    assert!(video.is_file());
}
