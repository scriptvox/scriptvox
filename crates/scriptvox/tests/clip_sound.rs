//! A scene's clip keeps its own sound (G059, docs/specs/scene-clip-sound.md). Stub voice: a 330 Hz tone with words
//! 0.35 s apart from 0.1 s, so a scene after n words starts at 0.1 + 0.35 n seconds. The clips carry tones far from it.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// A small clip: a test picture, with the tones `(hz, seconds)` one after the other as its sound (none: no sound).
fn clip(ff: &str, file: &Path, tones: &[(u32, f64)]) {
    let secs: f64 = tones.iter().map(|t| t.1).sum::<f64>().max(4.0);
    let mut c = Command::new(ff);
    c.args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=s=320x180:r=30:d={secs}"));
    for (hz, d) in tones {
        c.args(["-f", "lavfi", "-i"])
            .arg(format!("sine=f={hz}:d={d}:r=48000"));
    }
    if !tones.is_empty() {
        let ins: String = (1..=tones.len()).map(|i| format!("[{i}:a]")).collect();
        c.args(["-filter_complex"])
            .arg(format!("{ins}concat=n={}:v=0:a=1[a]", tones.len()))
            .args(["-map", "0:v", "-map", "[a]", "-c:a", "aac", "-b:a", "128k"]);
    }
    c.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-shortest"])
        .arg(file);
    assert!(c.status().unwrap().success());
}

/// The mean level in dB of a 50 Hz band at `hz`, from `from` for `secs` seconds of a file's sound.
fn band(ff: &str, file: &str, hz: u32, from: f64, secs: f64) -> f64 {
    let bp = format!("bandpass=f={hz}:width_type=h:w=50");
    let o = Command::new(ff)
        .args(["-hide_banner", "-nostdin", "-ss", &from.to_string()])
        .args(["-t", &secs.to_string(), "-i", file, "-vn", "-af"])
        .arg(format!("{bp},{bp},volumedetect"))
        .args(["-f", "null", "-"])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&o.stderr);
    let at = log.find("mean_volume:").expect("volumedetect gave a level");
    log[at + 12..]
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn go(e: &Env, mode: &str, args: &[&str]) -> (i32, Value) {
    let o = e.cmd(mode).args(args).arg("--json").output().unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap_or_else(|_| {
            panic!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
        }),
    )
}

fn make(e: &Env, scenes: Value) -> (i32, Value) {
    std::fs::write(
        e.dir.path().join("b.json"),
        json!({ "scenes": scenes }).to_string(),
    )
    .unwrap();
    go(e, "ok", &["make", "--brief", "b.json"])
}

/// Where each scene of a run starts, in seconds (`scenes.json`). The voice is read scene by scene since 13 Nov 2026,
/// and the stub reads 3 s for any text, so scene 2 starts near 3.4 s.
fn starts(e: &Env, v: &Value) -> Vec<f64> {
    let f = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap())
        .join("scenes.json");
    serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
}

/// No gate failed, and the loudness and drift gates passed (the silence gate speaks only when it finds a silence).
fn gates_pass(v: &Value) {
    let f = v["findings"].as_array().unwrap();
    assert!(f.iter().all(|g| g["status"] != "fail"), "{v}");
    for name in ["loudness", "silence", "av_drift"] {
        assert!(
            f.iter().any(|g| g["gate"] == name && g["status"] == "pass"),
            "{name}: {v}"
        );
    }
}

#[test]
fn keep_is_heard_in_its_scene_only_and_replay_and_a_patch_follow_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    clip(&ff, &e.dir.path().join("tone.mp4"), &[(1000, 4.0)]);
    let scenes = |sound: &str| {
        json!([
            {"script": "One two three four.", "media": "tone.mp4"},
            {"script": "Five six seven eight.", "media": "tone.mp4", "sound": sound},
        ])
    };
    let (code, v) = make(&e, scenes("keep"));
    assert_eq!(code, 0, "{v}");
    gates_pass(&v);
    let video = v["artifacts"]["video"].as_str().unwrap().to_string();
    // Inside scene 2.
    let at = starts(&e, &v)[1] + 0.3;
    let (first, second) = (
        band(&ff, &video, 1000, 0.2, 1.0),
        band(&ff, &video, 1000, at, 1.0),
    );
    assert!(
        second > -45.0 && second - first > 20.0,
        "the clip's tone: scene 1 {first} dB, scene 2 {second} dB"
    );
    // Replay: from the cache, with a worker that fails any new voice call.
    let (code, r) = go(&e, "flaky", &["replay", &video, "--out", "again.mp4"]);
    assert_eq!(code, 0, "{r}");
    let again = band(
        &ff,
        r["artifacts"]["video"].as_str().unwrap(),
        1000,
        at,
        1.0,
    );
    assert!(
        (again - second).abs() < 1.5,
        "replay {again} dB, first {second} dB"
    );
    // Resume with a patch: the same picture, the other sound choice. The new video has the new sound.
    let run = v["run_id"].as_str().unwrap().to_string();
    std::fs::write(
        e.dir.path().join("p.json"),
        json!({ "scenes": scenes("under") }).to_string(),
    )
    .unwrap();
    let (code, p) = go(&e, "ok", &["resume", &run, "--patch", "p.json", "--wait"]);
    assert_eq!(code, 0, "{p}");
    let under = band(
        &ff,
        p["artifacts"]["video"].as_str().unwrap(),
        1000,
        at,
        1.0,
    );
    assert!(
        under < second - 1.5 && under - first > 10.0,
        "under {under} dB, keep {second} dB, none {first} dB"
    );
}

#[test]
fn under_is_about_12_db_below_keep_where_the_voice_is_silent() {
    let Some(ff) = ready() else { return };
    let mut level = Vec::new();
    for sound in ["keep", "under"] {
        let e = Env::new(&ff);
        let p = e.dir.path();
        // A voice that speaks for 1 s and is then silent, so the duck lets go of the clip.
        let st = Command::new(&ff)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=f=330:d=1"])
            .args(["-af", "apad=whole_dur=6", "-c:a", "libmp3lame"])
            .arg(p.join("voice.mp3"))
            .status()
            .unwrap();
        assert!(st.success());
        clip(&ff, &p.join("tone.mp4"), &[(1000, 6.0)]);
        // Scene 2 starts at about 1.4 s (each read loses its silent end) and runs to the end: 1 s of voice, then a
        // pause of 4 s where only the clip sounds.
        let (_, v) = make(
            &e,
            json!([
                {"script": "One two."},
                {"script": "Three. [pause 4]", "media": "tone.mp4", "sound": sound},
            ]),
        );
        let video = v["artifacts"]["video"]
            .as_str()
            .unwrap_or_else(|| panic!("{v}"));
        // The clip's tone against the voice's tone of the same file: the last loudness step moves both the same.
        level.push(band(&ff, video, 1000, 3.0, 2.0) - band(&ff, video, 330, 0.2, 0.6));
    }
    let gap = level[0] - level[1];
    assert!(
        (9.0..=15.0).contains(&gap) && level[0].abs() < 4.0,
        "against the voice: keep {} dB, under {} dB",
        level[0],
        level[1]
    );
}

#[test]
fn a_scene_that_cannot_give_sound_is_refused_by_name_before_the_voice() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    let p = e.dir.path();
    clip(&ff, &p.join("mute.mp4"), &[]);
    clip(&ff, &p.join("tone.mp4"), &[(1000, 4.0)]);
    let first = json!({"script": "One two."});
    for (scene, word) in [
        (
            json!({"script": "Three.", "media": "mute.mp4", "sound": "keep"}),
            "mute.mp4",
        ),
        (
            json!({"script": "Three.", "media": "media/02-fractal.jpg", "sound": "under"}),
            "clip",
        ),
        (
            json!({"script": "Three.", "stock": "sea", "sound": "keep"}),
            "own",
        ),
        (
            json!({"script": "Three.", "open_image": "sea", "sound": "keep"}),
            "own",
        ),
        (
            json!({"script": "Three.", "generate": {"prompt": "the sea"}, "sound": "keep"}),
            "own",
        ),
        (json!({"script": "Three.", "sound": "keep"}), "own"),
        (
            json!({"script": "Three.", "media": "tone.mp4", "sound": "loud"}),
            "keep, under or none",
        ),
        (
            json!({"script": "", "media": "tone.mp4", "sound": "keep"}),
            "script",
        ),
    ] {
        std::fs::write(
            p.join("b.json"),
            json!({"scenes": [first, scene]}).to_string(),
        )
        .unwrap();
        // `plan` and `make` both stop; the flaky worker would fail exit 4 if a voice call were made.
        for cmd in ["plan", "make"] {
            let (code, v) = go(&e, "flaky", &[cmd, "--brief", "b.json"]);
            let msg = v["error"]["message"].as_str().unwrap_or_default();
            assert_eq!(code, 2, "{cmd} {scene}: {v}");
            assert!(
                msg.contains("scenes[1]") && msg.contains(word),
                "{cmd} {scene}: {msg}"
            );
        }
    }
}

#[test]
fn no_field_and_none_give_the_same_plan_and_the_same_master_bytes() {
    let Some(ff) = ready() else { return };
    let mut got = Vec::new();
    for sound in [None, Some("none")] {
        let e = Env::new(&ff);
        clip(&ff, &e.dir.path().join("tone.mp4"), &[(1000, 4.0)]);
        let mut scene = json!({"script": "Five six seven eight.", "media": "tone.mp4"});
        if let Some(s) = sound {
            scene["sound"] = json!(s);
        }
        let (code, v) = make(&e, json!([{"script": "One two three four."}, scene]));
        assert_eq!(code, 0, "{v}");
        let video = v["artifacts"]["video"].as_str().unwrap();
        let master = v["artifacts"]["master"].as_str().unwrap();
        assert!(
            band(&ff, video, 1000, 1.8, 1.0) < -60.0,
            "the clip is silent"
        );
        // `none` is not written into the recipe either.
        let (_, x) = go(&e, "ok", &["explain", video]);
        let recipe = &x["recipe"]["request"]["scenes"][1];
        assert!(
            recipe["media"].is_string() && recipe.get("sound").is_none(),
            "{x}"
        );
        let (_, plan) = go(&e, "ok", &["plan", "--brief", "b.json"]);
        assert!(plan["plan_id"].is_string(), "{plan}");
        got.push((plan["plan_id"].clone(), std::fs::read(master).unwrap()));
    }
    assert_eq!(got[0].0, got[1].0, "plan id");
    assert!(got[0].1 == got[1].1, "master.wav bytes");
}

#[test]
fn media_from_picks_the_part_of_the_sound_too() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    // 1000 Hz for the first 3 s of the file, then 2500 Hz.
    clip(
        &ff,
        &e.dir.path().join("two.mp4"),
        &[(1000, 3.0), (2500, 3.0)],
    );
    let (code, v) = make(
        &e,
        json!([
            {"script": "One two three four."},
            {"script": "Five six seven eight.", "media": "two.mp4", "media_from": 3.5, "sound": "keep"},
        ]),
    );
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap();
    let at = starts(&e, &v)[1] + 0.3;
    let (low, high) = (
        band(&ff, video, 1000, at, 1.0),
        band(&ff, video, 2500, at, 1.0),
    );
    assert!(
        high > -45.0 && high - low > 20.0,
        "1000 Hz {low} dB, 2500 Hz {high} dB"
    );
}

#[test]
fn media_to_stops_the_sound_and_it_is_not_stretched() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    clip(&ff, &e.dir.path().join("tone.mp4"), &[(1000, 4.0)]);
    // Scene 2 runs for about 3 s; its clip is only 0.6 s long, so the picture slows or holds. The sound does not.
    let (code, v) = make(
        &e,
        json!([
            {"script": "One two three four."},
            {"script": "Five six seven eight.", "media": "tone.mp4", "media_to": 0.6, "sound": "keep"},
        ]),
    );
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap();
    let at = starts(&e, &v)[1];
    let (early, late) = (
        band(&ff, video, 1000, at + 0.1, 0.4),
        band(&ff, video, 1000, at + 0.9, 0.5),
    );
    assert!(
        early > -45.0 && early - late > 20.0,
        "first 0.6 s {early} dB, after it {late} dB"
    );
}

#[test]
fn keep_and_under_mix_with_a_bed_and_sound_effects() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    clip(&ff, &p.join("low.mp4"), &[(1000, 4.0)]);
    clip(&ff, &p.join("high.mp4"), &[(2500, 4.0)]);
    std::fs::write(
        p.join("b.json"),
        json!({"music": "calm", "sfx": true, "scenes": [
            {"script": "One two three."},
            {"script": "Four five six.", "media": "low.mp4", "sound": "keep"},
            {"script": "Seven eight nine.", "media": "high.mp4", "sound": "under"},
        ]})
        .to_string(),
    )
    .unwrap();
    let (code, v) = go(&e, "ok", &["make", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    gates_pass(&v);
    let video = v["artifacts"]["video"].as_str().unwrap();
    let s = starts(&e, &v);
    for (hz, at) in [(1000, s[1] + 0.25), (2500, s[2] + 0.2)] {
        let (before, there) = (
            band(&ff, video, hz, 0.1, 0.5),
            band(&ff, video, hz, at, 0.5),
        );
        assert!(
            there - before > 15.0,
            "{hz} Hz: scene 1 {before} dB, its scene {there} dB"
        );
    }
}
