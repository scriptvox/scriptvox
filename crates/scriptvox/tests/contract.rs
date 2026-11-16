//! RVX-RPC contract (PLAN-R1 §2.4). The same fixtures replay here over a real
//! pipe and in python/tests/test_contract.py in-process.
mod common;
use scriptvox::provision::{worker_command, WORKER_FILES};
use scriptvox::rpc::{Worker, PROTO_MISMATCH, WORKER_CRASHED, WORKER_TIMEOUT};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn have_uv() -> bool {
    Command::new("uv")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn python_worker() -> Worker {
    Worker::spawn(worker_command(&root().join("python")).unwrap()).unwrap()
}

#[test]
fn fixtures_replay_over_a_real_pipe() {
    if !have_uv() {
        return;
    }
    let mut files: Vec<_> = std::fs::read_dir(root().join("fixtures/contract"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(files.len() >= 3);
    for f in files {
        let mut w = python_worker();
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            let step: Value = serde_json::from_str(line).unwrap();
            if let Some(msg) = step.get("core") {
                w.send(msg).unwrap();
            } else {
                assert_eq!(w.recv().unwrap(), step["worker"], "{}", f.display());
            }
        }
    }
}

fn fake(script: &str) -> Worker {
    let mut c = Command::new("python3");
    c.args(["-c", script]);
    Worker::spawn(c).unwrap()
}

#[test]
fn non_json_line_kills_the_worker() {
    let mut w = fake("import sys; sys.stdin.readline(); print('debug: oops', flush=True); import time; time.sleep(30)");
    let e = w.hello(false).unwrap_err();
    assert_eq!(e.code, WORKER_CRASHED);
    assert!(e.message.contains("non-JSON"), "{}", e.message);
}

#[test]
fn exited_worker_is_crashed() {
    let mut w = fake("pass");
    assert_eq!(w.hello(false).unwrap_err().code, WORKER_CRASHED);
}

#[test]
fn silent_worker_times_out() {
    let mut w = fake("import time; time.sleep(30)");
    w.timeout = Duration::from_millis(300);
    assert_eq!(w.hello(false).unwrap_err().code, WORKER_TIMEOUT);
}

#[test]
fn newer_major_in_reply_is_a_mismatch() {
    let mut w = fake(
        r#"import sys, json; m = json.loads(sys.stdin.readline()); print(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":{"proto":"2.0"}}), flush=True)"#,
    );
    assert_eq!(w.hello(false).unwrap_err().code, PROTO_MISMATCH);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap();
        if [".venv", "__pycache__", ".pytest_cache"]
            .iter()
            .any(|s| name == *s)
        {
            continue;
        }
        if p.is_dir() {
            copy_dir(&p, &to.join(name))
        } else {
            std::fs::copy(&p, to.join(name)).map(|_| ()).unwrap()
        }
    }
}

#[test]
fn proto_mismatch_is_exit_1_with_its_own_code() {
    if !have_uv() {
        return;
    }
    // An editable worker (SCRIPTVOX_WORKER_SRC) that speaks proto 2.0.
    let d = tempfile::tempdir().unwrap();
    copy_dir(&root().join("python"), d.path());
    let init = d.path().join("scriptvox_worker/__init__.py");
    let src = std::fs::read_to_string(&init)
        .unwrap()
        .replace("PROTO = \"1.0\"", "PROTO = \"2.0\"");
    std::fs::write(&init, src).unwrap();
    let out = common::bin()
        .env("SCRIPTVOX_WORKER_SRC", d.path())
        .args(["worker", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert_eq!(v["error"]["code"], "SVX_PROTO_MISMATCH");
}

#[test]
fn every_worker_file_is_embedded() {
    let dir = root().join("python/scriptvox_worker");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "py") {
            let rel = format!(
                "scriptvox_worker/{}",
                p.file_name().unwrap().to_string_lossy()
            );
            assert!(
                WORKER_FILES.iter().any(|(f, _)| *f == rel),
                "{rel} is not in provision::WORKER_FILES"
            );
        }
    }
}
