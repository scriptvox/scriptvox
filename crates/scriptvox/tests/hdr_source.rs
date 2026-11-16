//! An HDR source becomes SDR before a step works on it (docs/specs/hdr-source.md). A short synthetic HLG clip.

mod common;
use std::path::Path;
use std::process::Command;

fn ffmpeg() -> Option<String> {
    std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).is_file())
}

/// Average saturation of the first frame, as a reader that ignores the tags sees it.
fn saturation(ff: &str, file: &Path) -> f64 {
    let o = Command::new(ff)
        .args(["-hide_banner", "-i"])
        .arg(file)
        .args([
            "-vf",
            "format=yuv420p,signalstats,metadata=print:file=-",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("lavfi.signalstats.SATAVG=")?.parse().ok())
        .unwrap()
}

#[test]
fn hlg_is_tone_mapped_once_and_sdr_is_left_alone() {
    let Some(ff) = ffmpeg() else {
        eprintln!("skipped: SCRIPTVOX_FFMPEG is not set");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let make = |name: &str, vf: &str, enc: &[&str]| {
        let st = Command::new(&ff)
            .current_dir(dir.path())
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc2=s=320x240:r=30:d=1")
            .args(["-vf", vf])
            .args(enc)
            .arg(name)
            .status()
            .unwrap();
        assert!(st.success());
    };
    make(
        "hlg.mp4",
        "format=yuv420p,zscale=tin=bt709:pin=bt709:min=bt709:rin=limited:t=arib-std-b67:p=bt2020:m=bt2020nc:r=limited,format=yuv420p10le",
        &["-c:v", "libx265", "-preset", "ultrafast", "-x265-params", "log-level=error:colorprim=bt2020:transfer=arib-std-b67:colormatrix=bt2020nc"],
    );
    make(
        "sdr.mp4",
        "format=yuv420p",
        &["-c:v", "libx264", "-preset", "ultrafast"],
    );
    let edit = |name: &str| {
        let o = common::std_bin()
            .current_dir(dir.path())
            .env("SCRIPTVOX_DATA", dir.path().join("data"))
            .args(["edit", name, "--from", "0", "--to", "0.5", "--json"])
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["status"], "ok", "{v}");
        v
    };
    let v = edit("hlg.mp4");
    let gate = |v: &serde_json::Value| v["findings"].to_string().contains("\"hdr\"");
    assert!(gate(&v), "{v}");
    let out = dir.path().join(v["artifacts"]["video"].as_str().unwrap());
    let raw = saturation(&ff, &dir.path().join("hlg.mp4"));
    let mapped = saturation(&ff, &out);
    assert!(mapped > raw * 1.5, "washed out: {raw} then {mapped}");

    let copies = || {
        std::fs::read_dir(dir.path().join(".scriptvox/hdr"))
            .unwrap()
            .count()
    };
    assert_eq!(copies(), 1);
    edit("hlg.mp4");
    assert_eq!(copies(), 1, "the copy is made once");

    // A phone clip carries a rotation, and a real read of its tag must still find HDR.
    let st = Command::new(&ff)
        .current_dir(dir.path())
        .args([
            "-y",
            "-v",
            "error",
            "-display_rotation",
            "90",
            "-i",
            "hlg.mp4",
            "-c",
            "copy",
            "rot.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    assert!(gate(&edit("rot.mp4")), "a rotated clip reads as HDR");
    let rotated = copies();
    assert_eq!(rotated, 2);

    assert!(!gate(&edit("sdr.mp4")));
    assert_eq!(copies(), rotated);
}
