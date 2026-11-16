//! `scriptvox edit`, the small operations (docs/specs/edit-small-ops.md): `--remove-bars`, `--hold`, `--loop`,
//! `--loop-to`, `--deinterlace`, `--tilt`. Each on a clip made here, with the result measured. No network.
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

const TONE: &str = "sine=f=440:d=4";

fn clip(ff: &str, dir: &Path, picture: &str, more: &[&str], name: &str) {
    let mut a = vec!["-f", "lavfi", "-i", picture, "-f", "lavfi", "-i", TONE];
    a.extend(more);
    a.extend([
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", name,
    ]);
    ffmpeg(ff, dir, &a);
}

fn rv(ff: &str, dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
}

fn frames(ff: &str, file: &Path) -> u64 {
    let o = std::process::Command::new(Path::new(ff).with_file_name("ffprobe"))
        .args(["-v", "error", "-select_streams", "v:0", "-count_frames"])
        .args(["-show_entries", "stream=nb_read_frames", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().parse().unwrap()
}

/// The darkest and the brightest green value of the four corner pixels of a frame.
fn corners(ff: &str, file: &Path, (w, h): (usize, usize)) -> (u8, u8) {
    let px = frame(ff, file, 1.0, (w, h));
    let c = [px[0], px[w - 1], px[(h - 1) * w], px[h * w - 1]];
    let sum = |p: [u8; 4]| ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3) as u8;
    (
        c.iter().map(|p| sum(*p)).min().unwrap(),
        c.iter().map(|p| sum(*p)).max().unwrap(),
    )
}

#[test]
fn bars_are_found_and_cropped_and_a_clip_with_none_is_refused() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    // A light picture of 320x136 with 22 px of black over and under it.
    clip(
        &ff,
        p,
        "color=c=0x80A0C0:s=320x136:r=25:d=4",
        &["-vf", "pad=320:180:0:22:black"],
        "barred.mp4",
    );
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "barred.mp4", "--remove-bars", "--out", "o.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["bars"]["crop"], "320:136:0:22", "{v}");
    assert_eq!(v["bars"]["removed_px"]["top"], 22);
    assert_eq!(v["metrics"]["size"], "320x136");
    let (dark, _) = corners(&ff, &p.join("o.mp4"), (320, 136));
    assert!(dark > 100, "a corner is still black: {dark}");
    // No bars: a usage error with a fix, and no file.
    clip(
        &ff,
        p,
        "color=c=0x80A0C0:s=320x180:r=25:d=4",
        &[],
        "full.mp4",
    );
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "full.mp4", "--remove-bars", "--out", "n.mp4"],
    );
    assert_eq!(code, Some(2), "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("no black bars"));
    assert!(v["error"]["fix"]
        .as_str()
        .unwrap()
        .contains("--remove-bars"));
    assert!(!p.join("n.mp4").exists());
    // With another edit option: refused, with the order of work in the fix.
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "barred.mp4", "--remove-bars", "--speed", "1.5"],
    );
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("two calls")
            || v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("two calls"),
        "{v}"
    );
}

#[test]
fn a_hold_adds_its_seconds_and_a_loop_plays_every_frame_again() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    clip(
        &ff,
        p,
        "testsrc2=s=320x180:r=25:d=4",
        &["-shortest"],
        "in.mp4",
    );
    let before = frames(&ff, &p.join("in.mp4"));
    assert_eq!(before, 100);
    let (code, v) = rv(
        &ff,
        p,
        &[
            "edit", "in.mp4", "--hold", "1.0@2", "--hold", "3@0.5", "--out", "h.mp4",
        ],
    );
    assert_eq!(code, Some(0), "{v}");
    // 2.5 s more at 25 fps: 62 or 63 frames.
    let got = frames(&ff, &p.join("h.mp4"));
    assert!((162..=163).contains(&got), "{got}");
    // The sound ends with its last AAC frame, some ms after the picture.
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((6.5..6.56).contains(&after), "{v}");
    assert_eq!(v["hold"][0]["at"], 1.0);
    // The picture at 1.2 s and at 2.8 s is the held frame; at 3.5 s it moves again.
    let at = |t: f64| frame(&ff, &p.join("h.mp4"), t, (320, 180));
    let diff = |a: &[[u8; 4]], b: &[[u8; 4]]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| u64::from(x[1].abs_diff(y[1])))
            .sum::<u64>() as f64
            / a.len() as f64
    };
    let (held_a, held_b, moving) = (at(1.2), at(2.8), at(3.5));
    assert!(diff(&held_a, &held_b) < 1.0, "{}", diff(&held_a, &held_b));
    assert!(diff(&held_a, &moving) > 2.0, "{}", diff(&held_a, &moving));

    let (code, v) = rv(&ff, p, &["edit", "in.mp4", "--loop", "3", "--out", "l.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(frames(&ff, &p.join("l.mp4")), 300, "{v}");
    assert_eq!(v["loop"]["plays"], 3);
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "in.mp4", "--loop-to", "10", "--out", "t.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    let after = v["metrics"]["seconds_after"].as_f64().unwrap();
    assert!((9.95..10.05).contains(&after), "{v}");
    // The sound of a play is some ms longer than its picture, so each seam holds the last frame for that time.
    let got = frames(&ff, &p.join("t.mp4"));
    assert!((246..=250).contains(&got), "{got}");
    // Errors, each with a fix.
    for (args, word) in [
        (vec!["--hold", "9@1"], "inside the video"),
        (vec!["--hold", "nine"], "AT@SECONDS"),
        (vec!["--loop", "1"], "2 to 100"),
        (vec!["--loop-to", "2"], "not longer"),
        (vec!["--tilt", "40"], "-15 to 15"),
    ] {
        let mut a = vec!["edit", "in.mp4"];
        a.extend(args);
        let (code, v) = rv(&ff, p, &a);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(word),
            "{v}"
        );
        assert!(
            v["error"]["fix"].as_str().is_some_and(|f| !f.is_empty()),
            "{v}"
        );
    }
}

#[test]
fn deinterlace_needs_an_interlaced_stream_and_tilt_leaves_no_empty_corner() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    // 50 pictures a second woven into 25 interlaced frames, top field first.
    clip(
        &ff,
        p,
        "testsrc2=s=320x240:r=50:d=4",
        &[
            "-vf",
            "tinterlace=interleave_top,fieldorder=tff",
            "-flags",
            "+ilme+ildct",
            "-shortest",
        ],
        "i.mp4",
    );
    assert_eq!(
        probe(&ff, &p.join("i.mp4"), "v:0", "stream=field_order"),
        "tt"
    );
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "i.mp4", "--deinterlace", "--out", "p.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(
        probe(&ff, &p.join("p.mp4"), "v:0", "stream=field_order"),
        "progressive"
    );
    assert_eq!(frames(&ff, &p.join("p.mp4")), frames(&ff, &p.join("i.mp4")));
    let de = &v["deinterlace"];
    assert_eq!(de["stream_says_interlaced"], true, "{v}");
    assert!(
        de["before"]["interlaced"].as_u64() > de["before"]["progressive"].as_u64(),
        "{v}"
    );
    assert!(
        de["after"]["interlaced"].as_u64() < de["after"]["progressive"].as_u64(),
        "{v}"
    );

    // A progressive clip: refused, nothing written.
    clip(
        &ff,
        p,
        "color=c=0x80A0C0:s=320x180:r=25:d=4",
        &[],
        "flat.mp4",
    );
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "flat.mp4", "--deinterlace", "--out", "n.mp4"],
    );
    assert_eq!(code, Some(2), "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not interlaced"));
    assert!(!p.join("n.mp4").exists());

    // A light picture turned by 10 degrees: the size stays and every corner is still light.
    let (code, v) = rv(
        &ff,
        p,
        &["edit", "flat.mp4", "--tilt", "-10", "--out", "t.mp4"],
    );
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["size"], "320x180");
    assert!(v["tilt"]["zoom"].as_f64().unwrap() > 1.2, "{v}");
    let (dark, _) = corners(&ff, &p.join("t.mp4"), (320, 180));
    assert!(dark > 100, "an empty corner: {dark}");
}

/// A playlist with a video's name is never opened: each operation is refused by the media gate, and neither FFmpeg
/// nor ffprobe is started on it. `edit` checks the input at its start, and `Ffmpeg::run` checks every `-i` again.
#[test]
fn a_playlist_as_the_input_is_refused_by_each_operation_before_ffmpeg_starts() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    std::fs::write(p.join("secret.txt"), "not for FFmpeg").unwrap();
    std::fs::write(
        p.join("concat.mp4"),
        "ffconcat version 1.0\nfile 'secret.txt'\n",
    )
    .unwrap();
    std::fs::write(
        p.join("hls.mp4"),
        "#EXTM3U\n#EXTINF:1,\nhttp://127.0.0.1:9/x.ts\n",
    )
    .unwrap();
    let (rec, log) = record(&ff, p);
    let rec = rec.to_string_lossy().into_owned();
    for input in ["concat.mp4", "hls.mp4"] {
        for op in [
            vec!["--remove-bars"],
            vec!["--hold", "1@1"],
            vec!["--loop", "2"],
            vec!["--loop-to", "9"],
            vec!["--deinterlace"],
            vec!["--tilt", "3"],
        ] {
            let mut a = vec!["edit", input, "--out", "o.mp4"];
            a.extend(op.iter());
            let (code, v) = rv(&rec, p, &a);
            assert_eq!(code, Some(2), "{input} {op:?}: {v}");
            assert!(
                v["error"]["fix"].as_str().unwrap().contains("playlist"),
                "{input} {op:?}: {v}"
            );
            assert!(!p.join("o.mp4").exists());
        }
    }
    let ran = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !ran.contains("concat.mp4") && !ran.contains("hls.mp4"),
        "FFmpeg or ffprobe was started on a playlist: {ran}"
    );
}
