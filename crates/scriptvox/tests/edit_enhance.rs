//! `scriptvox edit --enhance`: the picture goes through the upscale row and comes back larger; the sound and the
//! length stay. The row is a fake queue here (`common::queue`): no service is called.
mod common;
use serde_json::Value;
use std::sync::atomic::Ordering;

struct Env {
    dir: tempfile::TempDir,
    q: common::Queue,
    ff: String,
}

/// A 3 s clip of 320x180 with sound, and a queue that answers with the same frames at 640x360.
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
            "clip.mp4",
        ],
    );
    common::ffmpeg(
        &ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=25:d=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "big-src.mp4",
        ],
    );
    let q = common::queue();
    *q.clip.lock().unwrap() = std::fs::read(p.join("big-src.mp4")).unwrap();
    std::fs::remove_file(p.join("big-src.mp4")).unwrap();
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
            .arg("edit")
            .args(args)
            .arg("--json")
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

    fn probe(&self, file: &str, select: &str, entry: &str) -> String {
        common::probe(&self.ff, &self.dir.path().join(file), select, entry)
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

#[test]
fn the_clip_comes_back_larger_with_its_own_sound_and_length() {
    let Some(e) = env(|r| r) else { return };
    // The plan names the row's size and a time, and calls nothing.
    let (code, plan) = e.edit(&["clip.mp4", "--enhance", "--plan"]);
    assert_eq!(code, Some(0), "{plan}");
    assert_eq!(plan["metrics"]["cost_usd"], 0.0, "{plan}");
    assert_eq!(plan["enhance"]["size_in"], "320x180", "{plan}");
    assert_eq!(plan["enhance"]["size_out_about"], "1920x1080", "{plan}");
    assert!(
        plan["enhance"]["takes_s_about"].as_f64().unwrap() > 0.0,
        "{plan}"
    );
    assert_eq!(e.submits(), 0);
    let (code, v) = e.edit(&["clip.mp4", "--enhance", "--out", "big.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    // The size is the one the row gave back.
    assert_eq!(e.probe("big.mp4", "v:0", "stream=width,height"), "640\n360");
    assert_eq!(v["metrics"]["size"], "640x360", "{v}");
    assert_eq!(v["enhance"]["size_in"], "320x180", "{v}");
    let len = |f: &str| e.probe(f, "v:0", "format=duration").parse::<f64>().unwrap();
    assert!((len("clip.mp4") - len("big.mp4")).abs() < 0.05);
    assert_eq!(
        e.sound("clip.mp4"),
        e.sound("big.mp4"),
        "the sound packets are the source's"
    );
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["gate"] == "av_drift" && f["status"] == "pass"),
        "{v}"
    );
    // The same clip again: the job is on record, no second call.
    let (code, v) = e.edit(&["clip.mp4", "--enhance", "--out", "again.mp4"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
}

#[test]
fn a_long_or_large_clip_and_a_mix_are_refused_before_any_call() {
    let Some(e) = env(|r| r) else { return };
    let p = e.dir.path();
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=160x90:r=10:d=12",
            "-pix_fmt",
            "yuv420p",
            "long.mp4",
        ],
    );
    common::ffmpeg(
        &e.ff,
        p,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=1920x1080:r=10:d=1",
            "-pix_fmt",
            "yuv420p",
            "hd.mp4",
        ],
    );
    for (args, says) in [
        (vec!["long.mp4", "--enhance"], "at most 10 s"),
        (vec!["hd.mp4", "--enhance"], "already"),
        (
            vec!["clip.mp4", "--enhance", "--label", "Demo"],
            "two calls",
        ),
        (
            vec!["clip.mp4", "--enhance", "--background", "blur"],
            "two calls",
        ),
        (
            vec!["clip.mp4", "--enhance", "--reframe", "9:16"],
            "two calls",
        ),
    ] {
        let (code, v) = e.edit(&args);
        assert_eq!(code, Some(2), "{args:?} {v}");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains(says), "{args:?}: {msg}");
    }
    // The fix line of a long clip says how to cut it.
    let (_, v) = e.edit(&["long.mp4", "--enhance"]);
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("--from 0 --to 10"),
        "{v}"
    );
    assert_eq!(e.submits(), 0);
}

#[test]
fn a_paid_upscale_row_needs_an_approved_plan() {
    let Some(e) = env(|r| {
        common::row_field(
            r,
            "seedvr2-3b-self",
            "usd_per_s = 0.0",
            "usd_per_s = 0.0\nusd_per_input_s = 0.05",
        )
    }) else {
        return;
    };
    let (code, v) = e.edit(&["clip.mp4", "--enhance", "--out", "big.mp4"]);
    assert_eq!(code, Some(4), "{v}");
    assert_eq!(v["command"], "edit", "{v}");
    assert_eq!(e.submits(), 0, "{v}");
    assert!(!e.dir.path().join("big.mp4").exists());
    let (code, plan) = e.edit(&["clip.mp4", "--enhance", "--plan"]);
    assert_eq!(code, Some(0), "{plan}");
    // 3 s of source at 5 cents a second.
    assert_eq!(plan["metrics"]["cost_usd"], 0.15, "{plan}");
    assert_eq!(e.submits(), 0, "{plan}");
    let id = plan["plan_id"].as_str().unwrap();
    let (code, v) = e.edit(&["clip.mp4", "--enhance", "--out", "big.mp4", "--approve", id]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(e.submits(), 1, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.15, "{v}");
}

/// The result is a file from a service: a list in place of a clip is refused, and nothing it names is opened.
#[test]
fn a_result_that_is_a_list_is_refused() {
    let Some(e) = env(|r| r) else { return };
    let outside = tempfile::tempdir().unwrap();
    common::ffmpeg(
        &e.ff,
        outside.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=25:d=3",
            "-pix_fmt",
            "yuv420p",
            "secret.mp4",
        ],
    );
    let secret = outside.path().join("secret.mp4");
    *e.q.clip.lock().unwrap() =
        format!("ffconcat version 1.0\nfile '{}'\n", secret.display()).into_bytes();
    let (code, v) = e.edit(&["clip.mp4", "--enhance", "--out", "big.mp4"]);
    assert_eq!(code, Some(1), "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("ffconcat"),
        "{v}"
    );
    assert!(
        !e.dir.path().join("big.mp4").exists(),
        "the outside clip did not become the result"
    );
}
