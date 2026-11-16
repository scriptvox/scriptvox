//! Quality gates, check and snapshot (RVX-024).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

fn gen(ff: &str, out: &Path, args: &[&str]) {
    let st = std::process::Command::new(ff)
        .args(["-v", "error", "-y"])
        .args(args)
        .arg(out)
        .status()
        .unwrap();
    assert!(st.success());
}

fn check(target: &Path) -> (i32, Value) {
    let o = common::bin()
        .args(["check"])
        .arg(target)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

fn gate<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == name)
        .unwrap()
}

#[test]
fn thirty_seconds_of_black_fails_with_a_fix() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("black.mp4");
    gen(
        &ff,
        &f,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x240:r=30:d=30",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=30",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ],
    );
    let (code, v) = check(&f);
    assert_eq!(code, 3, "{v}");
    assert_eq!(gate(&v, "black")["status"], "fail");
    assert!(v["error"]["fix"].is_string());
}

// G188: two measures with no model. A clean clip passes both.
#[test]
fn a_black_first_frame_and_a_strobe_are_warnings() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let tone = ["-f", "lavfi", "-i", "sine=f=440:d=4"];
    let tail = [
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-shortest",
    ];
    let clip = |name: &str, picture: &str| {
        let f = d.path().join(name);
        gen(
            &ff,
            &f,
            &[&["-f", "lavfi", "-i", picture][..], &tone, &tail].concat(),
        );
        check(&f)
    };
    // Two black frames, then a picture.
    let (code, v) = clip(
        "start.mp4",
        "testsrc2=s=320x240:r=30:d=4,geq=lum='if(lt(N,2),0,lum(X,Y))':cb='if(lt(N,2),128,cb(X,Y))':cr='if(lt(N,2),128,cr(X,Y))'",
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(gate(&v, "first_frame")["status"], "warn", "{v}");
    assert_eq!(gate(&v, "flashes")["status"], "pass", "{v}");
    // Short black stretches do not count for the black gate.
    assert_eq!(gate(&v, "black")["detail"], "0.0 s of black", "{v}");
    // Bright and dark, 5 times a second, for 1 s.
    let (code, v) = clip(
        "strobe.mp4",
        "color=c=white:s=320x240:r=30:d=4,geq=lum='if(between(T,1,2)*lt(mod(N,6),3),16,200)':cb=128:cr=128",
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(gate(&v, "flashes")["status"], "warn", "{v}");
    assert_eq!(gate(&v, "first_frame")["status"], "pass", "{v}");
    // A moving picture with no cut.
    let (_, v) = clip("clean.mp4", "testsrc2=s=320x240:r=30:d=4");
    assert_eq!(gate(&v, "flashes")["status"], "pass", "{v}");
    assert_eq!(gate(&v, "first_frame")["status"], "pass", "{v}");
}

#[test]
fn av_drift_above_one_frame_fails() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("drift.mp4");
    gen(
        &ff,
        &f,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=30:d=5",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=3",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ],
    );
    let (code, v) = check(&f);
    assert_eq!(code, 3, "{v}");
    assert_eq!(gate(&v, "av_drift")["status"], "fail");
}

#[test]
fn frozen_video_is_a_slideshow() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("still.mp4");
    gen(
        &ff,
        &f,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x336699:s=320x240:r=30:d=8",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=8",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ],
    );
    let (code, v) = check(&f);
    assert_eq!(code, 3, "{v}");
    assert_eq!(gate(&v, "slideshow")["status"], "fail");
}

#[test]
fn every_template_passes_the_gates() {
    let Some(ff) = ready() else { return };
    // Gradient template and own media (stills get a Ken Burns zoom).
    for media in [false, true] {
        let e = Env::new(&ff);
        if media {
            with_media(&e);
        }
        let extra: Vec<&str> = if media {
            vec!["--media", "media"]
        } else {
            vec![]
        };
        let (code, v) = e.make(
            "Every template must move and sound right. No slideshows here.",
            "ok",
            &extra,
        );
        assert_eq!(code, 0, "media={media}: {v}");
        assert_eq!(
            gate(&v, "slideshow")["status"],
            "pass",
            "media={media}: {v}"
        );
    }
}

#[test]
fn snapshot_sheet_writes_one_png() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("v.mp4");
    gen(
        &ff,
        &f,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=30:d=4",
            "-c:v",
            "libx264",
        ],
    );
    std::fs::write(
        d.path().join("v.srt"),
        "1\n00:00:00,000 --> 00:00:02,000\nfirst line\n\n",
    )
    .unwrap();
    let o = common::bin()
        .args(["snapshot"])
        .arg(&f)
        .args(["--sheet", "9", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let png = std::fs::read(v["artifacts"]["image"].as_str().unwrap()).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // Each frame carries its time and the caption on screen then (from the SRT next to the video).
    let frames = v["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 9);
    assert_eq!(frames[0]["caption"], "first line");
    assert!(
        frames[8]["caption"].is_null() && frames[8]["t"].as_f64().unwrap() > 3.0,
        "{v}"
    );
}

#[test]
fn a_title_too_wide_for_the_frame_fails_with_its_scene_and_keeps_the_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |text: &str| {
        serde_json::json!({"tts": "none", "aspect": "9:16", "scenes": [
            {"script": "A first scene with a short title.", "card": {"kind": "title", "text": "Short"}},
            {"script": "A second scene with a long word.", "card": {"kind": "title", "text": text}},
        ]})
        .to_string()
    };
    let make = |name: &str, text: &str| {
        std::fs::write(e.dir.path().join(name), brief(text)).unwrap();
        let o = e
            .cmd("ok")
            .args(["make", "--brief", name, "--json"])
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = make("wide.json", "Pneumonoultramicroscopicsilicovolcanoconiosis");
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "SVX_GATE_FAILED", "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("scene 2"),
        "{v}"
    );
    assert!(
        v["artifacts"]["video"].is_string(),
        "the video is kept: {v}"
    );
    // `check` on that run agrees with `make`: same gate, same exit.
    let o = e
        .cmd("ok")
        .args(["check", v["run_id"].as_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let c: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(3), "{c}");
    assert!(
        c["error"]["message"]
            .as_str()
            .unwrap()
            .contains("text_bounds"),
        "{c}"
    );
    let (code, v) = make("fits.json", "Pneumonia");
    assert_eq!(code, 0, "{v}");
    let found = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "text_bounds");
    assert_eq!(found.unwrap()["status"], "pass", "{v}");
}

#[test]
fn a_card_with_on_and_hold_lands_on_its_word_in_the_made_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |on: &str| {
        serde_json::json!({"tts": "none", "aspect": "16:9", "scenes": [
            {"script": "A first scene with a short title.", "card": {"kind": "title", "text": "Short"}},
            {"script": "Last year was quiet, but this year revenue grew by forty two percent for us all.",
             "card": {"kind": "stat", "text": "42%", "label": "growth", "on": on, "hold": 1.5}},
        ]})
        .to_string()
    };
    let run = |cmd: &str, on: &str| {
        std::fs::write(e.dir.path().join("b.json"), brief(on)).unwrap();
        let o = e
            .cmd("ok")
            .args([cmd, "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap(),
        )
    };
    // A word the scene does not say: refused before any work, with the word and the scene.
    let (code, v) = run("plan", "fifty");
    let msg = v["error"]["message"].as_str().unwrap_or_default();
    assert!(
        code != 0 && msg.contains("scene 2") && msg.contains("\"fifty\""),
        "{v}"
    );
    let (code, v) = run("make", "forty");
    assert_eq!(code, 0, "{v}");
    let dir = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    let read = |f: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(f)).unwrap()).unwrap()
    };
    let (cards, words, scenes) = (read("cards.json"), read("words.json"), read("scenes.json"));
    let forty = words
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["text"] == "forty")
        .unwrap()["start"]
        .as_f64()
        .unwrap();
    let num = |v: &serde_json::Value| v.as_f64().unwrap();
    // The title has neither field: it starts with its scene. The stat starts on its word and stays 1.5 s.
    assert_eq!(num(&cards[0]["start"]), num(&scenes[0]), "{cards}");
    let (start, end, landed) = (
        num(&cards[1]["start"]),
        num(&cards[1]["end"]),
        num(&cards[1]["landed"]),
    );
    assert!(forty > num(&scenes[1]) + 1.0, "{words}");
    assert_eq!(start, forty, "{cards}");
    assert!((end - start - 1.5).abs() < 1e-6, "{cards}");
    assert!(landed > start && landed < end, "{cards}");
    let found = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "text_bounds");
    assert_eq!(found.unwrap()["status"], "pass", "{v}");
}

#[test]
fn check_refuses_a_file_that_is_not_a_whole_video() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = |n: &str| d.path().join(n);
    let noise: Vec<u8> = (0..200_000u32)
        .map(|i| (i.wrapping_mul(i) % 251) as u8)
        .collect();
    std::fs::write(p("garbage.mp4"), noise).unwrap();
    std::fs::write(p("fake.mp4"), "hello\n").unwrap();
    std::fs::write(p("empty.mp4"), "").unwrap();
    gen(
        &ff,
        &p("audio.m4a"),
        &["-f", "lavfi", "-i", "sine=f=300:d=2", "-c:a", "aac"],
    );
    gen(
        &ff,
        &p("whole.mp4"),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=30:d=6",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=6",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
            "-movflags",
            "+faststart",
        ],
    );
    let whole = std::fs::read(p("whole.mp4")).unwrap();
    std::fs::write(p("cut.mp4"), &whole[..whole.len() / 3]).unwrap();
    for n in [
        "garbage.mp4",
        "fake.mp4",
        "empty.mp4",
        "audio.m4a",
        "cut.mp4",
    ] {
        let (code, v) = check(&p(n));
        assert_eq!(code, 2, "{n}: {v}");
    }
    let (code, v) = check(&p("whole.mp4"));
    assert_eq!(code, 0, "{v}");
    gen(
        &ff,
        &p("mute.mp4"),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=30:d=3",
            "-c:v",
            "libx264",
        ],
    );
    let (_, v) = check(&p("mute.mp4"));
    let gates = v["findings"].to_string();
    assert!(
        !gates.contains("\"silence\"") && gates.contains("\"loudness\""),
        "no sound, no silence pass: {v}"
    );
}

#[test]
fn snapshot_never_writes_over_its_video() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("v.mp4");
    gen(
        &ff,
        &v,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30:d=2",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let before = std::fs::read(&v).unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .args(["snapshot", "v.mp4", "--at", "1", "--out", "v.mp4", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert_eq!(std::fs::read(&v).unwrap(), before);
}

/// editing-6: a picture and a song's cover art are no video stream, under any file name.
#[test]
fn a_still_picture_and_cover_art_are_not_a_video() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let png = d.path().join("photo.png");
    gen(
        &ff,
        &png,
        &["-f", "lavfi", "-i", "color=c=red:s=64x64", "-frames:v", "1"],
    );
    let renamed = d.path().join("photo_as.mp4");
    std::fs::copy(&png, &renamed).unwrap();
    let song = d.path().join("song.m4a");
    let png_s = png.to_str().unwrap();
    gen(
        &ff,
        &song,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=2",
            "-i",
            png_s,
            "-map",
            "0",
            "-map",
            "1",
            "-c:a",
            "aac",
            "-c:v",
            "png",
            "-disposition:v",
            "attached_pic",
        ],
    );
    for f in [&png, &renamed, &song] {
        let (code, v) = check(f);
        assert_eq!(code, 2, "{f:?}: {v}");
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no video stream"),
            "{v}"
        );
    }
    let o = common::bin()
        .current_dir(d.path())
        .args(["export", "photo.png", "--json"])
        .output()
        .unwrap();
    assert_ne!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
}

/// contract-2: with no vision service, nothing tells the agent to run a command that exits 10.
#[test]
fn check_names_judge_only_when_a_vision_service_is_set() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("v.mp4");
    gen(
        &ff,
        &v,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30:d=2",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let o = common::bin()
        .env_remove("SCRIPTVOX_VISION_URL")
        .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
        .args(["check"])
        .arg(&v)
        .arg("--json")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(!text.contains("run `scriptvox judge`"), "{text}");
}

/// With no speech row on this install, `not_checked` names the one command that gives the free local one.
#[test]
fn check_with_no_speech_row_names_the_setup_command() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("v.mp4");
    gen(
        &ff,
        &v,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30:d=2",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let o = common::bin()
        .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .args(["check"])
        .arg(&v)
        .args(["--local-only", "--json"])
        .output()
        .unwrap();
    let c: Value = serde_json::from_slice(&o.stdout).unwrap();
    let words = c["not_checked"][0].as_str().unwrap();
    assert!(
        words.starts_with("words: ") && words.contains("`scriptvox setup speech-local`"),
        "{c}"
    );
}

#[test]
fn check_budget_stops_the_read_and_says_the_result_is_partial() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    // 150 s at 1920x1080: a full read takes a few seconds, the budget is 1 s.
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=1920x1080:r=30:d=150",
        ])
        .args(["-f", "lavfi", "-i", "sine=f=300:d=150"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "long.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["check", "long.mp4", "--json"])
            .args(args)
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    let gate = |v: &serde_json::Value, g: &str| {
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["gate"] == g)
            .cloned()
    };
    let (code, v) = run(&["--budget", "1"]);
    assert_eq!(code, 0, "{v}");
    // A machine that reads 150 s of 1080p in under 1 s gives the whole result: then there is nothing partial to test.
    if v["partial"] == true {
        let b = gate(&v, "budget").unwrap();
        assert_eq!(b["status"], "warn", "{v}");
        let read: f64 = b["detail"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .nth(3)
            .unwrap()
            .parse()
            .unwrap();
        assert!(read < 150.0, "{b}");
        assert!(
            gate(&v, "loudness").unwrap()["detail"]
                .as_str()
                .unwrap()
                .contains("not measured"),
            "{v}"
        );
    } else {
        assert!(gate(&v, "budget").is_none(), "{v}");
    }
    // No budget: the whole file, no `budget` gate, the loudness as a number.
    let (code, v) = run(&[]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["partial"].is_null() && gate(&v, "budget").is_none(),
        "{v}"
    );
    assert!(
        gate(&v, "loudness").unwrap()["detail"]
            .as_str()
            .unwrap()
            .contains("LUFS"),
        "{v}"
    );
    for bad in [
        &["--budget", "0.2"][..],
        &["--budget", "99999"],
        &["--budget", "5", "--hear"],
    ] {
        assert_eq!(run(bad).0, 2, "{bad:?}");
    }
}

/// G057: `check --platform` fails a frame of the wrong shape, and `snapshot --safe` shades zones only for a row that
/// has them.
#[test]
fn check_platform_fails_a_wrong_shape_and_snapshot_safe_needs_zones() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("v.mp4");
    gen(
        &ff,
        &f,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=180x320:r=30:d=2",
            "-c:v",
            "libx264",
        ],
    );
    let run = |args: &[&str]| {
        let o = common::bin().args(args).arg("--json").output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    let p = f.to_str().unwrap();
    let (code, v) = run(&["check", p, "--platform", "youtube-shorts"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(gate(&v, "platform")["status"], "pass", "{v}");
    let (code, v) = run(&["check", p, "--platform", "youtube"]);
    assert_eq!(code, 3, "{v}");
    assert!(
        gate(&v, "platform")["detail"]
            .as_str()
            .unwrap()
            .contains("9:16"),
        "{v}"
    );
    assert_eq!(run(&["check", p, "--platform", "myspace"]).0, 2);
    let (code, v) = run(&["snapshot", p, "--at", "1", "--safe", "tiktok"]);
    assert_eq!(code, 0, "{v}");
    let png = std::fs::read(v["artifacts"]["image"].as_str().unwrap()).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    assert_eq!(run(&["snapshot", p, "--at", "1", "--safe", "youtube"]).0, 2);
}

/// T3, 13 Nov 2026: `check <file.mp4> --hear` stopped with "--hear needs a script" though the file held its recipe.
/// The script now comes from the recipe; only a file with no recipe asks for `--script`. No speech model is here, so
/// the check of the file with a recipe stops one step later, at the missing speech to text.
#[test]
fn check_hear_on_a_file_takes_the_script_from_its_recipe() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Six words are said in here.", "ok", &["--out", "a.mp4"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = e.make(
        "Six words are said in here.",
        "ok",
        &["--out", "bare.mp4", "--no-embed"],
    );
    assert_eq!(code, 0, "{v}");
    // `--local-only`: this test never reaches a paid speech row, whatever keys the machine has.
    let hear = |file: &str| -> Value {
        let o = e
            .cmd("ok")
            .args(["check", file, "--hear", "--local-only", "--json"])
            .output()
            .unwrap();
        serde_json::from_slice(&o.stdout).unwrap()
    };
    let v = hear("a.mp4");
    let msg = v["error"]["message"].as_str().unwrap_or_default();
    assert!(
        !msg.starts_with("--hear needs a script"),
        "the script is in the recipe: {v}"
    );
    assert!(
        msg.contains("speech to text"),
        "what is missing here is the listening: {v}"
    );
    let v = hear("bare.mp4");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .starts_with("--hear needs a script"),
        "{v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap_or_default()
            .contains("--script"),
        "{v}"
    );
}
