//! Audio master (RVX-023): loudness target and ducking.
#![cfg(unix)]
mod common;
use common::*;
use std::process::Command;

fn lufs(ff: &str, file: &std::path::Path) -> f64 {
    let o = Command::new(ff)
        .args(["-hide_banner", "-i"])
        .arg(file)
        .args(["-af", "ebur128", "-f", "null", "-"])
        .output()
        .unwrap();
    let e = String::from_utf8_lossy(&o.stderr);
    let summary = &e[e.rfind("Summary:").unwrap()..];
    summary
        .split("I:")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn loudness_is_within_one_lu_of_target_with_and_without_music() {
    let Some(ff) = ready() else { return };
    for music in [None, Some("calm"), Some("pulse")] {
        let e = Env::new(&ff);
        let extra: Vec<&str> = music.map(|m| vec!["--music", m]).unwrap_or_default();
        let (code, v) = e.make("Loud enough for every phone speaker.", "ok", &extra);
        assert_eq!(code, 0, "{v}");
        let i = lufs(
            &ff,
            std::path::Path::new(v["artifacts"]["video"].as_str().unwrap()),
        );
        assert!(
            (i - scriptvox::audio::TARGET_LUFS).abs() <= 1.0,
            "{music:?}: {i} LUFS"
        );
    }
}

/// RMS level in dB of `file` between `a` and `b` seconds.
fn rms(ff: &str, file: &std::path::Path, a: f64, b: f64) -> f64 {
    let o = Command::new(ff)
        .args(["-hide_banner", "-i"])
        .arg(file)
        .args([
            "-af",
            &format!("atrim={a}:{b},astats=metadata=0:reset=0"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let e = String::from_utf8_lossy(&o.stderr);
    e.lines()
        .rfind(|l| l.contains("RMS level dB"))
        .unwrap()
        .split(':')
        .nth(1)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn music_is_ducked_while_the_voice_speaks() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("duck.wav");
    // Voice: 2 s of tone at the level of loud speech in the mix, then 2 s of silence. Bed: a steady tone (the built-in
    // beds breathe, and that would hide the duck).
    let graph = format!(
        "[0:a]asplit[x][vs];[x]anullsink;[1:a]anull[bed];{}",
        scriptvox::audio::duck_chain()
    );
    let st = Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=2,apad=pad_dur=2",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("aevalsrc='0.05*sin(2*PI*220*t)':s=48000:d=4")
        .args(["-filter_complex", &graph, "-map", "[duck]", "-y"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    let during = rms(&ff, &out, 0.5, 1.8);
    let after = rms(&ff, &out, 2.8, 3.9);
    // Down enough to clear the voice, and not so far that nobody hears the bed (it was 31 dB under the speech).
    assert!(
        (4.0..=14.0).contains(&(after - during)),
        "bed {during:.1} dB under the voice, {after:.1} dB alone"
    );
}
