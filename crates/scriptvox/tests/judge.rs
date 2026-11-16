//! `scriptvox judge` against a fake OpenAI-compatible endpoint: one weak frame gives exit 3 with its fix.
mod common;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

fn fake_vlm(reply: String) -> (String, std::sync::mpsc::Receiver<Value>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut line, mut len, mut auth) = (String::new(), 0usize, false);
            r.read_line(&mut line).unwrap();
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
                if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
                auth |= h.to_lowercase().starts_with("authorization:");
            }
            let mut body = vec![0u8; len];
            let _ = r.read_exact(&mut body);
            let mut got = serde_json::from_slice(&body).unwrap_or(json!({}));
            got["_auth"] = auth.into();
            let _ = tx.send(got);
            let out = json!({"choices": [{"message": {"content": reply}}]}).to_string();
            let mut w = s;
            let _ = write!(
                w,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                out.len()
            );
        }
    });
    (base, rx)
}

#[test]
fn a_weak_frame_fails_with_its_fix() {
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
            "testsrc2=size=320x240:rate=24:d=2",
        ])
        .args(["-pix_fmt", "yuv420p", "-y", "in.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(d.path().join("brief.txt"), "A date palm at sunrise.").unwrap();
    let reply = r#"{"frames":[{"score":4,"matches_brief":true,"text_ok":true,"issues":[],"fix":""},
        {"score":2,"matches_brief":false,"text_ok":true,"issues":["test pattern, no palm"],"fix":"Use a palm shot."}],
        "score":3,"fixes":["Use a palm shot."]}"#;
    let (base, rx) = fake_vlm(reply.into());
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "judge",
            "in.mp4",
            "--brief",
            "brief.txt",
            "--frames",
            "2",
            "--endpoint",
            &base,
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(3), "{v}");
    assert_eq!(v["metrics"]["judge_min"], 2, "{v}");
    assert_eq!(v["findings"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["error"]["fix"], "Use a palm shot.");
    assert_eq!(v["next"][0], "Use a palm shot.");
    // The request carried the brief and two pictures.
    let req = rx.recv().unwrap();
    let c = req["messages"][0]["content"].as_array().unwrap();
    assert!(c[0]["text"]
        .as_str()
        .unwrap()
        .contains("A date palm at sunrise."));
    assert_eq!(c.iter().filter(|p| p["type"] == "image_url").count(), 2);
}

/// security-2: the stored vision key goes only to the configured service, never to another `--endpoint`.
#[test]
fn the_vision_key_goes_only_to_the_configured_service() {
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
            "testsrc2=size=320x240:rate=24:d=2",
        ])
        .args(["-pix_fmt", "yuv420p", "-y", "in.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(d.path().join("brief.txt"), "A test pattern.").unwrap();
    let reply = r#"{"frames":[{"score":5,"matches_brief":true,"text_ok":true,"issues":[],"fix":""},{"score":5,"matches_brief":true,"text_ok":true,"issues":[],"fix":""}],"score":5,"fixes":[]}"#;
    let (configured, sent) = fake_vlm(reply.into());
    let (other, sent_other) = fake_vlm(reply.into());
    let judge = |endpoint: &str| {
        common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env("SCRIPTVOX_VISION_URL", &configured)
            .env("SCRIPTVOX_VISION_KEY", "fake-vision-key")
            .args([
                "judge",
                "in.mp4",
                "--brief",
                "brief.txt",
                "--frames",
                "2",
                "--endpoint",
                endpoint,
                "--json",
            ])
            .output()
            .unwrap();
    };
    judge(&configured);
    assert_eq!(
        sent.recv_timeout(std::time::Duration::from_secs(30))
            .unwrap()["_auth"],
        true,
        "the configured service gets its key"
    );
    judge(&other);
    assert_eq!(
        sent_other
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap()["_auth"],
        false,
        "another endpoint gets no key"
    );
}

#[test]
fn media_match_sends_at_make_not_at_plan_and_keeps_the_plan_id() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("clips")).unwrap();
    for (name, colour) in [("a.png", "red"), ("b.png", "blue")] {
        let st = std::process::Command::new(&ff)
            .args(["-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("color=c={colour}:s=64x64"))
            .args(["-frames:v", "1"])
            .arg(d.path().join("clips").join(name))
            .status()
            .unwrap();
        assert!(st.success());
    }
    std::fs::write(
        d.path().join("b.json"),
        r#"{"lang": "en", "tts": "edge", "media": "clips", "media_match": true,
           "script": "The sea is very blue. The sun is very red."}"#,
    )
    .unwrap();
    let run = |url: &str, args: &[&str]| {
        let mut c = common::bin();
        let o = c
            .current_dir(d.path())
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env("SCRIPTVOX_VISION_URL", url)
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("ELEVENLABS_API_KEY", "fake-eleven-key-123456")
            .args(args)
            .args(["--brief", "b.json", "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let vision_stage = |v: &Value| {
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["stage"] == "vision")
            .map(|s| s["explain"].as_str().unwrap().to_string())
    };
    let (base, rx) = fake_vlm(json!({"picks": [1, 0]}).to_string());
    // A plan sends nothing, and names the host that `make` will send to.
    let (_, v) = run(&base, &["plan"]);
    assert_eq!(v["ok"], true, "{v}");
    assert!(rx.try_recv().is_err(), "a plan sent a request: {v}");
    assert!(vision_stage(&v).unwrap().contains(&base), "{v}");
    assert!(v["media_match"].is_null(), "{v}");
    assert!(!v["findings"].to_string().contains("not matched"), "{v}");
    let id = v["plan_id"].clone();
    // The same brief with a paid voice: `make` stops for approval first and sends nothing, and the plan id is the plan's.
    let brief = std::fs::read_to_string(d.path().join("b.json")).unwrap();
    std::fs::write(
        d.path().join("b.json"),
        brief.replace("\"edge\"", "\"elevenlabs\""),
    )
    .unwrap();
    let (_, p) = run(&base, &["plan"]);
    assert!(rx.try_recv().is_err(), "a plan sent a request: {p}");
    let (code, m) = run(&base, &["make"]);
    assert_eq!(code, Some(4), "{m}");
    assert_eq!(m["plan_id"], p["plan_id"], "{m}");
    assert_ne!(id, p["plan_id"]);
    let (code, m) = run(&base, &["make", "--approve", "p_000000"]);
    assert_eq!(code, Some(4), "a stale approval sends nothing: {m}");
    assert!(
        rx.try_recv().is_err(),
        "frames left before the approval check: {m}"
    );
    std::fs::write(d.path().join("b.json"), &brief).unwrap();
    // No service set: no stage, and a warning that nothing went.
    std::fs::remove_dir_all(d.path().join(".scriptvox")).unwrap();
    let (_, v) = run("", &["plan"]);
    assert_eq!(v["ok"], true, "{v}");
    assert!(vision_stage(&v).is_none(), "{v}");
    assert!(
        v["findings"].to_string().contains("Nothing was sent"),
        "{v}"
    );
    // No folder is a usage error.
    std::fs::write(
        d.path().join("b.json"),
        r#"{"script": "Hello there.", "media_match": true}"#,
    )
    .unwrap();
    let (_, v) = run(&base, &["plan"]);
    assert_eq!(v["ok"], false, "{v}");
    assert!(v["error"]["fix"].as_str().unwrap().contains("media"), "{v}");
}

#[test]
fn no_vision_service_set_means_no_frame_leaves() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("any.mp4"), b"not read").unwrap();
    let mut c = common::bin();
    let o = c
        .current_dir(d.path())
        .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
        .env_remove("SCRIPTVOX_VISION_URL")
        .args(["judge", "any.mp4", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(10), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Nothing was sent"),
        "{v}"
    );
}

/// `analyze --like` and `clips` with no vision service: the same refusal, on a real video.
#[test]
fn like_and_clips_send_nothing_with_no_vision_service() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=s=320x180:d=3"])
        .args(["-c:v", "libx264"])
        .arg(d.path().join("talk.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let run = |args: &[&str]| {
        let mut c = common::bin();
        let o = c
            .current_dir(d.path())
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env_remove("SCRIPTVOX_VISION_URL")
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    // `clips` stops, before the transcript.
    let (code, v) = run(&["clips", "talk.mp4", "--min", "1", "--max", "2"]);
    assert_eq!(code, Some(10), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Nothing was sent"),
        "{v}"
    );
    // `--like` still gives the measured brief, and says the look did not come from a service.
    let (code, v) = run(&["analyze", "talk.mp4", "--like", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        v["findings"].to_string().contains("Nothing was sent"),
        "{v}"
    );
    assert!(d.path().join("b.json").is_file());
}

/// A `make` whose vision service does not answer still gives a video in folder order, and says so.
#[cfg(unix)]
#[test]
fn media_match_on_a_free_plan_sends_at_make_and_keeps_the_picks_for_the_plan() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    common::with_media(&e);
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"lang": "en", "media": "media", "media_match": true, "script": "The sea is very blue. The sun is very red."}"#,
    )
    .unwrap();
    let (base, rx) = fake_vlm(json!({"picks": [1, 0]}).to_string());
    let run = |cmd: &str| {
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_VISION_URL", &base)
            .env("SCRIPTVOX_VISION_MODEL", "my/model")
            .args([cmd, "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (_, before) = run("plan");
    assert!(rx.try_recv().is_err(), "a plan sent a request: {before}");
    let (code, m) = run("make");
    assert_eq!(code, Some(0), "{m}");
    let sent = rx.try_recv().expect("make sends the frames");
    assert_eq!(
        sent["model"], "my/model",
        "G136: the setting names the model"
    );
    while rx.try_recv().is_ok() {}
    // The kept picks show at plan, with no new call and the same plan id.
    let (_, v) = run("plan");
    assert_eq!(
        v["media_match"]["scenes"][0]["file"], "02-fractal.jpg",
        "{v}"
    );
    assert_eq!(
        v["plan_id"], before["plan_id"],
        "the picks do not change the plan: {v}"
    );
    assert!(rx.try_recv().is_err(), "kept picks need no call: {v}");
}

#[cfg(unix)]
#[test]
fn media_match_failed_call_at_make_is_only_a_warning() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    common::with_media(&e);
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"lang": "en", "media": "media", "media_match": true, "script": "The sea is very blue. The sun is very red."}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .env("SCRIPTVOX_VISION_URL", "http://127.0.0.1:9/v1")
        .args(["make", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert!(v["findings"].to_string().contains("not matched"), "{v}");
}
