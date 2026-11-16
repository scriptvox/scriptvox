//! The first video (RVX-004). Skipped when SCRIPTVOX_FFMPEG is unset.
mod common;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn ffmpeg() -> Option<String> {
    std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden/kinetic")
}

fn demo(out: &Path, extra: &[&str], path_env: Option<&str>) -> (i32, Value) {
    let mut c = common::bin();
    c.args(["demo", "--use-system", "--json", "--out"])
        .arg(out)
        .args(extra);
    if let Some(p) = path_env {
        c.env("PATH", p);
    }
    let o = c.output().unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    (o.status.code().unwrap(), v)
}

#[test]
fn demo_renders_with_readback() {
    if ffmpeg().is_none() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("demo.mp4");
    let (code, v) = demo(&out, &["--golden"], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["artifacts"]["video"], out.to_string_lossy().as_ref());
    assert!(out.is_file());
    assert_eq!(v["metrics"]["frames"], 48);
    assert_eq!(v["metrics"]["fps"], 12.0);
}

#[test]
fn demo_needs_no_python_and_no_node() {
    if ffmpeg().is_none() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    // An empty PATH: only the binary and SCRIPTVOX_FFMPEG exist for this run.
    let (code, v) = demo(&d.path().join("demo.mp4"), &["--golden"], Some(""));
    assert_eq!(code, 0, "{v}");
}

#[test]
fn golden_kinetic_twice() {
    let Some(ff) = ffmpeg() else { return };
    let d = tempfile::tempdir().unwrap();
    let (a, b) = (d.path().join("a.mp4"), d.path().join("b.mp4"));
    assert_eq!(demo(&a, &["--golden"], None).0, 0);
    assert_eq!(demo(&b, &["--golden"], None).0, 0);
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "two runs differ"
    );

    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(golden_dir().join("meta.json")).unwrap())
            .unwrap();
    let reference = golden_dir().join("ref.mp4");
    assert!(
        reference.is_file(),
        "no reference: run `just golden-bless kinetic`"
    );
    let o = std::process::Command::new(ff)
        .args(["-hide_banner", "-i"])
        .arg(&a)
        .arg("-i")
        .arg(&reference)
        .args(["-lavfi", "[0:v][1:v]psnr", "-f", "null", "-"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    let avg = err
        .split("average:")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap();
    let psnr: f64 = if avg == "inf" {
        f64::INFINITY
    } else {
        avg.parse().unwrap()
    };
    assert!(
        psnr >= meta["psnr_min"].as_f64().unwrap(),
        "PSNR {psnr} below threshold"
    );
}

/// The demo is the first video a person makes: `check` must pass its loudness (it was -27.9 LUFS, target -14).
#[test]
fn the_demo_is_as_loud_as_every_other_video() {
    let Some(ff) = ffmpeg() else { return };
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("demo.mp4");
    assert_eq!(demo(&out, &[], None).0, 0);
    let o = common::bin()
        .env("SCRIPTVOX_FFMPEG", ff)
        .args(["check"])
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "loudness")
        .unwrap();
    assert_eq!(f["status"], "pass", "{f}");
}
