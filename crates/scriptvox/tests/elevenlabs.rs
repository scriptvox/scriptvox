//! ElevenLabs through the real worker and a fake API (RVX-028).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const KEY: &str = "eleven-secret-key-0123456789";
const SCRIPT: &str = "Paid voice here.";

fn fake_api(mp3: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let (base, hits, _) = fake_api_log(mp3);
    (base, hits)
}

/// The fake API, and the JSON body of each call it got.
fn fake_api_log(mp3: Vec<u8>) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<Value>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let log = bodies.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut len = 0usize;
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            loop {
                let mut hl = String::new();
                if r.read_line(&mut hl).unwrap_or(0) == 0 || hl == "\r\n" {
                    break;
                }
                if let Some(v) = hl.to_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; len];
            let _ = r.read_exact(&mut body);
            log.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap_or_default());
            h.fetch_add(1, Ordering::SeqCst);
            let chars: Vec<String> = SCRIPT.chars().map(String::from).collect();
            let starts: Vec<f64> = (0..chars.len()).map(|i| i as f64 * 0.1).collect();
            let ends: Vec<f64> = (0..chars.len()).map(|i| i as f64 * 0.1 + 0.1).collect();
            use base64_lite::encode;
            let resp = json!({"audio_base64": encode(&mp3), "alignment": {"characters": chars, "character_start_times_seconds": starts, "character_end_times_seconds": ends}}).to_string();
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}", resp.len());
        }
    });
    (base, hits, bodies)
}

mod base64_lite {
    pub fn encode(b: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in b.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for i in 0..4 {
                s.push(if i <= c.len() {
                    T[(n >> (18 - 6 * i) & 63) as usize] as char
                } else {
                    '='
                });
            }
        }
        s
    }
}

fn run(e: &Env, base: &str, extra: &[&str]) -> (i32, Value, String) {
    let p = e.dir.path();
    std::fs::write(p.join("s.txt"), SCRIPT).unwrap();
    let o = e
        .cmd("ok")
        .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
        .env("SCRIPTVOX_ELEVENLABS_URL", base)
        .env("ELEVENLABS_API_KEY", KEY)
        .args(["make", "--script", "s.txt", "--tts", "elevenlabs", "--json"])
        .args(extra)
        .output()
        .unwrap();
    let all = String::from_utf8_lossy(&[o.stdout.clone(), o.stderr].concat()).into_owned();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
        all,
    )
}

#[test]
fn approved_paid_voice_renders_and_commits_the_ledger() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (code, v, _) = run(&e, &base, &[]);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call before approval");
    let (code, v, all) = run(&e, &base, &["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(!all.contains(KEY), "the key never reaches any output");
    let c = rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db")).unwrap();
    let (status, cost): (String, f64) = c
        .query_row("SELECT status, actual_usd FROM ledger", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(status, "committed");
    assert!(cost > 0.0);
}

#[test]
fn only_scene_makes_no_paid_call_without_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    std::fs::write(
        e.dir.path().join("b.json"),
        json!({"tts": "elevenlabs", "aspect": "9:16", "scenes": [
            {"script": "First sentence of the scene here."},
            {"script": "Second sentence of the scene here."},
        ]})
        .to_string(),
    )
    .unwrap();
    let go = |args: &[&str]| -> (i32, Value) {
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
            .env("SCRIPTVOX_ELEVENLABS_URL", &base)
            .env("ELEVENLABS_API_KEY", KEY)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    };
    let (code, v) = go(&["make", "--brief", "b.json"]);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = go(&["make", "--brief", "b.json", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    // One call per scene (docs/specs/resume-scene-voice.md): the same characters, the same price.
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let run = v["run_id"].as_str().unwrap().to_string();
    // Nothing changed: no plan, no call.
    let (code, v) = go(&["resume", &run, "--only-scene", "1"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no new call: {v}");
    // A change goes through the plan and needs its own approval first: no call without it.
    std::fs::write(
        e.dir.path().join("p.json"),
        r#"{"script": "Other words now."}"#,
    )
    .unwrap();
    let (code, v) = go(&["resume", &run, "--only-scene", "1", "--patch", "p.json"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "no call before approval: {v}"
    );
}

/// Brief `voice_rate` on the paid voice: another pace is another plan and another paid call, so it waits for its own
/// approval. The call then carries the pace as the voice's `speed`.
#[test]
fn a_changed_voice_rate_is_a_new_plan_and_makes_no_call_before_its_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits, bodies) = fake_api_log(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let go = |brief: Option<Value>, args: &[&str]| -> (i32, Value) {
        if let Some(b) = brief {
            std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
        }
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
            .env("SCRIPTVOX_ELEVENLABS_URL", &base)
            .env("ELEVENLABS_API_KEY", KEY)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    };
    let make = ["make", "--brief", "b.json"];
    let plain = json!({"script": SCRIPT, "tts": "elevenlabs"});
    let (code, v) = go(Some(plain), &make);
    assert_eq!(code, 4, "{v}");
    let first = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = go(None, &[&make[..], &["--approve", &first]].concat());
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(
        bodies.lock().unwrap()[0].get("voice_settings").is_none(),
        "no rate, no setting: the call is the one of every earlier version"
    );
    // The old approval does not cover the new pace.
    let slow = json!({"script": SCRIPT, "tts": "elevenlabs", "voice_rate": 0.9});
    let (code, v) = go(Some(slow), &[&make[..], &["--approve", &first]].concat());
    assert_eq!(code, 4, "{v}");
    let second = v["plan_id"].as_str().unwrap().to_string();
    assert_ne!(first, second);
    let (code, v) = go(None, &make);
    assert_eq!(code, 4, "{v}");
    // The same through a patch of the finished run.
    std::fs::write(e.dir.path().join("p.json"), r#"{"voice_rate": 0.8}"#).unwrap();
    let (code, v) = go(None, &["resume", &run, "--patch", "p.json", "--wait"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no call before approval");
    // `--plan` shows the plan the resume stops on: the same id and cost, no run and no call.
    let runs = || {
        std::fs::read_dir(e.dir.path().join(".scriptvox/runs"))
            .unwrap()
            .count()
    };
    let before = runs();
    let (code, p) = go(None, &["resume", &run, "--patch", "p.json", "--plan"]);
    assert_eq!(code, 0, "{p}");
    assert_eq!(p["plan_id"], v["plan_id"], "{p}");
    assert_eq!(p["metrics"]["cost_usd"], v["metrics"]["cost_usd"], "{p}");
    assert_eq!(runs(), before, "{p}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "a plan makes no call");
    // The plan of a paid resume asks for the person, before any command (13 Nov 2026: an agent approved it alone).
    assert_eq!(p["needs_input"][0]["kind"], "approval", "{p}");
    let first = p["next"][0].as_str().unwrap();
    assert!(
        first.starts_with("Ask the person") && !first.contains("--approve"),
        "{p}"
    );
    let (code, v) = go(None, &[&make[..], &["--approve", &second]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "one new paid call");
    assert_eq!(bodies.lock().unwrap()[1]["voice_settings"]["speed"], 0.9);
    assert_eq!(v["metrics"]["voice_rate_pct"], -10, "{v}");
    assert!(v["metrics"]["voice_reused"].is_null(), "{v}");
    // Past what this voice takes: refused before any plan or call.
    let fast = json!({"script": SCRIPT, "tts": "elevenlabs", "voice_rate": 1.25});
    let (code, v) = go(Some(fast), &make);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("1.20"),
        "{v}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/// `make --until voice` on the paid voice (docs/specs/make-until.md): the approval is for the voice alone, and the
/// `resume` that finishes the video takes the voice from the cache. One paid call in all.
#[test]
fn until_voice_then_resume_is_one_paid_voice_call() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (code, v, _) = run(&e, &base, &["--until", "voice"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["stages"].as_array().unwrap().len(), 1, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call before approval");
    let (code, v, _) = run(&e, &base, &["--until", "voice", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(v["artifacts"].get("video").is_none(), "{v}");
    assert!(v["metrics"]["cost_usd"].as_f64().unwrap() > 0.0, "{v}");
    let run_id = v["run_id"].as_str().unwrap().to_string();
    let resume = |extra: &[&str]| -> (i32, Value) {
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
            .env("SCRIPTVOX_ELEVENLABS_URL", &base)
            .env("ELEVENLABS_API_KEY", KEY)
            .args(["resume", &run_id, "--wait", "--json"])
            .args(extra)
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    };
    // The whole video is another plan, and its voice is in the cache: the plan prices it at 0, so the resume asks
    // for no second approval (docs/specs/voice-plan-cache.md).
    let (code, v) = resume(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the voice came from the cache"
    );
    assert_eq!(v["metrics"]["voice_reused"], true, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert!(
        std::path::Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file(),
        "{v}"
    );
}

#[test]
fn make_detach_with_no_approval_stops_at_once_and_starts_no_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (code, v, _) = run(&e, &base, &["--detach"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert!(v["plan_id"].as_str().is_some(), "{v}");
    assert!(v["run_id"].is_null(), "no run starts: {v}");
    assert!(
        !e.dir.path().join(".scriptvox/runs").exists()
            || std::fs::read_dir(e.dir.path().join(".scriptvox/runs"))
                .unwrap()
                .next()
                .is_none(),
        "no run folder"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call");
}

#[test]
fn make_over_the_cap_with_no_approval_offers_no_approve() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (code, v, _) = run(&e, &base, &["--max-spend", "0.0001"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");
    assert!(!v["error"].to_string().contains("--approve"), "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call");
    // A stale approval gets no new id either.
    let (code, v, _) = run(
        &e,
        &base,
        &["--max-spend", "0.0001", "--approve", "p_0000000000000000"],
    );
    assert_eq!(code, 4, "{v}");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call");
}

#[test]
fn refused_reserve_makes_no_http_call() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    // A cap below the estimate, approved: the worker's reserve is refused.
    let (_, plan, _) = run(&e, &base, &["--max-spend", "0.0001", "--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v, _) = run(&e, &base, &["--max-spend", "0.0001", "--approve", &id]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no HTTP request after a refused reserve"
    );
}

#[test]
fn a_voice_preview_makes_no_paid_call_without_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let voice = "21m00Tcm4TlvDq8ikWAM";
    let go = |extra: &[&str]| -> (i32, Value, String) {
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
            .env("SCRIPTVOX_ELEVENLABS_URL", &base)
            .env("ELEVENLABS_API_KEY", KEY)
            .args(["voices", "--preview", voice, "--text", SCRIPT, "--json"])
            .args(extra)
            .output()
            .unwrap();
        let all = String::from_utf8_lossy(&[o.stdout.clone(), o.stderr].concat()).into_owned();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
            all,
        )
    };
    let (code, v, _) = go(&[]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert!(v["metrics"]["cost_usd"].as_f64().unwrap() > 0.0, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    // --plan with a wrong or right approval, a stale approval, a cap under the price: none of them calls.
    assert_eq!(go(&["--plan", "--approve", &id]).0, 0);
    assert_eq!(go(&["--approve", "p_0000000000000000"]).0, 4);
    let (code, v, _) = go(&["--max-spend", "0.0001"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no call before approval");
    assert!(!e.dir.path().join("scriptvox-out").exists());
    // Approved: one call to the fake API, and the ledger holds it.
    let (code, v, all) = go(&["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(!all.contains(KEY), "the key never reaches any output");
    assert!(std::path::Path::new(v["artifacts"]["audio"].as_str().unwrap()).exists());
    let c = rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db")).unwrap();
    let status: String = c
        .query_row("SELECT status FROM ledger", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "committed");
    // The same preview again comes from the cache: no second call.
    let (code, v, _) = go(&["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "{v}");
    // And its plan says so: the same id, no charge, no approval.
    let (code, v, _) = go(&["--plan"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["plan_id"], id.as_str(), "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert!(
        v["stages"][0]["explain"]
            .as_str()
            .unwrap()
            .contains("already made: no call and no charge"),
        "{v}"
    );
    let (code, v, _) = go(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "{v}");
}

/// Any command in the env, with the real worker and the fake API.
fn go(e: &Env, base: &str, args: &[&str]) -> (i32, Value) {
    let o = e
        .cmd("ok")
        .env("SCRIPTVOX_WORKER_SRC", root().join("python"))
        .env("SCRIPTVOX_ELEVENLABS_URL", base)
        .env("ELEVENLABS_API_KEY", KEY)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

fn voice_stage(v: &Value) -> &Value {
    v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == "voice")
        .unwrap()
}

fn ledger_rows(e: &Env) -> i64 {
    rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM ledger", [], |r| r.get(0))
        .unwrap()
}

/// docs/specs/voice-plan-cache.md: a paid voice that is in the cache costs 0 in the plan and needs no approval. Other
/// words are priced in full again.
#[test]
fn a_cached_paid_voice_costs_0_in_the_plan_and_needs_no_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (code, v, _) = run(&e, &base, &[]);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let price = v["metrics"]["cost_usd"].as_f64().unwrap();
    assert!(price > 0.0, "{v}");
    let (code, v, _) = run(&e, &base, &["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // The plan of the same command: the same id, the voice at 0, nothing to approve.
    let (code, v, _) = run(&e, &base, &["--plan"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["plan_id"], id.as_str(), "{v}");
    assert_eq!(voice_stage(&v)["cost_usd"], 0.0, "{v}");
    assert!(
        voice_stage(&v)["explain"]
            .as_str()
            .unwrap()
            .contains("already made: no call and no charge"),
        "{v}"
    );
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");
    // The same command with no approval: a video, no call, no charge, no new ledger row.
    let (code, v, _) = run(&e, &base, &[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no new call: {v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert_eq!(v["metrics"]["voice_reused"], true, "{v}");
    assert_eq!(ledger_rows(&e), 1, "the run that paid keeps its one row");
    // Other words: the full price again, exit 4, no call.
    std::fs::write(e.dir.path().join("t.txt"), "Other words now.").unwrap();
    let (code, v) = go(
        &e,
        &base,
        &["make", "--script", "t.txt", "--tts", "elevenlabs"],
    );
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert_eq!(v["metrics"]["cost_usd"].as_f64().unwrap(), price, "{v}");
    assert_ne!(v["plan_id"], id.as_str(), "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no call before approval");
}

/// A cache entry whose file is gone: the plan had said 0, so the run may buy nothing, with `--max-spend` or without.
/// The entry goes, and the next plan shows the price again.
#[test]
fn a_cached_voice_that_cannot_be_read_back_is_not_bought_without_approval() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let (_, v, _) = run(&e, &base, &["--plan"]);
    let id = v["plan_id"].as_str().unwrap().to_string();
    assert_eq!(run(&e, &base, &["--approve", &id]).0, 0);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    std::fs::remove_dir_all(e.dir.path().join("data/cache/blobs")).unwrap();
    let (code, v, _) = run(&e, &base, &["--max-spend", "5"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no call: {v}");
    assert_eq!(ledger_rows(&e), 1, "{v}");
    let (code, v, _) = run(&e, &base, &[]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert!(v["metrics"]["cost_usd"].as_f64().unwrap() > 0.0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// A voice is cut into parts at pause tags (and at scenes that name a speaker). Each part has its own cache entry, so
/// a change in one part is priced alone.
#[test]
fn only_the_changed_part_of_a_voice_is_priced() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let long = "This first part is long so that its price is more than one cent. ".repeat(5);
    let script = |end: &str| {
        std::fs::write(
            e.dir.path().join("t.txt"),
            format!("{long}[pause 0.5] {end}"),
        )
        .unwrap();
    };
    let make = [
        "make",
        "--script",
        "t.txt",
        "--tts",
        "elevenlabs",
        "--until",
        "voice",
    ];
    script("A short end.");
    let (code, v) = go(&e, &base, &make);
    assert_eq!(code, 4, "{v}");
    let whole = v["metrics"]["cost_usd"].as_f64().unwrap();
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = go(&e, &base, &[&make[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "two parts, two calls");
    script("Another end now.");
    let (code, v) = go(&e, &base, &[&make[..], &["--plan"]].concat());
    assert_eq!(code, 0, "{v}");
    let part = v["metrics"]["cost_usd"].as_f64().unwrap();
    assert!(part > 0.0 && part < whole, "{part} of {whole}: {v}");
    assert!(
        voice_stage(&v)["explain"]
            .as_str()
            .unwrap()
            .contains("1 of 2 parts already made"),
        "{v}"
    );
    let (code, v) = go(&e, &base, &make);
    assert_eq!(code, 4, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no call before approval");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = go(&e, &base, &[&make[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 3, "one call, for the new part");
    let paid = v["metrics"]["cost_usd"].as_f64().unwrap();
    assert!(paid > 0.0 && paid <= part, "{paid} of {part}: {v}");
}

/// `duration` may read the voice again at another pace. That pace is known only after the first read is measured,
/// so a plan prices one re-read until a run of the same request has written how its fit ended.
#[test]
fn the_re_read_for_a_duration_is_priced_until_its_pace_is_known() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(e.dir.path().join("voice.mp3")).unwrap());
    let brief = |d: f64| {
        std::fs::write(
            e.dir.path().join("b.json"),
            json!({"script": "Paid voice here with seven words total.", "tts": "elevenlabs", "duration": d})
                .to_string(),
        )
        .unwrap();
    };
    let make = ["make", "--brief", "b.json"];
    let until = ["make", "--brief", "b.json", "--until", "voice"];
    // The fake voice is 3.0 s: for 2.7 s the fit reads again, 11 percent faster.
    brief(2.7);
    let (code, v) = go(&e, &base, &until);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = go(&e, &base, &[&until[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "the read and the re-read");
    assert_eq!(v["metrics"]["voice_rate_pct"], 11, "{v}");
    // The resume finds both reads: no approval, no call.
    let run = v["run_id"].as_str().unwrap().to_string();
    let (code, v) = go(&e, &base, &["resume", &run, "--wait"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no new call: {v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert!(v["artifacts"]["video"].is_string(), "{v}");
    // The same request: both reads are in the cache and the pace of the second is on record.
    let (code, v) = go(&e, &base, &[&make[..], &["--plan"]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    let (code, v) = go(&e, &base, &make);
    assert_eq!(code, 0, "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no new call: {v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert_eq!(v["metrics"]["voice_rate_pct"], 11, "{v}");
    // Another duration: the first read is in the cache, the pace of its re-read is not known. One read is priced.
    brief(3.3);
    let (code, v) = go(&e, &base, &make);
    assert_eq!(code, 4, "{v}");
    assert!(v["metrics"]["cost_usd"].as_f64().unwrap() > 0.0, "{v}");
    assert!(
        voice_stage(&v)["explain"]
            .as_str()
            .unwrap()
            .contains("a re-read at another pace may be needed to reach the duration: up to $"),
        "{v}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no call before approval");
}

/// A free voice is not looked up and not changed: its plan ids are the ones the build before this change gave
/// (read from the main tree's binary on 10 Nov 2026).
#[test]
fn a_free_voice_keeps_its_plan_id() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(
        e.dir.path().join("t.txt"),
        "A free voice reads this line. It has two short sentences.",
    )
    .unwrap();
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"tts":"edge","aspect":"9:16","duration":8,"scenes":[{"script":"The first scene says a few plain words here."},{"script":"The second scene says [pause 0.5] a few more words."}]}"#,
    )
    .unwrap();
    for (args, id) in [
        (["--script", "t.txt"], "p_e0dbc84426203d19"),
        (["--brief", "b.json"], "p_f7d1a72f7463daa5"),
    ] {
        let (code, v) = go(&e, "", &[&["make", "--plan"][..], &args].concat());
        assert_eq!(code, 0, "{v}");
        // The id holds the pinned FFmpeg and uv of the system (`make.rs`, "pins"): these values are for Linux. macOS
        // has no such pins, so its id is another one, and stays the same there.
        if cfg!(target_os = "linux") {
            assert_eq!(v["plan_id"], id, "{v}");
        }
        assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    }
}

/// docs/specs/remake-detach.md: a paid recipe in a project that does not have its voice. `remake --detach` with no
/// approval stops at once and starts no run; with the approval it makes the one voice call of a fresh remake.
#[test]
fn remake_detach_of_a_paid_recipe_needs_approval_and_starts_no_run_without_it() {
    let Some(ff) = ready() else { return };
    let first = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(first.dir.path().join("voice.mp3")).unwrap());
    let (_, v, _) = run(&first, &base, &[]);
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v, _) = run(&first, &base, &["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the first make: one voice call"
    );
    // A new project with its own empty cache: the file is all it has of the first one.
    let e = Env::new(&ff);
    std::fs::copy(
        v["artifacts"]["video"].as_str().unwrap(),
        e.dir.path().join("got.mp4"),
    )
    .unwrap();
    let no_runs = |e: &Env| {
        std::fs::read_dir(e.dir.path().join(".scriptvox/runs")).map_or(0, |d| d.count()) == 0
    };
    let (code, v) = go(&e, &base, &["remake", "got.mp4", "--detach"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert_eq!(v["command"], "remake", "{v}");
    assert!(v["run_id"].is_null(), "no run starts: {v}");
    assert!(no_runs(&e), "no run folder");
    assert!(
        v["next"][1]
            .as_str()
            .unwrap()
            .starts_with("scriptvox remake"),
        "{v}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no call before approval");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    // An approval of some other plan is no approval.
    let (code, v) = go(
        &e,
        &base,
        &["remake", "got.mp4", "--detach", "--approve", "p_other"],
    );
    assert_eq!(code, 4, "{v}");
    assert!(no_runs(&e), "no run folder");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "no call on a wrong approval"
    );
    let (code, v) = go(
        &e,
        &base,
        &["remake", "got.mp4", "--detach", "--approve", &plan],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "running", "{v}");
    let run_id = v["run_id"].as_str().unwrap().to_string();
    let (code, w) = go(&e, &base, &["wait", &run_id]);
    assert_eq!(code, 0, "{w}");
    assert!(std::path::Path::new(w["artifacts"]["video"].as_str().unwrap()).is_file());
    // One part of speech, an empty cache: one call, as a fresh foreground remake makes.
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "the approved remake: one voice call"
    );
    // The same call again finds the finished run and pays nothing more.
    let (code, v) = go(
        &e,
        &base,
        &["remake", "got.mp4", "--detach", "--approve", &plan],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["run_id"], run_id.as_str(), "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "a retry makes no call");
}

/// A recipe cannot set the spend cap: the cap is the user's `--max-spend`, or the cost of the plan they approved.
#[test]
fn the_spend_cap_of_a_recipe_is_not_used() {
    let Some(ff) = ready() else { return };
    let first = Env::new(&ff);
    let (base, hits) = fake_api(std::fs::read(first.dir.path().join("voice.mp3")).unwrap());
    let (_, v, _) = run(&first, &base, &[]);
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v, _) = run(&first, &base, &["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    let (code, x) = go(
        &first,
        &base,
        &["explain", v["artifacts"]["video"].as_str().unwrap()],
    );
    assert_eq!(code, 0, "{x}");
    let mut recipe = x["recipe"].clone();
    // A cap far under the cost: were it used, the answer would be SVX_SPEND_CAP. (A cap far over it is the danger.)
    recipe["request"]["max_spend"] = json!(0.000001);
    let e = Env::new(&ff);
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=1",
            "-metadata",
        ])
        .arg(format!("comment={recipe}"))
        .arg(e.dir.path().join("capped.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    for extra in [&[][..], &["--detach"][..]] {
        let (code, v) = go(&e, &base, &[&["remake", "capped.mp4"], extra].concat());
        assert_eq!(code, 4, "{v}");
        assert_ne!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
        assert!(
            v["error"]["fix"].as_str().unwrap().contains("--approve"),
            "{v}"
        );
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1, "only the first make called");
}
