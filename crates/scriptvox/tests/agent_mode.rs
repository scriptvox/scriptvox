mod common;
use assert_cmd::Command;
use serde_json::Value;

fn rvx() -> Command {
    let mut c = common::bin();
    for v in scriptvox::agent_mode::VARS
        .iter()
        .chain(scriptvox::redact::KEY_VARS.iter())
    {
        c.env_remove(v);
    }
    c
}

fn last_envelope(stdout: &[u8]) -> Value {
    let s = String::from_utf8_lossy(stdout);
    let line = s.lines().last().expect("stdout has a line");
    let v: Value = serde_json::from_str(line).expect("last stdout line is JSON");
    assert_eq!(v["schema"], "scriptvox.result/v1");
    v
}

#[test]
fn every_subcommand_is_agent_safe() {
    // A temp cwd: some commands (demo) write files where they run.
    let cwd = tempfile::tempdir().unwrap();
    for sub in scriptvox::cli::tree().get_subcommands() {
        let name = sub.get_name().to_string();
        if name == "mcp" {
            // A protocol server: stdout carries JSON-RPC only. With no input it must end at once, quietly.
            let out = rvx().current_dir(cwd.path()).arg("mcp").output().unwrap();
            assert!(
                out.status.success() && out.stdout.is_empty(),
                "mcp with empty stdin"
            );
            continue;
        }
        let out = rvx()
            .env("CLAUDECODE", "1")
            .current_dir(cwd.path())
            .arg(&name)
            .output()
            .unwrap();
        for stream in [&out.stdout, &out.stderr] {
            assert!(!stream.contains(&0x1b), "{name}: ANSI escape in output");
        }
        last_envelope(&out.stdout);
    }
}

#[test]
fn claudecode_switches_agent_mode_on() {
    let out = rvx().env("CLAUDECODE", "1").arg("status").output().unwrap();
    assert_eq!(last_envelope(&out.stdout)["agent_mode"], true);
}

#[test]
fn no_tty_alone_is_not_agent_mode() {
    let out = rvx().arg("status").output().unwrap();
    assert_eq!(last_envelope(&out.stdout)["agent_mode"], false);
}

#[test]
fn ci_run_stays_quiet_on_stderr() {
    let out = rvx().env("CI", "1").arg("status").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).lines().count() <= 15);
}

#[test]
fn key_values_are_redacted() {
    let key = "pexels-0123456789abcdef";
    let out = rvx()
        .env("PEXELS_API_KEY", key)
        .args([key, "--json"])
        .output()
        .unwrap();
    let all = [out.stdout, out.stderr].concat();
    assert!(!String::from_utf8_lossy(&all).contains(key), "key leaked");
    assert!(String::from_utf8_lossy(&all).contains("***"));
}

#[test]
fn help_shows_money_flags_only_where_they_count() {
    let help = |cmd: &str| {
        let out = rvx().args([cmd, "--help"]).output().unwrap();
        String::from_utf8(out.stdout).unwrap()
    };
    assert!(!help("check").contains("--approve"));
    assert!(help("make").contains("--approve"));
    assert!(help("resume").contains("--max-spend"));
    // Still accepted, so a command written from the top-level help never fails on them.
    let out = rvx()
        .args(["check", "--approve", "x", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn a_wrong_flag_is_named_with_the_right_one() {
    let o = common::bin()
        .args(["snapshot", "x", "--sheets", "9", "--json"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let m = v["error"]["message"].as_str().unwrap();
    assert!(m.contains("'--sheets'") && m.contains("'--sheet'"), "{m}");
}
