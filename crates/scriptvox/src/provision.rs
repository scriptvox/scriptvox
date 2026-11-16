//! The embedded Python worker (PLAN-R1 §2.4, D2). The binary carries the worker
//! sources and unpacks them by content hash, so a changed worker never runs
//! stale. `uv run --frozen` builds the environment from the embedded lockfile.

use std::path::{Path, PathBuf};
use std::process::Command;

macro_rules! worker_files {
    ($($p:literal),* $(,)?) => { &[$(($p, include_str!(concat!("../../../python/", $p)))),*] };
}

/// Every worker source file. A test checks that no `.py` file is missing here.
pub const WORKER_FILES: &[(&str, &str)] = worker_files![
    "pyproject.toml",
    "uv.lock",
    "scriptvox_worker/__init__.py",
    "scriptvox_worker/__main__.py",
    "scriptvox_worker/rpc.py",
    "scriptvox_worker/tools.py",
    "scriptvox_worker/tts_edge.py",
    "scriptvox_worker/tts_elevenlabs.py",
];

/// FNV-1a, stable across Rust versions (the std hasher is not).
fn fnv(data: &[(&str, &str)]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for (p, c) in data {
        for b in p.bytes().chain([0]).chain(c.bytes()).chain([0]) {
            h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// `SCRIPTVOX_DATA`, else the platform data folder.
pub fn data_dir() -> PathBuf {
    scriptvox_dir(
        "SCRIPTVOX_DATA",
        ("XDG_DATA_HOME", ".local/share"),
        "LOCALAPPDATA",
    )
}

/// A variable that changes what this install has: a provider key, or the file of the user's rows. A unit test gets
/// none of them, so `cargo test` gives one result with and without keys in the shell (`docs/specs/contributor-ci.md`).
pub fn user_env(name: &str) -> Option<std::ffi::OsString> {
    if cfg!(test) {
        return None;
    }
    std::env::var_os(name)
}

/// The override variable, else the platform folder (`docs/plan/MILESTONE-API.md`, G165).
pub fn scriptvox_dir(own: &str, xdg: (&str, &str), win: &str) -> PathBuf {
    // A unit test never reads the folders of the person who runs it: an installed voice or a row in their
    // generators.toml changed 2 results (13 Nov 2026). Each test process gets its own empty pair.
    #[cfg(test)]
    if true {
        let _ = (xdg, win);
        return std::env::temp_dir()
            .join(format!("scriptvox-unit-{}", std::process::id()))
            .join(own);
    }
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env(own) {
        return d;
    }
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    pick_dir(&env, os, xdg, win, &|p| p.exists())
}

/// Windows: `%win%\scriptvox`. macOS: `~/Library/Application Support/scriptvox`, but an XDG variable that is set wins, and a
/// `~/.config/scriptvox` or `~/.local/share/scriptvox` made before 8 Nov 2026 stays in use, so no key or tool is lost.
/// Else XDG.
fn pick_dir(
    env: &dyn Fn(&str) -> Option<PathBuf>,
    os: &str,
    (xdg_var, xdg_sub): (&str, &str),
    win: &str,
    exists: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    if os == "windows" {
        return env(win).unwrap_or_else(|| ".".into()).join("scriptvox");
    }
    let xdg = env(xdg_var).map(|d| d.join("scriptvox"));
    let home = env("HOME");
    let old = home.as_ref().map(|h| h.join(xdg_sub).join("scriptvox"));
    if os == "macos" && xdg.is_none() && !old.as_deref().is_some_and(exists) {
        if let Some(h) = &home {
            return h.join("Library/Application Support/scriptvox");
        }
    }
    xdg.or(old).unwrap_or_else(|| PathBuf::from("./scriptvox"))
}

#[cfg(test)]
mod dir_tests {
    use super::*;

    #[test]
    fn platform_folders() {
        let env = |k: &str| match k {
            "HOME" => Some(PathBuf::from("/h")),
            "APPDATA" => Some(PathBuf::from("C:/a")),
            _ => None,
        };
        let none = |_: &Path| false;
        let cfg = ("XDG_CONFIG_HOME", ".config");
        assert_eq!(
            pick_dir(&env, "linux", cfg, "APPDATA", &none),
            PathBuf::from("/h/.config/scriptvox")
        );
        assert_eq!(
            pick_dir(&env, "windows", cfg, "APPDATA", &none),
            PathBuf::from("C:/a/scriptvox")
        );
        assert_eq!(
            pick_dir(&env, "macos", cfg, "APPDATA", &none),
            PathBuf::from("/h/Library/Application Support/scriptvox")
        );
        // A folder from before stays in use on macOS.
        let old = |p: &Path| p == Path::new("/h/.config/scriptvox");
        assert_eq!(
            pick_dir(&env, "macos", cfg, "APPDATA", &old),
            PathBuf::from("/h/.config/scriptvox")
        );
        // A set XDG variable wins.
        let xdg = |k: &str| {
            (k == "XDG_CONFIG_HOME")
                .then(|| PathBuf::from("/x"))
                .or_else(|| env(k))
        };
        assert_eq!(
            pick_dir(&xdg, "macos", cfg, "APPDATA", &none),
            PathBuf::from("/x/scriptvox")
        );
    }
}

/// The worker project folder. `SCRIPTVOX_WORKER_SRC` (a checkout's `python/`)
/// wins, for editable development. Otherwise the embedded files are unpacked once.
pub fn worker_dir() -> Result<PathBuf, String> {
    if let Some(src) = std::env::var_os("SCRIPTVOX_WORKER_SRC") {
        return Ok(src.into());
    }
    let dir = data_dir().join("worker").join(format!(
        "{}-{:016x}",
        env!("CARGO_PKG_VERSION"),
        fnv(WORKER_FILES)
    ));
    if dir.join("pyproject.toml").is_file() {
        return Ok(dir);
    }
    // Write to a temp folder, then rename: a killed unpack never leaves half a worker.
    let tmp = dir.with_extension(format!("tmp{}", std::process::id()));
    let write = |rel: &str, body: &str| -> std::io::Result<()> {
        let p = tmp.join(rel);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, body)
    };
    for (rel, body) in WORKER_FILES {
        write(rel, body).map_err(|e| format!("cannot unpack the worker: {e}"))?;
    }
    match std::fs::rename(&tmp, &dir) {
        Ok(()) => Ok(dir),
        // Another process won the race: use its copy.
        Err(_) if dir.join("pyproject.toml").is_file() => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(dir)
        }
        Err(e) => Err(format!("cannot unpack the worker: {e}")),
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
}

/// The command that starts the worker. `SCRIPTVOX_UV` overrides the `uv` on PATH.
pub fn worker_command(dir: &Path) -> Result<Command, String> {
    // SCRIPTVOX_UV, then the pinned uv if installed, then uv on PATH, then a download.
    let uv = match std::env::var_os("SCRIPTVOX_UV").map(PathBuf::from) {
        Some(p) => p,
        None => match pin("uv")
            .and_then(|p| installed(&p))
            .or_else(|| find_on_path("uv").map(|p| vec![p]))
        {
            Some(b) => b[0].clone(),
            None => {
                let p =
                    pin("uv").ok_or("uv was not found and there is no uv pin for this platform")?;
                install(&p, &|m| eprintln!("scriptvox: {m}"))?[0].clone()
            }
        },
    };
    let mut cmd = Command::new(uv);
    cmd.args([
        "run",
        "--quiet",
        "--frozen",
        "--no-dev",
        "--python",
        "3.12",
        "--project",
    ])
    .arg(dir)
    .args(["python", "-m", "scriptvox_worker"])
    // The worker is not an installed package (`package = false`): put it on the path.
    .env("PYTHONPATH", dir)
    // uv uses the system certificate store (corporate proxies); SSL_CERT_FILE passes through.
    .env("UV_NATIVE_TLS", "1");
    // uv's cache and Python live in ScriptVox's data folder, so `uninstall --purge`
    // removes them, unless the user already points uv somewhere.
    for (var, sub) in [
        ("UV_CACHE_DIR", "uv-cache"),
        ("UV_PYTHON_INSTALL_DIR", "python"),
    ] {
        if std::env::var_os(var).is_none() {
            cmd.env(var, data_dir().join(sub));
        }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------- tool pins

const TOOLCHAIN: &str = include_str!("../../../toolchain.lock");

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub struct Pin {
    pub name: String,
    pub version: String,
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub licence: String,
    pub strip: u32,
    pub bins: Vec<String>,
    /// A plain file (a model), not an archive.
    #[serde(default)]
    pub raw: bool,
    /// Source B: a second address of the same file (same sha256), tried when `url` fails. Empty: none.
    #[serde(default)]
    pub mirror: String,
}

#[derive(serde::Deserialize)]
struct Toolchain {
    tool: Vec<Pin>,
}

/// This machine's pin target, as named in `toolchain.lock`.
pub fn target() -> String {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

/// The pins. `SCRIPTVOX_TOOLCHAIN` points at another lock file (tests, mirrors).
pub fn pins() -> Vec<Pin> {
    let text = std::env::var_os("SCRIPTVOX_TOOLCHAIN")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| TOOLCHAIN.to_string());
    toml::from_str::<Toolchain>(&text)
        .map(|t| t.tool)
        .unwrap_or_default()
}

pub fn pin(name: &str) -> Option<Pin> {
    let t = target();
    pins()
        .into_iter()
        .find(|p| p.name == name && (p.target == t || p.target == "any"))
}

pub fn tool_dir(p: &Pin) -> PathBuf {
    data_dir().join("tools").join(&p.name).join(&p.version)
}

/// The installed binaries of a pin, if every one is present. A model file found by `models scan` counts as installed.
pub fn installed(p: &Pin) -> Option<Vec<PathBuf>> {
    let d = tool_dir(p);
    let bins: Vec<PathBuf> = p.bins.iter().map(|b| d.join(b)).collect();
    if bins.iter().all(|b| b.is_file()) {
        return Some(bins);
    }
    let found = p.raw.then(|| found_models().remove(&p.name)).flatten()?;
    (std::fs::metadata(&found).ok()?.len() == p.size).then(|| vec![found])
}

/// The Hugging Face hub cache of this user, if the default place exists: `HF_HUB_CACHE`, `HF_HOME/hub`, else
/// `~/.cache/huggingface/hub`.
pub fn hf_cache() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let dir = env("HF_HUB_CACHE")
        .or_else(|| env("HF_HOME").map(|p| p.join("hub")))
        .or_else(|| env("HOME").map(|p| p.join(".cache/huggingface/hub")))?;
    dir.is_dir().then_some(dir)
}

fn found_file() -> PathBuf {
    data_dir().join("models-found.json")
}

/// Model files `models scan` matched: the pin name and the user's own file, which scriptvox only reads.
fn found_models() -> std::collections::BTreeMap<String, PathBuf> {
    std::fs::read(found_file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// (pin name, path, bytes) of one file found by `models scan`.
pub type FoundModel = (String, PathBuf, u64);

pub fn sha256_of(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect());
        }
        h.update(&buf[..n]);
    }
}

/// `scriptvox models scan`: each file whose size equals a raw model pin and whose sha256 matches is recorded, so that
/// setup skips the download. Files are read, never copied, moved or deleted. Symlinks are skipped, so nothing loops.
/// Returns (pin name, path, bytes) of the matches, and the counts of files seen and hashed.
pub fn scan_models(folder: &Path) -> Result<(Vec<FoundModel>, u64, u64), String> {
    let pins: Vec<Pin> = pins().into_iter().filter(|p| p.raw).collect();
    let (mut seen, mut hashed) = (0u64, 0u64);
    let mut hits: std::collections::BTreeMap<String, (PathBuf, u64)> =
        std::collections::BTreeMap::new();
    let mut stack = vec![(folder.to_path_buf(), 0u32)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let (path, Ok(meta)) = (e.path(), e.file_type()) else {
                continue;
            };
            if meta.is_dir() && depth < 8 {
                stack.push((path, depth + 1));
            } else if meta.is_file() {
                seen += 1;
                let Ok(size) = e.metadata().map(|m| m.len()) else {
                    continue;
                };
                let same_size: Vec<&Pin> = pins.iter().filter(|p| p.size == size).collect();
                if same_size.is_empty() {
                    continue;
                }
                hashed += 1;
                let Ok(sha) = sha256_of(&path) else { continue };
                for p in same_size.into_iter().filter(|p| p.sha256 == sha) {
                    hits.insert(p.name.clone(), (path.clone(), size));
                }
            }
        }
    }
    let mut found = found_models();
    for (name, (path, _)) in &hits {
        found.insert(name.clone(), path.clone());
    }
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(&found).unwrap_or_default();
    std::fs::write(found_file(), body).map_err(|e| e.to_string())?;
    let hits = hits.into_iter().map(|(n, (p, b))| (n, p, b)).collect();
    Ok((hits, seen, hashed))
}

/// Start of an error that a retry cannot fix: then the cause, `|`, and what to do.
pub const STOP: &str = "stop: ";

/// A download that a retry cannot help: a file gone from its host, no access, or TLS that fails (a clock, a proxy, a
/// certificate). A dropped connection is not here: it is retried.
fn stop_cause(e: &ureq::Error, url: &str) -> Option<String> {
    match e {
        ureq::Error::StatusCode(c @ (404 | 410)) => Some(format!(
            "{STOP}the file is gone from its host (HTTP {c}): {url}|This pin needs an update: report it, or update scriptvox."
        )),
        ureq::Error::StatusCode(c @ (401 | 403)) => Some(format!(
            "{STOP}the host refused this file (HTTP {c}): {url}|Check the file's page and any access it asks for."
        )),
        other if { let t = other.to_string().to_lowercase(); t.contains("tls") || t.contains("certificate") } => Some(format!(
            "{STOP}TLS failed to {url}: {other}|Check this machine's date and any proxy; a certificate problem does not go away with a retry."
        )),
        _ => None,
    }
}

/// Downloads, verifies and unpacks a pin. The download goes to `.part`, is
/// checked against the compiled-in sha256 and renamed; a mismatch deletes it.
/// ScriptVox never runs `sudo` and never touches shell profiles.
/// Start of the error for a model behind an Agree button: this, the page, a space, then what the user must do.
pub const GATED: &str = "gated model: ";

/// The model page of a Hugging Face file URL. None for any other host, which never gets the token.
fn hf_page(url: &str) -> Option<String> {
    let mut parts = url.strip_prefix("https://huggingface.co/")?.split('/');
    let (org, repo) = (parts.next()?, parts.next()?);
    (!org.is_empty() && !repo.is_empty()).then(|| format!("https://huggingface.co/{org}/{repo}"))
}

/// Measured 7 Nov 2026: no token and a wrong token both give 401 with `x-error-code: GatedRepo`. A good token that has
/// not agreed was not measured: forum posts say 403. Both ask the same thing of the user.
pub fn gated(page: &str, has_token: bool) -> String {
    let action = if has_token {
        "Open the page and click Agree with the account of your token, then run the same command again. If you agreed already, the token may be wrong: run `scriptvox keys set hf`."
    } else {
        "Open the page and click Agree, run `scriptvox keys set hf`, then run the same command again."
    };
    format!("{GATED}{page} {action}")
}

/// True for an error of the connection itself, which another try can cure. Every such error starts this way.
pub fn network_fault(e: &str) -> bool {
    // uv's own last line: a disk or a folder on this machine is not the network.
    let low = e.to_lowercase();
    let local = [
        "no space left",
        "permission denied",
        "read-only file system",
        "disk quota",
    ];
    e.starts_with("download of ")
        || (e.starts_with("preparing the voice worker") && !local.iter().any(|w| low.contains(w)))
}

/// One connection: what is new arrives at the end of `part`. A break keeps what came, so the next try asks for the rest.
fn fetch_once(
    agent: &ureq::Agent,
    p: &Pin,
    part: &Path,
    have: u64,
    token: Option<&str>,
) -> Result<(), String> {
    use std::io::{Read, Write};
    let page = hf_page(&p.url);
    let mut req = agent.get(&p.url);
    // Only to huggingface.co. A redirect to its file host drops this header (ureq's default).
    if let Some(token) = page.as_ref().and(token) {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(401 | 403)) if page.is_some() => {
            return Err(gated(page.as_deref().unwrap_or(""), token.is_some()));
        }
        Err(ureq::Error::StatusCode(416)) if have > 0 => {
            let _ = std::fs::remove_file(part);
            return Err(format!(
                "download of {} failed: HTTP 416 for the kept part",
                p.name
            ));
        }
        Err(e) => {
            return Err(stop_cause(&e, &p.url)
                .unwrap_or_else(|| format!("download of {} failed: {e}", p.name)));
        }
    };
    // A host that ignores Range sends the whole file with 200.
    let append = have > 0 && resp.status().as_u16() == 206;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(|e| e.to_string())?;
    let mut body = resp.body_mut().with_config().limit(p.size + 1024).reader();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = body
            .read(&mut buf)
            .map_err(|e| format!("download of {} broke: {e}", p.name))?;
        if n == 0 {
            return Ok(());
        }
        out.write_all(&buf[..n]).map_err(|e| {
            let _ = std::fs::remove_file(part);
            if e.kind() == std::io::ErrorKind::StorageFull {
                format!("{STOP}the disk is full while downloading {} ({} MB)|Free some space and run the same command again.", p.name, p.size / 1_000_000)
            } else {
                e.to_string()
            }
        })?;
    }
}

/// Fetches `p` into `part`. This network drops connections now and then: 3 tries in a row without a new byte end it.
fn fetch_part(p: &Pin, part: &Path, progress: &dyn Fn(&str)) -> Result<(), String> {
    use std::time::Duration;
    let token = crate::keys::resolve("hf").map(|(t, _)| t);
    // Per read, not for the whole file: 5 GB may take hours, but a silent connection must not hang forever.
    let idle = std::env::var("SCRIPTVOX_DOWNLOAD_IDLE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(idle)))
        .build()
        .new_agent();
    let size_of = || std::fs::metadata(part).map_or(0, |m| m.len());
    let mut tries = 0;
    loop {
        let have = size_of();
        if have > 0 && have == p.size {
            return Ok(());
        }
        match fetch_once(&agent, p, part, have, token.as_deref()) {
            Ok(()) => return Ok(()),
            Err(e) if network_fault(&e) && tries < 2 => {
                tries = if size_of() > have { 0 } else { tries + 1 };
                progress(&format!("{e}, retrying ({tries} of 2 without a new byte)"));
                std::thread::sleep(Duration::from_secs(2 * tries.max(1)));
            }
            Err(e) => return Err(e),
        }
    }
}

pub fn install(p: &Pin, progress: &dyn Fn(&str)) -> Result<Vec<PathBuf>, String> {
    if let Some(b) = installed(p) {
        return Ok(b);
    }
    // A pin with no public address yet (the Arabic speech model): never a silent skip.
    if p.url.is_empty() {
        return Err(format!("{STOP}{} has no download address yet|Give a local folder with its files: `--from <folder>` (`scriptvox setup speech-local --arabic --from <folder>`).", p.name));
    }
    let dl = data_dir().join("downloads");
    std::fs::create_dir_all(&dl).map_err(|e| format!("cannot create {}: {e}", dl.display()))?;
    let file = p.url.rsplit('/').next().unwrap_or("download").to_string();
    // One install of a file at a time: a second setup or make waits, then finds it installed. A dead process frees it.
    let lock = std::fs::File::create(dl.join(format!("{file}.lock"))).map_err(|e| e.to_string())?;
    if lock.try_lock().is_err() {
        progress(&format!(
            "another scriptvox is installing {}, waiting",
            p.name
        ));
        let _ = lock.lock();
    }
    if let Some(b) = installed(p) {
        return Ok(b);
    }
    let part = dl.join(crate::http_rows::part_name(&p.url));
    let archive = dl.join(&file);
    progress(&format!(
        "downloading {} {} ({} MB)",
        p.name,
        p.version,
        p.size / 1_000_000
    ));
    // An empty sha256: the host publishes none for this file, the caller records what it observed. Then nothing could
    // prove that bytes kept from an earlier run belong to this file.
    if p.sha256.is_empty() {
        let _ = std::fs::remove_file(&part);
    }
    let kept = part.exists();
    fetch_part(p, &part, progress).or_else(|e| {
        if p.mirror.is_empty() {
            return Err(e);
        }
        progress(&format!("{e}; trying the second source"));
        let b = Pin {
            url: p.mirror.clone(),
            ..p.clone()
        };
        fetch_part(&b, &part, progress)
    })?;
    let mut got = sha256_of(&part).map_err(|e| e.to_string())?;
    if kept && !p.sha256.is_empty() && got != p.sha256 {
        progress(&format!(
            "the kept part of {} is bad, downloading it again",
            p.name
        ));
        let _ = std::fs::remove_file(&part);
        fetch_part(p, &part, progress)?;
        got = sha256_of(&part).map_err(|e| e.to_string())?;
    }
    if !p.sha256.is_empty() && got != p.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "sha256 mismatch for {}: expected {}, got {got}. The file was deleted.",
            p.name, p.sha256
        ));
    }
    std::fs::rename(&part, &archive).map_err(|e| e.to_string())?;
    if p.raw {
        let dest = tool_dir(p);
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        std::fs::rename(&archive, dest.join(&p.bins[0])).map_err(|e| e.to_string())?;
        return installed(p).ok_or_else(|| format!("{} is not complete", p.name));
    }
    let dest = tool_dir(p);
    let tmp = dest.with_extension(format!("tmp{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let unpacked = if file.ends_with(".zip") && p.strip == 0 {
        // GNU tar cannot read a zip, and the Linux zip pins have their files at the top.
        std::fs::File::open(&archive)
            .map_err(|e| e.to_string())
            .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()))
            .and_then(|mut z| z.extract(&tmp).map_err(|e| e.to_string()))
            .is_ok()
    } else {
        // System tar reads .tar.xz, .tar.gz on Linux, macOS and Windows 10+, and .zip on Windows 10+ and macOS.
        std::process::Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg(format!("--strip-components={}", p.strip))
            .arg("-C")
            .arg(&tmp)
            .status()
            .map_err(|e| format!("cannot run tar: {e}"))?
            .success()
    };
    let _ = std::fs::remove_file(&archive);
    if !unpacked {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("cannot unpack {}", file));
    }
    if let Some(missing) = p.bins.iter().find(|b| !tmp.join(b).is_file()) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("{} has no {missing}", file));
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    installed(p).ok_or_else(|| format!("{} is not complete after unpacking", p.name))
}

/// Licence class from `ffmpeg -version`: gpl, lgpl or nonfree.
pub fn ffmpeg_class(ffmpeg: &Path) -> Result<&'static str, String> {
    let out = std::process::Command::new(ffmpeg)
        .arg("-version")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", ffmpeg.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let conf = text
        .lines()
        .find(|l| l.starts_with("configuration:"))
        .unwrap_or("");
    Ok(if conf.contains("--enable-nonfree") {
        "nonfree"
    } else if conf.contains("--enable-gpl") {
        "gpl"
    } else {
        "lgpl"
    })
}

/// Pins a run would download now: (pin, why). FFmpeg is skipped when
/// `SCRIPTVOX_FFMPEG` is set, or `--use-system` finds one; uv when `SCRIPTVOX_UV`
/// is set or uv is on PATH.
pub fn pending(use_system: bool, needs_worker: bool) -> Vec<Pin> {
    let mut out = Vec::new();
    let ff_env = std::env::var_os("SCRIPTVOX_FFMPEG").is_some_and(|p| Path::new(&p).is_file());
    let ff_sys = use_system && find_on_path("ffmpeg").is_some();
    if let Some(p) = pin("ffmpeg").filter(|p| installed(p).is_none() && !ff_env && !ff_sys) {
        out.push(p);
    }
    let uv_ok = std::env::var_os("SCRIPTVOX_UV").is_some() || find_on_path("uv").is_some();
    if let Some(p) = pin("uv").filter(|p| needs_worker && installed(p).is_none() && !uv_ok) {
        out.push(p);
    }
    out
}

/// Above this, an agent gets NEEDS_INPUT instead of a silent download.
pub const AGENT_DOWNLOAD_LIMIT: u64 = 200_000_000;

/// Builds the worker environment (`uv sync --frozen`) as its own step, before
/// the first worker start: on a slow network this can take minutes, which must
/// not count against the RPC hello timeout. A stamp file makes later runs skip it.
pub fn prepare_worker(dir: &Path, progress: &dyn Fn(&str)) -> Result<(), String> {
    let stamp = dir.join(".scriptvox-ready");
    if stamp.is_file() || std::env::var_os("SCRIPTVOX_WORKER_SRC").is_some() {
        return Ok(());
    }
    let cmd = worker_command(dir)?;
    let uv = cmd.get_program().to_owned();
    let mut last = String::new();
    for attempt in 1..=3 {
        progress(&format!(
            "preparing the voice worker (Python 3.12 and packages, about 100 MB on disk, once){}",
            if attempt > 1 {
                format!(", try {attempt} of 3")
            } else {
                String::new()
            }
        ));
        let mut sync = Command::new(&uv);
        sync.args([
            "sync",
            "--quiet",
            "--frozen",
            "--no-dev",
            "--python",
            "3.12",
            "--project",
        ])
        .arg(dir);
        for (k, v) in cmd.get_envs() {
            match v {
                Some(v) => sync.env(k, v),
                None => sync.env_remove(k),
            };
        }
        match sync.output() {
            Ok(o) if o.status.success() => {
                let _ = std::fs::write(&stamp, env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Ok(o) => {
                last = String::from_utf8_lossy(&o.stderr)
                    .lines()
                    .last()
                    .unwrap_or("uv sync failed")
                    .to_string()
            }
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
    }
    Err(format!("preparing the voice worker failed: {last}"))
}

// ---------------------------------------------------------------- Kokoro plugin

macro_rules! plugin_files {
    ($($p:literal),* $(,)?) => { &[$(($p, include_str!(concat!("../../../plugins/kokoro/", $p)))),*] };
}

/// The Kokoro plugin sources (GPL-3.0-or-later; a separate process).
pub const KOKORO_FILES: &[(&str, &str)] = plugin_files![
    "pyproject.toml",
    "uv.lock",
    "scriptvox_kokoro/__init__.py",
    "scriptvox_kokoro/__main__.py"
];

pub fn kokoro_dir() -> PathBuf {
    data_dir().join("plugins").join("kokoro").join(format!(
        "{}-{:016x}",
        env!("CARGO_PKG_VERSION"),
        fnv(KOKORO_FILES)
    ))
}

/// espeak-ng keeps its data path in a 160-byte buffer. Measured 5 Nov 2026 on Linux: 159 bytes speak, 160 crash the worker.
const ESPEAK_PATH_MAX: usize = 159;

/// When the plugin's espeak data path is over the limit: bytes over, and the longest data folder that fits. The venv
/// layout differs on Windows, 11 bytes shorter (limit computed per OS, not run on Windows).
pub fn kokoro_path_excess() -> Option<(usize, usize)> {
    let venv = if cfg!(windows) {
        ".venv/Lib/site-packages"
    } else {
        ".venv/lib/python3.12/site-packages"
    };
    let dir = kokoro_dir();
    let tail = dir.join(venv).join("espeakng_loader/espeak-ng-data");
    let over = tail.as_os_str().len().checked_sub(ESPEAK_PATH_MAX)?;
    let under_data = tail.as_os_str().len() - data_dir().as_os_str().len();
    (over > 0).then(|| (over, ESPEAK_PATH_MAX - under_data))
}

/// True when `setup voice-local` has run: plugin unpacked and prepared, model files present.
pub fn kokoro_ready() -> bool {
    kokoro_dir().join(".scriptvox-ready").is_file()
        && ["kokoro-model", "kokoro-voices"]
            .iter()
            .all(|n| pin(n).and_then(|p| installed(&p)).is_some())
}

/// The plugin process command: the worker's RPC module and the plugin on the path.
pub fn kokoro_command() -> Result<Command, String> {
    let wdir = worker_dir()?;
    let pdir = kokoro_dir();
    let mut c = plugin_command(&pdir, "scriptvox_kokoro")?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    c.env(
        "PYTHONPATH",
        format!("{}{sep}{}", wdir.display(), pdir.display()),
    );
    let file = |n: &str| {
        pin(n)
            .and_then(|p| installed(&p))
            .map(|b| b[0].clone())
            .unwrap_or_default()
    };
    c.env("SCRIPTVOX_KOKORO_MODEL", file("kokoro-model"))
        .env("SCRIPTVOX_KOKORO_VOICES", file("kokoro-voices"));
    Ok(c)
}

/// The worker's command for a plugin folder, running the plugin's own module.
fn plugin_command(dir: &Path, module: &str) -> Result<Command, String> {
    let cmd = worker_command(dir)?;
    let mut c = Command::new(cmd.get_program());
    for a in cmd.get_args() {
        c.arg(if a == "scriptvox_worker" {
            std::ffi::OsStr::new(module)
        } else {
            a
        });
    }
    for (k, v) in cmd.get_envs() {
        if let Some(v) = v {
            c.env(k, v);
        }
    }
    Ok(c)
}

/// `setup voice-local`: download the pinned model files, unpack and prepare the plugin.
pub fn install_kokoro(progress: &dyn Fn(&str)) -> Result<(), String> {
    for n in ["kokoro-model", "kokoro-voices"] {
        let p = pin(n).ok_or(format!("no pin for {n}"))?;
        install(&p, progress)?;
    }
    let dir = kokoro_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("LICENSE"), crate::licenses::GPL).map_err(|e| e.to_string())?;
    for (rel, body) in KOKORO_FILES {
        let f = dir.join(rel);
        std::fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(f, body).map_err(|e| e.to_string())?;
    }
    worker_dir()?;
    prepare_worker(&dir, progress)
}

// ---------------------------------------------------------------- Local speech to text plugin

/// The local speech to text plugin sources (Apache-2.0; a separate process only for its size).
pub const WHISPER_FILES: &[(&str, &str)] = &[
    (
        "pyproject.toml",
        include_str!("../../../plugins/whisper/pyproject.toml"),
    ),
    ("uv.lock", include_str!("../../../plugins/whisper/uv.lock")),
    (
        "scriptvox_whisper/__init__.py",
        include_str!("../../../plugins/whisper/scriptvox_whisper/__init__.py"),
    ),
    (
        "scriptvox_whisper/__main__.py",
        include_str!("../../../plugins/whisper/scriptvox_whisper/__main__.py"),
    ),
];

/// The pinned model files, kept together in one folder for the model loader.
pub const WHISPER_PINS: [&str; 4] = [
    "whisper-small-model",
    "whisper-small-config",
    "whisper-small-tokenizer",
    "whisper-small-vocabulary",
];

pub fn whisper_dir() -> PathBuf {
    data_dir().join("plugins").join("whisper").join(format!(
        "{}-{:016x}",
        env!("CARGO_PKG_VERSION"),
        fnv(WHISPER_FILES)
    ))
}

/// The larger model of `setup speech-large`, in its own folder of the same plugin.
pub const WHISPER_LARGE_PINS: [&str; 5] = [
    "whisper-turbo-model",
    "whisper-turbo-config",
    "whisper-turbo-preprocessor",
    "whisper-turbo-tokenizer",
    "whisper-turbo-vocabulary",
];

/// Free memory the larger model needs, or the small one runs if it is there. Peak measured 7 Nov: 2.05 GB, 29 s of Arabic.
pub const WHISPER_LARGE_MIN_BYTES: u64 = 3_000_000_000;

fn pins_there(names: &[&str]) -> bool {
    names
        .iter()
        .all(|n| pin(n).and_then(|p| installed(&p)).is_some())
}

fn whisper_prepared() -> bool {
    whisper_dir().join(".scriptvox-ready").is_file()
}

/// The tool files and their link in this plugin folder: a new version's folder has no model link until setup runs
/// again, and the plugin would then fail on a model that "is installed".
fn whisper_model_ready(pins: &[&str], folder: &str) -> bool {
    whisper_prepared() && pins_there(pins) && whisper_dir().join(folder).join("model.bin").is_file()
}

pub fn whisper_small_ready() -> bool {
    whisper_model_ready(&WHISPER_PINS, "model")
}

pub fn whisper_large_ready() -> bool {
    whisper_model_ready(&WHISPER_LARGE_PINS, "model-large")
}

/// The Arabic model of `setup speech-local --arabic` (docs/specs/speech-local-arabic.md), in `model-ar`.
pub const WHISPER_ARABIC_PINS: [&str; 5] = [
    "whisper-ar-model",
    "whisper-ar-config",
    "whisper-ar-preprocessor",
    "whisper-ar-tokenizer",
    "whisper-ar-vocabulary",
];

pub fn whisper_arabic_ready() -> bool {
    whisper_model_ready(&WHISPER_ARABIC_PINS, "model-ar")
}

/// The model folder for a request's language. The Arabic model is tested only with the language given, so it runs
/// only for an `ar` tag; any other language and auto-detect (None) keep the present model.
pub fn whisper_model_folder(lang: Option<&str>, arabic: bool, large: bool) -> &'static str {
    let ar = lang.is_some_and(|l| {
        l.split(['-', '_'])
            .next()
            .is_some_and(|t| t.eq_ignore_ascii_case("ar"))
    });
    if arabic && ar {
        "model-ar"
    } else if large {
        "model-large"
    } else {
        "model"
    }
}

/// The folder that runs now for `lang`.
pub fn whisper_folder_now(lang: Option<&str>) -> &'static str {
    whisper_model_folder(lang, whisper_arabic_ready(), use_large_now())
}

/// Checks the files of `pins` in `from` (size, then sha256) and copies them to their tool folders. Nothing is copied
/// unless all match. The source folder is only read.
pub fn install_from_folder(pins: &[&str], from: &Path) -> Result<(), String> {
    let mut todo = Vec::new();
    for n in pins {
        let p = pin(n).ok_or(format!("no pin for {n}"))?;
        let f = from.join(&p.bins[0]);
        let size = std::fs::metadata(&f).map(|m| m.len()).map_err(|_| {
            format!(
                "{STOP}{} has no file {}|Give the folder that holds all {} files: {}.",
                from.display(),
                p.bins[0],
                pins.len(),
                names(pins)
            )
        })?;
        let got = if size == p.size {
            sha256_of(&f).map_err(|e| e.to_string())?
        } else {
            format!("{size} bytes")
        };
        if got != p.sha256 {
            let want = if size == p.size {
                p.sha256.clone()
            } else {
                format!("{} bytes", p.size)
            };
            return Err(format!(
                "{STOP}{} does not match its pin ({}): expected {want}, got {got}. Nothing was copied.|Copy the files of record into the folder again, or give another folder with `--from`.",
                p.bins[0],
                f.display()
            ));
        }
        todo.push((p, f));
    }
    for (p, f) in todo {
        let dest = tool_dir(&p);
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        let tmp = dest.join(format!("{}.part", p.bins[0]));
        std::fs::copy(&f, &tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dest.join(&p.bins[0])).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn names(pins: &[&str]) -> String {
    pins.iter()
        .filter_map(|n| pin(n))
        .map(|p| p.bins[0].clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// True when a local speech model can run: `setup speech-local` or `setup speech-large` has run.
pub fn whisper_ready() -> bool {
    whisper_small_ready() || whisper_large_ready()
}

/// The larger model when it is there and free memory holds it. With only one model there is no choice.
pub fn whisper_use_large(large: bool, small: bool, free_bytes: Option<u64>) -> bool {
    large && (!small || free_bytes.is_none_or(|f| f >= WHISPER_LARGE_MIN_BYTES))
}

fn use_large_now() -> bool {
    whisper_use_large(
        whisper_large_ready(),
        whisper_small_ready(),
        crate::doctor::meminfo("MemAvailable:"),
    )
}

/// The name of the row that runs now, so a transcript of one model is not cached as the other's.
pub fn whisper_row_name() -> &'static str {
    if use_large_now() {
        "whisper-large-v3-turbo-local"
    } else {
        "whisper-small-local"
    }
}

/// The plugin process command, with the model folder (`whisper_folder_now`) in the environment.
pub fn whisper_command(folder: &str) -> Result<Command, String> {
    let dir = whisper_dir();
    let mut c = plugin_command(&dir, "scriptvox_whisper")?;
    c.env("PYTHONPATH", &dir)
        .env("SCRIPTVOX_WHISPER_MODEL", dir.join(folder));
    Ok(c)
}

/// `setup speech-local` and `setup speech-large`: download one model's pinned files, unpack and prepare the plugin.
pub fn install_whisper(pins: &[&str], folder: &str, progress: &dyn Fn(&str)) -> Result<(), String> {
    let dir = whisper_dir();
    let model = dir.join(folder);
    std::fs::create_dir_all(&model).map_err(|e| e.to_string())?;
    for n in pins {
        let p = pin(n).ok_or(format!("no pin for {n}"))?;
        let bin = install(&p, progress)?.remove(0);
        // The pin's own name: a scanned file may have another (a cache blob).
        let to = model.join(p.bins[0].as_str());
        // A link costs no second copy of the model; another drive falls back to a copy.
        let _ = std::fs::remove_file(&to);
        std::fs::hard_link(&bin, &to)
            .or_else(|_| std::fs::copy(&bin, &to).map(|_| ()))
            .map_err(|e| e.to_string())?;
    }
    for (rel, body) in WHISPER_FILES {
        let f = dir.join(rel);
        std::fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(f, body).map_err(|e| e.to_string())?;
    }
    prepare_worker(&dir, progress)
}

// ---------------------------------------------------------------- Local image model

/// The pinned files of `setup image-local`, the build first: if it does not start here, no 5 GB comes down.
pub const IMAGE_PINS: [&str; 4] = ["sd-cli", "klein-4b-dit", "klein-4b-text", "klein-4b-vae"];

/// The installed file of a pin.
pub fn pin_file(name: &str) -> Option<PathBuf> {
    pin(name)
        .and_then(|p| installed(&p))
        .map(|mut b| b.remove(0))
}

/// True when `setup image-local` has run: the build and the 3 model files are there.
pub fn image_ready() -> bool {
    IMAGE_PINS.iter().all(|n| pin_file(n).is_some())
}

/// `sd-cli --version`: the upstream Linux build needs a new glibc, so an old system fails here, with its reason.
pub fn sd_cli_starts(bin: &Path) -> Result<(), String> {
    let o = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&o.stderr);
    Err(text.lines().next().unwrap_or("no message").to_string())
}

#[cfg(test)]
mod gated_tests {
    use super::*;

    #[test]
    fn the_large_model_runs_only_when_memory_holds_it() {
        let free = Some(WHISPER_LARGE_MIN_BYTES);
        assert!(whisper_use_large(true, true, free));
        assert!(!whisper_use_large(true, true, Some(1_000_000_000)));
        assert!(
            whisper_use_large(true, false, Some(1_000_000_000)),
            "only one model: it runs"
        );
        assert!(whisper_use_large(true, true, None), "memory not read");
        assert!(!whisper_use_large(false, true, free));
    }

    #[test]
    fn the_arabic_model_runs_only_for_arabic() {
        for l in ["ar", "AR", "ar-AE", "ar_SA"] {
            assert_eq!(
                whisper_model_folder(Some(l), true, false),
                "model-ar",
                "{l}"
            );
            assert_eq!(whisper_model_folder(Some(l), true, true), "model-ar", "{l}");
        }
        assert_eq!(whisper_model_folder(Some("ar"), false, false), "model");
        assert_eq!(whisper_model_folder(Some("ar"), false, true), "model-large");
        for l in [Some("en"), Some("arn"), Some("fr"), None] {
            assert_eq!(whisper_model_folder(l, true, false), "model", "{l:?}");
            assert_eq!(whisper_model_folder(l, true, true), "model-large", "{l:?}");
        }
    }

    #[test]
    fn the_built_in_lock_parses_with_the_arabic_rows() {
        let all: Vec<Pin> = toml::from_str::<Toolchain>(TOOLCHAIN).unwrap().tool;
        assert!(all.len() > 20, "the lock parses whole");
        for n in WHISPER_ARABIC_PINS {
            let p = all.iter().find(|p| p.name == n).expect(n);
            assert!(p.raw && p.sha256.len() == 64 && p.size > 0, "{n}");
        }
    }

    #[test]
    fn only_a_hugging_face_url_has_a_page() {
        assert_eq!(
            hf_page("https://huggingface.co/org/model/resolve/abc/f.gguf").as_deref(),
            Some("https://huggingface.co/org/model")
        );
        for other in [
            "https://github.com/org/model/releases/f.zip",
            "https://huggingface.co.evil.example/org/model/f",
            "http://huggingface.co/org/model/f",
            "https://huggingface.co/org",
        ] {
            assert_eq!(hf_page(other), None, "{other}");
        }
    }

    #[test]
    fn the_message_says_what_to_do_with_and_without_a_token() {
        assert!(gated("p", false).contains("click Agree, run `scriptvox keys set hf`"));
        assert!(gated("p", true).contains("the token may be wrong"));
    }
}
