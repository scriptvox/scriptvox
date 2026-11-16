#![cfg(unix)]
//! The picture in groups of scenes (docs/specs/render-split.md), end to end with no voice and no network: the
//! video is the video of one process, a stopped render keeps its finished groups, and a group file that is not
//! what the render wrote is made again and never read.
mod common;
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

const BRIEF: &str = r#"{"tts": "none", "lang": "en", "aspect": "16:9", "size": 720, "transition": "slide",
 "outro": {"text": "example.org", "seconds": 1},
 "scenes": [
  {"script": "One short line.", "card": {"kind": "title", "text": "Groups"}},
  {"script": "A second line follows.", "transition": "dissolve"},
  {"script": "The third is here."},
  {"script": "Then the fourth.", "card": {"kind": "gauge", "values": [72], "label": "Battery"}},
  {"script": "And the last one."}]}"#;

fn make(p: &Path, ff: &str, out: &str, env: &[(&str, &str)]) -> (Option<i32>, Value) {
    let mut c = common::bin();
    c.current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("SCRIPTVOX_FFMPEG", ff)
        .env_remove("SCRIPTVOX_SPLIT_SCENES")
        .args(["make", "--brief", "b.json", "--json", "--out", out]);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

/// The MD5 of one stream of a file as it is stored (no decode): the same bytes mean the same picture or sound.
fn stream_md5(ff: &str, file: &Path, stream: &str) -> String {
    let o = std::process::Command::new(ff)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(file)
        .args(["-map", stream, "-c", "copy", "-f", "md5", "-"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The folder of the group files of a stopped render.
fn group_dir(p: &Path) -> PathBuf {
    std::fs::read_dir(p.join(".scriptvox/chunks"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|d| {
            d.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("split-"))
        })
        .expect("a stopped render keeps its group folder")
}

fn gates(v: &Value) -> Vec<(String, String)> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["gate"].to_string(), f["status"].to_string()))
        .collect()
}

#[test]
fn groups_give_the_video_of_one_process_and_resume_and_trust_no_planted_file() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    std::fs::write(p.join("b.json"), BRIEF).unwrap();

    // One process: 5 scenes at 1280x720 are far under the load where the split starts.
    let (code, one) = make(p, &ff, "one.mp4", &[]);
    assert_eq!(code, Some(0), "{one}");
    assert!(one["metrics"]["scene_groups"].is_null(), "{one}");
    let want = (
        stream_md5(&ff, &p.join("one.mp4"), "0:v"),
        stream_md5(&ff, &p.join("one.mp4"), "0:a"),
    );
    let same = |file: &str| {
        (
            stream_md5(&ff, &p.join(file), "0:v"),
            stream_md5(&ff, &p.join(file), "0:a"),
        ) == want
    };

    // Groups of 2 scenes: 3 groups, the same picture stream and sound stream, the same gates, no file left.
    let split = [("SCRIPTVOX_SPLIT_SCENES", "2")];
    let (code, v) = make(p, &ff, "split.mp4", &split);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["scene_groups"], 3, "{v}");
    assert_eq!(v["metrics"]["frames"], one["metrics"]["frames"], "{v}");
    assert!(same("split.mp4"), "the picture or the sound changed");
    assert_eq!(gates(&v), gates(&one));
    assert!(std::fs::read_dir(p.join(".scriptvox/chunks"))
        .unwrap()
        .next()
        .is_none());

    // Stopped after 2 groups, as a kill would: the rerun makes only the third.
    let stop2 = [split[0], ("SCRIPTVOX_FAULT_AFTER_GROUPS", "2")];
    let (code, v) = make(p, &ff, "resumed.mp4", &stop2);
    assert_eq!(code, Some(1), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("groups of scenes are kept"),
        "{v}"
    );
    let (code, v) = make(p, &ff, "resumed.mp4", &split);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["scene_groups_kept"], 2, "{v}");
    assert!(same("resumed.mp4"));

    // Stopped in the final pass (1 s chunks, after 2): the rerun keeps all 3 groups and the 2 chunks.
    let stop = [
        split[0],
        ("SCRIPTVOX_CHUNK_S", "1"),
        ("SCRIPTVOX_FAULT_AFTER_CHUNKS", "2"),
    ];
    let (code, v) = make(p, &ff, "final.mp4", &stop);
    assert_eq!(code, Some(1), "{v}");
    let (code, v) = make(p, &ff, "final.mp4", &stop[..2]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["scene_groups_kept"], 3, "{v}");
    assert_eq!(v["metrics"]["resumed_from_chunk"], 2, "{v}");
    assert_eq!(v["metrics"]["frames"], one["metrics"]["frames"], "{v}");

    // The last group ends with the last shot, so its frame count is not fixed to the frame: a short file under
    // its name is still made again.
    let (code, v) = make(p, &ff, "last.mp4", &stop);
    assert_eq!(code, Some(1), "{v}");
    let last = group_dir(p).join("group_0002.mkv");
    assert!(last.is_file());
    let o = std::process::Command::new(&ff)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
        .args(["-i", "testsrc2=s=1280x720:r=30:d=0.1", "-c:v", "ffv1"])
        .arg(&last)
        .output()
        .unwrap();
    assert!(o.status.success());
    let (code, v) = make(p, &ff, "last.mp4", &stop[..2]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["metrics"]["scene_groups_kept"], 2, "{v}");
    assert_eq!(v["metrics"]["frames"], one["metrics"]["frames"], "{v}");

    // A group file under the right name that the render did not write is made again, never read by the join.
    let stop1 = [split[0], ("SCRIPTVOX_FAULT_AFTER_GROUPS", "1")];
    for plant in ["link", "list", "short"] {
        let (code, v) = make(p, &ff, "planted.mp4", &stop1);
        assert_eq!(code, Some(1), "{plant}: {v}");
        let first = group_dir(p).join("group_0000.mkv");
        assert!(first.is_file(), "{plant}");
        match plant {
            // The real group file in another place, and a link to it: whole in every other way.
            "link" => {
                std::fs::rename(&first, p.join("elsewhere.mkv")).unwrap();
                std::os::unix::fs::symlink(p.join("elsewhere.mkv"), &first).unwrap();
            }
            // A playlist that names another file.
            "list" => {
                std::fs::write(&first, "ffconcat version 1.0\nfile '/etc/hostname'\n").unwrap()
            }
            // A real FFV1 Matroska file with 3 frames.
            _ => {
                let o = std::process::Command::new(&ff)
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
                    .args(["-i", "testsrc2=s=1280x720:r=30:d=0.1", "-c:v", "ffv1"])
                    .arg(&first)
                    .output()
                    .unwrap();
                assert!(o.status.success());
            }
        }
        let (code, v) = make(p, &ff, "planted.mp4", &split);
        assert_eq!(code, Some(0), "{plant}: {v}");
        assert_eq!(v["metrics"]["scene_groups_kept"], 0, "{plant}: {v}");
        assert!(same("planted.mp4"), "{plant}");
    }
    // The link went, not the file it named.
    assert!(p.join("elsewhere.mkv").is_file());
}

/// A render in one process that fails for memory is made once more, with the picture in groups of scenes
/// (14 Nov 2026: a draft of 34 scenes with 18 overlays was under the load for groups and failed, and no message
/// named the remedy). The FFmpeg here is the real one behind a script that fails as FFmpeg does without memory
/// when the chunked render (the last pass) reads more than 3 inputs: the 8 shots in one process. After groups it
/// reads one input, the group files.
#[test]
fn a_one_process_render_that_fails_for_memory_is_made_again_in_groups() {
    use std::os::unix::fs::PermissionsExt;
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let scenes: Vec<String> = (1..=8)
        .map(|i| format!(r#"{{"script": "This is line number {i} of eight."}}"#))
        .collect();
    std::fs::write(
        p.join("b.json"),
        format!(
            r#"{{"tts": "none", "lang": "en", "aspect": "16:9", "size": 720, "transition": "fade", "scenes": [{}]}}"#,
            scenes.join(",")
        ),
    )
    .unwrap();
    // The real FFmpeg: one process, 8 scenes at 1280x720 are under the load for groups.
    let (code, one) = make(p, &ff, "one.mp4", &[]);
    assert_eq!(code, Some(0), "{one}");
    assert!(one["metrics"].get("scene_groups").is_none(), "{one}");

    let bin = p.join("low/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = format!(
        "#!/bin/sh\nn=0\nlast=\nfor a in \"$@\"; do [ \"$a\" = -i ] && n=$((n+1)); last=$a; done\ncase \"$last\" in *chunk_%04d.mp4) if [ $n -gt 3 ]; then echo '[fc#0 @ 0x1] Terminating thread with return code -12 (Cannot allocate memory)' >&2; exit 244; fi;; esac\nexec '{ff}' \"$@\"\n"
    );
    std::fs::write(bin.join("ffmpeg"), script).unwrap();
    std::fs::set_permissions(bin.join("ffmpeg"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(
        Path::new(&ff).with_file_name("ffprobe"),
        bin.join("ffprobe"),
    )
    .unwrap();
    let low = bin.join("ffmpeg").to_string_lossy().into_owned();

    // No variable: the first attempt fails, and the second makes the picture in 2 groups.
    let (code, again) = make(p, &low, "again.mp4", &[]);
    assert_eq!(code, Some(0), "{again}");
    assert_eq!(again["metrics"]["scene_groups"], 2, "{again}");
    // The same frames and the same sound. The picture stream is not compared: this brief has plain backgrounds
    // only, and for it a render in groups marks the full colour range where one process marks the limited one
    // (seen 14 Nov 2026 with `SCRIPTVOX_SPLIT_SCENES` and the real FFmpeg too, PSNR 50 dB; open in render-split.md).
    assert_eq!(again["metrics"]["frames"], one["metrics"]["frames"]);
    assert_eq!(
        stream_md5(&ff, &p.join("one.mp4"), "0:a"),
        stream_md5(&ff, &p.join("again.mp4"), "0:a")
    );
    let events = std::fs::read_to_string(again["events_file"].as_str().unwrap()).unwrap();
    assert!(
        events.contains(
            "ran out of memory; one more attempt, with the picture in groups of 6 scenes"
        ),
        "{events}"
    );

    // The user said never: no second attempt, and the fix line names the variable.
    let (code, never) = make(p, &low, "never.mp4", &[("SCRIPTVOX_SPLIT_SCENES", "0")]);
    assert_eq!(code, Some(1), "{never}");
    let e = &never["error"];
    assert!(
        e["message"].as_str().unwrap().contains("ran out of memory"),
        "{never}"
    );
    assert!(
        e["fix"]
            .as_str()
            .unwrap()
            .contains("SCRIPTVOX_SPLIT_SCENES=4"),
        "{never}"
    );
}
