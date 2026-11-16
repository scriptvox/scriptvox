//! Detach, wait, cancel (RVX-016) with the stub worker. Linux only here;
//! Windows runs in the foreground in v0.1 (flip rule d).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;
use std::time::{Duration, Instant};

fn json(out: &std::process::Output) -> Value {
    let s = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(s.lines().last().unwrap_or("null")).unwrap_or(Value::Null)
}

fn detach(e: &Env, mode: &str) -> String {
    std::fs::write(
        e.dir.path().join("s.txt"),
        "Detached run here. It keeps going.",
    )
    .unwrap();
    let t0 = Instant::now();
    let out = e
        .cmd(mode)
        .args(["make", "--script", "s.txt", "--detach", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(v["status"], "running");
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "detach returns at once"
    );
    v["run_id"].as_str().unwrap().to_string()
}

#[test]
fn detach_then_wait_gets_the_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let id = detach(&e, "ok");
    let out = e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap();
    assert!(
        video.contains("scriptvox-out") && std::path::Path::new(video).is_file(),
        "{video}"
    );
}

/// `wait` shows the `claims` finding of a run with `sources`, as `plan`, `make` and `check <run>` do.
#[test]
fn wait_shows_the_claims_finding_of_the_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"script": "Blue light is near 450 nm. It was named in 1871.",
            "sources": [{"text": "Blue light has a wavelength of about 450 nm.", "title": "A book"}]}"#,
    )
    .unwrap();
    let out = e
        .cmd("ok")
        .args(["make", "--brief", "b.json", "--detach", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    let claim = |v: &serde_json::Value| {
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["gate"] == "claims")
            .cloned()
    };
    let out = e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    let f = claim(&v).unwrap_or_else(|| panic!("no claims finding: {v}"));
    assert_eq!(f["status"], "warn", "{f}");
    assert!(f["detail"].as_str().unwrap().contains("1871"), "{f}");
    // The same finding as `check <run>`.
    let out = e.cmd("ok").args(["check", &id, "--json"]).output().unwrap();
    assert_eq!(claim(&json(&out)), Some(f));
}

/// money-2: the same command with the same key is the same run: a retry gets its id, finished or not.
#[test]
fn make_detach_with_one_key_is_one_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "One key. One run.").unwrap();
    let start = || {
        let out = e
            .cmd("ok")
            .args([
                "make",
                "--script",
                "s.txt",
                "--detach",
                "--idempotency-key",
                "K2",
                "--json",
            ])
            .output()
            .unwrap();
        let v = json(&out);
        assert_eq!(out.status.code(), Some(0), "{v}");
        v["run_id"].as_str().unwrap().to_string()
    };
    let (a, b) = (start(), start());
    assert_eq!(a, b, "a retry is the same run");
    let out = e.cmd("ok").args(["wait", &a, "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", json(&out));
    assert_eq!(start(), a, "a finished run answers for itself");
}

#[test]
fn killing_the_callers_process_group_does_not_stop_the_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "Survive the kill. Then finish.").unwrap();
    let bin = common::bin_path();
    // The caller: a shell in its own process group that detaches a run, then sleeps.
    let mut sh = std::process::Command::new("sh");
    common::clean(&mut sh);
    sh.arg("-c")
        .arg(format!(
            "'{}' make --script s.txt --detach --json > d.json; sleep 30",
            bin.display()
        ))
        .current_dir(e.dir.path())
        .env("SCRIPTVOX_WORKER_SRC", e.dir.path().join("worker"))
        .env("SCRIPTVOX_DATA", e.dir.path().join("data"))
        // As `Env::cmd`: a user `stt` row would hear the stub tone and fail gate `words`.
        .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
        .env("STUB_AUDIO", e.dir.path().join("voice.mp3"))
        .env("STUB_MODE", "ok");
    {
        // The shell is not built by Env::cmd: give it the shared uv cache too.
        let mut probe = common::bin();
        shared_uv(&mut probe);
        for (k, v) in probe.get_envs() {
            if let Some(v) = v {
                sh.env(k, v);
            }
        }
    }
    use std::os::unix::process::CommandExt;
    sh.process_group(0);
    let mut child = sh.spawn().unwrap();
    let d = e.dir.path().join("d.json");
    let t0 = Instant::now();
    while std::fs::read_to_string(&d).map_or(true, |s| s.is_empty())
        && t0.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    // SIGKILL the whole caller group, as an agent harness does on timeout.
    // The shell's own kill, with `-s` and `--`, as `supervise::kill_line` does. `/usr/bin/kill -KILL -<pid>` with no
    // `--` is read in other ways by some kill programs (procps on Debian), and can signal far more than this group:
    // on 10 Nov 2026 a CI pod lost every process but PID 1 during this file.
    std::process::Command::new("sh")
        .args(["-c", &format!("kill -s KILL -- -{}", child.id())])
        .status()
        .unwrap();
    let _ = child.wait();
    let id = serde_json::from_str::<Value>(&std::fs::read_to_string(&d).unwrap()).unwrap()
        ["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", json(&out));
}

#[test]
fn wait_timeout_is_exit_9_and_cancel_is_exit_8() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let id = detach(&e, "slow");
    let out = e
        .cmd("slow")
        .args(["wait", &id, "--timeout", "1", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(9), "{v}");
    assert_eq!(v["next"][0], format!("scriptvox wait {id}"));
    // The fix has the exact next command and the stage; the run is alive, and `status` says so.
    let fix = v["error"]["fix"].as_str().unwrap();
    assert!(
        fix.contains(&format!("`scriptvox wait {id} --timeout 1`")) && fix.contains("Stage: "),
        "{v}"
    );
    assert_eq!(v["error"]["retryable"], true, "{v}");
    let s = json(
        &e.cmd("slow")
            .args(["status", &id, "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(s["alive"], true, "{s}");
    assert!(s["stage"].is_string() && s["idle_s"].is_u64(), "{s}");
    assert_eq!(s["next"][0], format!("scriptvox wait {id} --json"), "{s}");
    let out = e
        .cmd("slow")
        .args(["cancel", &id, "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(8), "{}", json(&out));
    // The result stays cancelled; the supervisor is gone.
    let out = e
        .cmd("slow")
        .args(["wait", &id, "--timeout", "2", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(8), "{}", json(&out));
}

#[test]
fn foreground_run_keeps_stderr_short() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "Short and quiet. For an agent.").unwrap();
    let out = e
        .cmd("ok")
        .env("CLAUDECODE", "1")
        .args(["make", "--script", "s.txt"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).lines().count() <= 15);
}

/// A run row in the state of a process that is at `pid`, with no result yet.
fn running_row(e: &Env, pid: u32) -> String {
    let (code, v) = e.make("A run for the state tests.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap().to_string();
    let p = e.dir.path();
    std::fs::remove_file(p.join(".scriptvox/runs").join(&run).join("result.json")).unwrap();
    rusqlite::Connection::open(p.join(".scriptvox/state.db"))
        .unwrap()
        .execute(
            "UPDATE runs SET status = 'running', finished = NULL, exit = NULL, pid = ?1 WHERE id = ?2",
            rusqlite::params![pid, run],
        )
        .unwrap();
    run
}

/// make-6: a dead supervisor no longer reports running, in the list and for one run.
#[test]
fn status_says_died_when_the_process_is_gone() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    let run = running_row(&e, pid);
    let v = json(
        &e.cmd("ok")
            .args(["status", &run, "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(v["run"]["status"], "died", "{v}");
    let v = json(&e.cmd("ok").args(["status", "--json"]).output().unwrap());
    assert_eq!(v["runs"][0]["status"], "died", "{v}");
    // A live process is still running.
    let mut live = std::process::Command::new("sleep")
        .arg("20")
        .spawn()
        .unwrap();
    let run = running_row(&e, live.id());
    let v = json(
        &e.cmd("ok")
            .args(["status", &run, "--json"])
            .output()
            .unwrap(),
    );
    let _ = live.kill();
    let _ = live.wait();
    assert_eq!(v["run"]["status"], "running", "{v}");
}

/// make-12: cancel stops a renamed supervisor by its run id, and says so when it stops nothing.
#[test]
fn cancel_finds_a_renamed_supervisor_and_reports_a_process_it_cannot_stop() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let result = |run: &str| {
        e.dir
            .path()
            .join(".scriptvox/runs")
            .join(run)
            .join("result.json")
    };

    // Not this run: nothing is signalled, and the answer is not "cancelled".
    let mut other = std::process::Command::new("sleep")
        .arg("20")
        .spawn()
        .unwrap();
    let run = running_row(&e, other.id());
    let out = e
        .cmd("ok")
        .args(["cancel", &run, "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    let still = other.try_wait().unwrap().is_none();
    let _ = other.kill();
    let _ = other.wait();
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert!(still, "the other process was left alone");
    assert!(!result(&run).exists(), "{v}");

    // Another file name, started with its run id, as a supervisor is. The id is only known after the row exists.
    let mut named = std::process::Command::new("sleep")
        .arg("20")
        .spawn()
        .unwrap();
    let run = running_row(&e, named.id());
    let _ = named.kill();
    let _ = named.wait();
    let mut renamed = std::process::Command::new("bash")
        .args(["-c", "while :; do sleep 0.2; done", "--run-id", &run])
        .spawn()
        .unwrap();
    rusqlite::Connection::open(e.dir.path().join(".scriptvox/state.db"))
        .unwrap()
        .execute(
            "UPDATE runs SET pid = ?1 WHERE id = ?2",
            rusqlite::params![renamed.id(), run],
        )
        .unwrap();
    let out = e
        .cmd("ok")
        .args(["cancel", &run, "--json"])
        .output()
        .unwrap();
    let t0 = Instant::now();
    let stopped = loop {
        match renamed.try_wait().unwrap() {
            Some(_) => break true,
            None if t0.elapsed() > Duration::from_secs(8) => break false,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let _ = renamed.kill();
    let _ = renamed.wait();
    assert_eq!(out.status.code(), Some(8), "{}", json(&out));
    assert!(stopped, "the renamed supervisor was stopped");
}

#[test]
fn a_run_id_that_is_not_one_is_refused_and_makes_no_folder() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(e.dir.path().join("s.txt"), "A short line.").unwrap();
    for bad in ["../../escape", "r_ok/../../escape", "not-an-id"] {
        let out = e
            .cmd("ok")
            .args(["make", "--script", "s.txt", "--run-id", bad, "--json"])
            .output()
            .unwrap();
        assert_ne!(out.status.code(), Some(0), "{bad}: {}", json(&out));
        assert!(
            json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not a run id"),
            "{bad}: {}",
            json(&out)
        );
    }
    assert!(!e.dir.path().join("escape").exists());
    assert!(!e.dir.path().parent().unwrap().join("escape").exists());
    assert!(!e.dir.path().join(".scriptvox/runs/not-an-id").exists());
}

/// The caller's shell limit: a foreground `make` whose whole process group gets SIGKILL, as an agent harness does.
/// The run stops with it; `status` and `wait` say so and name `resume`, and the child writes the `--out` name.
#[test]
fn a_foreground_make_killed_by_its_shell_is_resumed_under_its_out_name() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(
        e.dir.path().join("s.txt"),
        "Killed by the shell. Then resumed.",
    )
    .unwrap();
    let mut c = common::std_bin();
    c.args(["make", "--script", "s.txt", "--out", "named.mp4", "--json"])
        .current_dir(e.dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(e.dir.path().join("err.txt")).unwrap());
    for (k, v) in e.cmd("slow").get_envs() {
        if let Some(v) = v {
            c.env(k, v);
        }
    }
    use std::os::unix::process::CommandExt;
    c.process_group(0);
    let mut child = c.spawn().unwrap();
    // The run id is on stderr in the first seconds, with the command for a stopped shell.
    let t0 = Instant::now();
    let id = loop {
        let err = std::fs::read_to_string(e.dir.path().join("err.txt")).unwrap_or_default();
        let id = err
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .find(|w| w.starts_with("r_"))
            .map(String::from);
        if let Some(id) = id.filter(|id| err.contains(&format!("scriptvox resume {id}"))) {
            break id;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(6),
            "no early line: {err}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    // Only the group this test started.
    std::process::Command::new("sh")
        .args(["-c", &format!("kill -s KILL -- -{}", child.id())])
        .status()
        .unwrap();
    let _ = child.wait();
    let v = json(
        &e.cmd("ok")
            .args(["status", &id, "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(v["run"]["status"], "died", "{v}");
    assert_eq!(v["alive"], false, "{v}");
    assert_eq!(v["next"][0], format!("scriptvox resume {id} --json"), "{v}");
    let out = e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains(&format!("scriptvox resume {id}")),
        "{v}"
    );
    // The parent never wrote named.mp4, so the child does: no named.1.mp4.
    let out = e
        .cmd("ok")
        .args(["resume", &id, "--wait", "--json"])
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert!(
        v["artifacts"]["video"]
            .as_str()
            .unwrap()
            .ends_with("named.mp4"),
        "{v}"
    );
    assert!(e.dir.path().join("named.mp4").is_file());
}

/// A resume of a run that has its video writes next to it, and the answer says where.
#[test]
fn resume_says_where_the_child_video_goes() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("A video with a name.", "ok", &["--out", "named.mp4"]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap();
    let r = json(
        &e.cmd("ok")
            .args(["resume", run, "--json"])
            .output()
            .unwrap(),
    );
    assert!(r["out"].as_str().unwrap().ends_with("named.1.mp4"), "{r}");
    let child = r["run_id"].as_str().unwrap();
    let out = e
        .cmd("ok")
        .args(["wait", child, "--json"])
        .output()
        .unwrap();
    let w = json(&out);
    assert_eq!(out.status.code(), Some(0), "{w}");
    assert!(
        w["artifacts"]["video"]
            .as_str()
            .unwrap()
            .ends_with("named.1.mp4"),
        "{w}"
    );
    assert!(e.dir.path().join("named.mp4").is_file());
}
