mod common;
use serde_json::Value;

fn run(args: &[&str]) -> (i32, Value) {
    let mut cmd = common::bin();
    // The snapshot must not depend on who runs the tests (Claude Code sets CLAUDECODE).
    for v in scriptvox::agent_mode::VARS {
        cmd.env_remove(v);
    }
    let out = cmd.args(args).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        1,
        "stdout must hold exactly one document: {stdout}"
    );
    (
        out.status.code().unwrap(),
        serde_json::from_str(&stdout).unwrap(),
    )
}

fn run_in(dir: &std::path::Path, vision: Option<&str>, args: &[&str]) -> (i32, Value) {
    let mut cmd = common::bin();
    cmd.current_dir(dir)
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_VISION_URL");
    if let Some(u) = vision {
        cmd.env("SCRIPTVOX_VISION_URL", u);
    }
    let out = cmd.args(args).output().unwrap();
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn next_offers_judge_only_with_a_vision_service() {
    let d = tempfile::tempdir().unwrap();
    let run = d.path().join(".scriptvox/runs/r_1");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(run.join("result.json"), r#"{"ok": true, "run_id": "r_1"}"#).unwrap();
    let (code, v) = run_in(d.path(), None, &["wait", "r_1", "--json"]);
    assert_eq!(code, 0, "{v}");
    assert!(!v["next"].to_string().contains("judge"), "{v}");
    let (_, v) = run_in(
        d.path(),
        Some("http://127.0.0.1:1/v1"),
        &["wait", "r_1", "--json"],
    );
    assert!(v["next"].to_string().contains("judge"), "{v}");
}

#[test]
fn templates_say_how_to_use_a_row_and_stock_needs_no_key() {
    let (code, v) = run(&["templates", "--json"]);
    assert_eq!(code, 0);
    assert!(v["next"][0].as_str().unwrap().contains("`command`"), "{v}");
    let stock = v["templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "stock-short")
        .unwrap();
    assert_eq!(stock["keys"], "none", "{v}");
}

#[test]
fn a_missing_recipe_file_is_a_usage_error() {
    for c in ["explain", "replay", "remake"] {
        let (code, v) = run(&[c, "/nonexistent/a.mp4", "--json"]);
        assert_eq!(code, 2, "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains("a.mp4"),
            "{v}"
        );
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
}

#[test]
fn providers_kind_refuses_a_kind_no_row_has() {
    let (code, v) = run(&["providers", "--kind", "nosuch", "--json"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("video.gen"),
        "{v}"
    );
    for k in ["video", "upscale", "video.gen"] {
        assert_eq!(run(&["providers", "--kind", k, "--json"]).0, 0, "{k}");
    }
}

#[test]
fn version_json() {
    let (code, v) = run(&["--version", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(v["schema"], "scriptvox.result/v1");
    assert_eq!(v["ok"], true);
    assert_eq!(v["proto"], "1.0");
    assert!(v.as_object().unwrap().contains_key("ffmpeg_pin"));
}

#[test]
fn unknown_command_is_usage_error() {
    let (code, v) = run(&["nosuchcmd", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(v["ok"], false);
    assert!(v["error"]["code"].as_str().unwrap().starts_with("SVX_"));
    assert_eq!(
        v["error"]["fix"],
        "Run `scriptvox --help` to see the commands."
    );
    // A real command with a bad flag still points at its own usage.
    let (_, v) = run(&["wait", "--nosuch", "--json"]);
    assert_eq!(
        v["error"]["fix"],
        "Run `scriptvox wait --help` to see its usage."
    );
}

#[test]
fn envelope_shape_is_locked() {
    let (_, v) = run(&["status", "--json"]);
    insta::assert_json_snapshot!(v);
}

#[test]
fn no_tty_still_gives_envelope_without_flag() {
    // assert_cmd pipes stdout, so this is the no-TTY case.
    let (code, v) = run(&["status"]);
    assert_eq!(code, 0);
    assert_eq!(v["command"], "status");
}
