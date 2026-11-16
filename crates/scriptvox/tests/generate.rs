//! `scriptvox generate` against a local fake fal queue (P2-002): the money rules, with no network and no spend.
mod common;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// How the fake answers.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    /// Status stays IN_PROGRESS until switched to Normal.
    Busy,
    /// The submit connection closes with no answer.
    Drop,
    /// The submit is refused with 401.
    Refuse,
    /// The job finishes with an error.
    Failed,
    /// The result of a finished job answers 500 with no JSON.
    NoResult,
    /// The clip download fails; pictures still come.
    NoClip,
    /// The result of a finished job answers 500 with a JSON body.
    JsonError,
    /// The result of a finished job is 200 JSON with no transcript in it.
    NoTranscript,
    /// The result names a transcript file, as a self-hosted speech row does. Not in `Normal`: there the listening
    /// check of `make` stays "not checked".
    Transcript,
    /// The same, and the file is not JSON.
    BadTranscript,
    /// A cancel is only requested (202): the job may still finish.
    CancelAsked,
    /// The submit answer names a cancel URL off the queue host.
    CancelElsewhere,
}

struct Fake {
    port: u16,
    submits: Arc<AtomicUsize>,
    mode: Arc<std::sync::Mutex<Mode>>,
    /// The bytes served as the clip.
    clip: Arc<std::sync::Mutex<Vec<u8>>>,
    /// The bytes served as a picture.
    still: Arc<std::sync::Mutex<Vec<u8>>>,
    /// Submits that carried reference pictures.
    with_refs: Arc<AtomicUsize>,
    /// Image to video submits that carried a last frame in `tail_image_url`.
    with_last: Arc<AtomicUsize>,
}

fn fake() -> Fake {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let submits = Arc::new(AtomicUsize::new(0));
    let mode = Arc::new(std::sync::Mutex::new(Mode::Normal));
    // With no FFmpeg: not a real clip, but the first bytes of one, as a result must have (`media-inputs.md`).
    let clip = Arc::new(std::sync::Mutex::new(
        b"\0\0\0\x18ftypisom fake-mp4-bytes".to_vec(),
    ));
    let with_refs = Arc::new(AtomicUsize::new(0));
    let with_last = Arc::new(AtomicUsize::new(0));
    let last = with_last.clone();
    let still = Arc::new(std::sync::Mutex::new(
        b"\x89PNG\r\n\x1a\nfake-png-bytes".to_vec(),
    ));
    let (s, m, cl, refs, pic) = (
        submits.clone(),
        mode.clone(),
        clip.clone(),
        with_refs.clone(),
        still.clone(),
    );
    std::thread::spawn(move || {
        for stream in l.incoming().flatten() {
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).is_err() {
                continue;
            }
            let mut len = 0usize;
            let mut auth_ok = false;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap_or(0);
                if h == "\r\n" || h.is_empty() {
                    break;
                }
                let lower = h.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
                auth_ok |= lower.starts_with("authorization: key test-fal-key");
            }
            let mut body = vec![0; len];
            let _ = r.read_exact(&mut body);
            let mode = *m.lock().unwrap();
            let base = format!("http://127.0.0.1:{port}");
            let (code, ctype, out): (u16, &str, Vec<u8>) = if line.starts_with("POST ") {
                s.fetch_add(1, Ordering::SeqCst);
                match mode {
                    Mode::Drop => continue, // close without an answer
                    Mode::Refuse => (401, "application/json", br#"{"detail":"bad key"}"#.to_vec()),
                    _ => {
                        assert!(auth_ok, "the key header is sent");
                        let input: Value = serde_json::from_slice(&body).unwrap();
                        if input["image_urls"][0]
                            .as_str()
                            .is_some_and(|u| u.starts_with("data:image/"))
                        {
                            refs.fetch_add(1, Ordering::SeqCst);
                        }
                        if line.contains("image-to-video")
                            && input["tail_image_url"]
                                .as_str()
                                .is_some_and(|u| u.starts_with("data:image/png"))
                            && input["start_image_url"]
                                .as_str()
                                .is_some_and(|u| u.starts_with("data:image/png"))
                        {
                            last.fetch_add(1, Ordering::SeqCst);
                        }
                        if line.contains("kling") {
                            assert_eq!(input["duration"], "5");
                            assert_eq!(input["generate_audio"], false);
                        }
                        let cancel = if mode == Mode::CancelElsewhere {
                            format!("http://localhost:{port}/steal")
                        } else {
                            format!("{base}/q/requests/r1/cancel")
                        };
                        (200, "application/json", format!(r#"{{"request_id":"r1","status_url":"{base}/q/requests/r1/status","response_url":"{base}/q/requests/r1","cancel_url":"{cancel}"}}"#).into_bytes())
                    }
                }
            } else if line.starts_with("PUT ") {
                (
                    if mode == Mode::CancelAsked { 202 } else { 200 },
                    "application/json",
                    br#"{"status":"CANCELLATION_REQUESTED"}"#.to_vec(),
                )
            } else if line.contains("/status") {
                let st = match mode {
                    Mode::Busy | Mode::CancelAsked => r#""IN_PROGRESS""#,
                    Mode::Failed => r#""COMPLETED","error":"CUDA out of memory""#,
                    _ => r#""COMPLETED""#,
                };
                (
                    200,
                    "application/json",
                    format!(r#"{{"status":{st}}}"#).into_bytes(),
                )
            } else if line.contains("/q/requests/r1 ") && mode == Mode::NoResult {
                (500, "text/plain", b"no result yet".to_vec())
            } else if line.contains("/q/requests/r1 ") && mode == Mode::JsonError {
                (500, "application/json", br#"{"detail":"boom"}"#.to_vec())
            } else if line.contains("/q/requests/r1 ") && mode == Mode::NoTranscript {
                (200, "application/json", br#"{"detail":"late"}"#.to_vec())
            } else if line.contains("/q/requests/r1 ") {
                (
                    200,
                    "application/json",
                    // every result field, so video, image and audio rows all find their URL
                    format!(r#"{{"video":{{"url":"{base}/media/clip.mp4"}},"image":{{"url":"{base}/media/still.png"}},"audio":{{"url":"{base}/media/clip.mp4"}},"images":[{{"url":"{base}/media/still.png"}}],{}"text":"hello there","words":[{{"text":"hello","type":"word","start":0.0,"end":0.4,"speaker_id":"speaker_0"}}]}}"#, if matches!(mode, Mode::Transcript | Mode::BadTranscript) { format!(r#""transcript":{{"url":"{base}/media/talk.json"}},"#) } else { String::new() }).into_bytes(),
                )
            } else if line.contains("/media/talk.json") && mode == Mode::BadTranscript {
                (
                    200,
                    "application/json",
                    b"\0\0\0\x18ftypisom not a transcript".to_vec(),
                )
            } else if line.contains("/media/talk.json") {
                (200, "application/json", br#"{"text":"hello there","words":[{"text":"hello","type":"word","start":0.0,"end":0.4}]}"#.to_vec())
            } else if line.contains("/media/still.png") {
                (200, "image/png", pic.lock().unwrap().clone())
            } else if line.contains("/media/clip.mp4") && mode == Mode::NoClip {
                (500, "text/plain", b"no clip yet".to_vec())
            } else if line.contains("/media/clip.mp4") {
                (200, "video/mp4", cl.lock().unwrap().clone())
            } else {
                (404, "text/plain", b"no".to_vec())
            };
            let mut w = stream;
            let _ = write!(w, "HTTP/1.1 {code} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
            let _ = w.write_all(&out);
        }
    });
    Fake {
        still,
        with_refs,
        with_last,
        port,
        submits,
        mode,
        clip,
    }
}

struct Env {
    dir: tempfile::TempDir,
    fake: Fake,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let rows = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../registry/generators.toml"
    ))
    .unwrap();
    std::fs::write(
        dir.path().join("gen.toml"),
        rows.replace(r#"hosts = ["fal.media"]"#, r#"hosts = ["127.0.0.1"]"#)
            .replace(
                r#"hosts = ["gen.example.invalid"]"#,
                r#"hosts = ["127.0.0.1"]"#,
            )
            // The test queue is plain loopback http: no private CA.
            .replace("ca = \"selfhost\"\n", ""),
    )
    .unwrap();
    let fake = fake();
    // With FFmpeg, serve a real moving 5 s clip so the clip check passes.
    if let Some(ff) = common::ffmpeg_only() {
        *fake.clip.lock().unwrap() = clip(&ff, "testsrc2=size=360x640:rate=24", 5.0);
        let png = dir.path().join("still-src.png");
        let st = std::process::Command::new(&ff)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=360x640",
                "-frames:v",
                "1",
            ])
            .arg(&png)
            .status()
            .unwrap();
        assert!(st.success());
        *fake.still.lock().unwrap() = std::fs::read(&png).unwrap();
    }
    Env { dir, fake }
}

impl Env {
    fn run(&self, extra: &[&str]) -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(self.dir.path())
            .env("SCRIPTVOX_GENERATORS", self.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", self.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", self.dir.path().join("cfg"))
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args([
                "generate",
                "--model",
                "kling-2.6-pro",
                "--prompt",
                "aerial over an oasis",
                "--seconds",
                "5",
                "--json",
            ])
            .args(extra);
        let o = c.output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
        (o.status.code().unwrap_or(-1), v)
    }

    /// Any command, with the fake queue and a fal key.
    fn cli(&self, args: &[&str]) -> (i32, Value) {
        let o = common::bin()
            .current_dir(self.dir.path())
            .env("SCRIPTVOX_GENERATORS", self.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", self.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", self.dir.path().join("cfg"))
            .env("SCRIPTVOX_DATA", self.dir.path().join("data"))
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
        )
    }

    fn approved(&self, extra: &[&str]) -> (i32, Value) {
        let (_, plan) = self.run(&["--plan"]);
        let id = plan["plan_id"].as_str().unwrap().to_string();
        let mut a = vec!["--approve", id.as_str()];
        a.extend_from_slice(extra);
        self.run(&a)
    }

    fn submits(&self) -> usize {
        self.fake.submits.load(Ordering::SeqCst)
    }

    fn set(&self, m: Mode) {
        *self.fake.mode.lock().unwrap() = m;
    }
}

#[test]
fn a_clip_is_made_once_and_its_cost_is_committed() {
    let e = env();
    let (code, v) = e.run(&[]);
    assert_eq!(code, 4, "unapproved: {v}");
    assert_eq!(e.submits(), 0, "nothing is sent before approval");
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.35);
    assert!(Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file());
    assert_eq!(e.submits(), 1);
    // The same command again: the job is done, nothing is sent.
    let (code, _) = e.approved(&[]);
    assert_eq!(code, 0);
    assert_eq!(e.submits(), 1);
}

#[test]
fn a_last_frame_goes_only_to_a_row_that_takes_one() {
    let e = env();
    // The name is not proof: a file named .png with other bytes is refused before any row is called.
    std::fs::write(e.dir.path().join("first.png"), b"png-bytes").unwrap();
    let (code, v) = e.run(&["--image", "first.png", "--plan"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("first bytes"),
        "{v}"
    );
    for f in ["first.png", "last.png"] {
        std::fs::write(e.dir.path().join(f), b"\x89PNG\r\n\x1a\npng-bytes").unwrap();
    }
    let both = ["--image", "first.png", "--last-image", "last.png"];
    let (code, v) = e.run(&both);
    assert_eq!(
        code, 2,
        "kling-2.6-pro has no checked last-frame field: {v}"
    );
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("takes no last frame"),
        "{v}"
    );
    let (code, _) = e.run(&["--last-image", "last.png"]);
    assert_eq!(code, 2, "a last frame needs a first frame");
    // An overlay row that names its field.
    let rows = std::fs::read_to_string(e.dir.path().join("gen.toml")).unwrap();
    let rows = rows.replacen(
        "name = \"kling-2.6-pro\"\n",
        "name = \"kling-2.6-pro\"\ntakes_last_frame = \"tail_image_url\"\n",
        1,
    );
    std::fs::write(e.dir.path().join("gen.toml"), rows).unwrap();
    let (_, first_only) = e.run(&["--image", "first.png", "--plan"]);
    let (_, plan) = e.run(&[&both[..], &["--plan"]].concat());
    let id = plan["plan_id"].as_str().unwrap().to_string();
    assert_ne!(
        first_only["plan_id"], plan["plan_id"],
        "the last frame is part of the plan id"
    );
    let (code, v) = e.run(&[&both[..], &["--approve", id.as_str()]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1);
    assert_eq!(
        e.fake.with_last.load(Ordering::SeqCst),
        1,
        "both frames sent"
    );
}

#[test]
fn a_busy_job_resumes_without_a_second_submit() {
    let e = env();
    e.set(Mode::Busy);
    let (code, v) = e.approved(&["--wait-s", "0"]);
    assert_eq!(code, 9, "still running: {v}");
    assert_eq!(e.submits(), 1);
    e.set(Mode::Normal);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1, "the rerun polled the same job");
}

#[test]
fn a_submit_without_an_answer_is_an_unconfirmed_charge_and_never_retried() {
    let e = env();
    e.set(Mode::Drop);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 6, "{v}");
    e.set(Mode::Normal);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 6, "the marker stops a blind retry: {v}");
    assert_eq!(e.submits(), 1);
    assert!(v["error"]["fix"].as_str().unwrap().contains("delete"));
}

#[test]
fn a_refused_key_releases_the_reservation_and_may_submit_again() {
    let e = env();
    e.set(Mode::Refuse);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 10, "{v}");
    e.set(Mode::Normal);
    // Released, so the cap of one clip allows this submit.
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 2);
}

/// A brief with one narrated scene and one generated scene.
fn brief(e: &Env, model: Option<&str>, prompt: &str) {
    let mut g = serde_json::json!({"prompt": prompt});
    if let Some(m) = model {
        g["model"] = m.into();
    }
    let b = serde_json::json!({"scenes": [
        {"script": "The desert is quiet at dawn."},
        {"script": "Then the oasis comes alive with birds.", "generate": g},
    ]});
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
}

impl Env {
    fn make(&self, extra: &[&str], key: bool) -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(self.dir.path())
            .env("SCRIPTVOX_GENERATORS", self.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", self.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", self.dir.path().join("cfg"))
            .env_remove("CLAUDECODE")
            .env_remove("FAL_KEY")
            .args(["make", "--brief", "b.json", "--json"])
            .args(extra);
        if key {
            c.env("FAL_KEY", "test-fal-key");
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
        )
    }
}

#[test]
fn the_plan_prices_a_generated_scene_and_picks_the_cheapest_row() {
    let e = env();
    brief(&e, None, "aerial over an oasis");
    let (code, v) = e.make(&["--plan"], true);
    assert_eq!(code, 0, "{v}");
    let gen: Vec<&Value> = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["stage"] == "generate")
        .collect();
    assert_eq!(gen.len(), 1, "{v}");
    assert_eq!(gen[0]["tool"], "video.gen.minimax-h3-max-turbo");
    assert!(gen[0]["explain"]
        .as_str()
        .unwrap()
        .starts_with("scene 2: 5 s"));
    assert_eq!(v["metrics"]["cost_usd"], 0.23);
    // Another prompt is another plan; nothing was sent.
    brief(&e, None, "aerial over a city");
    let (_, w) = e.make(&["--plan"], true);
    assert_ne!(v["plan_id"], w["plan_id"]);
    let (code, _) = e.make(&[], true);
    assert_eq!(code, 4, "approval first");
    assert_eq!(e.submits(), 0);
}

#[test]
fn a_generated_scene_without_a_key_or_offline_asks_for_input() {
    let e = env();
    brief(&e, Some("kling-2.6-pro"), "aerial over an oasis");
    let (code, v) = e.make(&["--plan"], false);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["needs_input"][0]["field"], "generate.fal.api_key", "{v}");
    let (code, v) = e.make(&["--plan", "--local-only"], true);
    assert_eq!(code, 10, "{v}");
    brief(&e, Some("no-such-row"), "x");
    let (code, _) = e.make(&["--plan"], true);
    assert_eq!(code, 2);
}

#[test]
fn make_renders_a_generated_scene_once_and_a_rerun_pays_nothing() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let w = common::Env::new(&ff);
    common::copy_dir(&w.dir.path().join("worker"), &e.dir.path().join("worker"));
    std::fs::copy(
        w.dir.path().join("voice.mp3"),
        e.dir.path().join("voice.mp3"),
    )
    .unwrap();
    brief(&e, Some("kling-2.6-pro"), "aerial over an oasis");
    let run = |extra: &[&str]| {
        let p = e.dir.path();
        let mut c = common::bin();
        common::shared_uv(&mut c);
        c.current_dir(p)
            .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("STUB_AUDIO", p.join("voice.mp3"))
            .env("STUB_MODE", "ok")
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(["make", "--brief", "b.json", "--json"])
            .args(extra);
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (_, plan) = run(&["--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert!(Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file());
    assert_eq!(e.submits(), 1);
    let (code, v) = run(&["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        e.submits(),
        1,
        "the rerun is the same run: no second submit"
    );
}

#[test]
fn a_free_self_hosted_row_is_picked_first_and_needs_no_approval() {
    let e = env();
    brief(&e, None, "aerial over an oasis");
    let mut c = common::bin();
    c.current_dir(e.dir.path())
        .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
        .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
        .env("FAL_KEY", "test-fal-key")
        // A row at the placeholder host cannot run, so it is not picked (`why_not`): this says it is deployed. The
        // service must answer: a row on a service that does not answer is not chosen (docs/specs/reel-gaps.md).
        .env(
            "SCRIPTVOX_FAL_BASE",
            format!("http://127.0.0.1:{}", e.fake.port),
        )
        .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
        .env_remove("CLAUDECODE")
        .args(["make", "--brief", "b.json", "--json", "--plan"]);
    let v: Value = serde_json::from_slice(&c.output().unwrap().stdout).unwrap();
    let gen: Vec<&Value> = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["stage"] == "generate")
        .collect();
    assert_eq!(gen[0]["tool"], "video.gen.wan22-ti2v-5b-self", "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0);
    // `generate` on the free row runs with no --approve, and the ledger commits $0.
    let mut c = common::bin();
    c.current_dir(e.dir.path())
        .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
        .env(
            "SCRIPTVOX_FAL_BASE",
            format!("http://127.0.0.1:{}", e.fake.port),
        )
        .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
        .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
        .env_remove("CLAUDECODE")
        .args([
            "generate",
            "--model",
            "wan22-ti2v-5b-self",
            "--prompt",
            "aerial over an oasis",
            "--json",
        ]);
    let o = c.output().unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0);
    assert_eq!(e.submits(), 1);
}

/// A test clip made by FFmpeg from a lavfi source.
fn clip(ff: &str, src: &str, secs: f64) -> Vec<u8> {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("c.mp4");
    let st = std::process::Command::new(ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            src,
            "-t",
            &secs.to_string(),
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::read(out).unwrap()
}

#[test]
fn a_frozen_or_short_clip_fails_the_clip_check_and_is_kept() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    // A still picture for 2 s: too short for 5 s and it never moves.
    *e.fake.clip.lock().unwrap() = clip(&ff, "color=c=teal:size=720x1280:rate=24", 2.0);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 3, "{v}");
    let gates: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["status"] == "fail")
        .map(|f| f["gate"].as_str().unwrap())
        .collect();
    assert!(
        gates.contains(&"clip_length") && gates.contains(&"clip_motion"),
        "{gates:?}"
    );
    assert!(
        Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file(),
        "the paid clip is kept"
    );
}

#[test]
fn a_speech_row_returns_audio_and_speech_to_text_needs_audio() {
    let e = env();
    let extra = r#"
[[generator]]
name = "tts-test"
gateway = "selfhost"
kind = "tts"
key = "selfhost"
base = "https://example.invalid/q"
t2v = "tts-test/tts"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://example.invalid/card"
price_checked = "2026-11-02"
output = "video.url"
hosts = ["127.0.0.1"]
terms = { commercial = false, attribution_required = false, redistribution = false, note = "test row" }

[[generator]]
name = "stt-test"
gateway = "selfhost"
kind = "stt"
key = "selfhost"
base = "https://example.invalid/q"
t2v = "stt-test/stt"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://example.invalid/card"
price_checked = "2026-11-02"
output = "video.url"
hosts = ["127.0.0.1"]
terms = { commercial = false, attribution_required = false, redistribution = false, note = "test row" }
"#;
    let p = e.dir.path().join("gen.toml");
    let rows = std::fs::read_to_string(&p).unwrap() + extra;
    std::fs::write(&p, rows).unwrap();
    let run = |args: &[&str]| {
        let mut c = common::bin();
        c.current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", &p)
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .arg("generate")
            .args(args)
            .arg("--json");
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, v) = run(&[
        "--model",
        "tts-test",
        "--prompt",
        "مرحبا بكم",
        "--lang",
        "ar",
    ]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["artifacts"]["audio"].as_str().unwrap().ends_with(".wav"),
        "{v}"
    );
    assert_eq!(
        v["findings"],
        serde_json::json!([]),
        "no clip check on audio"
    );
    let (code, _) = run(&["--model", "stt-test"]);
    assert_eq!(code, 2, "speech to text without --audio is a usage error");

    // A free selfhost job that fails may run again: the same command submits a second time and succeeds.
    *e.fake.mode.lock().unwrap() = Mode::Failed;
    let before = e.fake.submits.load(Ordering::SeqCst);
    let again = [
        "--model",
        "tts-test",
        "--prompt",
        "hello there",
        "--lang",
        "en",
    ];
    let (code, v) = run(&again);
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["retryable"], true, "{v}");
    *e.fake.mode.lock().unwrap() = Mode::Normal;
    let (code, v) = run(&again);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.fake.submits.load(Ordering::SeqCst), before + 2);
}

#[test]
fn a_first_frame_scene_makes_an_image_then_animates_it() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let w = common::Env::new(&ff);
    common::copy_dir(&w.dir.path().join("worker"), &e.dir.path().join("worker"));
    std::fs::copy(
        w.dir.path().join("voice.mp3"),
        e.dir.path().join("voice.mp3"),
    )
    .unwrap();
    let b = serde_json::json!({"scenes": [
        {"script": "A dhow sails at dawn.", "generate": {"prompt": "the dhow glides right", "first_frame": "a wooden dhow on calm water at dawn"}},
    ]});
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
    let p = e.dir.path();
    let mut c = common::bin();
    common::shared_uv(&mut c);
    c.current_dir(p)
        .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("STUB_AUDIO", p.join("voice.mp3"))
        .env("STUB_MODE", "ok")
        .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
        .env(
            "SCRIPTVOX_FAL_BASE",
            format!("http://127.0.0.1:{}", e.fake.port),
        )
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
        .env_remove("FAL_KEY")
        .env_remove("CLAUDECODE")
        .args(["make", "--brief", "b.json", "--json"]);
    let o = c.output().unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    assert_eq!(o.status.code(), Some(0), "{v}");
    // One image, one image to video, then make listens to its video (the test rows include a free STT row).
    assert_eq!(e.submits(), 3, "image, image to video, listen");
    let run = p
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    assert!(run.join("scene1_frame.png").is_file() && run.join("scene1.mp4").is_file());
}

#[test]
fn a_music_prompt_is_planned_and_conflicts_are_refused() {
    let e = env();
    let run = |b: Value, selfhost: bool| {
        std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
        let mut c = common::bin();
        c.current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env_remove("SCRIPTVOX_SELFHOST_KEY")
            .env_remove("CLAUDECODE")
            .args(["make", "--brief", "b.json", "--plan", "--json"]);
        if selfhost {
            c.env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key");
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, v) = run(
        serde_json::json!({"script": "One two three four.", "music_prompt": "calm oud, no vocals"}),
        true,
    );
    assert_eq!(code, 0, "{v}");
    assert!(
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["tool"] == "music.gen.acestep-xl-turbo-self"),
        "{v}"
    );
    let (code, _) = run(
        serde_json::json!({"script": "One two.", "music": "calm", "music_prompt": "oud"}),
        true,
    );
    assert_eq!(code, 2, "music and music_prompt together");
    let (code, v) = run(
        serde_json::json!({"script": "One two.", "music_prompt": "oud"}),
        false,
    );
    assert_eq!(code, 10, "{v}");
}

#[test]
fn cancel_stops_an_open_job_and_keeps_one_it_cannot_stop() {
    let f = fake();
    let d = tempfile::tempdir().unwrap();
    let job = |url: Option<String>, done: Option<&str>| {
        serde_json::json!({"generator": "kling-2.6-pro", "request_id": "r1", "status_url": "x", "response_url": "x",
            "cancel_url": url, "est_usd": 0.5, "reservation": 1, "submitted": "t", "done": done})
        .to_string()
    };
    let base = format!("http://127.0.0.1:{}", f.port);
    let p = |n: &str| d.path().join(n);
    std::fs::write(p("a.job.json"), job(Some(format!("{base}/cancel")), None)).unwrap();
    std::fs::write(p("b.job.json"), job(None, None)).unwrap();
    std::fs::write(
        p("c.job.json"),
        job(Some(format!("{base}/cancel")), Some("clip.mp4")),
    )
    .unwrap();
    // security-1: a cancel URL off the queue host gets no key, and its job counts as still running.
    std::fs::write(
        p("d.job.json"),
        job(Some(format!("http://localhost:{}/steal", f.port)), None),
    )
    .unwrap();
    // Safe here: the only reader in this test binary is this test; other tests pass both to child processes.
    unsafe {
        std::env::set_var("FAL_KEY", "test-fal-key");
        std::env::set_var("SCRIPTVOX_FAL_BASE", &base);
    }
    assert_eq!(scriptvox::genjob::cancel_jobs(d.path(), d.path()), (1, 2));
    assert!(
        !p("a.job.json").exists(),
        "a stopped job can be submitted again"
    );
    assert!(p("b.job.json").exists() && p("c.job.json").exists());
    assert!(p("d.job.json").exists(), "a refused cancel keeps its job");
}

/// rows-5: 202 is a request to stop, and the job may still finish. It stays, so nothing is submitted or paid twice.
#[test]
fn a_cancel_that_is_only_requested_keeps_the_job_and_its_reservation() {
    let e = env();
    e.set(Mode::CancelAsked);
    let (_, plan) = e.run(&["--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = e.run(&["--approve", &id, "--detach"]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let job = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(&run)
        .join("clip.job.json");
    let t0 = std::time::Instant::now();
    while !job.is_file() {
        assert!(t0.elapsed().as_secs() < 30, "no job file");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (_, v) = e.cli(&["cancel", &run]);
    assert_eq!(v["metrics"]["provider_jobs_stopped"], 0, "{v}");
    assert_eq!(v["metrics"]["provider_jobs_running"], 1, "{v}");
    assert!(job.is_file(), "the job may still finish");
    let db = rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db")).unwrap();
    let status: String = db
        .query_row("SELECT status FROM ledger WHERE run = ?1", [&run], |r| {
            r.get(0)
        })
        .unwrap();
    assert_ne!(status, "released", "the job may still charge");
}

/// security-1, rows-7: the key never goes to a URL off the queue host, from a submit answer or from a job file.
#[test]
fn a_url_off_the_queue_host_gets_no_key() {
    let e = env();
    e.set(Mode::CancelElsewhere);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 6, "an answer off its host is not trusted: {v}");

    let e = env();
    e.set(Mode::Busy);
    let (code, v) = e.approved(&["--wait-s", "0"]);
    assert_eq!(code, 9, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let job = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(&run)
        .join("clip.job.json");
    let mut j: Value = serde_json::from_str(&std::fs::read_to_string(&job).unwrap()).unwrap();
    j["status_url"] = format!("http://localhost:{}/q/requests/r1/status", e.fake.port).into();
    std::fs::write(&job, j.to_string()).unwrap();
    e.set(Mode::Normal);
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .to_string()
            .contains("off the queue host"),
        "{v}"
    );
    assert_eq!(e.submits(), 1, "{v}");
}

/// rows-1: a failed result fetch, or an answer with no transcript, is never saved, charged or reported as done.
#[test]
fn a_bad_result_is_not_saved_as_the_transcript() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "sine=f=330:d=2", "-y"])
        .arg(e.dir.path().join("talk.wav"))
        .status()
        .unwrap();
    assert!(st.success());
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_DATA", e.dir.path().join("data"))
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let stt = ["generate", "--model", "scribe-fal", "--audio", "talk.wav"];
    let (_, plan) = run(&stt);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let approved = [&stt[..], &["--approve", id.as_str()]].concat();
    for mode in [Mode::JsonError, Mode::NoTranscript] {
        e.set(mode);
        let (code, v) = run(&approved);
        assert_eq!(code, 5, "{v}");
        assert!(v["artifacts"]["transcript"].is_null(), "{v}");
    }
    assert_eq!(e.submits(), 1, "a rerun fetches the same job");
    e.set(Mode::Normal);
    let (code, v) = run(&approved);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1, "{v}");
}

/// 11 Nov 2026: `generate --seconds 10` on a row of 5 s planned 5 s and said nothing; `snapshot <run id>` of a
/// `generate` run said "is not a video file or a finished run".
#[test]
fn a_shorter_clip_than_asked_is_said_and_snapshot_finds_a_generate_run() {
    if common::ready().is_none() {
        return;
    }
    let e = env();
    let (code, v) = e.cli(&[
        "generate",
        "--prompt",
        "a cat walks",
        "--seconds",
        "600",
        "--plan",
    ]);
    assert_eq!(code, 0, "{v}");
    let warn = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "length")
        .unwrap_or_else(|| panic!("no length finding: {v}"));
    assert_eq!(warn["status"], "warn", "{v}");
    let (code, v) = e.approved(&[]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["gate"] != "length"),
        "{v}"
    );
    let id = v["run_id"].as_str().unwrap();
    let (code, s) = e.cli(&["snapshot", id, "--sheet", "4"]);
    assert_eq!(code, 0, "{s}");
    assert!(
        s["artifacts"]["image"]
            .as_str()
            .unwrap()
            .ends_with("sheet4.png"),
        "{s}"
    );
    let (code, s) = e.cli(&["snapshot", "../../etc", "--sheet", "4"]);
    assert_eq!(code, 2, "{s}");
}

/// A self-hosted speech row answers with the URL of a transcript file. The file is JSON, not media, so the media check
/// does not judge it (11 Nov 2026: the check refused every transcript as "not plain media"). And `--local-only` sends
/// nothing to that row (12 Nov 2026: `analyze --transcript --local-only` sent the sound).
#[test]
fn a_downloaded_transcript_is_read_as_json_and_local_only_sends_nothing() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "sine=f=330:d=2", "-y"])
        .arg(e.dir.path().join("talk.wav"))
        .status()
        .unwrap();
    assert!(st.success());
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_DATA", e.dir.path().join("data"))
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .env_remove("FAL_KEY")
            .env_remove("CLAUDECODE")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, v) = run(&["analyze", "talk.wav", "--transcript", "--local-only"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(e.submits(), 0, "--local-only sent the sound: {v}");
    let (code, v) = run(&[
        "judge",
        "talk.wav",
        "--local-only",
        "--endpoint",
        "http://127.0.0.1:9/v1",
    ]);
    assert_eq!(code, 10, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--local-only"),
        "{v}"
    );

    e.set(Mode::BadTranscript);
    let (code, v) = run(&["analyze", "talk.wav", "--transcript"]);
    assert_ne!(code, 0, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not a transcript"),
        "{v}"
    );
    e.set(Mode::Transcript);
    let (code, v) = run(&["analyze", "talk.wav", "--transcript"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["transcript"]["words"], 1, "{v}");
}

/// A runner for real `make` commands against the fake queue, with the stub voice.
fn runner(e: &Env, ff: &str) -> impl Fn(&[&str]) -> (Option<i32>, Value) {
    runner_keyed(e, ff, ("SCRIPTVOX_SELFHOST_KEY", "test-fal-key"))
}

/// The same with a fal key, for the paid rows.
fn paid_runner(e: &Env, ff: &str) -> impl Fn(&[&str]) -> (Option<i32>, Value) {
    runner_keyed(e, ff, ("FAL_KEY", "test-fal-key"))
}

fn runner_keyed(
    e: &Env,
    ff: &str,
    key: (&'static str, &'static str),
) -> impl Fn(&[&str]) -> (Option<i32>, Value) {
    let w = common::Env::new(ff);
    common::copy_dir(&w.dir.path().join("worker"), &e.dir.path().join("worker"));
    std::fs::copy(
        w.dir.path().join("voice.mp3"),
        e.dir.path().join("voice.mp3"),
    )
    .unwrap();
    let p = e.dir.path().to_path_buf();
    let port = e.fake.port;
    move |args: &[&str]| {
        let mut c = common::bin();
        common::shared_uv(&mut c);
        c.current_dir(&p)
            .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("STUB_AUDIO", p.join("voice.mp3"))
            .env("STUB_MODE", "ok")
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env("SCRIPTVOX_FAL_BASE", format!("http://127.0.0.1:{port}"))
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env_remove("FAL_KEY")
            .env(key.0, key.1)
            .env_remove("CLAUDECODE")
            .args(args);
        let o = c.output().unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    }
}

#[test]
fn a_character_is_made_once_and_every_scene_starts_from_it() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path().to_path_buf();
    let brief = |look: &str| {
        let b = serde_json::json!({"look": look, "character": {"prompt": "a woman with red hair"}, "scenes": [
            {"script": "She walks through a market.", "generate": {"prompt": "the same woman in a market", "still": true}},
            {"script": "She reads a plan.", "generate": {"prompt": "the same woman reads a plan"}},
            {"script": "The city wakes.", "generate": {"prompt": "a skyline", "still": true, "character": false}}
        ]});
        std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    };
    brief("warm");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    // The picture, a still, a first frame and its clip, a still without the character, the listen.
    assert_eq!(e.submits(), 6, "{v}");
    assert_eq!(
        e.fake.with_refs.load(Ordering::SeqCst),
        2,
        "the still and the first frame"
    );
    // Another look: nothing new.
    brief("mono");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 6, "{v}");
    assert_eq!(v["metrics"]["reused_scenes"], 3, "{v}");
    // A new character makes new pictures.
    let b = std::fs::read_to_string(p.join("b.json"))
        .unwrap()
        .replace("red hair", "grey hair");
    std::fs::write(p.join("b.json"), b).unwrap();
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 10, "picture, still, frame, clip: {v}");
    // A reference picture through `generate`, on the image row's own endpoint.
    let refs = e.fake.with_refs.load(Ordering::SeqCst);
    let (code, v) = scriptvox(&[
        "generate",
        "--model",
        "flux2-klein-4b-self",
        "--prompt",
        "the same woman",
        "--image",
        "still-src.png",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.fake.with_refs.load(Ordering::SeqCst), refs + 1, "{v}");
    // An empty one is refused at once.
    std::fs::write(
        p.join("b.json"),
        r#"{"script": "x", "character": {"prompt": " "}}"#,
    )
    .unwrap();
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(2), "{v}");
}

#[test]
fn two_characters_are_made_once_each_and_a_scene_starts_from_who_is_in_it() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path().to_path_buf();
    let b = serde_json::json!({"characters": [
            {"name": "Sara", "prompt": "a woman with red hair"},
            {"name": "Omar", "prompt": "a man with a grey beard"}
        ], "scenes": [
            {"script": "She walks.", "generate": {"prompt": "the woman walks", "still": true, "who": ["Sara"]}},
            {"script": "They talk.", "generate": {"prompt": "the woman and the man talk", "still": true}},
            {"script": "The sea rolls.", "generate": {"prompt": "the sea", "still": true, "character": false}}
    ]});
    std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    // Two pictures, three stills, the listen. Sara's picture is asked for once, though two scenes need it.
    assert_eq!(e.submits(), 6, "{v}");
    assert_eq!(e.fake.with_refs.load(Ordering::SeqCst), 2, "{v}");
    // Five in one picture, with no `who`, stop at plan: the row takes 4.
    let five: Vec<Value> = ["A", "B", "C", "D", "E"]
        .iter()
        .map(|n| serde_json::json!({"name": n, "prompt": n}))
        .collect();
    let big = serde_json::json!({"characters": five, "scenes": [
        {"script": "All five.", "generate": {"prompt": "p", "still": true}}]});
    std::fs::write(p.join("b.json"), big.to_string()).unwrap();
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(2), "{v}");
    // A name that is not in the list, and a list with no names, stop at plan.
    let bad = serde_json::json!({"characters": [{"name": "Sara", "prompt": "x"}], "scenes": [
        {"script": "She walks.", "generate": {"prompt": "p", "still": true, "who": ["Omar"]}}]});
    std::fs::write(p.join("b.json"), bad.to_string()).unwrap();
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(2), "{v}");
    std::fs::write(
        p.join("b.json"),
        r#"{"script": "x", "characters": [{"prompt": "a"}]}"#,
    )
    .unwrap();
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(2), "{v}");
}

#[test]
fn a_made_clip_comes_back_free_and_its_terms_still_count() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path().to_path_buf();
    let brief = |look: &str| {
        let b = serde_json::json!({"look": look, "scenes": [
            {"script": "A dhow sails at dawn.", "generate": {"prompt": "the dhow glides right", "first_frame": "a dhow"}}
        ]});
        std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    };
    brief("warm");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    let first = e.submits();
    // Another look, the same shot and sound: no image, no clip job and no second listen.
    brief("mono");
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(v.to_string().contains("already made"), "{v}");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["reused_scenes"], 1, "{v}");
    assert_eq!(e.submits(), first, "no new job");
    let id = v["run_id"].as_str().unwrap().to_string();
    let (code, x) = scriptvox(&["export", &id, "--json"]);
    assert_eq!(code, Some(0), "{x}");
    let meta = std::fs::read_to_string(x["artifacts"]["metadata"].as_str().unwrap()).unwrap();
    assert!(
        meta.contains("\"generator\""),
        "a reused clip keeps its terms: {meta}"
    );
}

#[test]
fn a_lipsync_row_takes_video_and_voice_on_the_queue() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let p = e.dir.path();
    let make = |args: &[&str], name: &str| {
        let st = std::process::Command::new(&ff)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .args(args)
            .arg(p.join(name))
            .status()
            .unwrap();
        assert!(st.success());
    };
    make(&["testsrc2=size=360x640:rate=25", "-t", "2"], "v.mp4");
    make(&["sine=frequency=300", "-t", "2"], "voice.wav");
    let run = |extra: &[&str]| {
        let mut c = common::bin();
        let o = c
            .current_dir(p)
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(["generate", "--model", "latentsync-16-self", "--json"])
            .args(extra)
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, v) = run(&["--video", "v.mp4"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--video and --audio"),
        "{v}"
    );
    let (code, v) = run(&["--video", "v.mp4", "--audio", "voice.wav"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file(),
        "{v}"
    );
    assert_eq!(e.submits(), 1);
    assert_eq!(v["terms"]["commercial"], false, "{v}");
}

/// Rows not priced by time, on a fake queue: the plan shows the price, approval is asked, the cap holds, and no path
/// with no plan reaches them.
#[test]
fn image_and_transcript_rows_are_priced_and_need_approval() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "sine=f=330:d=120"])
        .arg(e.dir.path().join("talk.wav"))
        .status()
        .unwrap();
    assert!(st.success());
    let run = |args: &[&str]| {
        let mut c = common::bin();
        let o = c
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_DATA", e.dir.path().join("data"))
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let submits = || e.fake.submits.load(Ordering::SeqCst);

    let image = [
        "generate",
        "--model",
        "flux-pro-ultra-fal",
        "--prompt",
        "a kite",
    ];
    let (code, v) = run(&image);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.06, "{v}");
    assert!(
        v["stages"][0]["explain"]
            .as_str()
            .unwrap()
            .contains("one image at $0.06 an image"),
        "{v}"
    );
    assert_eq!(submits(), 0);
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&[&image[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert!(
        v["artifacts"]["image"].as_str().unwrap().ends_with(".png"),
        "{v}"
    );
    assert_eq!(v["metrics"]["cost_usd"], 0.06, "{v}");
    assert_eq!(submits(), 1);

    let stt = ["generate", "--model", "scribe-fal", "--audio", "talk.wav"];
    let (code, v) = run(&stt);
    assert_eq!(code, 4, "{v}");
    assert_eq!(
        v["metrics"]["cost_usd"], 0.06,
        "120 s at $0.03 a minute: {v}"
    );
    let id = v["plan_id"].as_str().unwrap().to_string();
    // The cap holds although the request does not show the length.
    let (code, v) = run(&[&stt[..], &["--approve", &id, "--max-spend", "0.01"]].concat());
    assert_ne!(code, 0, "{v}");
    assert_eq!(submits(), 1, "no submit over the cap: {v}");
    let (code, v) = run(&[&stt[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(submits(), 2);
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(v["artifacts"]["transcript"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(doc["text"], "hello there", "the result is the file: {doc}");

    // G141: with only a fal key, `analyze --transcript` asks to approve the paid row's price for this audio first.
    let (code, v) = run(&["analyze", "talk.wav", "--transcript"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("$0.06"),
        "{v}"
    );
    assert_eq!(submits(), 2, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&["analyze", "talk.wav", "--transcript", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(submits(), 3);
    assert_eq!(
        v["transcript"]["words"], 1,
        "the spacing entries are not words: {v}"
    );
    let (code, v) = run(&["analyze", "talk.wav", "--transcript", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(submits(), 3, "a rerun reads the same job");
    // G020: speakers through the same paid row, its own job (diarize on), the speaker named S1.
    let (code, v) = run(&["analyze", "talk.wav", "--transcript", "--speakers"]);
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&[
        "analyze",
        "talk.wav",
        "--transcript",
        "--speakers",
        "--approve",
        &id,
    ]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(submits(), 4);
    assert_eq!(v["transcript"]["sentences"][0]["speaker"], "S1", "{v}");
    // `edit --captions` takes the same route: approval first, nothing sent.
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=25",
            "-i",
            "talk.wav",
        ])
        .args([
            "-t",
            "6",
            "-pix_fmt",
            "yuv420p",
            "-shortest",
            "-y",
            "talk.mp4",
        ])
        .current_dir(e.dir.path())
        .status()
        .unwrap();
    assert!(st.success());
    let (code, v) = run(&["edit", "talk.mp4", "--captions", "karaoke"]);
    assert_eq!(code, 4, "{v}");
    assert!(v["plan_id"].as_str().is_some(), "{v}");
    assert_eq!(submits(), 4);

    // A paid first frame is on the bill. A free image row with a key goes first, and then costs nothing.
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"lang": "en", "tts": "edge", "scenes": [{"script": "The sea is calm today.",
            "generate": {"prompt": "a calm sea", "first_frame": "a sea at dawn", "model": "kling-2.6-pro", "seconds": 5}}]}"#,
    )
    .unwrap();
    let (code, v) = run(&["plan", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["metrics"]["cost_usd"], 0.41,
        "clip 0.35 + frame 0.06: {v}"
    );
    assert_eq!(submits(), 4, "{v}");
}

/// A built-in selfhost row still points at the `.invalid` placeholder: the plan says whose service it is, and a run
/// asks for the deploy step, not for a key that cannot help (audit H1). No network: the check comes first.
#[test]
fn a_placeholder_selfhost_row_asks_for_the_deploy_step() {
    let t = tempfile::tempdir().unwrap();
    let run = |plan: bool| {
        let mut c = common::bin();
        c.current_dir(t.path())
            .env("SCRIPTVOX_CONFIG", t.path().join("cfg"))
            .env("SCRIPTVOX_SELFHOST_KEY", "test-not-a-key")
            .env_remove("SCRIPTVOX_FAL_BASE")
            .env_remove("SCRIPTVOX_GENERATORS")
            .args([
                "generate",
                "--model",
                "wan22-ti2v-5b-self",
                "--prompt",
                "a kite",
                "--json",
            ]);
        if plan {
            c.arg("--plan");
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, v) = run(true);
    assert_eq!(code, 0, "{v}");
    let explain = v["stages"][0]["explain"].as_str().unwrap();
    assert!(
        explain.contains("your own GPU service") && explain.contains("not deployed yet"),
        "{v}"
    );
    let (code, v) = run(false);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["status"], "needs_input", "{v}");
    let fix = v["error"]["fix"].as_str().unwrap();
    assert!(
        fix.contains("services/gen") && fix.contains("generators.toml"),
        "{v}"
    );
}

/// Brief `cutaways` (G027): a generated clip and a file over a base video, in one plan and one approval.
#[test]
fn cutaways_are_one_plan_and_a_rerun_pays_nothing() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let p = e.dir.path();
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=360x640:rate=30",
            "-f",
            "lavfi",
            "-i",
        ])
        .args([
            "sine=frequency=440",
            "-t",
            "8",
            "-pix_fmt",
            "yuv420p",
            "-shortest",
            "-y",
        ])
        .arg(p.join("talk.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::copy(p.join("still-src.png"), p.join("map.png")).unwrap();
    let b = serde_json::json!({"base_video": "talk.mp4", "cutaways": [
        {"at": 1.0, "seconds": 2.0, "prompt": "aerial over an oasis", "model": "kling-2.6-pro"},
        {"at": 5.0, "seconds": 1.5, "file": "map.png"},
    ]});
    std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    let (code, plan) = e.make(&["--plan"], true);
    assert_eq!(code, 0, "{plan}");
    let stages: Vec<&str> = plan["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["stage"].as_str().unwrap())
        .collect();
    assert_eq!(stages, ["cutaway0", "edit"], "{plan}");
    assert!(
        plan["metrics"]["cost_usd"].as_f64().unwrap() > 0.0,
        "{plan}"
    );
    // money-10: over the cap the plan stops, the same as a plain scene brief.
    let (code, over) = e.make(&["--plan", "--max-spend", "0.01"], true);
    assert_eq!(code, 4, "{over}");
    assert_eq!(over["error"]["code"], "SVX_SPEND_CAP", "{over}");
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = e.make(&[], true);
    assert_eq!(code, 4, "approval first: {v}");
    assert_eq!(e.submits(), 0);
    let (code, v) = e.make(&["--approve", &id], true);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1);
    assert!(Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file());
    assert_eq!(v["broll"].as_array().unwrap().len(), 2, "{v}");
    let (code, v) = e.make(&["--approve", &id], true);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1, "the rerun resumes the same job");
    // A script with cutaways is refused before any plan.
    let b = serde_json::json!({"base_video": "talk.mp4", "script": "Hello.", "cutaways": [{"at": 1.0, "seconds": 2.0, "file": "map.png"}]});
    std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    assert_eq!(e.make(&["--plan"], true).0, 2);
}

/// `--detach` (what the MCP server always asks for): the id comes back at once, `wait` gets the clip, and a paid plan
/// with no approval still stops at exit 4 with no job started.
#[test]
fn generate_detach_returns_the_run_id_and_approval_still_stops_it() {
    let e = env();
    let (code, v) = e.run(&["--detach"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(
        e.fake.submits.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "{v}"
    );
    let (_, plan) = e.run(&["--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = e.run(&["--detach", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "running", "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let o = common::bin()
        .current_dir(e.dir.path())
        .args(["wait", &run, "--timeout", "60", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert!(
        std::path::Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file(),
        "{v}"
    );
}

/// A free plan: no approval hint, and a row with personal-use terms says so (L5).
#[test]
fn a_free_plan_gives_the_note_and_no_approve_hint() {
    let d = tempfile::tempdir().unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_CONFIG", d.path().join("c"))
        .env("SCRIPTVOX_DATA", d.path().join("d"))
        .args([
            "generate",
            "--model",
            "mmaudio-t2a-self",
            "--prompt",
            "rain",
            "--seconds",
            "5",
            "--plan",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    let next = serde_json::to_string(&v["next"]).unwrap();
    assert!(!next.contains("--approve"), "{v}");
    // No line about the terms in the output (owner, 13 Nov 2026): they are data in credits.txt and metadata.json.
    assert!(v.get("terms_note").is_none(), "{v}");
}

#[test]
fn a_plan_over_the_cap_stops_at_plan_time() {
    let e = env();
    let (code, v) = e.run(&["--plan", "--max-spend", "0.01"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
    assert_eq!(v["metrics"]["cost_usd"].as_f64(), Some(0.35), "{v}");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");
    // A cap that covers the cost still plans.
    let (code, v) = e.run(&["--plan", "--max-spend", "0.35"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].to_string().contains("--approve"), "{v}");
}

#[test]
fn over_the_cap_next_gives_runnable_cheaper_commands_with_prices() {
    let e = env();
    // Unknown price: it would sort first at $0 if it were offered.
    let toml = e.dir.path().join("gen.toml");
    // Kling far over the cap, so both cheaper rows fit under it.
    let rows =
        std::fs::read_to_string(&toml)
            .unwrap()
            .replacen("usd_per_s = 0.07", "usd_per_s = 0.2", 1);
    std::fs::write(&toml, format!("{rows}\n[[generator]]\nname = \"mine\"\ngateway = \"fal\"\nkind = \"video.gen\"\nt2v = \"x/y\"\n")).unwrap();

    let (code, v) = e.run(&["--plan", "--max-spend", "0.50"]);
    assert_eq!(code, 4, "{v}");
    let next: Vec<&str> = v["next"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert_eq!(next.len(), 4, "{v}");
    assert!(next[0].starts_with("Ask the person"), "{v}");
    // Over the cap the item names the cap: an approval of this id would be refused.
    let action = v["needs_input"][0]["human_action"].as_str().unwrap();
    assert!(
        action.contains("Raise --max-spend only after") && !action.contains("--approve"),
        "{v}"
    );
    let next = &next[1..];
    assert!(next[0].contains("Raise --max-spend"), "{v}");
    // The two cheapest that can run: the free selfhost rows have no key here, and the row with no price is left out.
    assert_eq!(next[1], "scriptvox generate --model minimax-h3-max-turbo --prompt 'aerial over an oasis' --seconds 5 --aspect 9:16 --plan --max-spend 0.5  # $0.23");
    assert_eq!(next[2], "scriptvox generate --model gemini-omni-flash-fal --prompt 'aerial over an oasis' --seconds 5 --aspect 9:16 --plan --max-spend 0.5  # $0.41");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");

    // Nothing fits: the free list stays.
    let (code, v) = e.run(&["--plan", "--max-spend", "0.01"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["next"][2], "scriptvox providers --ready --free", "{v}");

    // A stale approval over the cap gets no new id to approve either.
    let (code, v) = e.run(&["--max-spend", "0.01", "--approve", "p_0000000000000000"]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "SVX_SPEND_CAP", "{v}");
    assert!(!v["next"].to_string().contains("--approve"), "{v}");
    assert_eq!(e.fake.submits.load(Ordering::SeqCst), 0, "no call");
}

/// One approved `make` with the result fetch failing: POST 1, and the job record stays in the parent's folder.
fn paid_make_left_a_job(
    e: &Env,
    scriptvox: &impl Fn(&[&str]) -> (Option<i32>, Value),
) -> (String, String) {
    brief(e, Some("kling-2.6-pro"), "aerial over an oasis");
    let (_, plan) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    e.set(Mode::NoResult);
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json", "--approve", &id]);
    assert_eq!(code, Some(5), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    e.set(Mode::Normal);
    (id, v["run_id"].as_str().unwrap().to_string())
}

/// money-1: a resume runs in a child folder. It must fetch the parent's job, and it must stop on the parent's marker.
#[test]
fn resume_takes_over_the_parents_job_and_never_submits_again() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let (id, run) = paid_make_left_a_job(&e, &scriptvox);
    let (code, v) = scriptvox(&["resume", &run, "--wait", "--approve", &id, "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["run_id"].as_str().unwrap(), format!("{run}.1"), "{v}");
    assert_eq!(e.submits(), 1, "the resume fetched the parent's job: {v}");

    // A submit with no answer in the parent is exit 6 in the child, and its fix names the parent's marker.
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    brief(&e, Some("kling-2.6-pro"), "aerial over an oasis");
    let (_, plan) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    e.set(Mode::Drop);
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json", "--approve", &id]);
    assert_eq!(code, Some(6), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let (code, v) = scriptvox(&["resume", &run, "--wait", "--approve", &id, "--json"]);
    assert_eq!(code, Some(6), "{v}");
    assert_eq!(
        e.submits(),
        1,
        "nothing is sent while the marker stands: {v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains(&format!("{run}/scene2.submitting")),
        "{v}"
    );
}

/// money-2: `make --detach` is the plan's own run, so a retry after a foreground attempt pays nothing more.
#[test]
fn make_detach_is_the_plans_run_and_a_retry_pays_nothing() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let (id, run) = paid_make_left_a_job(&e, &scriptvox);
    let (code, v) = scriptvox(&[
        "make",
        "--brief",
        "b.json",
        "--json",
        "--approve",
        &id,
        "--detach",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["run_id"].as_str().unwrap(), run, "the same run: {v}");
    let (code, v) = scriptvox(&["wait", &run, "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "the same job: {v}");
    // Finished: the same command again answers from the run.
    let (code, v) = scriptvox(&[
        "make",
        "--brief",
        "b.json",
        "--json",
        "--approve",
        &id,
        "--detach",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["idempotent"], true, "{v}");
    assert_eq!(e.submits(), 1, "{v}");
}

fn extra_row(e: &Env, row: &str) {
    let p = e.dir.path().join("gen.toml");
    let rows = std::fs::read_to_string(&p).unwrap();
    std::fs::write(p, format!("{rows}\n{row}\n")).unwrap();
}

/// money-3: a brief step that nobody priced takes free rows only, so a paid row can never run with a $0 plan.
#[test]
fn a_free_step_of_a_brief_never_picks_a_paid_row() {
    let e = env();
    extra_row(
        &e,
        r#"[[generator]]
name = "v2a-by-source"
gateway = "fal"
kind = "sfx.gen"
key = "fal"
t2v = "x/mm/v2a"
durations = []
aspects = []
usd_per_input_s = 1.0
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "audio.url"
hosts = ["127.0.0.1"]
terms = { attribution_required = false, redistribution = true, note = "test" }
"#,
    );
    std::fs::write(
        e.dir.path().join("b.json"),
        serde_json::json!({"script": "Hello there friends.", "ambience": true}).to_string(),
    )
    .unwrap();
    let (code, v) = e.make(&["--plan"], true);
    assert_eq!(code, 10, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("free"),
        "{v}"
    );
    assert_eq!(e.submits(), 0);
}

/// money-4: make's open voice never hears through a paid row, with no plan to pay for it.
#[test]
fn make_keeps_its_open_voice_to_free_rows() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    // No selfhost key: the free self-hosted speech rows cannot be picked, and the paid row below needs none.
    let scriptvox = paid_runner(&e, &ff);
    let port = e.fake.port;
    extra_row(
        &e,
        &format!(
            r#"[[generator]]
name = "speech-free"
gateway = "openai-speech"
kind = "tts"
key = "none"
t2v = "kokoro"
base = "http://127.0.0.1:{port}"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "inline"
hosts = ["127.0.0.1"]
terms = {{ attribution_required = false, redistribution = true, note = "test" }}

[[generator]]
name = "stt-paid-self"
gateway = "selfhost"
kind = "stt"
key = "none"
base = "https://127.0.0.1/x"
t2v = "x/stt"
durations = []
aspects = []
usd_per_s = 0.5
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "inline"
hosts = ["127.0.0.1"]
terms = {{ attribution_required = false, redistribution = true, note = "test" }}
"#
        ),
    );
    std::fs::write(
        e.dir.path().join("b.json"),
        serde_json::json!({"script": "Hello there friends.", "tts": "selfhost"}).to_string(),
    )
    .unwrap();
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_ne!(code, Some(0), "no free speech to text row: {v}");
    assert_eq!(e.submits(), 0, "no call to a paid row with no plan: {v}");
}

/// money-5: the reserve is the plan's price. Luma writes its length "10s", and $2.00 is more than this cap.
#[test]
fn the_reserve_holds_the_price_of_the_plan() {
    let e = env();
    let row = [
        "generate",
        "--model",
        "luma-ray-3.2-fal",
        "--prompt",
        "a kite",
        "--seconds",
        "10",
    ];
    let (_, plan) = e.cli(&[&row[..], &["--plan"]].concat());
    assert_eq!(plan["metrics"]["cost_usd"], 2.0, "{plan}");
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = e.cli(&[&row[..], &["--approve", &id, "--max-spend", "1.5"]].concat());
    assert_eq!(code, 4, "{v}");
    assert_eq!(e.submits(), 0, "the cap held: {v}");
    // `make` has the same row and the same cap, through the row's own price.
    let Some(ff) = common::ready() else { return };
    let scriptvox = paid_runner(&e, &ff);
    let b = serde_json::json!({"scenes": [{"script": "A kite over the sea.",
        "generate": {"prompt": "a kite", "model": "luma-ray-3.2-fal", "seconds": 10}}]});
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
    let plan_args = ["make", "--brief", "b.json", "--json", "--max-spend", "1.5"];
    let (code, plan) = scriptvox(&[&plan_args[..], &["--plan"]].concat());
    assert_eq!(code, Some(4), "{plan}");
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = scriptvox(&[&plan_args[..], &["--approve", &id]].concat());
    assert_eq!(code, Some(4), "{v}");
    assert_eq!(e.submits(), 0, "the cap held in make: {v}");
}

/// money-8: a provider's confirmed cancel gives the money back, so the same approved command may submit again.
#[test]
fn a_confirmed_cancel_releases_the_reservation() {
    let e = env();
    e.set(Mode::Busy);
    let (_, plan) = e.run(&["--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (code, v) = e.run(&["--approve", &id, "--detach"]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let dir = e.dir.path().join(".scriptvox/runs").join(&run);
    let t0 = std::time::Instant::now();
    while !dir.join("clip.job.json").is_file() {
        assert!(t0.elapsed().as_secs() < 30, "no job file");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (_, v) = e.cli(&["cancel", &run]);
    assert_eq!(v["metrics"]["provider_jobs_stopped"], 1, "{v}");
    let db = rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db")).unwrap();
    let status: String = db
        .query_row("SELECT status FROM ledger WHERE run = ?1", [&run], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "released");
    e.set(Mode::Normal);
    let (code, v) = e.run(&["--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 2, "a stopped job is submitted again: {v}");
}

/// money-9: the plan id covers the whole price of a generated scene, so an approval does not survive a price rise.
#[test]
fn a_new_price_on_a_row_is_a_new_plan() {
    let e = env();
    brief(&e, Some("kling-2.6-pro"), "aerial over an oasis");
    let (_, a) = e.make(&["--plan"], true);
    let p = e.dir.path().join("gen.toml");
    let rows = std::fs::read_to_string(&p).unwrap();
    std::fs::write(
        &p,
        rows.replacen(
            "name = \"kling-2.6-pro\"\n",
            "name = \"kling-2.6-pro\"\nusd_min = 5.0\n",
            1,
        ),
    )
    .unwrap();
    let (_, b) = e.make(&["--plan"], true);
    assert_eq!(b["metrics"]["cost_usd"], 5.0, "{b}");
    assert_ne!(a["plan_id"], b["plan_id"], "{b}");
}

/// The frame was paid and is done, the clip job is open: a resume buys neither again.
#[test]
fn a_resume_buys_neither_a_finished_first_frame_nor_its_clip_again() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let b = serde_json::json!({"scenes": [
        {"script": "The desert is quiet at dawn."},
        {"script": "Then the oasis comes alive with birds.", "generate":
            {"prompt": "aerial over an oasis", "model": "kling-2.6-pro", "first_frame": "a still oasis at dawn"}},
    ]});
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
    let (_, plan) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    e.set(Mode::NoClip);
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json", "--approve", &id]);
    assert_eq!(code, Some(5), "{v}");
    assert_eq!(e.submits(), 2, "the frame and the clip: {v}");
    e.set(Mode::Normal);
    let run = v["run_id"].as_str().unwrap().to_string();
    let (code, v) = scriptvox(&["resume", &run, "--wait", "--approve", &id, "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 2, "nothing bought again: {v}");
}

/// A raised cap is the same plan and the same run, so the open job is found. The run then reports what it spent.
#[test]
fn a_raised_cap_finds_the_open_job_and_the_run_reports_its_spend() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let (id, run) = paid_make_left_a_job(&e, &scriptvox);
    let (_, plan) = scriptvox(&[
        "make",
        "--brief",
        "b.json",
        "--plan",
        "--json",
        "--max-spend",
        "1",
    ]);
    assert_eq!(plan["plan_id"].as_str().unwrap(), id, "{plan}");
    let (code, v) = scriptvox(&[
        "make",
        "--brief",
        "b.json",
        "--json",
        "--approve",
        &id,
        "--max-spend",
        "1",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["run_id"].as_str().unwrap(), run, "{v}");
    assert_eq!(e.submits(), 1, "the same job: {v}");
    let spent = v["metrics"]["cost_usd"].as_f64().unwrap();
    assert!((spent - 0.35).abs() < 1e-9, "{v}");
}

/// A patch that inserts a scene moves the same request to another shot name: the open job is still found.
#[test]
fn a_patch_that_moves_a_scene_still_takes_over_its_job() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let (_, run) = paid_make_left_a_job(&e, &scriptvox);
    let p = serde_json::json!({"scenes": [
        {"script": "A new first line."},
        {"script": "The desert is quiet at dawn."},
        {"script": "Then the oasis comes alive with birds.", "generate":
            {"prompt": "aerial over an oasis", "model": "kling-2.6-pro"}},
    ]});
    std::fs::write(e.dir.path().join("p.json"), p.to_string()).unwrap();
    let (_, v) = scriptvox(&["resume", &run, "--patch", "p.json", "--wait", "--json"]);
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = scriptvox(&[
        "resume",
        &run,
        "--patch",
        "p.json",
        "--wait",
        "--approve",
        &id,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "the parent's job, under scene2: {v}");
}

/// A plan whose only paid stage is music is the plan's own run, so a rerun finds the music job.
#[test]
fn paid_music_alone_gets_the_plans_run_and_a_retry_pays_nothing() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    extra_row(
        &e,
        r#"[[generator]]
name = "music-paid"
gateway = "fal"
kind = "music.gen"
key = "fal"
t2v = "x/music"
durations = [30]
aspects = []
usd_per_s = 0.01
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "audio.url"
hosts = ["127.0.0.1"]
terms = { attribution_required = false, redistribution = true, note = "test" }
"#,
    );
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"script": "A short line under some music.", "tts": "none", "music_prompt": "calm piano"}"#,
    )
    .unwrap();
    let (_, plan) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    e.set(Mode::NoResult);
    let (code, first) = scriptvox(&["make", "--brief", "b.json", "--json", "--approve", &id]);
    assert_eq!(code, Some(5), "{first}");
    let (code, second) = scriptvox(&["make", "--brief", "b.json", "--json", "--approve", &id]);
    assert_eq!(code, Some(5), "{second}");
    assert_eq!(first["run_id"], second["run_id"], "{second}");
    assert_eq!(e.submits(), 1, "the same job: {second}");
}

/// A row priced by the length of a source video has no price in a scene, which has no source: never a $0 plan.
#[test]
fn a_scene_never_uses_a_row_priced_by_its_source() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    extra_row(
        &e,
        r#"[[generator]]
name = "v2v-by-source"
gateway = "fal"
kind = "video.gen"
key = "fal"
t2v = "x/v2v"
durations = [5]
aspects = ["9:16", "16:9", "1:1"]
usd_per_input_s = 1.0
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "video.url"
hosts = ["127.0.0.1"]
terms = { attribution_required = false, redistribution = true, note = "test" }
"#,
    );
    brief(&e, Some("v2v-by-source"), "aerial over an oasis");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("source"),
        "{v}"
    );
    brief(&e, None, "aerial over an oasis");
    let (_, v) = scriptvox(&["make", "--brief", "b.json", "--plan", "--json"]);
    assert!(!v["stages"].to_string().contains("v2v-by-source"), "{v}");
}

/// The names in a folder, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut n: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    n.sort();
    n
}

fn meta_of(x: &Value) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(x["artifacts"]["metadata"].as_str().unwrap()).unwrap(),
    )
    .unwrap()
}

/// G116: `export --cover-prompt` on a free row makes the cover at once; the frame cover stays under another name, and
/// both are in `covers`. Without the flag the folder is what it was.
#[test]
fn export_makes_a_cover_with_a_free_row_and_keeps_the_frame() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path();
    std::fs::write(p.join("s.txt"), "A dhow sails at dawn.").unwrap();
    let (code, v) = scriptvox(&["make", "--script", "s.txt", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    // `make` may call a free row of its own: the cover's calls are counted from here.
    let base = e.submits();
    // The guard: no flag, the same folder and the same metadata keys as before this feature.
    let (code, x) = scriptvox(&["export", &id, "--dir", "plain", "--json"]);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(e.submits(), base);
    assert_eq!(
        names(&p.join("plain")),
        [
            "captions.srt",
            "captions.vtt",
            "cover.jpg",
            "metadata.json",
            "video.mp4"
        ]
    );
    let plain = meta_of(&x);
    let mut keys: Vec<&String> = plain.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "ai_parts",
            "aspect",
            "chapters",
            "chapters_in_mp4",
            "commercial_ok",
            "cost_usd",
            "covers",
            "description",
            "duration_s",
            "formats",
            "lang",
            "made_with",
            "overlays",
            "parts",
            "platforms",
            "run_id",
            "sources",
            "tags",
            "title"
        ]
    );
    assert_eq!(plain["covers"], serde_json::json!([]));
    assert!(x["metrics"].get("cover_cost_usd").is_none(), "{x}");
    assert!(x["artifacts"].get("cover_frame").is_none(), "{x}");

    let cover = [
        "export",
        id.as_str(),
        "--dir",
        "designed",
        "--cover-prompt",
        "a dhow at dawn; $(touch pwned) [in]=x",
        "--cover-model",
        "flux2-klein-4b-self",
        "--json",
    ];
    let (code, x) = scriptvox(&cover);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(e.submits(), base + 1, "one picture");
    assert!(!p.join("pwned").exists());
    assert_eq!(x["metrics"]["cover_cost_usd"], 0.0, "{x}");
    let d = p.join("designed");
    assert_eq!(
        names(&d),
        [
            "captions.srt",
            "captions.vtt",
            "cover-frame.jpg",
            "cover.jpg",
            "metadata.json",
            "video.mp4"
        ]
    );
    let made = std::fs::read(d.join("cover.jpg")).unwrap();
    assert_eq!(made[..3], [0xFF, 0xD8, 0xFF], "a JPEG");
    assert_ne!(made, std::fs::read(d.join("cover-frame.jpg")).unwrap());
    assert_eq!(
        std::fs::read(d.join("cover-frame.jpg")).unwrap(),
        std::fs::read(p.join("plain/cover.jpg")).unwrap(),
        "the frame cover is the cover of an export with no flag"
    );
    assert!(x["artifacts"]["cover"]
        .as_str()
        .unwrap()
        .ends_with("cover.jpg"));
    assert!(x["artifacts"]["cover_frame"]
        .as_str()
        .unwrap()
        .ends_with("cover-frame.jpg"));
    let m = meta_of(&x);
    assert_eq!(m["covers"][0]["file"], "cover.jpg", "{m}");
    assert_eq!(m["covers"][0]["kind"], "generated");
    assert_eq!(m["covers"][0]["row"], "flux2-klein-4b-self");
    assert_eq!(m["covers"][0]["cost_usd"], 0.0);
    assert_eq!(m["covers"][1]["file"], "cover-frame.jpg");
    assert_eq!(m["covers"][1]["kind"], "frame");
    // The cover's row is a part with its terms, like the row of a generated scene.
    let part = m["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "flux2-klein-4b-self")
        .unwrap_or_else(|| panic!("{m}"));
    assert_eq!(part["kind"], "generator");
    assert_eq!(part["commercial"], true);
    assert!(part["terms"].as_str().unwrap().contains("Apache-2.0"));
    assert!(m["ai_parts"].to_string().contains("flux2-klein-4b-self"));
    // The same cover again comes from the store: no call.
    let (code, x) = scriptvox(&cover);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(e.submits(), base + 1, "no second picture");
    assert_eq!(meta_of(&x)["covers"][0]["reused"], true);
    // --cover-model needs --cover-prompt, and an empty prompt is refused.
    let (code, _) = scriptvox(&[
        "export",
        &id,
        "--cover-model",
        "flux2-klein-4b-self",
        "--json",
    ]);
    assert_eq!(code, Some(2));
    let (code, x) = scriptvox(&[
        "export",
        &id,
        "--cover-prompt",
        " ",
        "--dir",
        "none",
        "--json",
    ]);
    assert_eq!(code, Some(2), "{x}");
    assert!(!p.join("none").exists());
    // A row that makes no picture is refused before any call.
    let (code, x) = scriptvox(&[
        "export",
        &id,
        "--cover-prompt",
        "a dhow",
        "--cover-model",
        "wan22-ti2v-5b-self",
        "--dir",
        "none",
        "--json",
    ]);
    assert_eq!(code, Some(2), "{x}");
    assert_eq!(e.submits(), base + 1);
}

/// G116, money: a paid cover row stops with exit 4 and writes nothing; `--plan` shows the price; the approved call
/// submits once; the same call again takes the picture from the store.
#[test]
fn export_with_a_paid_cover_row_needs_approval_and_pays_once() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    let p = e.dir.path();
    std::fs::write(p.join("s.txt"), "A dhow sails at dawn.").unwrap();
    let (code, v) = scriptvox(&["make", "--script", "s.txt", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    // `make` may call a free row of its own: the cover's calls are counted from here.
    let base = e.submits();
    let cover = [
        "export",
        id.as_str(),
        "--dir",
        "paid",
        "--cover-prompt",
        "a dhow at dawn",
        "--cover-model",
        "flux-pro-ultra-fal",
        "--json",
    ];
    let (code, x) = scriptvox(&cover);
    assert_eq!(code, Some(4), "{x}");
    assert_eq!(x["status"], "approval_needed");
    assert_eq!(x["command"], "export");
    assert_eq!(x["metrics"]["cost_usd"], 0.06, "{x}");
    let plan_id = x["plan_id"].as_str().unwrap().to_string();
    assert!(
        x["next"][0].as_str().unwrap().starts_with("Ask the person"),
        "{x}"
    );
    assert!(x["next"][1].as_str().unwrap().contains(&plan_id), "{x}");
    assert!(!x["next"].to_string().contains("scriptvox generate"), "{x}");
    assert_eq!(e.submits(), base);
    assert!(
        !p.join("paid").exists(),
        "nothing is written before approval"
    );
    // The plan: the price, and nothing runs.
    let (code, x) = scriptvox(&[&cover[..], &["--plan"]].concat());
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(x["plan_id"], plan_id.as_str());
    assert_eq!(x["metrics"]["cost_usd"], 0.06);
    assert_eq!(e.submits(), base);
    assert!(!p.join("paid").exists());
    // Under a cap below the price: stopped at plan time, even with the approval.
    let (code, x) = scriptvox(&[&cover[..], &["--max-spend", "0"]].concat());
    assert_eq!(code, Some(4), "{x}");
    assert_eq!(x["error"]["code"], "SVX_SPEND_CAP", "{x}");
    // An approval of another plan is no approval.
    let (code, x) = scriptvox(&[&cover[..], &["--approve", "p_000000"]].concat());
    assert_eq!(code, Some(4), "{x}");
    assert_eq!(e.submits(), base);
    let (code, x) = scriptvox(&[&cover[..], &["--approve", &plan_id]].concat());
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(e.submits(), base + 1);
    assert_eq!(x["metrics"]["cover_cost_usd"], 0.06, "{x}");
    let m = meta_of(&x);
    assert_eq!(m["covers"][0]["cost_usd"], 0.06, "{m}");
    assert_eq!(m["covers"][0]["row"], "flux-pro-ultra-fal");
    // The same picture again, with and without the approval, to another folder: from the store, no call, no charge.
    let again = [
        "export",
        id.as_str(),
        "--dir",
        "again",
        "--cover-prompt",
        "a dhow at dawn",
        "--cover-model",
        "flux-pro-ultra-fal",
        "--json",
    ];
    for args in [
        [&cover[..], &["--approve", plan_id.as_str()]].concat(),
        again.to_vec(),
    ] {
        let (code, x) = scriptvox(&args);
        assert_eq!(code, Some(0), "{x}");
        assert_eq!(e.submits(), base + 1, "no second submit");
        assert_eq!(x["metrics"]["cover_cost_usd"], 0.0, "{x}");
    }
    assert!(p.join("again/cover.jpg").is_file());
    // Another prompt is another picture: it needs its own approval.
    let (code, x) = scriptvox(&[
        "export",
        &id,
        "--cover-prompt",
        "a falcon",
        "--cover-model",
        "flux-pro-ultra-fal",
        "--approve",
        &plan_id,
        "--json",
    ]);
    assert_eq!(code, Some(4), "{x}");
    assert_eq!(e.submits(), base + 1);
}

/// G116, terms: the cover's row counts in `commercial_ok` and gives the terms note, like a generated scene's row.
#[test]
fn a_cover_row_with_a_use_limit_makes_the_export_personal_use() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    extra_row(
        &e,
        r#"[[generator]]
name = "limited-image-self"
gateway = "selfhost"
kind = "image.gen"
key = "selfhost"
base = "https://gen.example.invalid/scriptvox-gen"
t2v = "limited/t2i"
durations = []
aspects = ["9:16", "16:9", "1:1"]
usd_per_s = 0.0
price = "exact"
price_source = "https://example.com/model"
price_checked = "2026-11-08"
output = "image.url"
hosts = ["127.0.0.1"]
terms = { commercial = false, attribution_required = true, redistribution = false, note = "test model, research use only" }
"#,
    );
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path();
    // A video file has no voice part, so only the cover's row can decide the answer.
    std::fs::write(
        p.join("talk.mp4"),
        clip(&ff, "testsrc2=size=640x360:rate=24", 2.0),
    )
    .unwrap();
    let (code, x) = scriptvox(&["export", "talk.mp4", "--dir", "a", "--json"]);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(x["metrics"]["commercial_ok"], Value::Null, "{x}");
    let (code, x) = scriptvox(&[
        "export",
        "talk.mp4",
        "--dir",
        "b",
        "--cover-prompt",
        "a dhow",
        "--cover-model",
        "limited-image-self",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(e.submits(), 1);
    assert_eq!(x["metrics"]["commercial_ok"], false, "{x}");
    assert!(x.get("terms_note").is_none(), "{x}");
    let m = meta_of(&x);
    assert_eq!(m["commercial_ok"], false);
    assert_eq!(m["parts"][0]["name"], "limited-image-self", "{m}");
    assert_eq!(m["parts"][0]["commercial"], false);
    // A wide video asks the row for a wide picture.
    assert_eq!(m["covers"][0]["aspect"], "16:9", "{m}");
    let credits = std::fs::read_to_string(p.join("b/credits.txt")).unwrap();
    assert!(
        credits.contains("Made with limited-image-self"),
        "{credits}"
    );
    // With a cleared row the answer stays what it was (null: a file's own parts are not known).
    let (code, x) = scriptvox(&[
        "export",
        "talk.mp4",
        "--dir",
        "c",
        "--cover-prompt",
        "a dhow",
        "--cover-model",
        "flux2-klein-4b-self",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{x}");
    assert_eq!(x["metrics"]["commercial_ok"], Value::Null, "{x}");
}

/// Every file under a folder.
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// G054: `make --storyboard` makes one still per clip scene and a sheet, then stops: no clip, no voice, no render.
/// A later `make` starts a `first_frame` scene from its still.
#[test]
fn a_storyboard_makes_one_still_per_clip_scene_and_no_clip() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path();
    let b = serde_json::json!({"aspect": "16:9", "scenes": [
        {"script": "The harbour is quiet."},
        {"script": "A dhow sails at dawn.", "generate": {"prompt": "the dhow glides right", "model": "wan22-ti2v-5b-self"}},
        {"script": "It passes the old fort.", "generate": {"prompt": "the camera rises", "model": "wan22-ti2v-5b-self",
            "first_frame": "a wooden dhow by a stone fort"}},
        {"script": "A map of the coast.", "generate": {"prompt": "an old map of the coast", "still": true}},
    ]});
    std::fs::write(p.join("b.json"), b.to_string()).unwrap();
    let board = ["make", "--brief", "b.json", "--storyboard", "--json"];
    let (code, v) = scriptvox(&board);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["command"], "make");
    assert_eq!(e.submits(), 2, "two pictures, no clip, no listen: {v}");
    let sheet = Path::new(v["artifacts"]["storyboard"].as_str().unwrap());
    assert_eq!(std::fs::read(sheet).unwrap()[1..4], *b"PNG", "{v}");
    let stills = v["stills"].as_array().unwrap();
    assert_eq!(stills.len(), 2, "the clip scenes only: {v}");
    assert_eq!(stills[0]["scene"], 2);
    assert_eq!(stills[0]["from"], "prompt");
    assert_eq!(stills[0]["make_reuses"], false);
    assert_eq!(stills[1]["scene"], 3);
    assert_eq!(stills[1]["from"], "first_frame");
    assert_eq!(stills[1]["make_reuses"], true);
    assert_eq!(stills[1]["prompt"], "a wooden dhow by a stone fort");
    for s in stills {
        assert_eq!(s["row"], "flux2-klein-4b-self", "{v}");
        assert!(Path::new(s["file"].as_str().unwrap()).is_file(), "{v}");
    }
    assert_eq!(v["metrics"]["cost_usd"], 0.0);
    assert!(
        v["next"][0]
            .as_str()
            .unwrap()
            .contains("without --storyboard"),
        "{v}"
    );
    assert!(v["make_plan"]["plan_id"].as_str().is_some(), "{v}");
    assert!(v["artifacts"].get("video").is_none());
    // Nothing of a video: no voice, no words, no clip, no render.
    let files = [walk(&p.join(".scriptvox")), walk(&p.join("scriptvox-out"))].concat();
    for bad in [
        "voice.mp3",
        "voice.wav",
        "words.json",
        "scene2.mp4",
        "scene3.mp4",
    ] {
        assert!(
            !files
                .iter()
                .any(|f| f.file_name().is_some_and(|n| n == bad)),
            "{bad} was made"
        );
    }
    assert!(!files
        .iter()
        .any(|f| f.extension().is_some_and(|x| x == "mp4")));
    // The same storyboard again: the stills come from the store.
    let (code, v) = scriptvox(&board);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 2, "no new picture");
    assert_eq!(v["stills"][0]["reused"], true, "{v}");
    // The plan of the storyboard makes no call.
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--storyboard", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["command"], "plan");
    assert_eq!(e.submits(), 2);
    // `make` starts scene 3 from its still: one call fewer than the same brief with no storyboard before it.
    let alone = env();
    std::fs::write(alone.dir.path().join("b.json"), b.to_string()).unwrap();
    let (code, v) = runner(&alone, &ff)(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    let (code, v) = scriptvox(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(
        e.submits() - 2,
        alone.submits() - 1,
        "the first frame is the storyboard's: {v}"
    );
    let run = p
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    assert!(run.join("scene3.mp4").is_file());
    // The image row of the stored first frame left no job in this run, and still counts as a part with its terms.
    assert!(!run.join("scene3_frame.job.json").exists());
    let (code, x) = scriptvox(&["export", v["run_id"].as_str().unwrap(), "--json"]);
    assert_eq!(code, Some(0), "{x}");
    let m = meta_of(&x);
    assert!(
        m["parts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "flux2-klein-4b-self"),
        "{m}"
    );
    // With --detach there is nothing to wait for: refused.
    let (code, _) = scriptvox(&[
        "make",
        "--brief",
        "b.json",
        "--storyboard",
        "--detach",
        "--json",
    ]);
    assert_eq!(code, Some(2));
}

/// G054, money: a paid image row needs the approval of the storyboard's own plan, which holds the stills only.
#[test]
fn a_paid_storyboard_needs_its_own_approval_and_never_the_clips_cost() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = paid_runner(&e, &ff);
    brief(&e, Some("kling-2.6-pro"), "aerial over an oasis");
    let (code, clips) = scriptvox(&["plan", "--brief", "b.json", "--json"]);
    assert_eq!(code, Some(0), "{clips}");
    let clip_cost = clips["metrics"]["cost_usd"].as_f64().unwrap();
    assert!(clip_cost > 0.06, "{clips}");
    let clip_plan = clips["plan_id"].as_str().unwrap().to_string();
    let board = ["make", "--brief", "b.json", "--storyboard", "--json"];
    let (code, v) = scriptvox(&board);
    assert_eq!(code, Some(4), "{v}");
    assert_eq!(v["status"], "approval_needed");
    assert_eq!(
        v["metrics"]["cost_usd"], 0.06,
        "one picture, not the clip: {v}"
    );
    assert_eq!(v["stages"].as_array().unwrap().len(), 1, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    assert_ne!(id, clip_plan);
    assert_eq!(e.submits(), 0);
    let (code, v) = scriptvox(&["plan", "--brief", "b.json", "--storyboard", "--json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["plan_id"], id.as_str());
    assert_eq!(v["metrics"]["cost_usd"], 0.06);
    // The approval of the video's plan is not an approval of the storyboard, and a cap under the price stops it.
    let (code, v) = scriptvox(&[&board[..], &["--approve", &clip_plan]].concat());
    assert_eq!(code, Some(4), "{v}");
    let (code, v) = scriptvox(&[&board[..], &["--approve", &id, "--max-spend", "0.01"]].concat());
    assert_ne!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 0);
    let (code, v) = scriptvox(&[&board[..], &["--approve", &id]].concat());
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "one picture and no clip");
    assert_eq!(v["metrics"]["cost_usd"], 0.06, "{v}");
    assert_eq!(v["make_plan"]["cost_usd"], clip_cost, "{v}");
    assert_eq!(v["make_plan"]["plan_id"], clip_plan.as_str());
    // Paid once: again with no approval, from the store.
    let (code, v) = scriptvox(&board);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1);
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
}

#[test]
fn a_brief_with_no_clip_scene_has_nothing_to_storyboard() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let scriptvox = runner(&e, &ff);
    let p = e.dir.path();
    for scenes in [
        serde_json::json!([{"script": "The harbour is quiet."}]),
        serde_json::json!([{"script": "A map.", "generate": {"prompt": "an old map", "still": true}}]),
    ] {
        std::fs::write(
            p.join("b.json"),
            serde_json::json!({"scenes": scenes}).to_string(),
        )
        .unwrap();
        let (code, v) = scriptvox(&["make", "--brief", "b.json", "--storyboard", "--json"]);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("nothing to storyboard"),
            "{v}"
        );
    }
    assert_eq!(e.submits(), 0);
}

/// A row's result is a file from a service: a list in place of a clip (ffconcat, HLS) is refused as the service's
/// fault, and FFmpeg and ffprobe are never given it (`docs/specs/media-inputs.md`).
#[cfg(unix)]
#[test]
fn a_result_that_is_a_list_is_refused_and_never_reaches_ffmpeg() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret-target.mp4");
    std::fs::write(&secret, clip(&ff, "testsrc2=size=160x90:rate=24", 1.0)).unwrap();
    let (_, plan) = e.run(&["--plan"]);
    let id = plan["plan_id"].as_str().unwrap().to_string();
    let (rec, log) = common::record(&ff, e.dir.path());
    for list in [
        format!("ffconcat version 1.0\nfile '{}'\n", secret.display()),
        format!(
            "#EXTM3U\n#EXTINF:1.0,\n{}\n#EXT-X-ENDLIST\n",
            secret.display()
        ),
    ] {
        *e.fake.clip.lock().unwrap() = list.into_bytes();
        let o = common::bin()
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_FFMPEG", &rec)
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args([
                "generate",
                "--model",
                "kling-2.6-pro",
                "--prompt",
                "aerial over an oasis",
            ])
            .args([
                "--seconds",
                "5",
                "--json",
                "--approve",
                &id,
                "--out",
                "got.mp4",
            ])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
        assert_eq!(o.status.code(), Some(1), "{v}");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("kling-2.6-pro") && msg.contains("list") && msg.contains("Accepted:"),
            "{v}"
        );
        assert!(!e.dir.path().join("got.mp4").exists(), "nothing is written");
        let opened = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !opened.contains(" -i "),
            "FFmpeg was given a file: {opened}"
        );
        assert!(!opened.contains("secret-target"), "{opened}");
    }
    assert_eq!(
        e.submits(),
        1,
        "the job is on record: the rerun fetched the same result, with no second charge"
    );
}

/// A generated scene's own first frame (`generate.image`) that is a list under a picture's name: exit 2 at the plan
/// and at the run, nothing is sent to a row, and FFmpeg and ffprobe never get the list (`docs/specs/media-inputs.md`).
#[cfg(unix)]
#[test]
fn a_scene_image_that_is_a_list_is_refused_before_any_row_is_called() {
    let Some(ff) = common::ready() else { return };
    let e = env();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret-target.mp4");
    std::fs::write(&secret, clip(&ff, "testsrc2=size=160x90:rate=24", 1.0)).unwrap();
    std::fs::write(
        e.dir.path().join("list-frame.png"),
        format!("ffconcat version 1.0\nfile '{}'\n", secret.display()),
    )
    .unwrap();
    let b = serde_json::json!({"scenes": [
        {"script": "The desert is quiet at dawn."},
        {"script": "Then the oasis comes alive with birds.",
         "generate": {"prompt": "aerial over an oasis", "image": "list-frame.png"}},
    ]});
    std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
    let (rec, log) = common::record(&ff, e.dir.path());
    for extra in [&["--plan"][..], &["--max-spend", "5"]] {
        let o = common::bin()
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_GENERATORS", e.dir.path().join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.fake.port),
            )
            .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
            .env("SCRIPTVOX_FFMPEG", &rec)
            .env("FAL_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .args(["make", "--brief", "b.json", "--json"])
            .args(extra)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
        assert_eq!(o.status.code(), Some(2), "{extra:?}: {v}");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("list-frame.png")
                && msg.contains("an ffconcat list")
                && msg.contains("Accepted:"),
            "{v}"
        );
    }
    assert_eq!(e.submits(), 0, "no row was called");
    let opened = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(!opened.contains("list-frame.png"), "{opened}");
    assert!(!opened.contains("secret-target"), "{opened}");
}

/// A clip in the store stays on the row that made it when that row's own service is off: the plan says "already
/// made" at $0, and no other row is bought in its place. Only a shot that must still be made asks the service, and
/// then a row that does not answer is not chosen silently (docs/specs/reel-gaps.md).
#[test]
fn a_clip_in_the_store_stays_on_its_row_when_the_service_is_off() {
    if common::ready().is_none() {
        return;
    }
    let e = env();
    // No voice: the test is about the clip, and needs no worker.
    let brief = |prompt: &str| {
        let b = serde_json::json!({"tts": "none", "lang": "en", "scenes": [
            {"script": "The desert is quiet at dawn."},
            {"script": "Then the oasis comes alive with birds.", "generate": {"prompt": prompt}},
        ]});
        std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
    };
    brief("aerial over an oasis");
    let up = format!("http://127.0.0.1:{}", e.fake.port);
    let run = |base: &str, extra: &[&str]| {
        let p = e.dir.path();
        let mut c = common::bin();
        c.current_dir(p)
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env("SCRIPTVOX_FAL_BASE", base)
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            // A paid row has a key too: the row that would be bought in place of the free one.
            .env("FAL_KEY", "test-fal-key")
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .env_remove("CLAUDECODE")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .args(["make", "--brief", "b.json", "--json"])
            .args(extra);
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let gen = |v: &Value| {
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["stage"] == "generate")
            .unwrap()
            .clone()
    };
    let (code, v) = run(&up, &["--out", "one.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1);
    // The service is off. The clip is in the store: the same row, no charge, no finding, and no stop.
    let down = "http://127.0.0.1:9";
    let (code, plan) = run(down, &["--plan"]);
    assert_eq!(code, 0, "{plan}");
    let g = gen(&plan);
    assert_eq!(g["tool"], "video.gen.wan22-ti2v-5b-self", "{plan}");
    assert!(
        g["explain"].as_str().unwrap().contains("already made"),
        "{g}"
    );
    assert_eq!(plan["metrics"]["cost_usd"], 0.0);
    let findings = plan["findings"].as_array().unwrap();
    assert!(findings.iter().all(|f| f["gate"] != "services"), "{plan}");
    let (code, v) = run(down, &["--out", "two.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(e.submits(), 1, "the clip came from the store");
    // Another shot must be made: the row whose service does not answer is not used, and the plan says so.
    brief("a camel walks over a dune");
    let (_, plan) = run(down, &["--plan"]);
    assert_ne!(gen(&plan)["tool"], "video.gen.wan22-ti2v-5b-self", "{plan}");
    let said = plan["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "services")
        .unwrap_or_else(|| panic!("no finding: {plan}"));
    assert!(
        said["detail"]
            .as_str()
            .unwrap()
            .contains("wan22-ti2v-5b-self"),
        "{said}"
    );
}
