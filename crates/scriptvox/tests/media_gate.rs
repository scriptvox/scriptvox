//! Every media file goes to FFmpeg and ffprobe only as what its first bytes say it is (`docs/specs/media-inputs.md`).
//! A list under a picture's, a clip's or a song's name (ffconcat, HLS) is refused with exit 2, and neither the list
//! nor the file it names is ever given to FFmpeg or ffprobe.
#![cfg(unix)]
mod common;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A folder outside the project with a real clip in it: what a list would make FFmpeg open.
fn outside(ff: &str) -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    common::ffmpeg(
        ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=160x90:r=25:d=1",
            "-pix_fmt",
            "yuv420p",
            "secret-target.mp4",
        ],
    );
    let secret = d.path().join("secret-target.mp4");
    (d, secret)
}

fn ffconcat(target: &Path) -> String {
    format!("ffconcat version 1.0\nfile '{}'\n", target.display())
}

fn m3u8(target: &Path) -> String {
    format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:1\n#EXTINF:1.0,\n{}\n#EXT-X-ENDLIST\n",
        target.display()
    )
}

/// A 3 s test clip with a tone, a WAV, an MP3, a PNG and a JPEG in `dir`.
fn real_files(ff: &str, dir: &Path) {
    common::ffmpeg(
        ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=25:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "talk.mp4",
        ],
    );
    common::ffmpeg(
        ff,
        dir,
        &["-f", "lavfi", "-i", "sine=f=220:d=4", "song.wav"],
    );
    common::ffmpeg(
        ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=f=220:d=4",
            "-c:a",
            "libmp3lame",
            "song.mp3",
        ],
    );
    common::ffmpeg(
        ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=200x100",
            "-frames:v",
            "1",
            "mark.png",
        ],
    );
    common::ffmpeg(
        ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360",
            "-frames:v",
            "1",
            "photo.jpg",
        ],
    );
}

fn run(dir: &Path, ff: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env("SCRIPTVOX_DATA", dir.join("data"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

/// Exit 2, the message names the file, what it is and what is accepted, and no FFmpeg or ffprobe call was given the
/// list or the file it names.
fn refused(code: Option<i32>, v: &Value, log: &Path, list: &str, what: &str) {
    let msg = v["error"]["message"].as_str().unwrap_or("");
    assert_eq!(code, Some(2), "{list}: {v}");
    assert!(
        msg.contains(list) && msg.contains(what) && msg.contains("Accepted:"),
        "{list}: {v}"
    );
    let opened = std::fs::read_to_string(log).unwrap_or_default();
    assert!(!opened.contains(list), "{list} reached FFmpeg: {opened}");
    assert!(
        !opened.contains("secret-target"),
        "the outside file reached FFmpeg: {opened}"
    );
}

#[test]
fn edit_refuses_a_list_as_the_video_the_music_or_the_logo() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (_keep, secret) = outside(&ff);
    real_files(&ff, p);
    std::fs::write(p.join("list-video.mp4"), m3u8(&secret)).unwrap();
    std::fs::write(p.join("list-song.mp3"), ffconcat(&secret)).unwrap();
    std::fs::write(p.join("list-mark.png"), ffconcat(&secret)).unwrap();
    std::fs::write(p.join("list-broll.mp4"), ffconcat(&secret)).unwrap();
    let (rec, log) = common::record(&ff, p);
    for (args, list, what) in [
        (
            &["edit", "list-video.mp4", "--cut-silence"][..],
            "list-video.mp4",
            "an HLS playlist",
        ),
        (
            &["edit", "talk.mp4", "--music", "list-song.mp3"],
            "list-song.mp3",
            "an ffconcat list",
        ),
        (
            &["edit", "talk.mp4", "--logo", "list-mark.png"],
            "list-mark.png",
            "an ffconcat list",
        ),
        (
            &["edit", "talk.mp4", "--broll", "list-broll.mp4@0.5:1"],
            "list-broll.mp4",
            "an ffconcat list",
        ),
    ] {
        let _ = std::fs::remove_file(&log);
        let (code, v) = run(p, &rec, args);
        refused(code, &v, &log, list, what);
    }
}

#[test]
fn check_snapshot_analyze_and_compare_refuse_a_list() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (_keep, secret) = outside(&ff);
    real_files(&ff, p);
    std::fs::write(p.join("list-a.mp4"), ffconcat(&secret)).unwrap();
    std::fs::write(p.join("list-b.mp4"), m3u8(&secret)).unwrap();
    let (rec, log) = common::record(&ff, p);
    for (args, list, what) in [
        (
            &["check", "list-a.mp4"][..],
            "list-a.mp4",
            "an ffconcat list",
        ),
        (&["check", "list-b.mp4"], "list-b.mp4", "an HLS playlist"),
        (
            &["snapshot", "list-a.mp4", "--sheet", "4"],
            "list-a.mp4",
            "an ffconcat list",
        ),
        (&["analyze", "list-b.mp4"], "list-b.mp4", "an HLS playlist"),
        (
            &["compare", "talk.mp4", "list-a.mp4"],
            "list-a.mp4",
            "an ffconcat list",
        ),
        (
            &["join", "talk.mp4", "list-a.mp4"],
            "list-a.mp4",
            "an ffconcat list",
        ),
        (&["clips", "list-a.mp4"], "list-a.mp4", "an ffconcat list"),
    ] {
        let _ = std::fs::remove_file(&log);
        let (code, v) = run(p, &rec, args);
        refused(code, &v, &log, list, what);
    }
}

#[test]
fn make_refuses_a_scene_picture_or_a_logo_that_is_a_list() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    let (_keep, secret) = outside(&ff);
    real_files(&ff, p);
    std::fs::write(p.join("list-pic.png"), ffconcat(&secret)).unwrap();
    std::fs::write(p.join("list-clip.mp4"), m3u8(&secret)).unwrap();
    std::fs::write(p.join("list-song.mp3"), ffconcat(&secret)).unwrap();
    let (rec, log) = common::record(&ff, p);
    let make = |brief: &str, extra: &[&str]| {
        std::fs::write(p.join("b.json"), brief).unwrap();
        let _ = std::fs::remove_file(&log);
        let o = e
            .cmd("ok")
            .env("SCRIPTVOX_FFMPEG", &rec)
            .args(["make", "--brief", "b.json", "--json"])
            .args(extra)
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    let scene = |file: &str| {
        format!(r#"{{"scenes":[{{"script":"One two three four five.","media":"{file}"}}]}}"#)
    };
    let (code, v) = make(&scene("list-pic.png"), &[]);
    refused(code, &v, &log, "list-pic.png", "an ffconcat list");
    let (code, v) = make(&scene("list-clip.mp4"), &[]);
    refused(code, &v, &log, "list-clip.mp4", "an HLS playlist");
    // The plan refuses the same file: nothing is priced or bought for a brief that cannot run.
    let (code, v) = make(&scene("list-pic.png"), &["--plan"]);
    refused(code, &v, &log, "list-pic.png", "an ffconcat list");
    let (code, v) = make(
        r#"{"script":"One two three four five.","logo":{"file":"list-pic.png"}}"#,
        &[],
    );
    refused(code, &v, &log, "list-pic.png", "an ffconcat list");
    let (code, v) = make(
        r#"{"script":"One two three four five.","music":"list-song.mp3"}"#,
        &[],
    );
    refused(code, &v, &log, "list-song.mp3", "an ffconcat list");
    let (code, v) = make(
        r#"{"script":"One two three four five.","narration":"list-song.mp3"}"#,
        &[],
    );
    refused(code, &v, &log, "list-song.mp3", "an ffconcat list");
}

/// Real files still work through the main paths, and each is opened from the file protocol with its format named.
#[test]
fn real_pictures_clips_and_sounds_still_work() {
    let Some(ff) = common::ready() else { return };
    let e = common::Env::new(&ff);
    let p = e.dir.path();
    real_files(&ff, p);
    // A PNG under a JPEG's name is still a PNG.
    std::fs::copy(p.join("mark.png"), p.join("mark-as.jpg")).unwrap();
    let (rec, log) = common::record(&ff, p);
    for (args, opened_as) in [
        (
            &["edit", "talk.mp4", "--music", "song.wav", "--out", "o1.mp4"][..],
            "-protocol_whitelist file -f wav -i",
        ),
        (
            &["edit", "talk.mp4", "--music", "song.mp3", "--out", "o2.mp4"],
            "-protocol_whitelist file -f mp3 -i",
        ),
        (
            &["edit", "talk.mp4", "--logo", "mark.png", "--out", "o3.mp4"],
            "-f image2 -pattern_type none -c:v png",
        ),
        (
            &[
                "edit",
                "talk.mp4",
                "--logo",
                "mark-as.jpg",
                "--out",
                "o4.mp4",
            ],
            "-f image2 -pattern_type none -c:v png",
        ),
        (&["check", "talk.mp4"], "-protocol_whitelist file -f mov -i"),
        (
            &["snapshot", "talk.mp4", "--sheet", "4"],
            "-protocol_whitelist file -f mov -i",
        ),
    ] {
        let _ = std::fs::remove_file(&log);
        let (code, v) = run(p, &rec, args);
        assert!(matches!(code, Some(0)), "{args:?}: {v}");
        let opened = std::fs::read_to_string(&log).unwrap();
        assert!(opened.contains(opened_as), "{args:?}: {opened}");
    }
    // `make` with a PNG, a JPEG and a clip as the scenes' own pictures, a logo and a music file.
    std::fs::write(
        p.join("b.json"),
        r#"{"scenes":[{"script":"One two three.","media":"mark.png"},{"script":"Four five six.","media":"photo.jpg"},
            {"script":"Seven eight nine.","media":"talk.mp4"}],"logo":{"file":"mark.png"},"music":"song.mp3"}"#,
    )
    .unwrap();
    let _ = std::fs::remove_file(&log);
    let o = e
        .cmd("ok")
        .env("SCRIPTVOX_FFMPEG", &rec)
        .args(["make", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    assert_eq!(o.status.code(), Some(0), "{v}");
    let opened = std::fs::read_to_string(&log).unwrap();
    for want in ["-c:v png", "-c:v mjpeg", "-f mov -i", "-f mp3 -i"] {
        assert!(opened.contains(want), "{want}: {opened}");
    }
    // No input is left to FFmpeg's own guess: every `-i` has a format named before it.
    for line in opened.lines() {
        let words: Vec<&str> = line.split(' ').collect();
        let mut from = 0;
        for (i, w) in words.iter().enumerate() {
            if *w == "-i" {
                assert!(
                    words[from..i].contains(&"-f"),
                    "an input with no format: {line}"
                );
                from = i + 2;
            }
        }
    }
}
