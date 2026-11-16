//! Provisioner (RVX-018) against a local HTTP server and test archives.
#![cfg(unix)]
mod common;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Serves one body for every request; counts the hits.
fn serve(body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/ffmpeg.tar.gz", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            h.fetch_add(1, Ordering::SeqCst);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(&body);
        }
    });
    (url, hits)
}

/// A tar.gz with pkg/bin/{ffmpeg,ffprobe}; `-version` prints `conf`.
fn archive(dir: &Path, conf: &str) -> Vec<u8> {
    let bin = dir.join("src/pkg/bin");
    std::fs::create_dir_all(&bin).unwrap();
    for name in ["ffmpeg", "ffprobe"] {
        let f = bin.join(name);
        std::fs::write(
            &f,
            format!("#!/bin/sh\necho 'ffmpeg version test'\necho 'configuration: {conf}'\n"),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = dir.join("a.tar.gz");
    assert!(std::process::Command::new("tar")
        .arg("-czf")
        .arg(&out)
        .arg("-C")
        .arg(dir.join("src"))
        .arg("pkg")
        .status()
        .unwrap()
        .success());
    std::fs::read(out).unwrap()
}

fn target() -> String {
    format!(
        "{}-{}",
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        },
        std::env::consts::ARCH
    )
}

struct Case {
    dir: tempfile::TempDir,
    hits: Arc<AtomicUsize>,
}

impl Case {
    fn new(conf: &str, wrong_sha: bool) -> Self {
        Self::serving(conf, wrong_sha, serve)
    }

    fn serving(
        conf: &str,
        wrong_sha: bool,
        server: impl FnOnce(Vec<u8>) -> (String, Arc<AtomicUsize>),
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let body = archive(dir.path(), conf);
        let sha: String = Sha256::digest(&body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let sha = if wrong_sha { "0".repeat(64) } else { sha };
        let (url, hits) = server(body.clone());
        let lock = format!(
            "[[tool]]\nname = \"ffmpeg\"\nversion = \"t1\"\ntarget = \"{}\"\nurl = \"{url}\"\nsha256 = \"{sha}\"\nsize = {}\nlicence = \"GPL-3.0-or-later\"\nstrip = 1\nbins = [\"bin/ffmpeg\", \"bin/ffprobe\"]\n",
            target(),
            body.len()
        );
        std::fs::write(dir.path().join("toolchain.lock"), lock).unwrap();
        std::fs::create_dir_all(dir.path().join("home")).unwrap();
        for f in [".bashrc", ".profile", ".zshrc"] {
            std::fs::write(dir.path().join("home").join(f), format!("# {f}\n")).unwrap();
        }
        Self { dir, hits }
    }

    fn setup(&self) -> (i32, Value) {
        let p = self.dir.path();
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_TOOLCHAIN", p.join("toolchain.lock"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("HOME", p.join("home"))
            .env_remove("SCRIPTVOX_FFMPEG")
            .env("SCRIPTVOX_DOWNLOAD_IDLE_SECS", "2")
            .env(
                "SCRIPTVOX_WORKER_SRC",
                env!("CARGO_MANIFEST_DIR").to_string() + "/../../python",
            )
            .timeout(std::time::Duration::from_secs(90))
            .args(["setup", "--json"])
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    }

    fn tool(&self) -> std::path::PathBuf {
        self.dir.path().join("data/tools/ffmpeg/t1/bin/ffmpeg")
    }

    fn parts(&self) -> usize {
        std::fs::read_dir(self.dir.path().join("data/downloads"))
            .map(|r| {
                r.flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "part"))
                    .count()
            })
            .unwrap_or(0)
    }
}

#[test]
fn good_pin_installs_once() {
    let c = Case::new("--enable-gpl --enable-libx264", false);
    let (code, v) = c.setup();
    assert_eq!(code, 0, "{v}");
    assert!(c.tool().is_file());
    let (code, v) = c.setup();
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        c.hits.load(Ordering::SeqCst),
        1,
        "the second setup downloads nothing"
    );
}

#[test]
fn second_source_is_used_when_the_first_fails() {
    let c = Case::new("--enable-gpl", false);
    let lock = c.dir.path().join("toolchain.lock");
    // Source A is a port with no server; source B is the good server.
    let text = std::fs::read_to_string(&lock).unwrap().replace(
        "\nurl = ",
        "\nurl = \"http://127.0.0.1:1/a.tar.gz\"\nmirror = ",
    );
    std::fs::write(&lock, text).unwrap();
    let (code, v) = c.setup();
    assert_eq!(code, 0, "{v}");
    assert!(c.tool().is_file());
    assert_eq!(c.hits.load(Ordering::SeqCst), 1);
}

#[test]
fn bad_sha256_is_refused_and_the_part_file_removed() {
    let c = Case::new("--enable-gpl", true);
    let (code, v) = c.setup();
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["error"]["code"], "SVX_CHECKSUM");
    assert_eq!(c.parts(), 0);
    assert!(!c.tool().exists());
}

#[test]
fn nonfree_build_is_refused() {
    let c = Case::new("--enable-gpl --enable-nonfree --enable-libfdk-aac", false);
    let (code, v) = c.setup();
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["error"]["code"], "SVX_NONFREE_FFMPEG");
    assert!(!c.tool().exists());
}

#[test]
fn shell_profiles_are_never_touched() {
    let c = Case::new("--enable-gpl", false);
    let home = c.dir.path().join("home");
    let before: Vec<Vec<u8>> = [".bashrc", ".profile", ".zshrc"]
        .iter()
        .map(|f| std::fs::read(home.join(f)).unwrap())
        .collect();
    assert_eq!(c.setup().0, 0);
    let after: Vec<Vec<u8>> = [".bashrc", ".profile", ".zshrc"]
        .iter()
        .map(|f| std::fs::read(home.join(f)).unwrap())
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        std::fs::read_dir(&home).unwrap().count(),
        3,
        "no new files in HOME"
    );
}

#[test]
fn worker_gets_native_tls() {
    let cmd = scriptvox::provision::worker_command(Path::new("/tmp/w")).unwrap();
    assert!(cmd
        .get_envs()
        .any(|(k, v)| k == "UV_NATIVE_TLS" && v.is_some_and(|v| v == "1")));
}

#[test]
fn agent_run_never_starts_a_large_download() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let lock = format!(
        "[[tool]]\nname = \"ffmpeg\"\nversion = \"big\"\ntarget = \"{}\"\nurl = \"http://127.0.0.1:9/never\"\nsha256 = \"{}\"\nsize = 300000000\nlicence = \"GPL-3.0-or-later\"\nstrip = 1\nbins = [\"bin/ffmpeg\"]\n",
        target(),
        "0".repeat(64)
    );
    std::fs::write(p.join("toolchain.lock"), lock).unwrap();
    std::fs::write(p.join("s.txt"), "Hello there.").unwrap();
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_TOOLCHAIN", p.join("toolchain.lock"))
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("CLAUDECODE", "1")
        .env("SCRIPTVOX_UV", "/bin/true")
        .env_remove("SCRIPTVOX_FFMPEG")
        .args(["make", "--script", "s.txt"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(10), "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "download");
    assert!(v["needs_input"][0]["human_action"]
        .as_str()
        .unwrap()
        .contains("300 MB"));
}

#[test]
fn models_scan_records_a_matching_file_and_skips_the_rest() {
    let d = tempfile::tempdir().unwrap();
    let body = b"model bytes";
    let hash = sha2::Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let lock = d.path().join("lock.toml");
    std::fs::write(
        &lock,
        format!("[[tool]]\nname=\"m\"\nversion=\"1\"\ntarget=\"any\"\nurl=\"https://example.invalid/m\"\nsha256=\"{hash}\"\nsize={}\nlicence=\"MIT\"\nstrip=0\nraw=true\nbins=[\"m\"]\n", body.len()),
    )
    .unwrap();
    let folder = d.path().join("hub/models--x/blobs");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a"), body).unwrap();
    std::fs::write(folder.join("b"), b"model bytez").unwrap(); // same size, other bytes
    let data = d.path().join("data");
    let run = |args: &[&str]| -> Value {
        let o = common::bin()
            .env("SCRIPTVOX_DATA", &data)
            .env("SCRIPTVOX_TOOLCHAIN", &lock)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        serde_json::from_slice(&o.stdout).unwrap()
    };
    let v = run(&["models", "scan", d.path().join("hub").to_str().unwrap()]);
    assert_eq!(v["metrics"]["found"], 1, "{v}");
    assert!(
        v["found"][0]["file"].as_str().unwrap().ends_with("blobs/a"),
        "{v}"
    );
    assert_eq!(std::fs::read(folder.join("a")).unwrap(), body, "only read");
    assert!(data.join("models-found.json").is_file());
}

#[test]
fn a_missing_file_stops_at_once_with_the_url() {
    let d = tempfile::tempdir().unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/gone.bin", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut b = [0u8; 2048];
            let _ = s.read(&mut b);
            h.fetch_add(1, Ordering::SeqCst);
            let _ = write!(
                s,
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    });
    let lock = d.path().join("lock.toml");
    std::fs::write(
        &lock,
        format!("[[tool]]\nname = \"ffmpeg\"\nversion = \"t1\"\ntarget = \"{}\"\nurl = \"{url}\"\nsha256 = \"{}\"\nsize = 1\nlicence = \"MIT\"\nstrip = 0\nbins = [\"ffmpeg\"]\n", target(), "0".repeat(64)),
    )
    .unwrap();
    let o = common::bin()
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .env("SCRIPTVOX_TOOLCHAIN", &lock)
        .env_remove("SCRIPTVOX_FFMPEG")
        .args(["setup", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["error"]["code"], "SVX_ERROR", "{v}");
    let m = v["error"]["message"].as_str().unwrap();
    assert!(m.contains("gone") && m.contains("gone.bin"), "{v}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no retry on a missing file");
}

/// Who asked from which offset, one entry per request.
type Asked = Arc<std::sync::Mutex<Vec<Option<usize>>>>;

/// The first request gets half the body and then a close, or silence when `stall`. Later ones get the asked rest.
fn serve_in_halves(
    body: Vec<u8>,
    stall: bool,
    ranges: bool,
    asked: Asked,
) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/ffmpeg.tar.gz", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_lowercase();
            let from = head
                .split("range: bytes=")
                .nth(1)
                .and_then(|r| r.split('-').next()?.parse::<usize>().ok());
            let hit = {
                let mut a = asked.lock().unwrap();
                a.push(from);
                a.len()
            };
            let ok = |len: usize| {
                format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n")
            };
            match from.filter(|_| ranges) {
                _ if hit == 1 => {
                    let _ = s.write_all(ok(body.len()).as_bytes());
                    let _ = s.write_all(&body[..body.len() / 2]);
                    if stall {
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(40));
                            drop(s);
                        });
                    }
                }
                Some(f) => {
                    let _ = write!(
                        s,
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {f}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len() - 1,
                        body.len(),
                        body.len() - f
                    );
                    let _ = s.write_all(&body[f..]);
                }
                None => {
                    let _ = s.write_all(ok(body.len()).as_bytes());
                    let _ = s.write_all(&body);
                }
            }
        }
    });
    (url, Arc::new(AtomicUsize::new(0)))
}

#[test]
fn a_broken_or_silent_body_is_retried_and_ends_whole() {
    // (stall, the host honours Range)
    for (stall, ranges) in [(false, true), (true, true), (false, false)] {
        let asked = Asked::default();
        let a = asked.clone();
        let c = Case::serving("--enable-gpl", false, move |b| {
            serve_in_halves(b, stall, ranges, a)
        });
        let (code, v) = c.setup();
        assert_eq!(code, 0, "stall {stall}, ranges {ranges}: {v}");
        assert!(c.tool().is_file());
        let asked = asked.lock().unwrap();
        assert_eq!(asked.len(), 2, "one break, one retry: {asked:?}");
        assert!(
            asked[0].is_none() && asked[1].is_some_and(|f| f > 0),
            "{asked:?}"
        );
    }
}

#[test]
fn two_setups_at_once_download_one_file_once() {
    let c = Case::serving("--enable-gpl", false, |body| {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/ffmpeg.tar.gz", l.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                h.fetch_add(1, Ordering::SeqCst);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(&body[..body.len() / 2]);
                std::thread::sleep(std::time::Duration::from_millis(700));
                let _ = s.write_all(&body[body.len() / 2..]);
            }
        });
        (url, hits)
    });
    std::thread::scope(|t| {
        let runs = [t.spawn(|| c.setup()), t.spawn(|| c.setup())];
        for r in runs {
            let (code, v) = r.join().unwrap();
            assert_eq!(code, 0, "{v}");
        }
    });
    assert_eq!(c.hits.load(Ordering::SeqCst), 1);
    assert!(c.tool().is_file());
}

#[test]
fn a_data_folder_that_cannot_be_made_is_not_called_a_network_fault() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("file"), "").unwrap();
    let lock = d.path().join("lock.toml");
    std::fs::write(
        &lock,
        format!("[[tool]]\nname = \"ffmpeg\"\nversion = \"t1\"\ntarget = \"{}\"\nurl = \"http://127.0.0.1:1/f\"\nsha256 = \"{}\"\nsize = 1\nlicence = \"MIT\"\nstrip = 0\nbins = [\"ffmpeg\"]\n", target(), "0".repeat(64)),
    )
    .unwrap();
    let o = common::bin()
        .env("SCRIPTVOX_DATA", d.path().join("file/data"))
        .env("SCRIPTVOX_TOOLCHAIN", &lock)
        .env_remove("SCRIPTVOX_FFMPEG")
        .args(["setup", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1), "{v}");
    assert!(
        !v["error"]["fix"].as_str().unwrap().contains("network"),
        "{v}"
    );
}

#[test]
fn a_refused_ffmpeg_download_in_another_command_stays_retryable() {
    let d = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let lock = d.path().join("lock.toml");
    std::fs::write(
        &lock,
        format!("[[tool]]\nname = \"ffmpeg\"\nversion = \"t1\"\ntarget = \"{}\"\nurl = \"http://127.0.0.1:{port}/f\"\nsha256 = \"{}\"\nsize = 1\nlicence = \"GPL-3.0-or-later\"\nstrip = 0\nbins = [\"ffmpeg\"]\n", target(), "0".repeat(64)),
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .env("SCRIPTVOX_TOOLCHAIN", &lock)
        .env_remove("SCRIPTVOX_FFMPEG")
        .args(["demo", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(5), "{v}");
    assert_eq!(v["command"], "demo");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("network"),
        "{v}"
    );
}

#[test]
fn speech_local_is_not_installed_by_the_large_model_alone() {
    use scriptvox::provision as p;
    let d = tempfile::tempdir().unwrap();
    let data = d.path().join("data");
    for n in p::WHISPER_LARGE_PINS {
        let pin = p::pin(n).unwrap();
        let f = data
            .join("tools")
            .join(&pin.name)
            .join(&pin.version)
            .join(&pin.bins[0]);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, "stub").unwrap();
    }
    let plugin = data
        .join("plugins/whisper")
        .join(p::whisper_dir().file_name().unwrap());
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join(".scriptvox-ready"), "t").unwrap();
    std::fs::create_dir_all(plugin.join("model-large")).unwrap();
    std::fs::write(plugin.join("model-large/model.bin"), "stub").unwrap();
    let o = common::bin()
        .env("SCRIPTVOX_DATA", &data)
        .args(["setup", "speech-local", "--dry-run", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(v.get("speech_local").is_none(), "{v}");
    assert_eq!(
        v["downloads"].as_array().unwrap().len(),
        p::WHISPER_PINS.len(),
        "{v}"
    );
}

/// robustness-2: 3 tries in a row with no new byte end it, also after a try that made progress.
#[test]
fn three_tries_with_no_new_byte_end_a_download_that_had_progress() {
    let c = Case::serving("--enable-gpl", false, |body| {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/ffmpeg.tar.gz", l.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                if h.fetch_add(1, Ordering::SeqCst) == 0 {
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(&body[..body.len() / 2]);
                }
            }
        });
        (url, hits)
    });
    let (code, v) = c.setup();
    assert_eq!(code, 5, "{v}");
    assert_eq!(
        c.hits.load(Ordering::SeqCst),
        4,
        "1 with bytes, 3 without: {v}"
    );
}

/// robustness-4: a full disk under uv is a fault on this machine, exit 1, in bare `setup`.
#[test]
#[cfg(unix)]
fn a_disk_fault_while_preparing_the_voice_worker_is_not_a_network_fault() {
    use std::os::unix::fs::PermissionsExt;
    let c = Case::new("--enable-gpl", false);
    let (code, v) = c.setup();
    assert_eq!(code, 0, "{v}");
    let p = c.dir.path();
    let uv = p.join("fake-uv");
    std::fs::write(
        &uv,
        "#!/bin/sh\necho 'error: No space left on device (os error 28)' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&uv, std::fs::Permissions::from_mode(0o755)).unwrap();
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_TOOLCHAIN", p.join("toolchain.lock"))
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("HOME", p.join("home"))
        .env_remove("SCRIPTVOX_FFMPEG")
        .env_remove("SCRIPTVOX_WORKER_SRC")
        .env("SCRIPTVOX_UV", &uv)
        .timeout(std::time::Duration::from_secs(60))
        .args(["setup", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1), "{v}");
    assert!(
        !v["error"]["fix"].as_str().unwrap().contains("network"),
        "{v}"
    );
}
