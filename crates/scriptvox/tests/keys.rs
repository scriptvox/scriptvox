//! Keys (RVX-020): env first, keys.toml user-only, values never shown.
#![cfg(unix)]
mod common;
use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

const KEY: &str = "pexels-secret-0123456789";

fn rvx(cfg: &Path) -> Command {
    let mut c = common::bin();
    c.env("SCRIPTVOX_CONFIG", cfg)
        .env_remove("PEXELS_API_KEY")
        .env_remove("CLAUDECODE")
        .env_remove("CI");
    c
}

fn json(o: &std::process::Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn stdin_set_writes_a_user_only_file_and_never_echoes() {
    let d = tempfile::tempdir().unwrap();
    let o = rvx(d.path())
        .args(["keys", "set", "pexels", "--stdin", "--json"])
        .write_stdin(format!("{KEY}\n"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", json(&o));
    let all = [o.stdout.clone(), o.stderr.clone()].concat();
    assert!(
        !String::from_utf8_lossy(&all).contains(KEY),
        "the value is never printed"
    );
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(d.path().join("keys.toml"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn env_wins_over_keys_toml_and_doctor_names_only_the_source() {
    let d = tempfile::tempdir().unwrap();
    rvx(d.path())
        .args(["keys", "set", "pexels", "--stdin"])
        .write_stdin(KEY)
        .output()
        .unwrap();
    let list = |env: Option<&str>| {
        let mut c = rvx(d.path());
        if let Some(v) = env {
            c.env("PEXELS_API_KEY", v);
        }
        let o = c.args(["keys", "list", "--json"]).output().unwrap();
        json(&o)["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["provider"] == "pexels")
            .unwrap()["source"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(list(None), "keys.toml");
    assert_eq!(list(Some("from-env-9876543210")), "env");
    let o = rvx(d.path())
        .args(["doctor", "--offline", "--json"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("\"source\":\"keys.toml\""), "{text}");
    assert!(!text.contains(KEY));
}

#[test]
fn agent_without_stdin_gets_needs_input_secret() {
    let d = tempfile::tempdir().unwrap();
    let o = rvx(d.path())
        .env("CLAUDECODE", "1")
        .args(["keys", "set", "pexels"])
        .output()
        .unwrap();
    let v = json(&o);
    assert_eq!(o.status.code(), Some(10), "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "secret");
    assert!(!d.path().join("keys.toml").exists());
}

#[test]
fn keys_toml_values_are_redacted_everywhere() {
    let d = tempfile::tempdir().unwrap();
    rvx(d.path())
        .args(["keys", "set", "pexels", "--stdin"])
        .write_stdin(KEY)
        .output()
        .unwrap();
    // The value used as an unknown command still never reaches the output.
    let o = rvx(d.path()).args([KEY, "--json"]).output().unwrap();
    let all = String::from_utf8_lossy(&[o.stdout, o.stderr].concat()).into_owned();
    assert!(!all.contains(KEY) && all.contains("***"), "{all}");
}

#[test]
fn a_keys_file_that_does_not_parse_is_kept_and_named_without_its_values() {
    let d = tempfile::tempdir().unwrap();
    let bad = "fal = \"fake-value-kept-private\"\nnot toml \"fake-value-kept-private\n";
    let path = d.path().join("keys.toml");
    std::fs::write(&path, bad).unwrap();
    let o = rvx(d.path())
        .args(["keys", "set", "pexels", "--stdin", "--json"])
        .write_stdin(KEY)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", json(&o));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        bad,
        "nothing written"
    );
    let said = String::from_utf8_lossy(&[o.stdout, o.stderr].concat()).into_owned();
    assert!(said.contains("line 2"), "{said}");
    assert!(
        !said.contains("fake-value"),
        "the line is not quoted: {said}"
    );
    for cmd in [
        vec!["keys", "list", "--json"],
        vec!["doctor", "--offline", "--json"],
    ] {
        let o = rvx(d.path()).args(&cmd).output().unwrap();
        let said = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(
            said.contains("keys_file") && !said.contains("fake-value"),
            "{cmd:?}: {said}"
        );
    }
}
