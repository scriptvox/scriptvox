//! `scriptvox edit --clean-voice`: mains hum and low rumble are found, removed and reported; a clean sound gets
//! no notch. No network: FFmpeg makes every sound.
mod common;
use serde_json::Value;
use std::path::Path;

/// 6 s of picture with a 440 Hz "voice", soft white noise, and `extra` (an aevalsrc term or empty).
fn clip(ff: &str, dir: &Path, name: &str, extra: &str) {
    let voice = format!("aevalsrc='0.3*sin(2*PI*440*t)+0.004*(random(0)-0.5){extra}':s=48000:d=6");
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=320x180:r=25:d=6")
        .args(["-f", "lavfi", "-i", &voice])
        .args([
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k",
        ])
        .args(["-shortest", name])
        .status()
        .unwrap();
    assert!(st.success());
}

fn rv(ff: &str, dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
}

fn samples(ff: &str, video: &Path) -> Vec<f64> {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(video)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
        .output()
        .unwrap();
    assert!(o.status.success());
    o.stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f64::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

/// The amplitude of the `hz` sine from 1 to 5 s, in dB.
fn db(s: &[f64], hz: f64) -> f64 {
    let part = &s[48000..5 * 48000];
    let w = std::f64::consts::TAU * hz / 48000.0;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, x) in part.iter().enumerate() {
        re += x * (w * i as f64).cos();
        im += x * (w * i as f64).sin();
    }
    20.0 * (2.0 * re.hypot(im) / part.len() as f64).max(1e-9).log10()
}

#[test]
fn hum_is_found_removed_and_reported_and_the_voice_keeps_its_level() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let tones = |base: f64| {
        format!(
            "+0.05*sin(2*PI*{base}*t)+0.02*sin(2*PI*{}*t)+0.01*sin(2*PI*{}*t)",
            base * 2.0,
            base * 3.0
        )
    };
    for base in [50.0, 60.0] {
        let name = format!("hum{base}.mp4");
        clip(&ff, d.path(), &name, &tones(base));
        let (code, v) = rv(
            &ff,
            d.path(),
            &["edit", &name, "--clean-voice", "light", "--out", "c.mp4"],
        );
        assert_eq!(code, Some(0), "{v}");
        let hum = &v["clean_voice"]["hum"];
        assert_eq!(hum["found"], true, "{v}");
        assert_eq!(hum["hz"], base, "{v}");
        assert_eq!(
            hum["removed_hz"],
            serde_json::json!([base, base * 2.0, base * 3.0]),
            "{v}"
        );
        assert_eq!(v["clean_voice"]["rumble"]["found"], false, "{v}");
        // The old keys stay.
        assert_eq!(v["clean_voice"]["level"], "light");
        assert_eq!(v["metrics"]["picture_copied"], true, "{v}");
        let (before, after) = (
            samples(&ff, &d.path().join(&name)),
            samples(&ff, &d.path().join("c.mp4")),
        );
        // The loudness pass moves every level, so each tone is read against the voice.
        let voice = db(&after, 440.0) - db(&before, 440.0);
        for (i, k) in [1.0, 2.0, 3.0].iter().enumerate() {
            let fall = db(&before, base * k) - (db(&after, base * k) - voice);
            assert!(fall > 20.0, "{} Hz fell {fall} dB", base * k);
            let told = hum["db_before"][i].as_f64().unwrap() - hum["db_after"][i].as_f64().unwrap();
            assert!(told > 15.0, "{v}");
        }
        // Not a wide cut: a tone 20 Hz above the base tone's notch would stay, and the voice did.
        assert!(db(&after, 440.0) > -30.0, "{}", db(&after, 440.0));
    }
}

#[test]
fn rumble_is_found_and_lowered_and_a_clean_sound_gets_nothing() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    clip(
        &ff,
        d.path(),
        "rumble.mp4",
        "+0.03*sin(2*PI*23*t)+0.03*sin(2*PI*37*t)",
    );
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "rumble.mp4",
            "--clean-voice",
            "light",
            "--out",
            "r.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    let r = &v["clean_voice"]["rumble"];
    assert_eq!(r["found"], true, "{v}");
    assert_eq!(v["clean_voice"]["hum"]["found"], false, "{v}");
    assert!(
        r["db_before"].as_f64().unwrap() - r["db_after"].as_f64().unwrap() > 8.0,
        "{v}"
    );
    let (before, after) = (
        samples(&ff, &d.path().join("rumble.mp4")),
        samples(&ff, &d.path().join("r.mp4")),
    );
    let voice = db(&after, 440.0) - db(&before, 440.0);
    // 24 dB an octave under 60 Hz: 23 Hz falls far, 37 Hz less.
    let fall = |hz: f64| db(&before, hz) - (db(&after, hz) - voice);
    assert!(fall(23.0) > 25.0, "{}", fall(23.0));
    assert!(fall(37.0) > 10.0, "{}", fall(37.0));

    clip(&ff, d.path(), "clean.mp4", "");
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "clean.mp4",
            "--clean-voice",
            "light",
            "--out",
            "k.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(
        v["clean_voice"]["hum"],
        serde_json::json!({"found": false}),
        "{v}"
    );
    assert_eq!(v["clean_voice"]["rumble"]["found"], false, "{v}");
}
