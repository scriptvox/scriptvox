//! Provider keys (PLAN-R1 §2.3 "Keys"). Order: environment variable, then
//! `keys.toml` in the config folder (mode 0600; a user-only ACL on Windows).
//! A value never travels in argv, RPC messages, logs or artefacts.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// (provider, environment variable).
pub const PROVIDERS: [(&str, &str); 7] = [
    ("elevenlabs", "ELEVENLABS_API_KEY"),
    ("pexels", "PEXELS_API_KEY"),
    ("pixabay", "PIXABAY_API_KEY"),
    ("fal", "FAL_KEY"),
    // Your vision service, when it needs a key (see `judge::vision_url`).
    ("vision", "SCRIPTVOX_VISION_KEY"),
    // A self-hosted queue with the fal protocol (open models on your own GPUs).
    ("selfhost", "SCRIPTVOX_SELFHOST_KEY"),
    // Hugging Face: a model repo behind an Agree button.
    ("hf", "HF_TOKEN"),
];

/// One `[[key]]` block of the generators overlay (`docs/specs/key-names.md`).
#[derive(serde::Deserialize)]
struct Declared {
    name: String,
    env: String,
}

#[derive(serde::Deserialize)]
struct Overlay {
    #[serde(default)]
    key: Vec<Declared>,
}

/// One line worded as the fix, or nothing when the entry is fine. A key variable ends in KEY, TOKEN or SECRET, so
/// PATH, HOME and the like never become a key that a row sends to a host.
fn declared_error(d: &Declared, earlier: &[Declared]) -> Option<String> {
    let at = format!("key {:?}", d.name);
    if d.name.is_empty()
        || d.name.len() > 32
        || !d
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))
    {
        return Some(format!(
            "{at}: name must be 1 to 32 of a-z, 0-9, '_' or '-'"
        ));
    }
    if PROVIDERS.iter().any(|(p, _)| *p == d.name) {
        return Some(format!("{at}: this name is built in; choose another name"));
    }
    if !d.env.starts_with(|c: char| c.is_ascii_uppercase())
        || d.env.len() > 64
        || !d
            .env
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return Some(format!(
            "{at}: env must be upper case: A-Z, 0-9 and '_', 64 at most, first a letter"
        ));
    }
    if d.env.starts_with("LD_")
        || d.env.starts_with("DYLD_")
        || !["KEY", "TOKEN", "SECRET"]
            .iter()
            .any(|s| d.env.ends_with(s))
    {
        return Some(format!(
            "{at}: env {:?} is not a key variable; it must end in KEY, TOKEN or SECRET and not start with LD_ or DYLD_",
            d.env
        ));
    }
    if PROVIDERS.iter().any(|(_, v)| *v == d.env) {
        return Some(format!(
            "{at}: env {:?} belongs to a built-in key; choose another variable",
            d.env
        ));
    }
    if earlier.iter().any(|o| o.name == d.name || o.env == d.env) {
        return Some(format!("{at}: name or env is used twice"));
    }
    None
}

/// The valid entries and one error line for each bad one. A bad entry is never used.
fn parse_declared(text: &str) -> (Vec<Declared>, Vec<String>) {
    let (mut good, mut bad) = (Vec::new(), Vec::new());
    // Block by block: a mistake in a row must not cost the keys.
    for block in crate::generators::blocks(text) {
        if crate::generators::block_is(block, "generator") {
            continue;
        }
        match toml::from_str::<Overlay>(block) {
            Ok(o) => {
                for d in o.key {
                    match declared_error(&d, &good) {
                        Some(e) => bad.push(e),
                        None => good.push(d),
                    }
                }
            }
            Err(e) => bad.push(format!("[[key]]: {}", e.message())),
        }
    }
    (good, bad)
}

/// The file that holds the user's rows and keys: `SCRIPTVOX_GENERATORS`, else generators.toml in the config folder.
pub fn overlay_file() -> PathBuf {
    crate::provision::user_env("SCRIPTVOX_GENERATORS")
        .map(PathBuf::from)
        .unwrap_or_else(|| config_dir().join("generators.toml"))
}

fn declared() -> (Vec<Declared>, Vec<String>) {
    let path = overlay_file();
    let (good, bad) = std::fs::read_to_string(&path)
        .map(|t| parse_declared(&t))
        .unwrap_or_default();
    let bad = bad
        .into_iter()
        .map(|e| format!("{}: {e}", path.display()))
        .collect();
    (good, bad)
}

/// What is wrong with the `[[key]]` blocks of the overlay, for `providers`.
pub fn declared_errors() -> Vec<String> {
    declared().1
}

/// Every key: the built-in table, then the user's `[[key]]` blocks.
pub fn all() -> Vec<(String, String)> {
    PROVIDERS
        .iter()
        .map(|(p, v)| (p.to_string(), v.to_string()))
        .chain(declared().0.into_iter().map(|d| (d.name, d.env)))
        .collect()
}

pub fn env_var(provider: &str) -> Option<String> {
    all()
        .into_iter()
        .find(|(p, _)| p == provider)
        .map(|(_, v)| v)
}

/// `SCRIPTVOX_CONFIG`, else `%APPDATA%\scriptvox`, `~/Library/Application Support/scriptvox` on macOS, `$XDG_CONFIG_HOME/scriptvox` or `~/.config/scriptvox`.
pub fn config_dir() -> PathBuf {
    crate::provision::scriptvox_dir(
        "SCRIPTVOX_CONFIG",
        ("XDG_CONFIG_HOME", ".config"),
        "APPDATA",
    )
}

pub fn file() -> PathBuf {
    config_dir().join("keys.toml")
}

/// What is wrong with keys.toml, when it exists and cannot be read as keys. Only the line number: TOML's own message
/// quotes the line, and that line can hold a key.
pub fn file_error() -> Option<String> {
    let path = file();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(format!("{}: {e}", path.display())),
    };
    let e = toml::from_str::<BTreeMap<String, String>>(&text).err()?;
    let at = e.span().map_or(0, |s| s.start.min(text.len()));
    let line = text.as_bytes()[..at]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1;
    Some(format!(
        "{} does not parse at line {line}. Correct that line, or move the file away, and run the command again.",
        path.display()
    ))
}

fn read_file() -> BTreeMap<String, String> {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// The key for `provider` and where it came from (`env` or `keys.toml`).
pub fn resolve(provider: &str) -> Option<(String, &'static str)> {
    if let Some(v) = env_var(provider)
        .and_then(|k| crate::provision::user_env(&k)?.into_string().ok())
        .filter(|v| !v.is_empty())
    {
        return Some((v, "env"));
    }
    read_file()
        .remove(provider)
        .filter(|v| !v.is_empty())
        .map(|v| (v, "keys.toml"))
}

/// Every known key value, for the redactor.
pub fn all_values() -> Vec<String> {
    all()
        .iter()
        .filter_map(|(p, _)| resolve(p).map(|(v, _)| v))
        .chain(read_file().into_values())
        .collect()
}

/// Writes `provider = value` into keys.toml, user-only, through a temp file.
pub fn set(provider: &str, value: &str) -> std::io::Result<PathBuf> {
    // A file that cannot be read would count as empty, and the write below would erase every other key.
    if let Some(e) = file_error() {
        return Err(std::io::Error::other(e));
    }
    let mut all = read_file();
    all.insert(provider.to_string(), value.trim().to_string());
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = file();
    let tmp = path.with_extension("tmp");
    // A leftover temp file would keep its old mode: start from a fresh one.
    let _ = std::fs::remove_file(&tmp);
    let body = toml::to_string(&all).map_err(std::io::Error::other)?;
    {
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        use std::io::Write;
        o.open(&tmp)?.write_all(body.as_bytes())?;
    }
    std::fs::rename(&tmp, &path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    {
        // User-only ACL: drop inherited entries, grant the current user full control. A key that stays readable by
        // other users is an error, not a silent pass: `USERNAME` failed under the SYSTEM account and left
        // "Authenticated Users: modify" on the file (Windows 11 checklist, 5 Nov 2026).
        let who = std::process::Command::new("whoami").output()?;
        let user = String::from_utf8_lossy(&who.stdout).trim().to_string();
        let done = std::process::Command::new("icacls")
            .arg(&path)
            .args(["/inheritance:r", "/grant:r", &format!("{user}:F")])
            .output()?;
        if !done.status.success() {
            let _ = std::fs::remove_file(&path);
            return Err(std::io::Error::other(format!(
                "could not make {} private: {}",
                path.display(),
                String::from_utf8_lossy(&done.stdout).trim()
            )));
        }
    }
    Ok(path)
}

/// Reads a secret from the terminal with echo off (Unix `stty`).
pub fn prompt(provider: &str) -> std::io::Result<String> {
    eprint!("{provider} key (input hidden): ");
    #[cfg(unix)]
    let _ = std::process::Command::new("stty")
        .arg("-echo")
        .stdin(std::process::Stdio::inherit())
        .status();
    let mut s = String::new();
    let r = std::io::stdin().read_line(&mut s);
    #[cfg(unix)]
    let _ = std::process::Command::new("stty")
        .arg("echo")
        .stdin(std::process::Stdio::inherit())
        .status();
    eprintln!();
    r.map(|_| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_declared;

    fn errors(name: &str, env: &str) -> Vec<String> {
        parse_declared(&format!("[[key]]\nname = {name:?}\nenv = {env:?}\n")).1
    }

    #[test]
    fn a_declared_key_needs_a_clean_name_and_a_key_variable() {
        assert!(errors("acme", "ACME_API_KEY").is_empty());
        assert!(errors("acme-2", "ACME_TOKEN").is_empty());
        for (name, env) in [
            ("acme", "PATH"),
            ("acme", "HOME"),
            ("acme", "LD_PRELOAD"),
            ("acme", "LD_KEY"),
            ("acme", "DYLD_SECRET"),
            ("acme", "acme_key"),
            ("acme", "1_KEY"),
            ("Acme", "ACME_KEY"),
            ("", "ACME_KEY"),
            ("fal", "ACME_KEY"),
            ("acme", "FAL_KEY"),
        ] {
            assert_eq!(errors(name, env).len(), 1, "{name} {env}");
        }
    }

    #[test]
    fn a_repeated_name_or_variable_is_refused_and_rows_are_ignored() {
        let (good, bad) = parse_declared(
            "[[generator]]\nname = \"x\"\n[[key]]\nname = \"a\"\nenv = \"A_KEY\"\n[[key]]\nname = \"a\"\nenv = \"B_KEY\"\n[[key]]\nname = \"c\"\nenv = \"A_KEY\"\n",
        );
        assert_eq!(good.len(), 1);
        assert_eq!(bad.len(), 2);
    }
}
