//! Run store (PLAN-R1 D5, §2.6, §2.7): `.scriptvox/runs/<id>/` with
//! `events.jsonl` and `result.json`, and one SQLite `state.db` per project.
//!
//! `state.db` lives in `.scriptvox/` with WAL on a local filesystem. WAL needs
//! shared-memory mmap, which is not reliable on 9p/drvfs (WSL `/mnt/c`), SMB,
//! NFS or FUSE (cloud sync). There it lives in `<data>/state/<path hash>.db`
//! with `journal_mode=DELETE` and a busy timeout.

use crate::envelope::Envelope;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

/// UTC time as RFC 3339 (`2026-11-02T01:02:03Z`), from the system clock.
pub fn now_utc() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rem) = (s / 86_400, s % 86_400);
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Filesystem type of the mount that holds `path`, from /proc/self/mountinfo.
#[cfg(target_os = "linux")]
pub fn fs_type(path: &Path) -> Option<String> {
    let real = path.canonicalize().ok()?;
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    info.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            let sep = f.iter().position(|x| *x == "-")?;
            Some((f.get(4)?.replace("\\040", " "), f.get(sep + 1)?.to_string()))
        })
        .filter(|(mnt, _)| real.starts_with(mnt))
        .max_by_key(|(mnt, _)| mnt.len())
        .map(|(_, t)| t)
}

#[cfg(not(target_os = "linux"))]
pub fn fs_type(_path: &Path) -> Option<String> {
    None
}

/// True when WAL is not safe for this project folder.
pub fn needs_rollback_journal(project: &Path) -> bool {
    let remote = [
        "9p",
        "drvfs",
        "cifs",
        "smb3",
        "smbfs",
        "nfs",
        "nfs4",
        "fuse",
        "fuseblk",
        "fuse.rclone",
        "v9fs",
    ];
    let ty = fs_type(project).unwrap_or_default();
    let onedrive = project
        .canonicalize()
        .is_ok_and(|p| p.to_string_lossy().contains("OneDrive"));
    remote.iter().any(|r| ty == *r || ty.starts_with("fuse.")) || onedrive
}

/// FNV-1a of the canonical project path, for the data-folder db name.
fn path_hash(p: &Path) -> String {
    let s = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let h = s
        .to_string_lossy()
        .bytes()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
    format!("{h:016x}")
}

/// Opens (and creates) a SQLite file the CLI and a server can share: a busy timeout, and WAL, or the DELETE journal
/// where WAL is not safe (`needs_rollback_journal`).
pub fn open_db(path: &Path, rollback: bool) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(10))?;
    c.pragma_update(
        None,
        "journal_mode",
        if rollback { "DELETE" } else { "WAL" },
    )?;
    Ok(c)
}

/// Brings a database to the newest schema: step `i` runs when `PRAGMA user_version` is below `i + 1`, in one
/// `BEGIN IMMEDIATE` transaction with the new version. A database newer than this binary is refused, not changed.
pub fn migrate(c: &mut Connection, steps: &[&str]) -> Result<(), String> {
    let have: usize = c
        .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        .try_into()
        .unwrap_or(0);
    if have > steps.len() {
        return Err(format!(
            "the database is at version {have}, newer than this scriptvox ({}): update scriptvox",
            steps.len()
        ));
    }
    for (i, sql) in steps.iter().enumerate().skip(have) {
        let t = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        t.execute_batch(sql)
            .map_err(|e| format!("migration {}: {e}", i + 1))?;
        t.pragma_update(None, "user_version", (i + 1) as i64)
            .map_err(|e| e.to_string())?;
        t.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The server's schema (`docs/plan/MILESTONE-API.md`, G165). Only add steps at the end; never change one.
const SERVER_DB: &[&str] = &["CREATE TABLE tokens(
        id TEXT PRIMARY KEY, sha256 TEXT NOT NULL UNIQUE, scopes TEXT NOT NULL, label TEXT,
        created TEXT NOT NULL, revoked TEXT);
    CREATE TABLE jobs(
        run_id TEXT PRIMARY KEY, project TEXT NOT NULL, token_id TEXT, status TEXT NOT NULL, created TEXT NOT NULL);
    CREATE TABLE idempotency(
        key TEXT NOT NULL, token_id TEXT NOT NULL, run_id TEXT NOT NULL, created TEXT NOT NULL,
        PRIMARY KEY(key, token_id));
    CREATE TABLE spends(
        plan_id TEXT NOT NULL, run_id TEXT, token_id TEXT NOT NULL, max_usd REAL NOT NULL, at TEXT NOT NULL);"];

/// Opens `<data>/server.db` at the newest schema: tokens metadata, the job index across projects, idempotency keys and
/// who approved each spend. The tokens themselves are never stored, only their sha256.
pub fn open_server_db() -> Result<(Connection, PathBuf), String> {
    let dir = crate::provision::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("server.db");
    let mut c = open_db(&path, needs_rollback_journal(&dir))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    migrate(&mut c, SERVER_DB)?;
    Ok((c, path))
}

/// Opens (and creates) the project's state.db. Returns it and its path.
pub fn open_state(project: &Path) -> rusqlite::Result<(Connection, PathBuf)> {
    let dot = project.join(".scriptvox");
    let _ = std::fs::create_dir_all(&dot);
    let _ = std::fs::write(dot.join(".gitignore"), "*\n");
    let rollback = needs_rollback_journal(project);
    let path = if rollback {
        let d = crate::provision::data_dir().join("state");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{}.db", path_hash(project)))
    } else {
        dot.join("state.db")
    };
    let c = open_db(&path, rollback)?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS runs(
            id TEXT PRIMARY KEY, command TEXT NOT NULL, status TEXT NOT NULL,
            started TEXT NOT NULL, finished TEXT, exit INTEGER, video TEXT, pid INTEGER);",
    )?;
    Ok((c, path))
}

pub const EVENT_SCHEMA_V: u64 = 1;

/// One run's folder and event log.
pub struct Run {
    pub id: String,
    pub dir: PathBuf,
}

/// True for a run id as ScriptVox writes one: `r_` and letters or digits, then `.` and a number for each resume
/// (`r_ab12cd`, `r_ab12cd.1.2`). Nothing else may name a run folder: an id can come from a command line, from an MCP
/// client or from a hidden flag, and it becomes a path under `.scriptvox/runs`.
pub fn safe_id(id: &str) -> bool {
    let mut parts = id.split('.');
    let head = parts.next().unwrap_or("");
    let stem = head.strip_prefix("r_").unwrap_or("");
    id.len() <= 64
        && !stem.is_empty()
        && stem.chars().all(|c| c.is_ascii_alphanumeric())
        && parts.all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn not_a_run_id(id: &str) -> std::io::Result<()> {
    if safe_id(id) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{id} is not a run id"),
        ))
    }
}

impl Run {
    pub fn new_id() -> String {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!(
            "r_{:06x}",
            (t.as_micros() as u64 ^ u64::from(std::process::id())) & 0xff_ffff
        )
    }

    pub fn create(project: &Path, command: &str) -> std::io::Result<Self> {
        Self::create_unique(project, command, Self::new_id)
    }

    /// Takes a run folder no other run holds: `create_dir` refuses an id already in use, and the next id is tried.
    fn create_unique(
        project: &Path,
        command: &str,
        mut next: impl FnMut() -> String,
    ) -> std::io::Result<Self> {
        let runs = project.join(".scriptvox/runs");
        std::fs::create_dir_all(&runs)?;
        loop {
            let id = next();
            match std::fs::create_dir(runs.join(&id)) {
                Ok(()) => return Self::create_with_id(project, command, &id),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
    }

    pub fn create_with_id(project: &Path, command: &str, id: &str) -> std::io::Result<Self> {
        not_a_run_id(id)?;
        let id = id.to_string();
        let dir = std::path::absolute(project.join(".scriptvox/runs").join(&id))?;
        std::fs::create_dir_all(&dir)?;
        if let Ok((c, _)) = open_state(project) {
            let _ = c.execute(
                "INSERT INTO runs(id, command, status, started, pid) VALUES (?1, ?2, 'running', ?3, ?4)",
                params![id, command, now_utc(), std::process::id()],
            );
        }
        let run = Self { id, dir };
        run.event("run.start", json!({"command": command}));
        Ok(run)
    }

    /// Opens an existing run (the detached supervisor uses it).
    pub fn attach(project: &Path, id: &str) -> std::io::Result<Self> {
        not_a_run_id(id)?;
        let dir = std::path::absolute(project.join(".scriptvox/runs").join(id))?;
        if !dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no run {id}"),
            ));
        }
        Ok(Self {
            id: id.to_string(),
            dir,
        })
    }

    /// Appends one `scriptvox.event/v1` line. Event loss never fails a run.
    pub fn event(&self, ty: &str, fields: Value) {
        let mut e = json!({"v": EVENT_SCHEMA_V, "t": now_utc(), "run": self.id, "type": ty});
        if let (Some(m), Value::Object(f)) = (e.as_object_mut(), fields) {
            m.extend(f);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.jsonl"))
        {
            let _ = writeln!(
                f,
                "{}",
                crate::redact::Redactor::from_env().apply(&e.to_string())
            );
        }
    }

    /// Records the final envelope: `result.json`, a `run.end` event and the db row.
    pub fn finish(&self, project: &Path, env: &Envelope) {
        // The numeric exit code goes along: `wait` replays it exactly.
        let mut doc: Value = serde_json::from_str(&env.to_json()).unwrap_or_default();
        doc["exit"] = json!(env.exit.code());
        // Files get the same key redaction as stdout.
        let _ = std::fs::write(
            self.dir.join("result.json"),
            crate::redact::Redactor::from_env().apply(&doc.to_string()),
        );
        self.event(
            "run.end",
            json!({"status": env.status, "exit": env.exit.code()}),
        );
        if let Ok((c, _)) = open_state(project) {
            let video = env
                .artifacts
                .get("video")
                .and_then(|v| v.as_str())
                .map(String::from);
            let _ = c.execute(
                "UPDATE runs SET status = ?2, finished = ?3, exit = ?4, video = ?5 WHERE id = ?1",
                params![self.id, env.status, now_utc(), env.exit.code(), video],
            );
        }
    }
}

/// The latest runs, newest first.
pub fn list(project: &Path, limit: usize) -> Vec<Value> {
    let Ok((c, _)) = open_state(project) else {
        return vec![];
    };
    let Ok(mut q) = c.prepare("SELECT id, command, status, started, finished, exit, video, pid FROM runs ORDER BY started DESC, rowid DESC LIMIT ?1") else {
        return vec![];
    };
    q.query_map(params![limit as i64], |r| {
        let (id, mut status) = (r.get::<_, String>(0)?, r.get::<_, String>(2)?);
        if status == "running" && crate::supervise::died(project, &id, r.get(7)?) {
            status = "died".into();
        }
        Ok(
            json!({"id": id, "command": r.get::<_, String>(1)?, "status": status,
                  "started": r.get::<_, String>(3)?, "finished": r.get::<_, Option<String>>(4)?,
                  "exit": r.get::<_, Option<i64>>(5)?, "video": r.get::<_, Option<String>>(6)?}),
        )
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_run_id_names_a_run_folder() {
        for ok in ["r_ab12cd", "r_ab12cd.1", "r_ab12cd.1.12", "r_0", "r_ZZ99"] {
            assert!(super::safe_id(ok), "{ok}");
        }
        for bad in [
            "",
            "r_",
            "ab12cd",
            "r_ab/../x",
            "../../etc",
            "r_ab12cd.",
            "r_ab12cd..1",
            "r_ab12cd.x",
            "r_ab 12",
            "r_ab12cd/1",
            "/tmp/x",
            "r_ab\\cd",
            ".",
            "r_ab12cd.1/..",
        ] {
            assert!(!super::safe_id(bad), "{bad}");
        }
        assert!(!super::safe_id(&format!("r_{}", "a".repeat(70))));
        let d = tempfile::tempdir().unwrap();
        for bad in ["../../escape", "r_ok/../../escape"] {
            assert!(
                super::Run::create_with_id(d.path(), "make", bad).is_err(),
                "{bad}"
            );
            assert!(super::Run::attach(d.path(), bad).is_err(), "{bad}");
        }
        assert!(
            !d.path().join("escape").exists()
                && !d.path().parent().unwrap().join("escape").exists()
        );
    }

    use super::*;

    #[test]
    fn a_new_run_never_takes_the_folder_of_an_earlier_run() {
        let d = tempfile::tempdir().unwrap();
        let mut ids = ["r_same", "r_same", "r_other"].into_iter();
        let first = Run::create_unique(d.path(), "make", || ids.next().unwrap().into()).unwrap();
        std::fs::write(first.dir.join("result.json"), "{}").unwrap();
        let second = Run::create_unique(d.path(), "make", || ids.next().unwrap().into()).unwrap();
        assert_eq!(
            (first.id.as_str(), second.id.as_str()),
            ("r_same", "r_other")
        );
        assert!(first.dir.join("result.json").is_file());
    }

    #[test]
    fn utc_format() {
        let t = now_utc();
        assert_eq!(t.len(), 20);
        assert!(t.starts_with("20") && t.ends_with('Z') && &t[10..11] == "T");
    }

    #[test]
    fn migrations_run_once_in_order_and_refuse_a_newer_file() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("x.db");
        let steps = [
            "CREATE TABLE a(x)",
            "CREATE TABLE b(y); INSERT INTO a VALUES (1)",
        ];
        let mut c = open_db(&path, false).unwrap();
        migrate(&mut c, &steps[..1]).unwrap();
        migrate(&mut c, &steps).unwrap();
        migrate(&mut c, &steps).unwrap();
        let rows: i64 = c
            .query_row("SELECT count(*) FROM a", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "step 2 ran once");
        let v: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
        assert!(migrate(&mut c, &steps[..1]).unwrap_err().contains("newer"));
        // A failed step leaves the version where it was.
        let bad = [
            "CREATE TABLE a(x)",
            "CREATE TABLE b(y); INSERT INTO a VALUES (1)",
            "NOT SQL",
        ];
        assert!(migrate(&mut c, &bad).is_err());
        let v: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
    }

    #[test]
    fn server_db_has_its_tables() {
        let d = tempfile::tempdir().unwrap();
        let mut c = open_db(&d.path().join("server.db"), false).unwrap();
        migrate(&mut c, SERVER_DB).unwrap();
        for t in ["tokens", "jobs", "idempotency", "spends"] {
            let n: i64 = c
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{t}");
        }
    }

    #[test]
    fn local_tmp_uses_wal_and_writes_gitignore() {
        let d = tempfile::tempdir().unwrap();
        let (c, path) = open_state(d.path()).unwrap();
        let mode: String = c
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        if !needs_rollback_journal(d.path()) {
            assert_eq!(mode, "wal");
            assert_eq!(path, d.path().join(".scriptvox/state.db"));
        }
        assert_eq!(
            std::fs::read_to_string(d.path().join(".scriptvox/.gitignore")).unwrap(),
            "*\n"
        );
    }
}
