//! `scriptvox setup mcp --client <name>`: add this binary to an MCP client's config. Spec: `docs/specs/mcp-setup.md`.
//! `--dry-run` writes nothing. A real run writes a backup copy first, keeps every other entry, reads the file back, and
//! puts the old file back when the check fails. No value of another entry reaches the output: it can hold a key.

use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const CLIENTS: [&str; 3] = ["claude-desktop", "cursor", "codex"];

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var_os(k)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> PathBuf {
    env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .unwrap_or_else(|| ".".into())
}

/// The file each client reads, from its own docs. Read 7 Nov 2026; not checked against a running client.
fn default_config(client: &str) -> PathBuf {
    match client {
        "claude-desktop" if cfg!(target_os = "macos") => {
            home().join("Library/Application Support/Claude/claude_desktop_config.json")
        }
        "claude-desktop" if cfg!(windows) => env_path("APPDATA")
            .unwrap_or_else(home)
            .join("Claude/claude_desktop_config.json"),
        "claude-desktop" => env_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home().join(".config"))
            .join("Claude/claude_desktop_config.json"),
        "cursor" => home().join(".cursor/mcp.json"),
        _ => env_path("CODEX_HOME")
            .unwrap_or_else(|| home().join(".codex"))
            .join("config.toml"),
    }
}

fn is_toml(client: &str) -> bool {
    client == "codex"
}

fn servers_key(client: &str) -> &'static str {
    if is_toml(client) {
        "mcp_servers"
    } else {
        "mcpServers"
    }
}

/// The whole file as data. An empty file is an empty document.
fn parse(client: &str, text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let doc = if is_toml(client) {
        let t: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        serde_json::to_value(t).map_err(|e| e.to_string())?
    } else {
        serde_json::from_str(text).map_err(|e| e.to_string())?
    };
    let servers = doc.get(servers_key(client));
    if !doc.is_object() || servers.is_some_and(|s| !s.is_object()) {
        return Err(format!(
            "it is not an object with a {} table",
            servers_key(client)
        ));
    }
    Ok(doc)
}

fn entry_of(client: &str, doc: &Value) -> Option<Value> {
    doc.get(servers_key(client))?.get("scriptvox").cloned()
}

/// The document without our entry, to compare what must not change.
fn others(client: &str, doc: &Value) -> Value {
    let mut d = doc.clone();
    if let Some(s) = d
        .get_mut(servers_key(client))
        .and_then(|s| s.as_object_mut())
    {
        s.remove("scriptvox");
        if s.is_empty() {
            d.as_object_mut().map(|o| o.remove(servers_key(client)));
        }
    }
    d
}

/// The new file. `None`: nothing to change.
fn new_text(client: &str, old: &str, doc: &Value, entry: &Value) -> Result<Option<String>, String> {
    let have = entry_of(client, doc);
    if have.as_ref() == Some(entry) {
        return Ok(None);
    }
    if is_toml(client) {
        // ponytail: text, so the comments in the file stay. A different entry is refused, not rewritten; a TOML editor
        // crate can do that if people ask.
        if have.is_some() {
            return Err(format!(
                "it has a [mcp_servers.scriptvox] entry with other values; change that entry by hand to {entry}"
            ));
        }
        let mut t = old.to_string();
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        return Ok(Some(format!(
            "{t}\n[mcp_servers.scriptvox]\ncommand = {}\nargs = [\"mcp\"]\n",
            entry["command"]
        )));
    }
    // ponytail: JSON keys come out sorted and the indent is ours; the data stays. A client's JSON has no comments.
    let mut d = doc.clone();
    let root = d.as_object_mut().ok_or("not an object")?;
    let servers = root
        .entry(servers_key(client))
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("no servers table")?;
    servers.insert("scriptvox".into(), entry.clone());
    serde_json::to_string_pretty(&d)
        .map(|t| Some(t + "\n"))
        .map_err(|e| e.to_string())
}

/// A name that no earlier backup holds, so the first backup is never replaced.
fn backup_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let at = |n: u32| {
        let suffix = if n == 0 {
            String::new()
        } else {
            format!(".{n}")
        };
        path.with_file_name(format!("{name}.scriptvox.bak{suffix}"))
    };
    (0..).map(at).find(|p| !p.exists()).unwrap_or_else(|| at(0))
}

/// Backup, write, read back, check. A failed check puts the old file back.
fn apply(
    path: &Path,
    old: Option<&str>,
    text: &str,
    check: impl Fn(&str) -> Result<(), String>,
) -> Result<Option<PathBuf>, String> {
    check(text).map_err(|e| format!("the new text fails the check, nothing was written: {e}"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let backup = old.map(|_| backup_path(path));
    if let Some(b) = &backup {
        std::fs::copy(path, b)
            .map_err(|e| format!("cannot write the backup {}: {e}", b.display()))?;
    }
    let tmp = path.with_extension("scriptvox.tmp");
    if old.is_some() {
        std::fs::copy(path, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    }
    std::fs::write(&tmp, text)
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("cannot write {}: {e}; the old file stays", path.display())
        })?;
    let back = std::fs::read_to_string(path).map_err(|e| e.to_string());
    if let Err(e) = back.and_then(|t| check(&t)) {
        let undone = match old {
            Some(o) => std::fs::write(path, o),
            None => std::fs::remove_file(path),
        };
        return Err(match undone {
            Ok(()) => {
                format!("the file failed the check after the write, the old state is back: {e}")
            }
            Err(u) => format!(
                "the file failed the check ({e}) and the restore failed ({u}); the backup is {:?}",
                backup
            ),
        });
    }
    Ok(backup)
}

pub fn run(client: Option<&str>, config: Option<&Path>, dry_run: bool) -> Envelope {
    let fail =
        |code, msg: String, fix: &str| Envelope::fail("setup", code, msg, Some(fix.to_string()));
    let names = CLIENTS.join(", ");
    let Some(client) = client.filter(|c| CLIENTS.contains(c)) else {
        return fail(
            Exit::Usage,
            format!("setup mcp needs --client, one of: {names}"),
            "Run `scriptvox setup mcp --client cursor --dry-run`.",
        );
    };
    let path = config.map_or_else(|| default_config(client), Path::to_path_buf);
    let path = path.canonicalize().unwrap_or(path);
    let old = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return fail(
                Exit::Error,
                format!("cannot read {}: {e}", path.display()),
                "Check the path and its rights.",
            )
        }
    };
    let text = old.as_deref().unwrap_or("");
    let not_valid = |e: String| {
        fail(
            Exit::Usage,
            format!("{} cannot be changed: {e}", path.display()),
            "The file stays as it is. Fix it, or add the entry by hand.",
        )
    };
    let doc = match parse(client, text) {
        Ok(d) => d,
        Err(e) => return not_valid(e),
    };
    let exe = std::env::current_exe().unwrap_or_else(|_| "scriptvox".into());
    let entry = json!({"command": exe, "args": ["mcp"]});
    let new = match new_text(client, text, &doc, &entry) {
        Ok(n) => n,
        Err(e) => return not_valid(e),
    };
    let other_names: Vec<String> = doc
        .get(servers_key(client))
        .and_then(|s| s.as_object())
        .map_or(vec![], |s| {
            s.keys().filter(|k| *k != "scriptvox").cloned().collect()
        });
    let action = match (&new, entry_of(client, &doc)) {
        (None, _) => "none",
        (Some(_), Some(_)) => "update",
        (Some(_), None) => "add",
    };
    let mut env = Envelope::ok("setup")
        .with("client", json!(client))
        .with("config", json!(path))
        .with("action", json!(action))
        .with("entry", entry.clone())
        .with("other_servers", json!(other_names))
        .with("dry_run", json!(dry_run));
    if dry_run {
        return env.with(
            "would_backup",
            json!(old.as_ref().map(|_| backup_path(&path))),
        );
    }
    if let Some(new) = new {
        let want = others(client, &doc);
        let check = |t: &str| {
            let d = parse(client, t)?;
            if entry_of(client, &d).as_ref() != Some(&entry) {
                return Err("our entry is not there".to_string());
            }
            if others(client, &d) != want {
                return Err("another part of the file changed".to_string());
            }
            Ok(())
        };
        match apply(&path, old.as_deref(), &new, check) {
            Ok(b) => env = env.with("backup", json!(b)),
            Err(e) => {
                return fail(
                    Exit::Error,
                    e,
                    "Add the entry by hand; `entry` in a --dry-run shows it.",
                )
            }
        }
    }
    env.with(
        "next",
        json!([format!(
            "Restart {client}: it reads this file when it starts."
        )]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_check_after_the_write_puts_the_old_file_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.json");
        std::fs::write(&p, "old").unwrap();
        // Passes on the new text only the first time: the read back fails.
        let n = std::cell::Cell::new(0);
        let e = apply(&p, Some("old"), "new", |_| {
            n.set(n.get() + 1);
            if n.get() == 1 {
                Ok(())
            } else {
                Err("bad".into())
            }
        })
        .unwrap_err();
        assert!(e.contains("old state is back"), "{e}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "old");
        assert_eq!(
            std::fs::read_to_string(backup_path(&p).with_file_name("c.json.scriptvox.bak"))
                .unwrap(),
            "old"
        );
        assert!(!p.with_extension("scriptvox.tmp").exists());

        let fresh = d.path().join("f.json");
        n.set(0);
        apply(&fresh, None, "new", |_| {
            n.set(n.get() + 1);
            if n.get() == 1 {
                Ok(())
            } else {
                Err("bad".into())
            }
        })
        .unwrap_err();
        assert!(!fresh.exists(), "no file before, no file after");
    }

    #[test]
    fn a_second_backup_keeps_the_first() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.json");
        std::fs::write(&p, "one").unwrap();
        apply(&p, Some("one"), "two", |_| Ok(())).unwrap();
        let b = apply(&p, Some("two"), "three", |_| Ok(()))
            .unwrap()
            .unwrap();
        assert!(b.to_string_lossy().ends_with(".bak.1"));
        assert_eq!(
            std::fs::read_to_string(p.with_file_name("c.json.scriptvox.bak")).unwrap(),
            "one"
        );
        assert_eq!(std::fs::read_to_string(b).unwrap(), "two");
    }
}
