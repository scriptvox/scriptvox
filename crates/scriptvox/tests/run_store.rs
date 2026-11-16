//! state.db placement (PLAN-R1 D5) through the real CLI.
mod common;
use std::path::Path;

#[test]
fn local_project_keeps_state_db_in_dot_scriptvox() {
    let d = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", data.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(d.path().join(".scriptvox/state.db").is_file());
    assert_eq!(
        std::fs::read_to_string(d.path().join(".scriptvox/.gitignore")).unwrap(),
        "*\n"
    );
}

#[test]
fn windows_drive_project_puts_state_db_in_the_data_folder() {
    // WSL only: /mnt/c is 9p/drvfs, where SQLite WAL is not safe.
    let Some(tmp) = std::env::var_os("USER")
        .map(|_| Path::new("/mnt/c/Users").to_path_buf())
        .filter(|p| p.is_dir())
        .and_then(|p| {
            std::fs::read_dir(p)
                .ok()?
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("AppData/Local/Temp"))
                .find(|t| t.is_dir())
        })
    else {
        return;
    };
    let proj = tmp.join(format!("rvx-test-{}", std::process::id()));
    std::fs::create_dir_all(&proj).unwrap();
    let data = tempfile::tempdir().unwrap();
    let o = common::bin()
        .current_dir(&proj)
        .env("SCRIPTVOX_DATA", data.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let has_local_db = proj.join(".scriptvox/state.db").exists();
    let dbs: Vec<_> = std::fs::read_dir(data.path().join("state"))
        .map(|r| r.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&proj);
    assert!(o.status.success());
    assert!(!has_local_db, "no state.db on drvfs");
    assert_eq!(dbs.len(), 1, "one db in <data>/state/");
    let c = rusqlite::Connection::open(dbs[0].path()).unwrap();
    let mode: String = c
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
}
