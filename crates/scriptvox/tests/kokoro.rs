//! Opt-in local voice (RVX-029) without downloading it: the notice gate and
//! the local-only guidance. The live check (setup, then a --local-only video)
//! is in docs/specs/RVX-029.md.
mod common;
use serde_json::Value;

/// A folder with a short path. espeak has a path limit (the data folder is at most 55 signs), and the temporary folder
/// of macOS (`/var/folders/../T/`) alone is about 50: there the tests failed on the limit, not on the voice.
fn short_tempdir() -> tempfile::TempDir {
    #[cfg(unix)]
    if let Ok(d) = tempfile::Builder::new().prefix("sv").tempdir_in("/tmp") {
        return d;
    }
    tempfile::tempdir().unwrap()
}

fn run(data: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let o = common::bin()
        .env("SCRIPTVOX_DATA", data)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

#[test]
fn voice_local_dry_run_shows_the_gpl_notice_and_asks_nothing() {
    let d = short_tempdir();
    // The old flag is still accepted (ignored for one version).
    let (code, v) = run(
        d.path(),
        &["setup", "voice-local", "--dry-run", "--accept-gpl"],
    );
    assert_eq!(code, 0, "{v}");
    assert!(v["notice"].as_str().unwrap().contains("GPL-3.0"));
    assert!(v["metrics"]["download_bytes"].as_u64().unwrap() > 100_000_000);
    assert!(v["needs_input"].as_array().unwrap().is_empty(), "{v}");
    let names: Vec<&str> = v["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"kokoro-model"), "{v}");
    assert!(v["downloads"][0]["licence"].is_string(), "{v}");
    assert!(v["gpus"].is_array(), "{v}");
    assert!(
        !d.path().join("plugins").exists(),
        "a dry run fetches nothing"
    );
    #[cfg(unix)]
    assert!(v["metrics"]["disk_free_bytes"].as_u64().unwrap() > 0, "{v}");
}

#[test]
fn local_only_english_points_to_setup_voice_local() {
    let d = short_tempdir();
    std::fs::write(d.path().join("s.txt"), "Offline please.").unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .args(["make", "--script", "s.txt", "--local-only", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(10), "{v}");
    assert!(v["needs_input"][0]["human_action"]
        .as_str()
        .unwrap()
        .contains("setup voice-local"));
}

/// `setup speech-local` shows its size and the licence with no download; with nothing installed, a step that must hear
/// points to it. Then a stub plugin stands in for the model: `analyze --transcript` reads the plugin's JSON, nothing
/// sent and no key. The real model run is in docs/specs/speech-local.md.
#[cfg(unix)]
#[test]
fn speech_local_is_opt_in_and_steps_use_it_when_installed() {
    use scriptvox::provision as p;
    let d = short_tempdir();
    let (code, v) = run(d.path(), &["setup", "speech-local", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["metrics"]["download_bytes"].as_u64().unwrap() > 480_000_000,
        "{v}"
    );
    assert!(v["licence"].as_str().unwrap().contains("MIT"), "{v}");

    let Some(ff) = common::ready() else { return };
    let st = std::process::Command::new(&ff)
        .args(["-v", "error", "-f", "lavfi", "-i", "sine=f=330:d=2"])
        .arg(d.path().join("a.wav"))
        .status()
        .unwrap();
    assert!(st.success());
    let cfg = d.path().join("cfg");
    let uv = d.path().join("stub-uv");
    let analyze = || {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_DATA", d.path().join("data"))
            .env("SCRIPTVOX_CONFIG", &cfg)
            .env("SCRIPTVOX_FFMPEG", &ff)
            .env("SCRIPTVOX_UV", &uv)
            .args(["analyze", "a.wav", "--transcript", "--json"])
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (_, v) = analyze();
    assert!(v.to_string().contains("setup speech-local"), "{v}");

    // The paths come from this process's data folder, the same one the command gets.
    std::env::set_var("SCRIPTVOX_DATA", d.path().join("data"));
    for n in p::WHISPER_PINS {
        let pin = p::pin(n).unwrap();
        for b in &pin.bins {
            let f = p::tool_dir(&pin).join(b);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, b"stub").unwrap();
        }
    }
    let plugin = p::whisper_dir();
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join(".scriptvox-ready"), b"").unwrap();
    std::fs::create_dir_all(plugin.join("model")).unwrap();
    std::fs::write(plugin.join("model/model.bin"), b"stub").unwrap();
    std::env::remove_var("SCRIPTVOX_DATA");
    // Its arguments end with: -m scriptvox_whisper IN OUT [--lang L] [--prompt P].
    std::fs::write(
        &uv,
        r#"#!/bin/sh
while [ "$1" != "scriptvox_whisper" ]; do shift; done
printf '{"language":"en","text":"hello there.","segments":[{"start":0,"end":1}],"words":[{"text":"hello","start":0,"end":0.5},{"text":"there.","start":0.5,"end":1}]}' > "$3"
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&uv, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (code, v) = analyze();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["transcript"]["text"], "hello there.", "{v}");
    assert_eq!(v["transcript"]["words"], 2, "{v}");
}

/// The limit is on the espeak data path: over it, `setup` refuses before any download, with the fix.
#[test]
fn voice_local_refuses_a_data_folder_too_long_for_espeak_before_any_download() {
    let d = short_tempdir();
    let long = d.path().join("x".repeat(100));
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", &long)
        .args(["setup", "voice-local", "--accept-gpl", "--json"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(2), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("SCRIPTVOX_DATA"),
        "{v}"
    );
    assert!(!long.exists(), "nothing was downloaded: {v}");
    // A short folder gets past the check: a dry run downloads nothing.
    let short = d.path().join("d");
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", &short)
        .args(["setup", "voice-local", "--dry-run", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
}
