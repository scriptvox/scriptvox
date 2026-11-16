//! `scriptvox edit --bleep`: a spoken word is replaced by a tone or by silence, the length stays, and the burned
//! caption shows a mask. The heard words come from `--words`, so no speech model runs here.
mod common;
use serde_json::Value;
use std::path::Path;

/// 6 s of picture with a 440 Hz "voice", and the words a speech row would hear in it. "pine apple" is said once in
/// order (2.0 to 2.9 s) and once the other way round (4.0 to 4.9 s).
fn talk(ff: &str, dir: &Path) {
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=320x180:r=25:d=6")
        .args(["-f", "lavfi", "-i", "sine=f=440:d=6"])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
        .args(["-shortest", "-y", "talk.mp4"])
        .status()
        .unwrap();
    assert!(st.success());
    let words = serde_json::json!({"words": [
        {"text": "We", "start": 0.4, "end": 0.7},
        {"text": "like", "start": 0.7, "end": 1.1},
        {"text": "Pine", "start": 2.0, "end": 2.4},
        {"text": "apple.", "start": 2.4, "end": 2.9},
        {"text": "Apple,", "start": 4.0, "end": 4.5},
        {"text": "pine", "start": 4.5, "end": 4.9},
    ]});
    std::fs::write(dir.join("words.json"), words.to_string()).unwrap();
}

fn rv(ff: &str, dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data")) // No speech row: the words come from --words.
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
}

/// The sound of a video as mono 48 kHz samples.
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

/// How strong `hz` is from `from` to `to` seconds: the amplitude of that sine in the part.
fn level(s: &[f64], from: f64, to: f64, hz: f64) -> f64 {
    let part = &s[(from * 48000.0) as usize..(to * 48000.0) as usize];
    let w = std::f64::consts::TAU * hz / 48000.0;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, x) in part.iter().enumerate() {
        re += x * (w * i as f64).cos();
        im += x * (w * i as f64).sin();
    }
    2.0 * re.hypot(im) / part.len() as f64
}

fn rms(s: &[f64], from: f64, to: f64) -> f64 {
    let part = &s[(from * 48000.0) as usize..(to * 48000.0) as usize];
    (part.iter().map(|x| x * x).sum::<f64>() / part.len() as f64).sqrt()
}

#[test]
fn bleep_with_silence_empties_the_word_and_keeps_the_rest_and_the_length() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "talk.mp4",
            "--bleep",
            "PINE apple!, banana",
            "--bleep-with",
            "silence",
            "--words",
            "words.json",
            "--out",
            "b.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    // (c) Two words in a row, in this order only.
    let m = v["bleep"]["matches"].as_array().unwrap();
    assert_eq!(m.len(), 1, "{v}");
    assert_eq!(m[0]["word"], "PINE apple!");
    assert_eq!(m[0]["start"], 2.0);
    assert_eq!(m[0]["end"], 2.9);
    assert_eq!(v["bleep"]["with"], "silence");
    // The replaced stretch: 0.05 s before the word and 0.2 s after it.
    assert_eq!(v["bleep"]["replaced"], serde_json::json!([[1.95, 3.1]]));
    // (d) The word that was not heard is a warning with its name.
    assert_eq!(v["bleep"]["not_heard"], serde_json::json!(["banana"]));
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "bleep_not_heard")
        .unwrap_or_else(|| panic!("{v}"));
    assert_eq!(f["status"], "warn");
    assert!(f["detail"].as_str().unwrap().contains("banana"), "{v}");
    // (b) Not a cut.
    assert_eq!(
        v["metrics"]["seconds_after"], v["metrics"]["seconds_before"],
        "{v}"
    );
    let s = samples(&ff, &d.path().join("b.mp4"));
    assert!((s.len() as f64 / 48000.0 - 6.0).abs() < 0.1, "{}", s.len());
    // (a) The word is gone: 60 dB under the sound around it, which is as loud after it as before it.
    let (before, inside, after) = (rms(&s, 0.5, 1.8), rms(&s, 2.0, 2.9), rms(&s, 3.2, 3.9));
    assert!(inside < before / 1000.0, "{inside} {before}");
    assert!((before / after - 1.0).abs() < 0.05, "{before} {after}");
    // The same words the other way round are still there.
    assert!(
        (rms(&s, 4.0, 4.9) / before - 1.0).abs() < 0.05,
        "the reversed words were touched"
    );
}

#[test]
fn bleep_with_a_tone_replaces_the_word_and_mixes_nothing_under_it() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "talk.mp4",
            "--bleep",
            "like",
            "--words",
            "words.json",
            "--out",
            "b.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["bleep"]["with"], "tone");
    assert_eq!(v["bleep"]["matches"][0]["start"], 0.7);
    let s = samples(&ff, &d.path().join("b.mp4"));
    // Inside: the 1 kHz tone, and the 440 Hz "voice" 40 dB or more under it.
    let (tone, voice) = (level(&s, 0.7, 1.1, 1000.0), level(&s, 0.7, 1.1, 440.0));
    assert!(tone > 0.01 && voice < tone / 100.0, "{tone} {voice}");
    // Outside: the voice, and no tone.
    let (tone, voice) = (level(&s, 2.0, 3.0, 1000.0), level(&s, 2.0, 3.0, 440.0));
    assert!(voice > 0.01 && tone < voice / 100.0, "{tone} {voice}");
    // The tone is not louder than the voice it replaces.
    assert!(
        rms(&s, 0.7, 1.1) < rms(&s, 2.0, 3.0),
        "the tone is louder than the voice"
    );
    assert_eq!(
        v["metrics"]["seconds_after"], v["metrics"]["seconds_before"],
        "{v}"
    );
}

#[test]
fn a_bleeped_word_is_a_mask_in_the_captions() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    let (code, v) = rv(
        &ff,
        d.path(),
        &[
            "edit",
            "talk.mp4",
            "--bleep",
            "pine apple",
            "--captions",
            "karaoke",
            "--words",
            "words.json",
            "--out",
            "b.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    let srt = std::fs::read_to_string(d.path().join("b.srt")).unwrap();
    // The first "Pine apple." is masked letter for letter and keeps its full stop; the reversed pair stays.
    assert!(srt.contains("**** *****."), "{srt}");
    assert!(srt.contains("Apple, pine"), "{srt}");
    assert!(!srt.contains("Pine"), "{srt}");
    let ass = std::fs::read_to_string(d.path().join(".scriptvox/edit/talk/captions.ass")).unwrap();
    assert!(ass.contains("****") && !ass.contains("Pine"), "{ass}");
    assert_eq!(v["captions"]["words"], 6, "{v}");
    assert_eq!(v["captions"]["masked"], 2, "{v}");
}

#[test]
fn bleep_refuses_a_change_of_the_timeline_and_a_run_that_hides_nothing() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    talk(&ff, d.path());
    let base = [
        "edit",
        "talk.mp4",
        "--words",
        "words.json",
        "--out",
        "b.mp4",
    ];
    // (f) Bleep times are the source's times.
    for extra in [
        &["--bleep", "pine", "--speed", "1.25"][..],
        &["--bleep", "pine", "--keep", "0-3"],
        &["--bleep", "pine", "--cut-silence"],
        &["--bleep", "pine", "--from", "1"],
    ] {
        let (code, v) = rv(&ff, d.path(), &[&base[..], extra].concat());
        assert_eq!(code, Some(2), "{extra:?} {v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains("--bleep") && msg.contains("two calls"), "{v}");
    }
    for extra in [
        &["--bleep", "pine", "--bleep-with", "horn"][..],
        &["--bleep", " , !!"],
        &["--bleep", "pine", "--bleep-pad", "3"],
        &["--bleep-with", "silence", "--reframe", "1:1"],
    ] {
        let (code, v) = rv(&ff, d.path(), &[&base[..], extra].concat());
        assert_eq!(code, Some(2), "{extra:?} {v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains("--bleep"),
            "{v}"
        );
    }
    // No word was heard at all: no file, so nobody takes an unchanged video for a clean one.
    let (code, v) = rv(&ff, d.path(), &[&base[..], &["--bleep", "banana"]].concat());
    assert_eq!(code, Some(3), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("banana"),
        "{v}"
    );
    assert!(!d.path().join("b.mp4").exists());
    // No --words and no speech row: the tool's usual answer.
    let (code, v) = rv(
        &ff,
        d.path(),
        &["edit", "talk.mp4", "--bleep", "pine", "--out", "b.mp4"],
    );
    assert_eq!(code, Some(10), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no speech to text"),
        "{v}"
    );
}
