//! Gaps of the reel test of 14 Nov 2026 (docs/specs/reel-gaps.md): brief types, no terms note in the flow, the
//! rows that take a start picture. No network.
mod common;
use serde_json::{json, Value};
use std::path::Path;

/// Exit code, the envelope and stderr.
fn sv(dir: &Path, args: &[&str]) -> (Option<i32>, Value, String) {
    sv_env(dir, args, &[])
}

fn sv_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, Value, String) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_DATA", dir.join("data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .env_remove("FAL_KEY")
        .env_remove("HTTPS_PROXY")
        .env_remove("https_proxy")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .envs(env.iter().copied())
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn plan(dir: &Path, b: &Value) -> (Option<i32>, Value) {
    std::fs::write(dir.join("b.json"), b.to_string()).unwrap();
    let (code, v, _) = sv(dir, &["plan", "--brief", "b.json"]);
    (code, v)
}

#[test]
fn music_from_takes_a_number_or_text_and_gives_one_plan() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(sv(d.path(), &["setup", "fetch"]).0, Some(0));
    let brief = |from: Value, to: Value| {
        json!({"script": "Dates are an old food. People like them.", "tts": "none", "lang": "en",
            "music": "https://media.example.org/song.mp3", "music_from": from, "music_to": to})
    };
    let (code, n) = plan(d.path(), &brief(json!(0), json!(12.5)));
    assert_eq!(code, Some(0), "{n}");
    let (code, t) = plan(d.path(), &brief(json!("0"), json!("12.5")));
    assert_eq!(code, Some(0), "{t}");
    assert_eq!(
        n["plan_id"], t["plan_id"],
        "a number is the same cut as its text"
    );
    let (code, v) = plan(d.path(), &brief(json!([1]), json!("0:12")));
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("music_from: seconds as a number"),
        "{v}"
    );
}

#[test]
fn a_type_error_names_its_field() {
    let d = tempfile::tempdir().unwrap();
    for (b, want) in [
        (
            json!({"script": "Hello.", "duration": "12"}),
            "duration: invalid type: string \"12\"",
        ),
        (json!({"script": "Hello.", "sfx": 1}), "sfx: invalid type"),
        (
            json!({"script": "Hello.", "logo": {"file": "a.png", "width": "wide"}}),
            "logo: invalid type",
        ),
        (
            json!({"scenes": [{"script": "One."}, {"script": "Two.", "media_from": "3"}]}),
            "scenes[1].media_from: invalid type",
        ),
        // An unknown field keeps its own message, with the closest name.
        (
            json!({"script": "Hello.", "captoins": "pop"}),
            "unknown field `captoins`",
        ),
    ] {
        let (code, v) = plan(d.path(), &b);
        assert_eq!(code, Some(2), "{v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains(want), "{msg}");
    }
}

#[test]
fn the_plan_and_setup_fetch_show_no_terms_note_twice_or_at_all() {
    let d = tempfile::tempdir().unwrap();
    // A JSON run: the notice is in the envelope, and not on stderr too.
    let (code, v, err) = sv(d.path(), &["setup", "fetch"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        v["notice"]
            .as_str()
            .unwrap()
            .contains("responsibility are yours"),
        "{v}"
    );
    assert!(!err.contains("responsibility"), "printed twice: {err}");
    let (code, v) = plan(
        d.path(),
        &json!({"script": "Dates are an old food. People like them.", "tts": "none",
        "lang": "en", "music": "https://media.example.org/song.mp3", "stock": ["dates"],
        "scenes": null}),
    );
    assert_eq!(code, Some(0), "{v}");
    let text = v["stages"].to_string().to_lowercase();
    assert!(text.contains("https://media.example.org/song.mp3"), "{v}");
    for word in [
        "right to use",
        "licen",
        "commercial",
        "terms",
        "redistribut",
    ] {
        assert!(
            !text.contains(word),
            "a terms note in the plan ({word}): {v}"
        );
    }
}

/// A brief with a character: one clip scene and one still.
fn cat(model: Option<&str>) -> Value {
    let mut b = json!({"aspect": "9:16", "tts": "none", "lang": "en",
    "character": {"name": "cat", "prompt": "an orange tabby cat"},
    "scenes": [
        {"script": "Paws up.", "generate": {"prompt": "The cat stands up.", "seconds": 5}},
        {"script": "Look at me.", "generate": {"prompt": "The cat leans in.", "still": true}}
    ]});
    if let Some(m) = model {
        b["scenes"][0]["generate"]["model"] = json!(m);
    }
    b
}

#[test]
fn providers_say_which_video_row_takes_a_start_picture_and_the_error_names_them() {
    let d = tempfile::tempdir().unwrap();
    let (code, v, _) = sv(d.path(), &["providers", "--kind", "video"]);
    assert_eq!(code, Some(0), "{v}");
    let rows = v["generators"].as_array().unwrap();
    let takes = |name: &str| {
        rows.iter().find(|g| g["name"] == name).unwrap()["takes_start_picture"].clone()
    };
    assert_eq!(takes("kling-2.6-pro"), json!(true));
    assert_eq!(takes("kling-3.0-pro"), json!(false));
    assert!(
        rows.iter().all(|g| g["takes_start_picture"].is_boolean()),
        "{v}"
    );
    // A character makes each clip start from a picture: a row with no image to video is refused at plan, by name.
    std::fs::write(
        d.path().join("b.json"),
        cat(Some("kling-3.0-pro")).to_string(),
    )
    .unwrap();
    // The picture of the character comes from the image row of the user's own service, which answers here.
    let open = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", open.local_addr().unwrap().port());
    let (code, v, _) = sv_env(
        d.path(),
        &["plan", "--brief", "b.json"],
        &[
            ("FAL_KEY", "test"),
            ("SCRIPTVOX_SELFHOST_KEY", "test"),
            ("SCRIPTVOX_FAL_BASE", &base),
        ],
    );
    assert_eq!(code, Some(2), "{v}");
    let (msg, fix) = (
        v["error"]["message"].as_str().unwrap(),
        v["error"]["fix"].as_str().unwrap(),
    );
    assert!(
        msg.contains("kling-3.0-pro does not fit") && msg.contains("takes no start picture"),
        "{msg}"
    );
    assert!(
        fix.contains("Rows that take a start picture") && fix.contains("kling-2.6-pro"),
        "{fix}"
    );
    assert!(!fix.contains("kling-3.0-pro"), "{fix}");
}

#[test]
fn a_plan_lists_each_service_call_and_does_not_choose_a_service_that_does_not_answer() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("b.json"), cat(None).to_string()).unwrap();
    std::fs::write(
        d.path().join("named.json"),
        cat(Some("wan22-ti2v-5b-self")).to_string(),
    )
    .unwrap();
    // A loopback port that answers, and one that does not.
    let open = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let up = format!("http://127.0.0.1:{}", open.local_addr().unwrap().port());
    let down = format!("http://127.0.0.1:{closed}");
    let with = |base: &str, brief: &str, more: &[&str]| {
        let args = [&["plan", "--brief", brief][..], more].concat();
        let (code, v, _) = sv_env(
            d.path(),
            &args,
            &[
                ("SCRIPTVOX_SELFHOST_KEY", "test"),
                ("SCRIPTVOX_FAL_BASE", base),
            ],
        );
        (code, v)
    };
    // The service answers: the plan has a stage for the character's picture, the first frame, the clip and the still.
    let (code, v) = with(&up, "b.json", &[]);
    assert_eq!(code, Some(0), "{v}");
    let stages = v["stages"].as_array().unwrap();
    for (stage, tool) in [
        ("character", "image.gen.flux2-klein-4b-self"),
        ("first_frame", "image.gen.flux2-klein-4b-self"),
        ("generate", "video.gen.wan22-ti2v-5b-self"),
    ] {
        let s = stages
            .iter()
            .find(|s| s["stage"] == stage)
            .unwrap_or_else(|| panic!("no {stage} stage: {v}"));
        assert_eq!(s["tool"], tool, "{v}");
        assert!(
            s["explain"]
                .as_str()
                .unwrap()
                .contains("your own service answers now"),
            "{s}"
        );
    }
    let (code, v) = with(&up, "b.json", &["--storyboard"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        v["stages"][0]["explain"]
            .as_str()
            .unwrap()
            .contains("your own service answers now"),
        "{v}"
    );
    // The service does not answer, and no other row has a key: the plan stops and says why, with what to do.
    let (code, v) = with(&down, "b.json", &[]);
    assert_ne!(code, Some(0), "{v}");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("does not answer now") && msg.contains("scriptvox providers test"),
        "{msg}"
    );
    let (code, v) = with(&down, "b.json", &["--storyboard"]);
    assert_ne!(code, Some(0), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not answer now"),
        "{v}"
    );
    // A row named in the brief is not replaced by another one.
    let mut own = cat(Some("wan22-ti2v-5b-self"));
    own.as_object_mut().unwrap().remove("character");
    std::fs::write(d.path().join("named.json"), own.to_string()).unwrap();
    let (code, v) = with(&down, "named.json", &[]);
    assert_ne!(code, Some(0), "{v}");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("wan22-ti2v-5b-self is on your own GPU service")
            && msg.contains("does not answer now"),
        "{msg}"
    );
}
