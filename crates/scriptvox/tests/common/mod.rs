//! Shared stub environment: a copy of the real worker whose `tts.edge` writes
//! fixture audio (no network). Needs uv and SCRIPTVOX_FFMPEG.
#![allow(dead_code)]
use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Tests share the user's uv cache: a fresh Python and package cache per test
/// would download everything again for every test.
pub fn shared_uv(c: &mut Command) {
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        c.env(
            "UV_CACHE_DIR",
            std::env::var_os("UV_CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| h.join(".cache/uv")),
        );
        c.env(
            "UV_PYTHON_INSTALL_DIR",
            std::env::var_os("UV_PYTHON_INSTALL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| h.join(".local/share/uv/python")),
        );
    }
}

/// What every child `scriptvox` of a test gets, so that `cargo test` gives one result with and without keys in the
/// shell (`docs/specs/contributor-ci.md`): no key variable (the built-in names and the `[[key]]` names of the user's
/// generators.toml), no rows file of the user, and a config and a data folder that are not the user's. A test sets
/// what it needs after this.
fn clean_env() -> (Vec<String>, [(&'static str, PathBuf); 2]) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let mut gone: Vec<String> = scriptvox::keys::all().into_iter().map(|(_, v)| v).collect();
    gone.push("SCRIPTVOX_GENERATORS".into());
    let dirs = [
        ("SCRIPTVOX_CONFIG", tmp.join("test-config")),
        ("SCRIPTVOX_DATA", tmp.join("test-data")),
    ];
    for (_, d) in &dirs {
        std::fs::create_dir_all(d).unwrap();
    }
    (gone, dirs)
}

/// The `scriptvox` binary with the clean environment. Use this, never `Command::cargo_bin("scriptvox")`.
pub fn bin() -> Command {
    let mut c = Command::cargo_bin("scriptvox").unwrap();
    let (gone, dirs) = clean_env();
    for k in gone {
        c.env_remove(k);
    }
    c.envs(dirs);
    c
}

/// The same for a test that needs a `std::process::Command` (a child it leaves, a pipe it holds).
pub fn std_bin() -> std::process::Command {
    let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin("scriptvox"));
    clean(&mut c);
    c
}

/// The path of the binary, for a shell line. Give that shell `clean`.
pub fn bin_path() -> PathBuf {
    assert_cmd::cargo::cargo_bin("scriptvox")
}

/// The clean environment on any command that starts `scriptvox` (a shell that calls it).
pub fn clean(c: &mut std::process::Command) {
    let (gone, dirs) = clean_env();
    for k in gone {
        c.env_remove(k);
    }
    c.envs(dirs);
}

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The FFmpeg that the binary under test also gets, with no other tool. A test that serves media the binary checks with
/// FFmpeg must ask here: `ready()` also needs uv, and a machine with FFmpeg and no uv got fake bytes that the binary
/// then refused (7 `generate` tests on the macOS runner, 14 Nov 2026).
pub fn ffmpeg_only() -> Option<String> {
    std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
}

pub fn ready() -> Option<String> {
    let ff = ffmpeg_only()?;
    std::process::Command::new("uv")
        .arg("--version")
        .output()
        .ok()?;
    Some(ff)
}

/// Stub tool: `mode` = ok | refuse | flaky.
pub const STUB: &str = r#"
import json, os, shutil
from pathlib import Path
from .rpc import ToolError, tool

@tool("tts.edge", network=True, cost="free", deterministic=False, timing="exact",
      terms="unofficial, personal or non-commercial, may break")
def synthesize(input, ctx):
    mode = os.environ.get("STUB_MODE", "ok")
    if mode == "refuse":
        raise ToolError(-32010, f"Edge refused voice {input['voice']} (403)")
    if mode == "slow":
        import time
        time.sleep(8)
    if mode == "flaky":
        raise ToolError(-32011, "Edge TTS failed: timeout")
    out = Path(input["out_dir"])
    shutil.copy(os.environ["STUB_AUDIO"], out / "voice.mp3")
    words = [{"text": w.strip(".,"), "start": round(0.1 + i * 0.35, 3), "end": round(0.4 + i * 0.35, 3)}
             for i, w in enumerate(input["text"].split())]
    (out / "words.json").write_text(json.dumps(words, ensure_ascii=False), encoding="utf-8")
    # The speaking rate this call got, for tests/voice_rate.rs (the stub's audio does not change with it).
    (out / "rate.txt").write_text(input.get("rate", ""), encoding="utf-8")
    # The text this call got, for tests/delivery_cues.rs.
    (out / "text.txt").write_text(input["text"], encoding="utf-8")
    return {"artifacts": {"audio": str(out / "voice.mp3"), "words": str(out / "words.json")}, "timing": "exact", "cost_usd": 0}
"#;

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap();
        if [
            ".venv",
            "__pycache__",
            ".pytest_cache",
            "cassettes",
            "tests",
        ]
        .iter()
        .any(|s| name == *s)
        {
            continue;
        }
        if p.is_dir() {
            copy_dir(&p, &to.join(name))
        } else {
            std::fs::copy(&p, to.join(name)).unwrap();
        }
    }
}

pub struct Env {
    pub dir: tempfile::TempDir,
}

impl Env {
    pub fn new(ff: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        copy_dir(&root().join("python"), &dir.path().join("worker"));
        std::fs::write(dir.path().join("worker/scriptvox_worker/tts_edge.py"), STUB).unwrap();
        let st = std::process::Command::new(ff)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=f=330:d=3",
                "-c:a",
                "libmp3lame",
                "-y",
            ])
            .arg(dir.path().join("voice.mp3"))
            .status()
            .unwrap();
        assert!(st.success());
        Self { dir }
    }

    pub fn make(&self, script: &str, mode: &str, extra: &[&str]) -> (i32, Value) {
        let p = self.dir.path();
        std::fs::write(p.join("s.txt"), script).unwrap();
        let mut c = bin();
        shared_uv(&mut c);
        let out = c
            .current_dir(p)
            .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            // Its own config too: a user `stt` row in ~/.config/scriptvox would hear the stub tone and fail gate `words`.
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("STUB_AUDIO", p.join("voice.mp3"))
            .env("STUB_MODE", mode)
            .args(["make", "--script", "s.txt", "--json"])
            .args(extra)
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            serde_json::from_slice(&out.stdout).unwrap(),
        )
    }
}

impl Env {
    /// A scriptvox command in the env folder, with the stub worker and its own data folder.
    pub fn cmd(&self, mode: &str) -> Command {
        let p = self.dir.path();
        let mut c = bin();
        shared_uv(&mut c);
        c.current_dir(p)
            .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            // Its own config too: a user `stt` row in ~/.config/scriptvox would hear the stub tone and fail gate `words`.
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("STUB_AUDIO", p.join("voice.mp3"))
            .env("STUB_MODE", mode);
        c
    }
}

/// Copies fixtures/media into the env as `media/`.
pub fn with_media(e: &Env) {
    let dst = e.dir.path().join("media");
    std::fs::create_dir_all(&dst).unwrap();
    for f in std::fs::read_dir(root().join("fixtures/media")).unwrap() {
        let p = f.unwrap().path();
        std::fs::copy(&p, dst.join(p.file_name().unwrap())).unwrap();
    }
}

/// A small fake job queue for a row that takes a clip and gives one back (matte, upscale). The same protocol as the
/// fake in `tests/generate.rs`: a POST is a submit (counted), then the status, the result JSON and the file.
pub struct Queue {
    pub port: u16,
    pub submits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// The bytes served as the result clip.
    pub clip: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

pub fn queue() -> Queue {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::sync::atomic::Ordering;
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let submits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clip = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (s, c) = (submits.clone(), clip.clone());
    std::thread::spawn(move || {
        for stream in l.incoming().flatten() {
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).is_err() {
                continue;
            }
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap_or(0);
                if h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            let _ = r.read_exact(&mut body);
            let base = format!("http://127.0.0.1:{port}");
            let (ctype, out): (&str, Vec<u8>) = if line.starts_with("POST ") {
                s.fetch_add(1, Ordering::SeqCst);
                let sent: Value = serde_json::from_slice(&body).unwrap();
                assert!(
                    sent["video_url"]
                        .as_str()
                        .is_some_and(|u| u.starts_with("data:video/mp4;base64,")),
                    "the clip is sent"
                );
                ("application/json", format!(r#"{{"request_id":"r1","status_url":"{base}/q/requests/r1/status","response_url":"{base}/q/requests/r1","cancel_url":"{base}/q/requests/r1/cancel"}}"#).into_bytes())
            } else if line.contains("/status") {
                ("application/json", br#"{"status":"COMPLETED"}"#.to_vec())
            } else if line.contains("/q/requests/r1 ") {
                (
                    "application/json",
                    format!(r#"{{"video":{{"url":"{base}/media/clip.mp4"}}}}"#).into_bytes(),
                )
            } else {
                ("video/mp4", c.lock().unwrap().clone())
            };
            let mut w = stream;
            let _ = write!(w, "HTTP/1.1 200 X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
            let _ = w.write_all(&out);
        }
    });
    Queue {
        port,
        submits,
        clip,
    }
}

/// The built-in rows for a test folder, with the fake queue's host. `change` edits the text first (a price, say).
pub fn rows(dir: &Path, change: impl Fn(String) -> String) {
    let rows = std::fs::read_to_string(root().join("registry/generators.toml")).unwrap();
    std::fs::write(
        dir.join("gen.toml"),
        change(rows)
            .replace(
                r#"hosts = ["gen.example.invalid"]"#,
                r#"hosts = ["127.0.0.1"]"#,
            )
            // The test queue is plain loopback http: no private CA.
            .replace("ca = \"selfhost\"\n", ""),
    )
    .unwrap();
}

/// `text` with the first `from` after the row `name` replaced by `to`: one field of one row.
pub fn row_field(text: String, name: &str, from: &str, to: &str) -> String {
    let at = text.find(&format!("name = \"{name}\"")).unwrap();
    let (head, tail) = text.split_at(at);
    format!("{head}{}", tail.replacen(from, to, 1))
}

/// Runs FFmpeg in `dir`; the test stops if it fails.
pub fn ffmpeg(ff: &str, dir: &Path, args: &[&str]) {
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "{args:?}");
}

/// One frame of a video at `at` seconds as RGBA bytes of `w` x `h`.
pub fn frame(ff: &str, video: &Path, at: f64, (w, h): (usize, usize)) -> Vec<[u8; 4]> {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-ss", &at.to_string(), "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let px = o.stdout.as_chunks::<4>().0.to_vec();
    assert_eq!(px.len(), w * h, "{}", video.display());
    px
}

/// One ffprobe answer for a file, for example `stream=pix_fmt`.
pub fn probe(ff: &str, file: &Path, select: &str, entry: &str) -> String {
    let o = std::process::Command::new(Path::new(ff).with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-select_streams",
            select,
            "-show_entries",
            entry,
        ])
        .args(["-of", "default=nw=1:nk=1"])
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// An FFmpeg and ffprobe in `dir` that write every argument list to a file, then run the real ones.
/// Returns the new `ffmpeg` path and that file.
pub fn record(ff: &str, dir: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("rec");
    std::fs::create_dir_all(&bin).unwrap();
    let log = dir.join("rec.log");
    for tool in ["ffmpeg", "ffprobe"] {
        let real = if tool == "ffmpeg" {
            Path::new(ff).to_path_buf()
        } else {
            Path::new(ff).with_file_name(tool)
        };
        std::fs::write(
            bin.join(tool),
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                log.display(),
                real.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(bin.join(tool), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (bin.join("ffmpeg"), log)
}
