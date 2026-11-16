//! A video with no words (docs/specs/no-words.md): scenes with `seconds`, `duration` alone, a pause at the end of a
//! script, and a plan that gives the answer of the run for `tts: none`. No network and no voice.
mod common;
use serde_json::{json, Value};
use std::path::Path;

/// Three noise pictures, each with one strong colour: not black, and a slow zoom shows as motion.
fn pictures(ff: &str, dir: &Path) {
    for (c, i) in [("red", 0), ("green", 1), ("blue", 2)] {
        let ch = |k: usize| match k == i {
            true => "200+55*random(1)",
            false => "60+60*random(1)",
        };
        let geq = format!(
            "nullsrc=s=320x180,geq=r='{}':g='{}':b='{}'",
            ch(0),
            ch(1),
            ch(2)
        );
        let out = format!("{c}.png");
        common::ffmpeg(
            ff,
            dir,
            &["-f", "lavfi", "-i", &geq, "-frames:v", "1", &out],
        );
    }
}

fn sv(ff: &str, dir: &Path, cmd: &str, name: &str, b: &Value) -> (Option<i32>, Value) {
    std::fs::write(dir.join(format!("{name}.json")), b.to_string()).unwrap();
    let mut c = common::bin();
    c.current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args([cmd, "--brief", &format!("{name}.json"), "--local-only"]);
    if cmd == "make" {
        c.args(["--out", &format!("{name}.mp4")]);
    }
    let o = c.arg("--json").output().unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

/// The times where the picture changes its main colour: one pixel per frame at 30 fps.
fn picture_cuts(ff: &str, video: &Path) -> Vec<f64> {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(video)
        .args(["-vf", "scale=1:1:flags=area", "-pix_fmt", "rgb24"])
        .args(["-f", "rawvideo", "-"])
        .output()
        .unwrap();
    assert!(o.status.success());
    let main = |p: &[u8; 3]| (0..3).max_by_key(|i| p[*i]).unwrap();
    let px: Vec<usize> = o.stdout.as_chunks::<3>().0.iter().map(main).collect();
    (1..px.len())
        .filter(|i| px[*i] != px[i - 1])
        .map(|i| i as f64 / 30.0)
        .collect()
}

fn length(ff: &str, video: &Path) -> f64 {
    common::probe(ff, video, "v:0", "format=duration")
        .parse()
        .unwrap()
}

fn stage<'a>(v: &'a Value, name: &str) -> &'a str {
    v["stages"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["stage"] == name))
        .and_then(|s| s["explain"].as_str())
        .unwrap_or_default()
}

#[test]
fn scenes_with_seconds_make_a_video_with_no_words() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    pictures(&ff, d.path());
    // No `tts` in the brief: a video with no words asks for no voice row, also with --local-only.
    let b = json!({"aspect": "16:9", "transition": "cut", "music": "pulse", "scenes": [
        {"seconds": 1, "media": "red.png"},
        {"seconds": 2.5, "media": "green.png"},
        {"seconds": 1.5, "media": "blue.png"}
    ]});
    let (code, plan) = sv(&ff, d.path(), "plan", "three", &b);
    assert_eq!(code, Some(0), "{plan}");
    assert!(stage(&plan, "render").contains("for a 5 s video"), "{plan}");
    let (code, v) = sv(&ff, d.path(), "make", "three", &b);
    assert_eq!(code, Some(0), "{v}");
    let video = d.path().join("three.mp4");
    assert!((length(&ff, &video) - 5.0).abs() < 0.07, "{v}");
    let cuts = picture_cuts(&ff, &video);
    assert_eq!(cuts.len(), 2, "{cuts:?}");
    assert!(
        (cuts[0] - 1.0).abs() < 0.07 && (cuts[1] - 3.5).abs() < 0.07,
        "{cuts:?}"
    );
    let gates = v["findings"].as_array().unwrap();
    assert!(gates.iter().all(|f| f["status"] != "fail"), "{v}");
    assert!(
        gates.iter().all(|f| f["gate"] != "captions"),
        "no caption, so no captions gate: {v}"
    );
    assert!(
        gates
            .iter()
            .any(|f| f["gate"] == "silence" && f["status"] == "pass"),
        "{v}"
    );
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    assert!(!ass.contains("Dialogue:"), "no text on the picture");
    assert!(
        !d.path().join("three.srt").exists(),
        "no empty caption file beside the video"
    );
    assert!(v["artifacts"].get("sidecar_srt").is_none(), "{v}");
}

#[test]
fn words_and_seconds_mix_and_the_caption_leaves_before_a_scene_with_no_words() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    pictures(&ff, d.path());
    let b = json!({"aspect": "16:9", "tts": "none", "lang": "en", "transition": "cut", "music": "calm", "scenes": [
        {"script": "Look at this red picture.", "media": "red.png"},
        {"seconds": 3, "media": "green.png"}
    ]});
    let (code, v) = sv(&ff, d.path(), "make", "mix", &b);
    assert_eq!(code, Some(0), "{v}");
    // Five words at 2.5 a second after a 0.1 s start, then exactly 3 s.
    let video = d.path().join("mix.mp4");
    assert!((length(&ff, &video) - 5.1).abs() < 0.07, "{v}");
    let cuts = picture_cuts(&ff, &video);
    assert!(cuts.len() == 1 && (cuts[0] - 2.1).abs() < 0.07, "{cuts:?}");
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    let last = ass.lines().rfind(|l| l.starts_with("Dialogue:")).unwrap();
    let end = last.split(',').nth(2).unwrap();
    assert!(
        end < "0:00:03.00",
        "the page leaves after its words, not at the end of the video: {last}"
    );
}

#[test]
fn duration_alone_is_a_video_with_no_words() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let (code, v) = sv(
        &ff,
        d.path(),
        "make",
        "bed",
        &json!({"duration": 3, "music": "pulse", "aspect": "16:9"}),
    );
    assert_eq!(code, Some(0), "{v}");
    assert!(
        (length(&ff, &d.path().join("bed.mp4")) - 3.0).abs() < 0.07,
        "{v}"
    );
    // No length at all: exit 2, and the fix line says how to make a video with no words.
    for b in [
        json!({"script": ""}),
        json!({"scenes": [{"media": "x.png"}]}),
    ] {
        let (code, v) = sv(&ff, d.path(), "plan", "empty", &b);
        assert_eq!(code, Some(2), "{v}");
        assert_eq!(v["error"]["message"], "the script is empty");
        assert!(
            v["error"]["fix"].as_str().unwrap().contains("`seconds`"),
            "{v}"
        );
    }
}

#[test]
fn seconds_errors_name_the_scene_and_give_a_fix() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    for (b, want) in [
        (
            json!({"scenes": [{"script": "Hello there.", "seconds": 4}]}),
            "scenes[0] has `seconds` and a script",
        ),
        (
            json!({"scenes": [{"seconds": 0}]}),
            "scenes[0].seconds is a number from 0.2 to 600",
        ),
        (
            json!({"duration": 12, "scenes": [{"seconds": 4}, {"seconds": 5}]}),
            "add up to 9 s",
        ),
    ] {
        let (code, v) = sv(&ff, d.path(), "plan", "bad", &b);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(want),
            "{v}"
        );
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
}

#[test]
fn a_pause_at_the_end_holds_the_picture_and_a_long_tag_is_said() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let b = json!({"tts": "none", "lang": "en", "aspect": "16:9", "music": "pulse", "script": "Dance. [pause 4]"});
    let (code, plan) = sv(&ff, d.path(), "plan", "hold", &b);
    assert_eq!(code, Some(0), "{plan}");
    // One word at 2.5 a second and 4 s: not "a 0 s video".
    assert!(stage(&plan, "render").contains("for a 4 s video"), "{plan}");
    let (code, v) = sv(&ff, d.path(), "make", "hold", &b);
    assert_eq!(code, Some(0), "{v}");
    // 0.1 s, one word (0.4 s), the 4 s of the tag, and the tail after the narration.
    let len = length(&ff, &d.path().join("hold.mp4"));
    assert!((len - 5.1).abs() < 0.07, "{len} {v}");
    let long = json!({"tts": "none", "lang": "en", "script": "Dance. [pause 11]"});
    let (_, plan) = sv(&ff, d.path(), "plan", "long", &long);
    let said = plan["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["gate"] == "pauses" && f["status"] == "warn");
    assert!(said, "{plan}");
}

#[test]
fn plan_gives_the_answer_of_make_for_no_voice() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    // The brief of the 14 Nov test: the plan said ok, and the run stopped after "the voice".
    let fits = json!({"tts": "none", "lang": "en", "aspect": "16:9", "duration": 12, "music": "pulse",
        "scenes": [{"script": "Dance. [pause 10]"}]});
    let (code, plan) = sv(&ff, d.path(), "plan", "fits", &fits);
    assert_eq!(code, Some(0), "{plan}");
    let (code, v) = sv(&ff, d.path(), "make", "fits", &fits);
    assert_eq!(code, Some(0), "{v}");
    assert!(
        (length(&ff, &d.path().join("fits.mp4")) - 12.0).abs() < 0.07,
        "{v}"
    );
    // Too short by far: the same code and exit from both, and nothing is made.
    let short = json!({"tts": "none", "lang": "en", "duration": 12, "script": "Dance. [pause 4]"});
    let (pc, p) = sv(&ff, d.path(), "plan", "short", &short);
    let (mc, m) = sv(&ff, d.path(), "make", "short", &short);
    assert_eq!((pc, mc), (Some(3), Some(3)), "{p} {m}");
    assert_eq!(p["error"]["code"], "SVX_NEEDS_LONGER_SCRIPT");
    assert_eq!(p["error"]["message"], m["error"]["message"]);
    assert!(m["run_id"].is_null(), "stopped before a run folder: {m}");
    // A voice that is made: the plan says where the fit is checked.
    let (_, p) = sv(
        &ff,
        d.path(),
        "plan",
        "edge",
        &json!({"tts": "kokoro-rust", "lang": "en", "duration": 12,
        "script": "Dance with me in the kitchen tonight, one more time."}),
    );
    if p["ok"] == true {
        assert!(stage(&p, "voice").contains("checked at make"), "{p}");
    }
}

#[test]
fn a_scene_without_seconds_keeps_its_old_plan_id_input() {
    let sc: scriptvox::brief::Scene = serde_json::from_value(json!({"script": "Hello."})).unwrap();
    let v = serde_json::to_value(&sc).unwrap();
    assert_eq!(
        v,
        json!({"script": "Hello."}),
        "`seconds` enters the plan id only when it is set"
    );
    let sc: scriptvox::brief::Scene = serde_json::from_value(json!({"seconds": 4})).unwrap();
    assert_eq!(serde_json::to_value(&sc).unwrap()["seconds"], 4.0);
}
