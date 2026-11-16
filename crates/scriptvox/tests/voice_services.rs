//! How a user adds a voice service that speaks the OpenAI speech API (docs/specs/voice-services.md), against a fake
//! service on 127.0.0.1: no network, no real key, no paid call.
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

/// One request the fake got: its path, its Authorization header and its JSON body.
type Got = Arc<Mutex<Vec<(String, Option<String>, Value)>>>;

fn fake() -> (u16, Got) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got: Got = Arc::new(Mutex::new(vec![]));
    let seen = got.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut len, mut auth, mut path) = (0, None, String::new());
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if path.is_empty() {
                    path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                } else if let Some(v) = lower.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                } else if lower.starts_with("authorization:") {
                    auth = Some(line["authorization:".len()..].trim().to_string());
                }
            }
            let mut body = vec![0; len];
            let _ = r.read_exact(&mut body);
            seen.lock().unwrap().push((
                path,
                auth,
                serde_json::from_slice(&body).unwrap_or(Value::Null),
            ));
            let answer = wav();
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.len());
            let _ = s.write_all(&answer);
        }
    });
    (port, got)
}

fn run(cfg: &std::path::Path, key: Option<&str>, args: &[&str]) -> (i32, Value) {
    let mut c = common::bin();
    c.current_dir(cfg)
        .env("SCRIPTVOX_CONFIG", cfg)
        .env("SCRIPTVOX_DATA", cfg.join("data"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY");
    if let Some(k) = key {
        c.env("SCRIPTVOX_SELFHOST_KEY", k);
    }
    let o = c.args(args).arg("--json").output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

/// The steps of the how-to: `providers add`, the price in the row, then `providers test`. A local service needs no key.
#[test]
fn providers_add_makes_a_row_that_speaks_to_a_local_service_with_no_key() {
    let d = tempfile::tempdir().unwrap();
    let (port, got) = fake();
    let base = format!("http://127.0.0.1:{port}");
    let (code, v) = run(
        d.path(),
        None,
        &[
            "providers",
            "add",
            "my-speech",
            "--gateway",
            "openai-speech",
            "--kind",
            "tts",
            "--t2v",
            "my-model",
            "--base",
            &base,
        ],
    );
    assert_eq!(code, 0, "{v}");
    // `providers add` writes no price: until the row states one, a call needs a cap.
    let (code, v) = run(d.path(), None, &["providers", "test", "my-speech"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--max-spend"),
        "{v}"
    );
    assert!(got.lock().unwrap().is_empty());
    // The service is your own and free: the row says so, and then no cap and no approval is needed.
    let file = d.path().join("generators.toml");
    let row = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        format!("{row}usd_per_s = 0.0\nprice = \"exact\"\nprice_source = \"https://example.com/my-service\"\nprice_checked = \"2026-11-13\"\n"),
    )
    .unwrap();
    let (code, v) = run(d.path(), None, &["providers", "test", "my-speech"]);
    assert_eq!(code, 0, "{v}");
    let got = got.lock().unwrap();
    let (path, auth, body) = &got[0];
    assert_eq!(path, "/v1/audio/speech");
    assert_eq!(
        auth, &None,
        "a row with no key sends no Authorization header"
    );
    assert_eq!(body["model"], "my-model");
    assert_eq!(body["response_format"], "wav");
    // The row is the open voice of `make`, also for a language of docs/specs/latin-languages.md.
    std::fs::write(
        d.path().join("s.txt"),
        "El agua de lluvia llega primero a las calles y después a los canales de la ciudad.",
    )
    .unwrap();
    let (code, v) = run(
        d.path(),
        None,
        &["plan", "--script", "s.txt", "--tts", "selfhost"],
    );
    assert_eq!(code, 0, "{v}");
    let voice = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == "voice")
        .unwrap();
    assert_eq!(voice["tool"], "tts.selfhost", "{v}");
    assert!(voice["explain"].as_str().unwrap().contains("own-es"), "{v}");
}

/// A service that wants a token: the row names the key `selfhost`, and the call carries it as a Bearer token. The
/// voice of the service is `extra.voice`.
#[test]
fn a_row_with_a_key_sends_it_as_a_bearer_token_and_the_voice_of_the_row() {
    let d = tempfile::tempdir().unwrap();
    let (port, got) = fake();
    std::fs::write(
        d.path().join("generators.toml"),
        format!(
            r#"[[generator]]
name = "my-speech"
gateway = "openai-speech"
kind = "tts"
key = "selfhost"
t2v = "my-model"
base = "http://127.0.0.1:{port}"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://example.com/my-service"
price_checked = "2026-11-13"
output = "inline"
hosts = ["127.0.0.1"]
extra = {{ voice = "maria" }}
terms = {{ attribution_required = false, redistribution = true, note = "my own service" }}
"#
        ),
    )
    .unwrap();
    // No key: the row says what to do, and the service gets no call.
    let (code, v) = run(d.path(), None, &["providers", "test", "my-speech"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("keys set selfhost"), "{v}");
    assert!(got.lock().unwrap().is_empty());
    let (code, v) = run(
        d.path(),
        Some("test-token"),
        &["providers", "test", "my-speech"],
    );
    assert_eq!(code, 0, "{v}");
    let got = got.lock().unwrap();
    let (_, auth, body) = &got[0];
    assert_eq!(auth.as_deref(), Some("Bearer test-token"));
    assert_eq!(body["voice"], "maria");
    assert!(
        !v.to_string().contains("test-token"),
        "the key is never printed: {v}"
    );
}
