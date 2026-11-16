//! `scriptvox setup mcp`. HOME and every config path point at a temp folder: no test reads or writes a real client config.

mod common;
use serde_json::{json, Value};
use std::path::Path;

fn run(home: &Path, args: &[&str]) -> (Value, i32) {
    let o = common::std_bin()
        .args(["setup", "mcp"])
        .args(args)
        .arg("--json")
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("CODEX_HOME", home.join(".codex"))
        .env("SCRIPTVOX_CONFIG", home.join("rv"))
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap_or(json!({}));
    (v, o.status.code().unwrap_or(-1))
}

#[test]
fn a_dry_run_writes_nothing_and_a_real_run_adds_the_entry() {
    let h = tempfile::tempdir().unwrap();
    let p = h.path().join(".cursor/mcp.json");
    let (v, code) = run(h.path(), &["--client", "cursor", "--dry-run"]);
    assert_eq!((code, v["action"].as_str()), (0, Some("add")), "{v}");
    assert!(!p.exists() && !p.parent().unwrap().exists());

    let (v, code) = run(h.path(), &["--client", "cursor"]);
    assert_eq!(code, 0, "{v}");
    let got: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_eq!(got["mcpServers"]["scriptvox"]["args"][0], "mcp");
    assert!(v["backup"].is_null(), "no file before, no backup");

    let (v, _) = run(h.path(), &["--client", "cursor"]);
    assert_eq!(v["action"], "none");
}

#[test]
fn other_entries_stay_their_values_are_not_printed_and_a_backup_is_kept() {
    let h = tempfile::tempdir().unwrap();
    let p = h.path().join("c.json");
    let old = json!({"theme": "dark", "mcpServers": {"other": {"command": "x", "env": {"A_KEY": "made-up-value"}}}});
    std::fs::write(&p, old.to_string()).unwrap();
    let cfg = p.to_str().unwrap();

    let (v, _) = run(
        h.path(),
        &["--client", "claude-desktop", "--config", cfg, "--dry-run"],
    );
    assert_eq!(v["other_servers"], json!(["other"]));
    assert!(!v.to_string().contains("made-up-value"), "{v}");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), old.to_string());

    let (v, code) = run(h.path(), &["--client", "claude-desktop", "--config", cfg]);
    assert_eq!(code, 0, "{v}");
    assert!(!v.to_string().contains("made-up-value"), "{v}");
    let got: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_eq!(got["theme"], "dark");
    assert_eq!(got["mcpServers"]["other"], old["mcpServers"]["other"]);
    assert!(got["mcpServers"]["scriptvox"].is_object());
    let b = v["backup"].as_str().unwrap();
    assert_eq!(std::fs::read_to_string(b).unwrap(), old.to_string());
}

#[test]
fn codex_toml_keeps_its_comments_and_a_different_entry_is_refused() {
    let h = tempfile::tempdir().unwrap();
    let p = h.path().join(".codex/config.toml");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let old = "# my comment\nmodel = \"m\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
    std::fs::write(&p, old).unwrap();
    let (v, code) = run(h.path(), &["--client", "codex"]);
    assert_eq!(code, 0, "{v}");
    let new = std::fs::read_to_string(&p).unwrap();
    assert!(new.starts_with(old), "{new}");
    assert!(new.contains("[mcp_servers.scriptvox]"));
    let t: toml::Table = toml::from_str(&new).unwrap();
    assert_eq!(
        t["mcp_servers"]["scriptvox"]["args"][0].as_str(),
        Some("mcp")
    );
    let (v, _) = run(h.path(), &["--client", "codex"]);
    assert_eq!(v["action"], "none");

    let other = "[mcp_servers.scriptvox]\ncommand = \"/old/path\"\n";
    std::fs::write(&p, other).unwrap();
    let (v, code) = run(h.path(), &["--client", "codex"]);
    assert_ne!(code, 0, "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("other values"));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), other);
}

#[test]
fn a_broken_file_or_an_unknown_client_changes_nothing() {
    let h = tempfile::tempdir().unwrap();
    let p = h.path().join("c.json");
    std::fs::write(&p, "{ broken").unwrap();
    let (_, code) = run(
        h.path(),
        &["--client", "cursor", "--config", p.to_str().unwrap()],
    );
    assert_ne!(code, 0);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ broken");
    assert_eq!(std::fs::read_dir(h.path()).unwrap().count(), 1, "no backup");
    let (v, code) = run(h.path(), &["--client", "nope"]);
    assert_ne!(code, 0);
    assert!(v["error"]["message"].as_str().unwrap().contains("cursor"));
    let (_, code) = run(h.path(), &[]);
    assert_ne!(code, 0);
}
