//! Row gateway `openai-speech` (G049) against a fake local speech service: no network, no key.
mod common;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A 0.5 s silent 16 kHz WAV.
fn wav() -> Vec<u8> {
    let data = vec![0u8; 16_000];
    let mut w = b"RIFF".to_vec();
    w.extend((36 + data.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(16_000u32.to_le_bytes());
    w.extend(32_000u32.to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend((data.len() as u32).to_le_bytes());
    w.extend(data);
    w
}

/// Answers each POST with `answer`; keeps the bodies it got.
fn fake(answer: Vec<u8>) -> (u16, Arc<Mutex<Vec<Value>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(vec![]));
    let seen = got.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut len = 0;
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            let _ = r.read_exact(&mut body);
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap_or(Value::Null));
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.len());
            let _ = s.write_all(&answer);
        }
    });
    (port, got)
}

fn row(base: &str, kind: &str) -> String {
    format!(
        r#"[[generator]]
name = "speech-test"
gateway = "openai-speech"
kind = "{kind}"
key = "none"
t2v = "kokoro"
base = "{base}"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "inline"
hosts = ["127.0.0.1"]
extra = {{ voice = "am_michael" }}
terms = {{ attribution_required = false, redistribution = true, note = "test" }}
"#
    )
}

fn run(cfg: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let o = common::bin()
        .current_dir(cfg)
        .env("SCRIPTVOX_CONFIG", cfg)
        .env("SCRIPTVOX_DATA", cfg.join("data"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

#[test]
fn a_speech_row_speaks_through_the_openai_api_and_says_when_it_was_not_heard() {
    let d = tempfile::tempdir().unwrap();
    let (port, got) = fake(wav());
    std::fs::write(
        d.path().join("generators.toml"),
        row(&format!("http://127.0.0.1:{port}"), "tts"),
    )
    .unwrap();
    let (code, v) = run(d.path(), &["providers", "test", "speech-test"]);
    assert_eq!(code, 0, "{v}");
    assert!(std::path::Path::new(v["artifacts"]["audio"].as_str().unwrap()).is_file());
    // No free speech row in this data folder: the listening check says it did not run.
    assert_eq!(v["findings"][0]["gate"], "listen", "{v}");
    assert_eq!(v["findings"][0]["status"], "warn", "{v}");
    let body = got.lock().unwrap()[0].clone();
    assert_eq!(body["model"], "kokoro");
    assert_eq!(body["voice"], "am_michael");
    assert_eq!(body["input"], "This is a test.");
    assert_eq!(body["response_format"], "wav");
}

#[test]
fn a_service_that_does_not_answer_audio_fails_and_bad_rows_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let (port, _) = fake(b"{\"error\": \"no model\"}".to_vec());
    std::fs::write(
        d.path().join("generators.toml"),
        row(&format!("http://127.0.0.1:{port}"), "tts"),
    )
    .unwrap();
    let (code, v) = run(d.path(), &["providers", "test", "speech-test"]);
    assert_eq!(code, 5, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not a WAV"),
        "{v}"
    );
    for (base, kind) in [
        ("http://example.com", "tts"),
        ("http://127.0.0.1:9", "video.gen"),
    ] {
        std::fs::write(d.path().join("generators.toml"), row(base, kind)).unwrap();
        let (_, v) = run(d.path(), &["providers"]);
        assert!(
            !v["generator_errors"].as_array().unwrap().is_empty(),
            "{base} {kind}: {v}"
        );
    }
}

/// A row with a known price, on the same fake service.
fn paid_row(base: &str) -> String {
    row(base, "tts").replace("usd_per_s = 0.0", "usd_per_s = 0.0\nusd_min = 0.5")
}

/// money-6: a known price goes against the cap and into the ledger, with the plan's approval.
#[test]
fn a_paid_speech_row_is_reserved_and_held_to_the_cap() {
    let d = tempfile::tempdir().unwrap();
    let (port, got) = fake(wav());
    let cfg = d.path();
    std::fs::write(
        cfg.join("generators.toml"),
        paid_row(&format!("http://127.0.0.1:{port}")),
    )
    .unwrap();
    let gen = [
        "generate",
        "--model",
        "speech-test",
        "--prompt",
        "Hello there.",
    ];
    let (_, plan) = run(cfg, &[&gen[..], &["--plan"]].concat());
    assert_eq!(plan["metrics"]["cost_usd"], 0.5, "{plan}");
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(
        cfg,
        &[&gen[..], &["--approve", &id, "--max-spend", "0.01"]].concat(),
    );
    assert_eq!(code, 4, "{v}");
    assert_eq!(got.lock().unwrap().len(), 0, "no call over the cap: {v}");
    let (code, v) = run(cfg, &[&gen[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.5, "{v}");
    assert_eq!(got.lock().unwrap().len(), 1, "{v}");
}

/// money-7: a paid call with no clear answer may be charged: a rerun stops at exit 6, it does not call again.
#[test]
fn a_paid_speech_call_with_no_clear_answer_is_never_retried() {
    let d = tempfile::tempdir().unwrap();
    let (port, got) = fake(b"{\"error\": \"no model\"}".to_vec());
    let cfg = d.path();
    std::fs::write(
        cfg.join("generators.toml"),
        paid_row(&format!("http://127.0.0.1:{port}")),
    )
    .unwrap();
    let gen = [
        "generate",
        "--model",
        "speech-test",
        "--prompt",
        "Hello there.",
    ];
    let (_, plan) = run(cfg, &[&gen[..], &["--plan"]].concat());
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(
        cfg,
        &[&gen[..], &["--approve", &id, "--max-spend", "1"]].concat(),
    );
    assert_eq!(code, 6, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("delete"),
        "{v}"
    );
    let (code, v) = run(
        cfg,
        &[&gen[..], &["--approve", &id, "--max-spend", "1"]].concat(),
    );
    assert_eq!(code, 6, "{v}");
    assert_eq!(got.lock().unwrap().len(), 1, "one call only: {v}");
}
