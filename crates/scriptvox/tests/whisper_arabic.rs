//! `setup speech-local --arabic` with no network (docs/specs/speech-local-arabic.md): a lock of tiny pins, a folder of
//! tiny model files, and a stub plugin that answers with the model folder it was given.
mod common;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const AR: [(&str, &str); 5] = [
    ("whisper-ar-model", "model.bin"),
    ("whisper-ar-config", "config.json"),
    ("whisper-ar-preprocessor", "preprocessor_config.json"),
    ("whisper-ar-tokenizer", "tokenizer.json"),
    ("whisper-ar-vocabulary", "vocabulary.json"),
];

fn row(name: &str, bin: &str, body: &[u8]) -> String {
    let sha: String = Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("[[tool]]\nname = \"{name}\"\nversion = \"t\"\ntarget = \"any\"\nurl = \"\"\nsha256 = \"{sha}\"\nsize = {}\nlicence = \"MIT\"\nstrip = 0\nraw = true\nbins = [\"{bin}\"]\n\n", body.len())
}

struct Setup {
    d: tempfile::TempDir,
}

impl Setup {
    /// The present model is installed (stub files and a ready plugin); the Arabic files wait in `src`.
    fn new() -> Self {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let mut lock = String::new();
        for n in scriptvox::provision::WHISPER_PINS {
            let bin = scriptvox::provision::pin(n).unwrap().bins[0].clone();
            lock += &row(n, &bin, b"small");
            let f = data.join("tools").join(n).join("t").join(&bin);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, b"small").unwrap();
        }
        for (n, bin) in AR {
            let body = format!("arabic {bin}");
            lock += &row(n, bin, body.as_bytes());
            std::fs::write(src.join(bin), body).unwrap();
        }
        std::fs::write(d.path().join("toolchain.lock"), lock).unwrap();
        let plugin = data
            .join("plugins/whisper")
            .join(scriptvox::provision::whisper_dir().file_name().unwrap());
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(plugin.join(".scriptvox-ready"), b"").unwrap();
        std::fs::create_dir_all(plugin.join("model")).unwrap();
        std::fs::write(plugin.join("model/model.bin"), b"small").unwrap();
        Setup { d }
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.d.path().join(rel)
    }

    fn run(&self, args: &[&str]) -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(self.d.path())
            .env("SCRIPTVOX_DATA", self.p("data"))
            .env("SCRIPTVOX_CONFIG", self.p("cfg"))
            .env("SCRIPTVOX_TOOLCHAIN", self.p("toolchain.lock"))
            .env("SCRIPTVOX_UV", self.p("stub-uv"));
        if let Ok(ff) = std::env::var("SCRIPTVOX_FFMPEG") {
            c.env("SCRIPTVOX_FFMPEG", ff);
        }
        let o = c.args(args).arg("--json").output().unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    }

    fn installed(&self, bin: &str) -> bool {
        let name = AR.iter().find(|a| a.1 == bin).unwrap().0;
        self.p("data/tools")
            .join(name)
            .join("t")
            .join(bin)
            .is_file()
    }
}

#[test]
fn with_no_address_and_no_folder_setup_names_from() {
    let s = Setup::new();
    let (code, v) = s.run(&["setup", "speech-local", "--arabic"]);
    assert_eq!(code, 10, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("--from <folder>"),
        "{v}"
    );
}

#[test]
fn a_changed_or_missing_file_is_refused_by_name_and_nothing_is_copied() {
    let s = Setup::new();
    std::fs::write(s.p("src/tokenizer.json"), "arabic tokenizer.jsoX").unwrap();
    let (code, v) = s.run(&["setup", "speech-local", "--arabic", "--from", "src"]);
    assert_eq!(code, 1, "{v}");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("tokenizer.json") && msg.contains("does not match"),
        "{v}"
    );
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--from"),
        "{v}"
    );
    assert!(
        !s.installed("config.json"),
        "nothing copied before all match"
    );

    std::fs::remove_file(s.p("src/tokenizer.json")).unwrap();
    let (code, v) = s.run(&["setup", "speech-local", "--arabic", "--from", "src"]);
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no file tokenizer.json"),
        "{v}"
    );
}

/// The 5 good files install; then the plugin gets `model-ar` for `ar` only, the present model for `en` and for no
/// language (auto-detect), and the Arabic transcript has its own cache file.
#[cfg(unix)]
#[test]
fn good_files_install_and_only_arabic_gets_the_arabic_model() {
    let s = Setup::new();
    let (code, v) = s.run(&["providers"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["speech_local"]["arabic"]["installed"], false, "{v}");

    let (code, v) = s.run(&["setup", "speech-local", "--arabic", "--from", "src"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["speech_arabic"], "installed", "{v}");
    assert!(AR.iter().all(|a| s.installed(a.1)));
    assert!(
        s.p("src/model.bin").is_file(),
        "the source folder is only read"
    );
    let (_, v) = s.run(&["providers"]);
    assert_eq!(v["speech_local"]["arabic"]["installed"], true, "{v}");
    assert!(
        v["speech_local"]["arabic"]["training_data"]
            .to_string()
            .contains("CC-BY-4.0"),
        "{v}"
    );

    let Some(ff) = common::ready() else { return };
    std::fs::write(
        s.p("stub-uv"),
        r#"#!/bin/sh
while [ "$1" != "scriptvox_whisper" ]; do shift; done
printf '{"language":"x","text":"%s","segments":[{"start":0,"end":1}],"words":[{"text":"w","start":0,"end":0.5}]}' "$SCRIPTVOX_WHISPER_MODEL" > "$3"
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(s.p("stub-uv"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let heard = |wav: &str, f: u32, lang: Option<&str>| -> String {
        common::ffmpeg(
            &ff,
            s.d.path(),
            &["-f", "lavfi", "-i", &format!("sine=f={f}:d=1"), wav],
        );
        let mut a = vec!["analyze", wav, "--transcript"];
        if let Some(l) = lang {
            a.extend(["--lang", l]);
        }
        let (code, v) = s.run(&a);
        assert_eq!(code, 0, "{v}");
        v["transcript"]["text"].as_str().unwrap().to_string()
    };
    let folder = |t: &str| {
        Path::new(t)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
    };
    assert_eq!(folder(&heard("ar.wav", 330, Some("ar"))), "model-ar");
    assert_eq!(folder(&heard("en.wav", 440, Some("en"))), "model");
    assert_eq!(folder(&heard("auto.wav", 550, None)), "model");
    // The same audio heard in English first, then in Arabic: the Arabic model runs, not the cached transcript.
    assert_eq!(folder(&heard("en.wav", 440, Some("ar"))), "model-ar");
    // Heard in Arabic first, then with no language: the present model runs, not the Arabic transcript.
    assert_eq!(folder(&heard("ar.wav", 330, None)), "model");
}

/// Tool files of an earlier setup with no `model-ar` in this plugin folder (a new version): not ready, so `ar` keeps the
/// present model; `--arabic` with no folder links them again.
#[test]
fn tool_files_without_their_plugin_link_are_not_ready_and_setup_links_them() {
    let s = Setup::new();
    for (n, bin) in AR {
        let f = s.p("data/tools").join(n).join("t").join(bin);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::copy(s.p("src").join(bin), f).unwrap();
    }
    let (_, v) = s.run(&["providers"]);
    assert_eq!(v["speech_local"]["arabic"]["installed"], false, "{v}");
    let (code, v) = s.run(&["setup", "speech-local", "--arabic"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = s.run(&["providers"]);
    assert_eq!(v["speech_local"]["arabic"]["installed"], true, "{v}");
}

/// The publication step only fills `url`: then `--arabic` downloads and checks the files with no `--from`.
#[test]
fn with_an_address_the_files_download_with_no_folder() {
    use std::io::{BufRead, BufReader, Write};
    let s = Setup::new();
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let src = s.p("src");
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut line = String::new();
            let mut r = BufReader::new(&c);
            r.read_line(&mut line).unwrap();
            let name = line
                .split(' ')
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/');
            let body = std::fs::read(src.join(name)).unwrap_or_default();
            let mut c = &c;
            let _ = write!(
                c,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = c.write_all(&body);
        }
    });
    let mut lock = std::fs::read_to_string(s.p("toolchain.lock")).unwrap();
    for (n, bin) in AR {
        lock = lock.replace(
            &format!("name = \"{n}\"\nversion = \"t\"\ntarget = \"any\"\nurl = \"\""),
            &format!("name = \"{n}\"\nversion = \"t\"\ntarget = \"any\"\nurl = \"http://127.0.0.1:{port}/{bin}\""),
        );
    }
    std::fs::write(s.p("toolchain.lock"), lock).unwrap();
    let (code, v) = s.run(&["setup", "speech-local", "--arabic"]);
    assert_eq!(code, 0, "{v}");
    assert!(AR.iter().all(|a| s.installed(a.1)));
    let (_, v) = s.run(&["providers"]);
    assert_eq!(v["speech_local"]["arabic"]["installed"], true, "{v}");
}

/// A dry run with no folder lists the 5 Arabic files beside the present model's, and counts their bytes.
#[test]
fn a_dry_run_download_lists_the_arabic_files_and_their_bytes() {
    let s = Setup::new();
    let lock = std::fs::read_to_string(s.p("toolchain.lock")).unwrap();
    std::fs::write(
        s.p("toolchain.lock"),
        lock.replace("url = \"\"", "url = \"http://127.0.0.1:9/f\""),
    )
    .unwrap();
    let (code, v) = s.run(&["setup", "speech-local", "--arabic", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    let names: Vec<&str> = v["downloads"]
        .as_array()
        .unwrap_or_else(|| panic!("{v}"))
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    for (n, _) in AR {
        assert!(names.contains(&n), "{n} not in {names:?}");
    }
    let ar: u64 = v["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["name"].as_str().unwrap().starts_with("whisper-ar-"))
        .map(|d| d["bytes"].as_u64().unwrap())
        .sum();
    assert!(
        v["metrics"]["download_bytes"].as_u64().unwrap() >= ar,
        "{v}"
    );
}
