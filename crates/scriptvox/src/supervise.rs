//! Detach, wait and cancel (PLAN-R1 §2.3 "Detach", spike answer 6).
//!
//! `make --detach` re-runs this binary as the supervisor with a hidden
//! `--run-id`, in a new process group with stdio sent to files, so a kill of
//! the caller's process group never stops the run. `wait` polls the run's
//! `result.json`; `cancel` stops the supervisor's whole group (FFmpeg too).
//! Windows: flip rule (d) applies, the run stays in the foreground.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::run::{now_utc, open_state, Run};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Starts the supervisor for `run` with `args` (after the binary name).
pub fn spawn(run: &Run, args: &[String]) -> std::io::Result<u32> {
    let exe = std::env::current_exe()?;
    let log = std::fs::File::create(run.dir.join("supervisor.log"))?;
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    if let Ok((c, _)) = open_state(Path::new(".")) {
        let _ = c.execute(
            "UPDATE runs SET pid = ?2 WHERE id = ?1",
            rusqlite::params![run.id, pid],
        );
    }
    Ok(pid)
}

/// The image name in `tasklist /FO CSV` output: `"scriptvox.exe","1234",...`. No match prints an INFO line with no quotes.
#[cfg(any(windows, test))]
fn tasklist_image(csv: &str, pid: u32) -> Option<String> {
    csv.lines().find_map(|l| {
        let mut f = l.trim().strip_prefix('"')?.split("\",\"");
        let (name, p) = (f.next()?, f.next()?);
        (p.trim_end_matches('"') == pid.to_string()).then(|| name.to_string())
    })
}

/// The image name of a live process. When `tasklist` cannot run the name is empty: the pid counts as alive, so `cancel`
/// never reports a stop it did not make.
#[cfg(windows)]
fn windows_image(pid: u32) -> Option<String> {
    match Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stderr(Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => tasklist_image(&String::from_utf8_lossy(&o.stdout), pid),
        _ => Some(String::new()),
    }
}

#[cfg(windows)]
pub fn alive(pid: u32) -> bool {
    windows_image(pid).is_some()
}

/// True while the process lives and is not a zombie (Linux /proc, else `ps`).
#[cfg(not(windows))]
pub fn alive(pid: u32) -> bool {
    if Path::new("/proc").is_dir() {
        return Path::new(&format!("/proc/{pid}")).exists()
            && !std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .is_ok_and(|s| s.split(' ').nth(2) == Some("Z"));
    }
    alive_by_ps(pid)
}

/// `kill -0` succeeds on a zombie, so a killed child that is not yet reaped looked alive and `cancel`
/// said "did not stop" (the detach test on the macOS runner, 16 Nov 2026). `ps` shows the state.
#[cfg(not(windows))]
fn alive_by_ps(pid: u32) -> bool {
    ps(pid, "stat").is_some_and(|s| !s.starts_with('Z'))
}

fn row(project: &Path, id: &str) -> Option<(String, Option<u32>)> {
    let (c, _) = open_state(project).ok()?;
    c.query_row("SELECT status, pid FROM runs WHERE id = ?1", [id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<u32>>(1)?))
    })
    .ok()
}

/// A supervisor is working on this run now, and it has no result yet.
pub fn running(project: &Path, id: &str) -> bool {
    read_result(project, id).is_none()
        && matches!(row(project, id), Some((_, Some(pid))) if pid != std::process::id() && alive(pid))
}

/// The row says running, but its process is gone and left no result. `wait` reports the same state.
pub fn died(project: &Path, id: &str, pid: Option<u32>) -> bool {
    pid.is_some_and(|p| !alive(p)) && read_result(project, id).is_none()
}

/// A finished run answers a rerun only while its video is still on disk, not empty and the size the run left it.
pub fn replayable(project: &Path, prev: &Value) -> bool {
    prev["ok"] == true
        && prev["artifacts"]["video"].as_str().is_none_or(|v| {
            std::fs::metadata(project.join(v)).is_ok_and(|m| {
                m.is_file()
                    && m.len() > 0
                    && prev["metrics"]["video_bytes"]
                        .as_u64()
                        .is_none_or(|b| b == m.len())
            })
        })
}

fn run_dir(project: &Path, id: &str) -> std::path::PathBuf {
    project.join(".scriptvox/runs").join(id)
}

fn read_result(project: &Path, id: &str) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(run_dir(project, id).join("result.json")).ok()?)
        .ok()
}

/// `resume` can go on from this run: it has a saved make request (a `generate` run has none).
pub fn resumable(project: &Path, id: &str) -> bool {
    run_dir(project, id).join("request.json").is_file()
}

/// Where a run is: the text of its last `progress` event, and the seconds since its last event of any kind (the age
/// of `events.jsonl`).
pub fn progress(project: &Path, id: &str) -> (Option<String>, Option<u64>) {
    let file = run_dir(project, id).join("events.jsonl");
    let stage = std::fs::read_to_string(&file).ok().and_then(|s| {
        s.lines()
            .rev()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|e| e["type"] == "progress")
            .and_then(|e| e["explain"].as_str().map(String::from))
    });
    let idle = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs());
    (stage, idle)
}

/// Re-emits a stored result envelope as this command's answer.
pub fn replay(command: &str, v: Value) -> Envelope {
    // Prefer the stored numeric exit; fall back to the error code name.
    let by_code = v["exit"]
        .as_u64()
        .and_then(|c| Exit::ALL.into_iter().find(|e| u64::from(e.code()) == c));
    let by_name = Exit::ALL.into_iter().find(|e| {
        Some(e.name())
            == v["error"]["code"]
                .as_str()
                .and_then(|c| c.strip_prefix("SVX_"))
    });
    let exit = by_code.or(by_name).unwrap_or(if v["ok"] == true {
        Exit::Ok
    } else {
        Exit::Error
    });
    let mut env = if v["ok"] == true {
        Envelope::ok(command)
    } else {
        let e = &v["error"];
        Envelope::fail_code(
            command,
            exit,
            e["code"].as_str().unwrap_or("SVX_ERROR"),
            e["message"].as_str().unwrap_or(""),
            e["fix"].as_str().map(String::from),
        )
    };
    // The run's own word for how it stopped.
    if let Some(s) = ["cancelled", "approval_needed"]
        .into_iter()
        .find(|s| v["status"] == *s)
    {
        env.status = s;
    }
    if v["status"] == "needs_input" {
        env.status = "needs_input";
        env.exit = Exit::NeedsInput;
        env.needs_input = v["needs_input"].as_array().cloned().unwrap_or_default();
    }
    env.run_id = v["run_id"].as_str().map(String::from);
    env.events_file = v["events_file"].as_str().map(String::from);
    for (k, dst) in [
        ("artifacts", &mut env.artifacts),
        ("metrics", &mut env.metrics),
    ] {
        if let Some(m) = v[k].as_object() {
            dst.extend(m.clone());
        }
    }
    // Every command-specific field (explain, lang, parent_run, idempotent, ...) goes along.
    const CORE: [&str; 14] = [
        "schema",
        "ok",
        "status",
        "command",
        "run_id",
        "plan_id",
        "needs_input",
        "findings",
        "next",
        "error",
        "artifacts",
        "metrics",
        "events_file",
        "exit",
    ];
    if let Some(o) = v.as_object() {
        for (k, val) in o.iter().filter(|(k, _)| !CORE.contains(&k.as_str())) {
            env = env.with(k, val.clone());
        }
    }
    env.findings = v["findings"].as_array().cloned().unwrap_or_default();
    env.next = v["next"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    env.plan_id = v["plan_id"].as_str().map(String::from);
    env
}

pub fn wait(project: &Path, id: &str, timeout: Duration) -> Envelope {
    let t0 = Instant::now();
    loop {
        if let Some(v) = read_result(project, id) {
            return replay("wait", v);
        }
        match row(project, id) {
            None => {
                return Envelope::fail(
                    "wait",
                    Exit::Usage,
                    format!("no run {id}"),
                    Some("Run `scriptvox status` to see the runs.".into()),
                )
            }
            Some((_, Some(pid))) if !alive(pid) => {
                // The supervisor can write its result and exit between the read above and this check
                // (the macOS runner, 16 Nov 2026): read once more before calling it dead.
                if let Some(v) = read_result(project, id) {
                    return replay("wait", v);
                }
                // The supervisor died without a result (killed, crashed or out of memory).
                return Envelope::fail_code(
                    "wait",
                    Exit::Error,
                    "SVX_SUPERVISOR_DIED",
                    format!("the supervisor of {id} (pid {pid}) stopped without a result"),
                    // A foreground run that its shell stopped has no supervisor.log: only `spawn` writes one.
                    Some(if resumable(project, id) {
                        format!(
                            "Run `scriptvox resume {id}`: it makes the video in a child run, and what is finished is not made again. A detached run left a log: {}/supervisor.log.",
                            run_dir(project, id).display()
                        )
                    } else {
                        format!(
                            "Read {}/supervisor.log, then run the command again.",
                            run_dir(project, id).display()
                        )
                    }),
                );
            }
            _ => {}
        }
        if t0.elapsed() >= timeout {
            // The command first: the stage is a full progress line and can be long.
            let stage = progress(project, id).0.map_or(String::new(), |s| {
                format!(" Stage: {}", s.trim_start_matches(&format!("{id}: ")))
            });
            let mut env = Envelope::fail(
                "wait",
                Exit::StillRunning,
                format!("{id} is still running"),
                Some(format!(
                    "The run is alive; this is not a failure. Run `scriptvox wait {id} --timeout {}` again, with a larger number when your shell allows a longer command.{stage}",
                    timeout.as_secs().max(1)
                )),
            );
            if let Some(e) = env.error.as_mut() {
                e.retryable = true;
            }
            env.status = "running";
            env.run_id = Some(id.to_string());
            env.next = vec![format!("scriptvox wait {id}")];
            return env;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn cancel(project: &Path, id: &str) -> Envelope {
    if read_result(project, id).is_some() {
        return Envelope::fail(
            "cancel",
            Exit::Usage,
            format!("{id} has already finished"),
            Some(format!("Run `scriptvox wait {id}` to read its result.")),
        );
    }
    let Some((_, pid)) = row(project, id) else {
        return Envelope::fail(
            "cancel",
            Exit::Usage,
            format!("no run {id}"),
            Some("Run `scriptvox status` to see the runs.".into()),
        );
    };
    if let Some(pid) = pid.filter(|p| alive(*p) && *p != std::process::id()) {
        if !is_scriptvox(pid, id) {
            return Envelope::fail(
                "cancel",
                Exit::Error,
                format!(
                    "process {pid} of {id} is alive but is not this run, so nothing was stopped"
                ),
                Some(format!(
                    "If process {pid} is this run, stop it, then run `scriptvox cancel {id}` again."
                )),
            );
        }
        // Signal the whole group only when this scriptvox process leads it (a detached
        // supervisor); a foreground run gets the signal itself. Never a reused pid.
        let target = if group_leader(pid) {
            format!("-{pid}")
        } else {
            pid.to_string()
        };
        signal(&target, "TERM", pid);
        let t0 = Instant::now();
        while alive(pid) && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if alive(pid) {
            signal(&target, "KILL", pid);
            let t0 = Instant::now();
            while alive(pid) && t0.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(100));
            }
            if alive(pid) {
                return Envelope::fail(
                    "cancel",
                    Exit::Error,
                    format!("process {pid} of {id} did not stop, so {id} is still running"),
                    Some(format!(
                        "Stop process {pid} by hand, then run `scriptvox cancel {id}` again."
                    )),
                );
            }
        }
    }
    let mut env = Envelope::fail(
        "cancel",
        Exit::Cancelled,
        format!("{id} was cancelled"),
        None,
    );
    env.status = "cancelled";
    env.run_id = Some(id.to_string());
    let dir = run_dir(project, id);
    env.next = vec![format!(
        "scriptvox resume {id} --json continues it; what is finished is not made again."
    )];
    let (stopped, running) = crate::genjob::cancel_jobs(project, &dir);
    if stopped + running > 0 {
        env.metrics
            .insert("provider_jobs_stopped".into(), json!(stopped));
        env.metrics
            .insert("provider_jobs_running".into(), json!(running));
    }
    if running > 0 {
        env.findings.push(json!({"gate": "cancel", "status": "warn",
            "detail": format!("{running} generator job(s) could not be stopped and keep running on the provider; a paid job is charged"),
            "fix": "A resume fetches the result instead of paying twice."}));
    }
    let _ = std::fs::write(dir.join("result.json"), env.to_json());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("events.jsonl"))
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "{}",
            json!({"v": 1, "t": now_utc(), "run": id, "type": "run.end", "status": "cancelled", "exit": 8})
        );
    }
    if let Ok((c, _)) = open_state(project) {
        let _ = c.execute(
            "UPDATE runs SET status = 'cancelled', finished = ?2, exit = 8 WHERE id = ?1",
            rusqlite::params![id, now_utc()],
        );
    }
    env
}

/// True when the process is this run's scriptvox under any file name (guards against a reused pid):
/// the name, the run id a supervisor is started with, or the same binary as this one.
fn is_scriptvox(pid: u32, id: &str) -> bool {
    let Ok(c) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        // A reused pid must not get `taskkill /F`: the image is this binary's, whatever its name.
        #[cfg(windows)]
        return windows_image(pid).is_some_and(|image| {
            image.eq_ignore_ascii_case("scriptvox.exe")
                || std::env::current_exe().is_ok_and(|me| {
                    me.file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case(&image))
                })
        });
        // No /proc (macOS): `ps` gives the command line. No answer stops nothing: the run row alone let `cancel`
        // stop an other process under a reused pid (the detach test on the macOS runner, 16 Nov 2026).
        #[cfg(not(windows))]
        return ps(pid, "command").is_some_and(|c| {
            let mut args = c.split_whitespace();
            args.next().is_some_and(|exe| exe.ends_with("scriptvox")) || args.any(|a| a == id)
        });
    };
    let c = String::from_utf8_lossy(&c);
    let mut args = c.split('\0');
    args.next()
        .is_some_and(|exe| exe.ends_with("scriptvox") || exe.ends_with("scriptvox.exe"))
        || args.any(|a| a == id)
        || std::fs::read_link(format!("/proc/{pid}/exe")).ok() == std::env::current_exe().ok()
}

/// One column of `ps` for one process (macOS has no /proc). None when the process is gone or `ps` fails.
#[cfg(not(windows))]
fn ps(pid: u32, col: &str) -> Option<String> {
    let o = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", &format!("{col}=")])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (o.status.success() && !t.is_empty()).then_some(t)
}

/// True when `pid` leads its own process group (`setpgid` by `make --detach`).
fn group_leader(pid: u32) -> bool {
    #[cfg(not(windows))]
    if !Path::new("/proc").is_dir() {
        return ps(pid, "pgid").and_then(|g| g.parse::<u32>().ok()) == Some(pid);
    }
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            s.rsplit(") ")
                .next()
                .and_then(|r| r.split(' ').nth(2))
                .and_then(|g| g.parse::<u32>().ok())
        })
        == Some(pid)
}

/// `kill -s SIG -- target`: dash, the `sh` of Debian and Ubuntu, refuses `kill -SIG -- -pid` with "Illegal number".
#[cfg(unix)]
fn kill_line(sig: &str, target: &str) -> String {
    format!("kill -s {sig} -- {target}")
}

pub(crate) fn signal(target: &str, sig: &str, pid: u32) {
    #[cfg(unix)]
    {
        let _ = pid;
        // The shell's own kill: util-linux 2.42 /usr/bin/kill can refuse a negative group id in a container.
        // `target` is a number we built, never user text.
        let _ = Command::new("sh")
            .args(["-c", &kill_line(sig, target)])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = (target, sig);
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .status();
    }
}

#[cfg(test)]
mod tests {
    /// Fails with the old `kill -TERM -- -pid` on a machine that has dash.
    #[cfg(unix)]
    #[test]
    fn the_group_kill_line_works_in_dash() {
        use super::{kill_line, Command, Duration};
        if Command::new("dash")
            .args(["-c", "exit 0"])
            .status()
            .is_err()
        {
            return;
        }
        let Ok(mut child) = Command::new("setsid").args(["sleep", "30"]).spawn() else {
            return;
        };
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(200));
        let st = Command::new("dash")
            .args(["-c", &kill_line("TERM", &format!("-{pid}"))])
            .status()
            .unwrap();
        let _ = child.kill();
        let _ = child.wait();
        assert!(st.success());
    }

    #[test]
    fn a_replay_keeps_cancelled() {
        let v = serde_json::json!({"ok": false, "status": "cancelled", "exit": 8,
            "error": {"code": "SVX_CANCELLED", "message": "cancelled"}});
        let e = super::replay("wait", v);
        assert_eq!(e.status, "cancelled");
        assert_eq!(e.exit, super::Exit::Cancelled);
    }

    #[test]
    fn tasklist_csv_names_only_the_asked_pid() {
        let hit = "\"scriptvox.exe\",\"1234\",\"Console\",\"1\",\"9,000 K\"\r\n";
        assert_eq!(
            super::tasklist_image(hit, 1234).as_deref(),
            Some("scriptvox.exe")
        );
        assert_eq!(super::tasklist_image(hit, 123), None);
        assert_eq!(
            super::tasklist_image("INFO: No tasks are running.\r\n", 1234),
            None
        );
    }
}

#[cfg(all(test, unix))]
mod ps_tests {
    #[test]
    fn ps_reads_a_live_process_and_nothing_for_a_gone_one() {
        let me = super::ps(std::process::id(), "command").unwrap();
        assert!(!me.is_empty());
        assert_eq!(super::ps(4_000_000, "command"), None);
        assert_eq!(
            super::ps(std::process::id(), "pid").as_deref(),
            Some(std::process::id().to_string().as_str())
        );
    }

    #[test]
    fn a_zombie_is_not_alive() {
        let mut c = std::process::Command::new("sleep")
            .arg("20")
            .spawn()
            .unwrap();
        assert!(super::alive_by_ps(c.id()));
        let pid = c.id().to_string();
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid])
            .status();
        std::thread::sleep(std::time::Duration::from_millis(300));
        // Not reaped yet: a zombie.
        assert!(!super::alive_by_ps(c.id()));
        let _ = c.wait();
        assert!(!super::alive_by_ps(c.id()));
    }
}
