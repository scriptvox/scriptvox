//! A transition must never lose frames (docs/specs/transitions.md). Every name of `media::TRANSITIONS` is rendered
//! with the shot graph `make` uses, at two frame rates, and the frames of the result are counted. The scene times
//! are those of the brief that lost its last scene on 13 Nov 2026 (97 frames of 149 at 24 fps with `cut`): FFmpeg's
//! xfade ends the video when a shot stops before the next one starts, so a count that is short means a lost scene.
//! Skipped when SCRIPTVOX_FFMPEG is unset. No network, no voice: the shots are a still, as in that brief (a still's
//! length rounds to whole frames at its input, which is where the frame was lost).
use scriptvox::media::{graph, transition, xfade_name, Item, Kind, TRANSITIONS};
use std::path::Path;
use std::process::Command;

const CUTS: [(f64, f64); 4] = [(0.0, 1.317), (1.317, 2.645), (2.645, 4.028), (4.028, 6.21)];

/// The frames FFmpeg wrote for this shot graph (the last `frame=` of its report).
fn frames(ff: &str, still: &Path, name: &str, rtl: bool, fps: u32) -> u64 {
    let item = Item {
        path: still.into(),
        kind: Kind::Image,
        from: 0.0,
        to: None,
    };
    let tr = transition(Some(name), rtl, fps).unwrap();
    let (inputs, g) = graph(&[item], &[], &[], &CUTS, (160, 284), fps, tr, &[]);
    // `zoom` adds its own step on the shots in `make`; under it is a fade, which is what the graph holds.
    assert!(
        g.contains(&format!("xfade=transition={}:", xfade_name(tr.0))),
        "{g}"
    );
    let o = Command::new(ff)
        .args(["-hide_banner", "-nostats", "-y"])
        .args(&inputs)
        .args(["-filter_complex", &g, "-map", "[bg]", "-f", "null", "-"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{name} at {fps} fps: {err}");
    err.rsplit("frame=")
        .next()
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("{name}: no frame count in {err}"))
}

#[test]
fn every_transition_keeps_every_frame() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    // A test picture with detail, made by FFmpeg itself.
    let dir = tempfile::tempdir().unwrap();
    let still = dir.path().join("still.png");
    let o = Command::new(&ff)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc2=s=320x568:r=1:d=1")
        .args(["-frames:v", "1"])
        .arg(&still)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let mut short = vec![];
    for name in TRANSITIONS.split(", ") {
        for (fps, rtl) in [(24u32, false), (30, true)] {
            // The video is as long as its last scene's end, whatever the transition.
            let want = (CUTS[3].1 * f64::from(fps)).round() as u64;
            let got = frames(&ff, &still, name, rtl, fps);
            if got != want {
                short.push(format!("{name} at {fps} fps: {got} frames of {want}"));
            }
        }
    }
    assert!(short.is_empty(), "frames lost or added: {short:?}");
}

mod common;

/// The same through `make`, for the five names of 13 Nov 2026: the frames of the made video are read back and are
/// the video's length times its rate, at 24 fps, where one frame of overlap is shortest.
#[test]
fn make_keeps_every_frame_with_each_new_transition() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    for name in ["reveal", "iris", "split", "slice", "rise"] {
        std::fs::write(
            p.join("b.json"),
            format!(
                r#"{{"tts": "none", "fps": 24, "aspect": "16:9", "transition": "{name}", "scenes": [
                {{"script": "One short line."}}, {{"script": "A second line follows."}}, {{"script": "And the last one."}}]}}"#
            ),
        )
        .unwrap();
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["make", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(o.status.code(), Some(0), "{name}: {v}");
        let (frames, secs) = (
            v["metrics"]["frames"].as_i64().unwrap(),
            v["metrics"]["duration_s"].as_f64().unwrap(),
        );
        assert_eq!(frames, (secs * 24.0).round() as i64, "{name}: {v}");
    }
}

/// The MD5 of every frame FFmpeg makes from `label` of this graph, in order.
fn frame_sums(ff: &str, inputs: &[String], graph: &str, label: &str) -> Vec<String> {
    let o = Command::new(ff)
        .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"])
        .args(inputs)
        .args(["-filter_complex", graph, "-map", label])
        .args(["-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next().map(|h| h.trim().to_string()))
        .collect()
}

/// A render in groups of scenes (docs/specs/render-split.md) gives the frames of the render in one process: the
/// same count and the same pixels, for every transition, at two frame rates, with a border after every shot and
/// after every second shot. The shots are a still and the plain background in turn, so FFmpeg has to choose one
/// pixel format for both, and the groups must use that one.
#[test]
fn groups_of_scenes_give_the_frames_of_one_process_for_every_transition() {
    use scriptvox::media::{graph_part, shot_frame, Part};
    use scriptvox::split;
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let still = dir.path().join("still.png");
    let o = Command::new(&ff)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
        .args(["-i", "testsrc2=s=320x568:r=1:d=1", "-frames:v", "1"])
        .arg(&still)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let items = [
        Item {
            path: still,
            kind: Kind::Image,
            from: 0.0,
            to: None,
        },
        Item {
            path: "".into(),
            kind: Kind::Gradient,
            from: 0.0,
            to: None,
        },
    ];
    let engine = scriptvox::engine_ffmpeg::Ffmpeg {
        ffmpeg: ff.clone().into(),
        ffprobe: Path::new(&ff).with_file_name("ffprobe"),
    };
    let frame = (160, 284);
    let mut bad = vec![];
    for name in TRANSITIONS.split(", ") {
        for (fps, rtl) in [(24u32, false), (30, true)] {
            let tr = transition(Some(name), rtl, fps).unwrap();
            let shots = split::Shots {
                items: items.to_vec(),
                lens: vec![],
                moves: vec![],
                cuts: CUTS.to_vec(),
                frame,
                fps,
                tr,
                into: vec![],
            };
            let (inputs, g) = graph(&items, &[], &[], &CUTS, frame, fps, tr, &[]);
            let one = frame_sums(&ff, &inputs, &g, "[bg]");
            let total = one.len() as u64;
            let track = shots.track(&engine, dir.path(), None).unwrap();
            let format = track.pin();
            for per in [1, 2] {
                let groups = split::groups(&CUTS, |_| tr.1, fps, total, per);
                assert_eq!(groups.len(), 4 / per, "{name} at {fps} fps");
                let mut many = vec![];
                for gr in &groups {
                    let origin = shot_frame(&CUTS, gr.shots.0, fps);
                    let part = Part {
                        shots: gr.shots,
                        origin,
                        format: &format,
                    };
                    let (inputs, g) =
                        graph_part(&items, &[], &[], &CUTS, frame, fps, tr, &[], Some(&part));
                    let end = gr
                        .end
                        .map_or(String::new(), |e| format!(":end_frame={}", e - origin));
                    let g = format!("{g};[bg]trim=start_frame={}{end}[v]", gr.start - origin);
                    many.extend(frame_sums(&ff, &inputs, &g, "[v]"));
                }
                if many.len() != one.len() {
                    bad.push(format!(
                        "{name} at {fps} fps, {per} a group: {} frames of {}",
                        many.len(),
                        one.len()
                    ));
                } else if let Some(at) = (0..one.len()).find(|&i| one[i] != many[i]) {
                    bad.push(format!(
                        "{name} at {fps} fps, {per} a group ({format}): frame {at} differs"
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "not the frames of one process: {bad:#?}");
}
