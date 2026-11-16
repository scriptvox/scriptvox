#![cfg(unix)]
//! `scriptvox doctor` and `uninstall` (RVX-019).
mod common;
use serde_json::Value;
use std::path::Path;

fn run(home: &Path, data: &Path, ffmpeg: Option<&str>, args: &[&str]) -> (i32, Value, String) {
    let mut c = common::bin();
    c.current_dir(home)
        .env("HOME", home)
        .env("SCRIPTVOX_DATA", data)
        .env("PATH", "")
        .env_remove("SCRIPTVOX_UV");
    match ffmpeg {
        Some(f) => c.env("SCRIPTVOX_FFMPEG", f),
        None => c.env_remove("SCRIPTVOX_FFMPEG"),
    };
    let o = c.args(args).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    (
        o.status.code().unwrap(),
        serde_json::from_str(&text).unwrap(),
        text,
    )
}

#[test]
fn clean_home_with_ffmpeg_is_ok_and_lists_modes() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    let (home, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (code, v, _) = run(
        home.path(),
        data.path(),
        Some(&ff),
        &["doctor", "--offline", "--json"],
    );
    assert_eq!(code, 0, "{v}");
    assert!(v["mode_ready"]
        .as_array()
        .unwrap()
        .contains(&Value::from("local")));
    for f in v["findings"].as_array().unwrap() {
        assert!(f["fix_safe"].is_boolean(), "{f}");
        if f["status"] != "ok" {
            assert!(f["fix"].is_string(), "{f}");
        }
    }
    assert!(v["gpus"].is_array(), "{v}");
}

#[test]
fn missing_ffmpeg_fails_with_a_safe_fix() {
    let (home, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (code, v, _) = run(
        home.path(),
        data.path(),
        None,
        &["doctor", "--offline", "--json"],
    );
    assert_eq!(code, 1, "{v}");
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "ffmpeg")
        .unwrap()
        .clone();
    assert_eq!(
        (f["status"].as_str(), f["fix_safe"].as_bool()),
        (Some("fail"), Some(true))
    );
    assert_eq!(v["error"]["fix"], "scriptvox doctor --fix");
    assert_eq!(v["mode_ready"], serde_json::json!([]));
}

#[test]
fn redact_hides_the_home_folder() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let data = home.path().join(".local/share/scriptvox");
    // FFmpeg reached through a path under HOME, so its path would leak without --redact.
    let bin = home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for n in ["ffmpeg", "ffprobe"] {
        std::os::unix::fs::symlink(Path::new(&ff).with_file_name(n), bin.join(n)).unwrap();
    }
    let ff_home = bin.join("ffmpeg");
    let (_, _, plain) = run(
        home.path(),
        &data,
        Some(ff_home.to_str().unwrap()),
        &["doctor", "--offline", "--json"],
    );
    let (_, _, red) = run(
        home.path(),
        &data,
        Some(ff_home.to_str().unwrap()),
        &["doctor", "--offline", "--redact", "--json"],
    );
    let h = home.path().to_string_lossy();
    assert!(
        plain.contains(h.as_ref()),
        "the test needs a home path to hide"
    );
    let findings = &serde_json::from_str::<Value>(&red).unwrap()["findings"];
    assert!(!findings.to_string().contains(h.as_ref()), "{findings}");
}

#[test]
fn uninstall_purge_removes_the_data_folder() {
    let (home, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let d = data.path().join("scriptvox");
    std::fs::create_dir_all(d.join("tools/x")).unwrap();
    let (code, v, _) = run(home.path(), &d, None, &["uninstall", "--json"]);
    assert_eq!(code, 0, "{v}");
    assert!(d.exists(), "without --purge nothing is deleted");
    let (code, _, _) = run(home.path(), &d, None, &["uninstall", "--purge", "--json"]);
    assert_eq!(code, 0);
    assert!(!d.exists());
}
