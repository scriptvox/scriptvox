//! `scriptvox edit --background` and `--alpha`: the subject stays by the matte row's mask, the rest is replaced. The
//! matte row is a fake queue here (`common::queue`): no service is called. Its mask is the left half white.
mod common;
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::Ordering;

const W: usize = 320;
const H: usize = 180;

struct Env {
    dir: tempfile::TempDir,
    q: common::Queue,
    ff: String,
}

/// A 3 s talk with sound, and a queue whose mask keeps the left half. `change` edits the rows (a price).
fn env(change: impl Fn(String) -> String) -> Option<Env> {
    let ff = common::ready()?;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    common::rows(p, change);
    common::ffmpeg(
        &ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "talk.mp4",
        ],
    );
    // As the service gives it: a grey picture in an H.264 MP4, white where the subject is.
    common::ffmpeg(
        &ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=white:s=160x180:r=25:d=3",
            "-f",
            "lavfi",
            "-i",
            "color=black:s=160x180:r=25:d=3",
            "-filter_complex",
            "hstack",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "mask-src.mp4",
        ],
    );
    let q = common::queue();
    *q.clip.lock().unwrap() = std::fs::read(p.join("mask-src.mp4")).unwrap();
    std::fs::rename(p.join("mask-src.mp4"), p.join("mask-keep.mp4")).unwrap();
    Some(Env { dir, q, ff })
}

impl Env {
    fn edit(&self, args: &[&str]) -> (Option<i32>, Value) {
        let p = self.dir.path();
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_FFMPEG", &self.ff)
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", self.q.port),
            )
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .env_remove("FAL_KEY")
            .env_remove("CLAUDECODE")
            .args(["edit", "talk.mp4", "--json"])
            .args(args)
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
        )
    }

    fn submits(&self) -> usize {
        self.q.submits.load(Ordering::SeqCst)
    }

    fn frame(&self, file: &str) -> Vec<[u8; 4]> {
        common::frame(&self.ff, &self.dir.path().join(file), 1.0, (W, H))
    }

    /// The sound packets of a file, as they are stored.
    fn sound(&self, file: &str) -> Vec<u8> {
        let o = std::process::Command::new(&self.ff)
            .current_dir(self.dir.path())
            .args([
                "-v", "error", "-i", file, "-vn", "-c:a", "copy", "-f", "adts", "-",
            ])
            .output()
            .unwrap();
        assert!(o.status.success());
        o.stdout
    }
}

/// Mean difference of two pictures over the columns `x0..x1`, in levels of 255.
fn differ(a: &[[u8; 4]], b: &[[u8; 4]], x0: usize, x1: usize) -> f64 {
    let mut sum = 0.0;
    for y in 0..H {
        for x in x0..x1 {
            for c in 0..3 {
                sum += (f64::from(a[y * W + x][c]) - f64::from(b[y * W + x][c])).abs();
            }
        }
    }
    sum / (H * (x1 - x0) * 3) as f64
}

/// How sharp the columns `x0..x1` are: the mean step between neighbours in a row.
fn sharp(a: &[[u8; 4]], x0: usize, x1: usize) -> f64 {
    let mut sum = 0.0;
    for y in 0..H {
        for x in x0..x1 - 1 {
            sum += (f64::from(a[y * W + x][1]) - f64::from(a[y * W + x + 1][1])).abs();
        }
    }
    sum / (H * (x1 - x0 - 1)) as f64
}

fn drift_passes(v: &Value) -> bool {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["gate"] == "av_drift" && f["status"] == "pass")
}

#[test]
fn a_colour_replaces_all_but_the_subject_and_the_sound_is_copied() {
    let Some(e) = env(|r| r) else { return };
    let (code, v) = e.edit(&["--background", "#00ff00", "--out", "green.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert!(drift_passes(&v), "{v}");
    let (src, got) = (e.frame("talk.mp4"), e.frame("green.mp4"));
    // The subject half keeps the source; the other half is the colour, with no ghost of the source.
    assert!(
        differ(&src, &got, 0, 150) < 6.0,
        "{}",
        differ(&src, &got, 0, 150)
    );
    let green = vec![[0u8, 255, 0, 255]; W * H];
    assert!(
        differ(&green, &got, 170, W) < 6.0,
        "{}",
        differ(&green, &got, 170, W)
    );
    assert_eq!(
        e.sound("talk.mp4"),
        e.sound("green.mp4"),
        "the sound packets are the source's"
    );
    let len = |f: &str| {
        common::probe(&e.ff, &e.dir.path().join(f), "v:0", "format=duration")
            .parse::<f64>()
            .unwrap()
    };
    assert!((len("talk.mp4") - len("green.mp4")).abs() < 0.05);
    // The same clip with another background: the mask is on record, no second call.
    let (code, v) = e.edit(&["--background", "#102030", "--out", "navy.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
}

#[test]
fn blur_keeps_the_subject_sharp_and_a_picture_can_stand_behind() {
    let Some(e) = env(|r| r) else { return };
    let (code, v) = e.edit(&["--background", "blur", "--out", "soft.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    let (src, got) = (e.frame("talk.mp4"), e.frame("soft.mp4"));
    assert!(
        sharp(&got, 0, 150) > 0.8 * sharp(&src, 0, 150),
        "the subject half stays sharp"
    );
    assert!(
        sharp(&got, 170, W) < 0.5 * sharp(&src, 170, W),
        "the other half is blurred"
    );
    // A picture of another shape, inside the project: it covers the frame behind the subject.
    common::ffmpeg(
        &e.ff,
        e.dir.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=200x200",
            "-frames:v",
            "1",
            "wall.png",
        ],
    );
    let (code, v) = e.edit(&["--background", "wall.png", "--out", "wall.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    let got = e.frame("wall.mp4");
    let red = vec![[255u8, 0, 0, 255]; W * H];
    assert!(
        differ(&red, &got, 170, W) < 8.0,
        "{}",
        differ(&red, &got, 170, W)
    );
    assert!(differ(&src, &got, 0, 150) < 6.0);
    // A clip behind: shorter than the talk, so it repeats to the length.
    common::ffmpeg(
        &e.ff,
        e.dir.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "color=blue:s=160x90:r=30:d=1",
            "-pix_fmt",
            "yuv420p",
            "sea.mp4",
        ],
    );
    let (code, v) = e.edit(&["--background", "sea.mp4", "--out", "sea-out.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    let late = common::frame(&e.ff, &e.dir.path().join("sea-out.mp4"), 2.5, (W, H));
    let blue = vec![[0u8, 0, 255, 255]; W * H];
    assert!(
        differ(&blue, &late, 170, W) < 8.0,
        "{}",
        differ(&blue, &late, 170, W)
    );
    assert_eq!(e.submits(), 1, "one mask for all three");
}

#[test]
fn alpha_writes_the_subject_on_a_clear_background() {
    let Some(e) = env(|r| r) else { return };
    let (code, v) = e.edit(&["--alpha", "--out", "cut.mov"]);
    assert_eq!(code, Some(0), "{v}");
    let p = e.dir.path();
    let fmt = common::probe(&e.ff, &p.join("cut.mov"), "v:0", "stream=pix_fmt");
    assert!(fmt.starts_with("yuva444p"), "{fmt}");
    let webm = common::probe(&e.ff, &p.join("cut.webm"), "v:0", "stream_tags=alpha_mode");
    assert_eq!(webm, "1", "the WebM carries an alpha plane");
    let got = e.frame("cut.mov");
    assert!(
        got[90 * W + 40][3] > 250,
        "the subject is solid: {:?}",
        got[90 * W + 40]
    );
    assert!(
        got[90 * W + 280][3] < 5,
        "the rest is clear: {:?}",
        got[90 * W + 280]
    );
    assert_eq!(e.sound("talk.mp4"), e.sound("cut.mov"));
    assert!(drift_passes(&v), "{v}");
    assert_eq!(
        v["artifacts"]["video"],
        serde_json::json!(p.canonicalize().unwrap().join("cut.mov"))
    );
}

#[test]
fn a_bad_colour_an_outside_file_and_a_crop_are_refused_before_any_call() {
    let Some(e) = env(|r| r) else { return };
    let outside = tempfile::tempdir().unwrap();
    common::ffmpeg(
        &e.ff,
        outside.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=64x64",
            "-frames:v",
            "1",
            "wall.png",
        ],
    );
    let wall = outside.path().join("wall.png");
    for (args, says) in [
        (vec!["--background", "#00ff0"], "#RRGGBB"),
        (vec!["--background", "#00ff00,drawtext=text=x"], "#RRGGBB"),
        (vec!["--background", "nothing.png"], "does not exist"),
        (
            vec!["--background", wall.to_str().unwrap()],
            "outside this project",
        ),
        (vec!["--background", "gen.toml"], "picture or a video"),
        (
            vec!["--background", "blur", "--reframe", "9:16"],
            "two calls",
        ),
        (vec!["--background", "blur", "--speed", "1.5"], "two calls"),
        (vec!["--background", "blur", "--from", "1"], "two calls"),
        (vec!["--background", "blur", "--alpha"], "--alpha"),
        (vec!["--alpha", "--hook", "Hello"], "--alpha"),
        (vec!["--alpha", "--out", "cut.mp4"], ".mov"),
    ] {
        let (code, v) = e.edit(&args);
        assert_eq!(code, Some(2), "{args:?} {v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains(says), "{args:?}: {msg}");
    }
    assert_eq!(e.submits(), 0);
}

/// A matte row that costs money: the plan shows the price, nothing is sent with no approval, and the approved plan
/// is one call.
fn paid(field: &'static str) {
    let Some(e) = env(move |r| common::row_field(r, "birefnet-self", "usd_per_s = 0.0", field))
    else {
        return;
    };
    let (code, v) = e.edit(&["--background", "#00ff00", "--out", "green.mp4"]);
    assert_eq!(code, Some(4), "{v}");
    assert_eq!(v["command"], "edit", "{v}");
    assert_eq!(e.submits(), 0, "{v}");
    assert!(!e.dir.path().join("green.mp4").exists());
    let (code, plan) = e.edit(&["--background", "#00ff00", "--out", "green.mp4", "--plan"]);
    assert_eq!(code, Some(0), "{plan}");
    // 6 cents for a call, or 3 s of source at 2 cents a second.
    assert_eq!(plan["metrics"]["cost_usd"], 0.06, "{plan}");
    assert!(
        plan["next"][1]
            .as_str()
            .unwrap()
            .starts_with("scriptvox edit"),
        "{plan}"
    );
    assert_eq!(e.submits(), 0, "{plan}");
    let id = plan["plan_id"].as_str().unwrap();
    assert_eq!(v["plan_id"], id, "the refusal names the same plan");
    // A cap under the price stops it, approved or not.
    let (code, v) = e.edit(&[
        "--background",
        "#00ff00",
        "--approve",
        id,
        "--max-spend",
        "0",
    ]);
    assert_eq!(code, Some(4), "{v}");
    assert_eq!(e.submits(), 0, "{v}");
    let (code, v) = e.edit(&[
        "--background",
        "#00ff00",
        "--out",
        "green.mp4",
        "--approve",
        id,
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.06, "{v}");
    // Another background on the same clip is the same plan: the mask is on record and nothing is bought again.
    let (code, v) = e.edit(&["--background", "blur", "--out", "soft.mp4", "--approve", id]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
}

#[test]
fn a_paid_matte_row_needs_an_approved_plan() {
    // The registry takes no price by the second of the result on a matte row: a smallest charge for a call, or a
    // price by the source (below).
    paid("usd_per_s = 0.0\nusd_min = 0.06");
}

#[test]
fn a_matte_row_priced_by_its_source_needs_one_too() {
    paid("usd_per_s = 0.0\nusd_per_input_s = 0.02");
}

#[test]
fn a_free_plan_calls_nothing_and_captions_combine() {
    let Some(e) = env(|r| r) else { return };
    let (code, plan) = e.edit(&["--background", "blur", "--plan"]);
    assert_eq!(code, Some(0), "{plan}");
    assert_eq!(plan["metrics"]["cost_usd"], 0.0, "{plan}");
    assert_eq!(e.submits(), 0);
    // A label is burned over the new background in the same call; the default name is the source's.
    let (code, v) = e.edit(&["--background", "#00ff00", "--label", "Demo"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1);
    let out = v["artifacts"]["video"].as_str().unwrap();
    assert!(out.ends_with("scriptvox-out/talk-edit.mp4"), "{out}");
    let got = common::frame(&e.ff, Path::new(out), 1.0, (W, H));
    let green = vec![[0u8, 255, 0, 255]; W * H];
    assert!(
        differ(&green, &got, 170, W) < 12.0,
        "{}",
        differ(&green, &got, 170, W)
    );
}

/// A background file is opened only as what its first bytes say it is: a list that names other files (HLS,
/// ffconcat), under a clip's name, never reaches FFmpeg, and neither does a link to a file outside the project.
#[test]
fn a_list_or_a_link_under_a_clips_name_is_refused_before_ffmpeg_opens_it() {
    let Some(e) = env(|r| r) else { return };
    let p = e.dir.path();
    let outside = tempfile::tempdir().unwrap();
    common::ffmpeg(
        &e.ff,
        outside.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=160x90:r=25:d=1",
            "-pix_fmt",
            "yuv420p",
            "secret.mp4",
        ],
    );
    let secret = outside.path().join("secret.mp4");
    // An FFmpeg that records every file it is asked to open: the refused files must not be among them.
    let bin = p.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = p.join("opened.log");
    for tool in ["ffmpeg", "ffprobe"] {
        let real = if tool == "ffmpeg" {
            Path::new(&e.ff).to_path_buf()
        } else {
            Path::new(&e.ff).with_file_name(tool)
        };
        std::fs::write(
            bin.join(tool),
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                log.display(),
                real.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join(tool), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let watched = bin.join("ffmpeg");
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_FFMPEG", &watched)
            .env("SCRIPTVOX_GENERATORS", p.join("gen.toml"))
            .env(
                "SCRIPTVOX_FAL_BASE",
                format!("http://127.0.0.1:{}", e.q.port),
            )
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("SCRIPTVOX_SELFHOST_KEY", "test-fal-key")
            .args(["edit", "talk.mp4", "--json"])
            .args(args)
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap_or(Value::Null),
        )
    };
    std::fs::write(
        p.join("concat.mp4"),
        format!("ffconcat version 1.0\nfile '{}'\n", secret.display()),
    )
    .unwrap();
    std::fs::write(
        p.join("list.mp4"),
        format!(
            "#EXTM3U\n#EXTINF:1.0,\nfile://{}\n#EXT-X-ENDLIST\n",
            secret.display()
        ),
    )
    .unwrap();
    std::fs::write(
        p.join("list.m3u8"),
        "#EXTM3U\n#EXTINF:1.0,\nhttp://127.0.0.1:9/x.ts\n",
    )
    .unwrap();
    std::fs::write(p.join("graph.png"), "color=red:s=64x64").unwrap();
    std::os::unix::fs::symlink(&secret, p.join("link.mp4")).unwrap();
    for (file, says) in [
        ("concat.mp4", "ffconcat"),
        ("list.mp4", "playlist"),
        ("list.m3u8", "playlist"),
        ("graph.png", "not a picture or a video"),
        ("link.mp4", "outside this project"),
    ] {
        let (code, v) = run(&["--background", file]);
        assert_eq!(code, Some(2), "{file} {v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains(says), "{file}: {msg}");
    }
    assert_eq!(e.submits(), 0, "refused before any call to the row");
    let opened = std::fs::read_to_string(&log).unwrap_or_default();
    for name in [
        "concat.mp4",
        "list.mp4",
        "list.m3u8",
        "graph.png",
        "link.mp4",
        "secret.mp4",
    ] {
        assert!(!opened.contains(name), "FFmpeg was given {name}: {opened}");
    }
    // A real picture and a real clip still work, whatever their names say, and each is opened with the file
    // protocol only and a named demuxer.
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=200x200",
            "-frames:v",
            "1",
            "wall.png",
        ],
    );
    std::fs::rename(p.join("wall.png"), p.join("wall.dat")).unwrap();
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=blue:s=160x90:r=30:d=1",
            "-pix_fmt",
            "yuv420p",
            "sea.mp4",
        ],
    );
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:s=200x200",
            "-frames:v",
            "1",
            "wall.jpg",
        ],
    );
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=blue:s=160x90:r=30:d=1",
            "-pix_fmt",
            "yuv420p",
            "sea.mkv",
        ],
    );
    let red = vec![[255u8, 0, 0, 255]; W * H];
    let blue = vec![[0u8, 0, 255, 255]; W * H];
    for (file, out, want) in [
        ("wall.dat", "wall-out.mp4", &red),
        ("sea.mp4", "sea-out.mp4", &blue),
        ("wall.jpg", "jpg-out.mp4", &red),
        ("sea.mkv", "mkv-out.mp4", &blue),
    ] {
        let (code, v) = run(&["--background", file, "--out", out]);
        assert_eq!(code, Some(0), "{file} {v}");
        let got = e.frame(out);
        assert!(
            differ(want, &got, 170, W) < 8.0,
            "{file}: {}",
            differ(want, &got, 170, W)
        );
    }
    let opened = std::fs::read_to_string(&log).unwrap();
    for name in ["wall.dat", "sea.mp4", "wall.jpg", "sea.mkv"] {
        for line in opened.lines().filter(|l| l.contains(name)) {
            assert!(line.contains("-protocol_whitelist file -f "), "{line}");
        }
    }
    assert!(opened.contains("-f image2 -pattern_type none"), "{opened}");
}

/// An FFmpeg and ffprobe that write every argument list to a file, then run the real ones. Returns that file.
fn record(e: &mut Env) -> std::path::PathBuf {
    let (ff, log) = common::record(&e.ff, e.dir.path());
    e.ff = ff.to_str().unwrap().to_string();
    log
}

/// The mask is a file from a service: a list in place of a clip is refused before FFmpeg opens it, and a real mask is
/// opened with the file protocol only and its demuxer named.
#[test]
fn a_mask_that_is_a_list_is_refused_before_ffmpeg_opens_it() {
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.mp4");
    for (list, says) in [
        (
            format!("ffconcat version 1.0\nfile '{}'\n", secret.display()),
            "ffconcat",
        ),
        (
            format!(
                "#EXTM3U\n#EXTINF:3.0,\nfile://{}\n#EXT-X-ENDLIST\n",
                secret.display()
            ),
            "playlist",
        ),
    ] {
        let Some(mut e) = env(|r| r) else { return };
        common::ffmpeg(
            &e.ff,
            outside.path(),
            &[
                "-f",
                "lavfi",
                "-i",
                "color=white:s=320x180:r=25:d=3",
                "-pix_fmt",
                "yuv420p",
                "secret.mp4",
            ],
        );
        let log = record(&mut e);
        *e.q.clip.lock().unwrap() = list.into_bytes();
        let (code, v) = e.edit(&["--background", "#00ff00", "--out", "green.mp4"]);
        assert_eq!(code, Some(1), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(says),
            "{v}"
        );
        assert!(!e.dir.path().join("green.mp4").exists());
        let opened = std::fs::read_to_string(&log).unwrap();
        assert!(!opened.contains("secret.mp4"), "{opened}");
        // The copy that was sent is `matte-in-<hash>`; the mask is `matte-<hash>`.
        assert!(
            !opened.replace("matte-in", "").contains("matte-"),
            "FFmpeg was given the mask: {opened}"
        );
        // The row is free: its bad answer is forgotten, so the same command asks it again and a good mask works.
        *e.q.clip.lock().unwrap() = std::fs::read(e.dir.path().join("mask-keep.mp4")).unwrap();
        let (code, v) = e.edit(&["--background", "#00ff00", "--out", "green.mp4"]);
        assert_eq!(code, Some(0), "{v}");
        assert_eq!(e.submits(), 2, "{v}");
        let opened = std::fs::read_to_string(&log).unwrap();
        assert!(!opened.contains("secret.mp4"), "{opened}");
    }
    let Some(mut e) = env(|r| r) else { return };
    let log = record(&mut e);
    let (code, v) = e.edit(&["--background", "#00ff00", "--out", "green.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    let opened = std::fs::read_to_string(&log).unwrap();
    let with_mask: Vec<&str> = opened
        .lines()
        .filter(|l| l.replace("matte-in", "").contains("matte-"))
        .collect();
    assert!(with_mask.len() >= 2, "{opened}");
    for line in with_mask {
        assert!(line.contains("-protocol_whitelist file -f mov "), "{line}");
    }
}
