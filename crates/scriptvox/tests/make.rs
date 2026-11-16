//! `scriptvox make --script` end to end with the stub worker (tests/common).
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

#[test]
fn script_becomes_a_captioned_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make(
        "ScriptVox reads this script. Then it renders a video.",
        "ok",
        &[],
    );
    assert_eq!(code, 0, "{v}");
    assert!(Path::new(v["artifacts"]["video"].as_str().unwrap()).is_file());
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    assert!(
        ass.contains("{\\k") && ass.contains("script."),
        "captions keep the script punctuation"
    );
    assert_eq!(v["lang"], "en");
    // Every events.jsonl line follows scriptvox.event/v1 (schemas/event.json).
    let run = v["run_id"].as_str().unwrap();
    let events = std::fs::read_to_string(v["events_file"].as_str().unwrap()).unwrap();
    let types = [
        "run.start",
        "progress",
        "stage.start",
        "stage.done",
        "fallback",
        "range_done",
        "run.end",
    ];
    for line in events.lines() {
        let e: Value = serde_json::from_str(line).unwrap();
        assert_eq!(e["v"], 1);
        assert_eq!(e["run"], run);
        assert!(types.contains(&e["type"].as_str().unwrap()), "{line}");
        let t = e["t"].as_str().unwrap();
        assert!(t.len() == 20 && t.ends_with('Z'), "{line}");
    }
    assert!(events.lines().last().unwrap().contains("\"run.end\""));
    // The voice line says who gets the text; no line about commercial use (owner, 13 Nov 2026).
    let line = v["explain"][0].as_str().unwrap();
    assert!(
        line.starts_with("TTS edge") && !line.contains("commercial"),
        "{line}"
    );
    // 3 s of audio plus the 0.6 s tail at 30 fps.
    assert!(
        (v["metrics"]["frames"].as_u64().unwrap() as i64 - 108).abs() <= 1,
        "{}",
        v["metrics"]
    );
}

#[test]
fn arabic_refusal_is_needs_input_without_english_engines() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("مرحبا بكم في راويفوكس.", "refuse", &[]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["status"], "needs_input");
    let alts = v["needs_input"][0]["alternatives"].as_array().unwrap();
    assert!(!alts.is_empty());
    assert!(alts.iter().all(|a| a["args"][1] != "edge"), "{alts:?}");
}

#[test]
fn edge_hiccup_is_retryable_exit_5() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Hello there.", "flaky", &[]);
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["retryable"], true);
}

#[test]
fn local_only_with_edge_is_needs_input_and_names_edge() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("s.txt"), "Hello there.").unwrap();
    let out = common::bin()
        .current_dir(d.path())
        .args([
            "make",
            "--script",
            "s.txt",
            "--local-only",
            "--tts",
            "edge",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(10), "{v}");
    assert!(v["needs_input"][0]["human_action"]
        .as_str()
        .unwrap()
        .contains("Edge is a network option"));
}

#[test]
fn pause_tag_gives_exact_silence_and_shifted_words() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("one two [pause 1.5] three four", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let voice = v["artifacts"]["audio"].as_str().unwrap();
    assert!(voice.ends_with("voice.wav"));
    let out = std::process::Command::new(&ff)
        .args([
            "-hide_banner",
            "-i",
            voice,
            "-af",
            "silencedetect=noise=-60dB:d=0.5",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    let d: f64 = err
        .split("silence_duration: ")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap()
        .parse()
        .unwrap();
    assert!((d - 1.5).abs() < 0.005, "silence {d} s");
    // The second segment's words start after 3 s of audio plus the 1.5 s pause.
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(srt.contains("00:00:04,600 --> "), "{srt}");
    assert!(!srt.contains("pause"), "tags never reach captions");
}

fn frames(v: &Value) -> i64 {
    v["metrics"]["frames"].as_i64().unwrap()
}

#[test]
fn duration_within_3_percent_is_kept() {
    let Some(ff) = ready() else { return };
    let (code, v) = Env::new(&ff).make(
        "one two three four five six seven eight nine ten",
        "ok",
        &["--duration", "3.05"],
    );
    assert_eq!(code, 0, "{v}");
    assert!((frames(&v) - 92).abs() <= 1, "{v}");
    assert!(
        v["explain"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| !l.as_str().unwrap().starts_with("fit:")),
        "no fit step: {v}"
    );
    assert_eq!(v["metrics"]["voice_rate_pct"], 0);
    assert_eq!(v["metrics"]["voice_tempo"], 1.0);
}

#[test]
fn duration_within_8_percent_uses_atempo() {
    let Some(ff) = ready() else { return };
    let env = Env::new(&ff); // keep the temp folder alive until ffprobe reads it
    let (code, v) = env.make("one two three four five", "ok", &["--duration", "2.85"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["explain"][2].as_str().unwrap().contains("atempo 1.05"),
        "{v}"
    );
    assert_eq!(v["metrics"]["voice_tempo"], 1.053, "{v}");
    let fitted = std::process::Command::new(Path::new(&ff).with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(v["artifacts"]["audio"].as_str().unwrap())
        .output()
        .unwrap();
    let d: f64 = String::from_utf8_lossy(&fitted.stdout)
        .trim()
        .parse()
        .unwrap();
    assert!((d - 2.85).abs() < 0.02, "fitted audio {d} s");
}

#[test]
fn far_too_long_stops_before_render() {
    let Some(ff) = ready() else { return };
    // The stub ignores the TTS rate, so the retry stays 2x long: exit 3, no video.
    let (code, v) = Env::new(&ff).make("one two three four five", "ok", &["--duration", "1.5"]);
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "SVX_NEEDS_SHORTER_SCRIPT");
    assert!(v["error"]["fix"].as_str().unwrap().starts_with("Cut about"));
    assert!(v["artifacts"].as_object().unwrap().is_empty());
}

#[test]
fn clear_miss_stops_before_tts() {
    let Some(ff) = ready() else { return };
    let script = "word ".repeat(150);
    // STUB_MODE=flaky would give exit 5 if TTS ran: exit 3 proves it never did.
    let (code, v) = Env::new(&ff).make(&script, "flaky", &["--duration", "10"]);
    assert_eq!(code, 3, "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("before TTS"));
}

#[test]
fn resynth_that_stays_far_off_is_exit_3_not_a_panic() {
    let Some(ff) = ready() else { return };
    // 3 s for 2.5 s asks for a faster TTS rate; the stub ignores it, so the
    // second pass is still 20% long: a clean exit 3.
    let (code, v) = Env::new(&ff).make("one two three four five", "ok", &["--duration", "2.5"]);
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "SVX_NEEDS_SHORTER_SCRIPT");
}

#[test]
fn failing_encoder_falls_back_and_says_why() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    std::fs::write(p.join("s.txt"), "one two three").unwrap();
    let out = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
        .env("SCRIPTVOX_DATA", p.join("data"))
        // As `Env::cmd`: a user `stt` row would hear the stub tone and fail gate `words`.
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("STUB_AUDIO", p.join("voice.mp3"))
        .env("SCRIPTVOX_ENCODER", "h264_fake_hw,libx264")
        .args(["make", "--script", "s.txt", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{v}");
    let explain: Vec<&str> = v["explain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(
        explain
            .iter()
            .any(|l| l.starts_with("encoder h264_fake_hw skipped:")),
        "{explain:?}"
    );
    assert!(explain.contains(&"encoder: libx264"));
    let video = Path::new(v["artifacts"]["video"].as_str().unwrap());
    assert!(video.is_file());
    assert!(
        !video.with_file_name("video.mp4.part.mp4").exists(),
        "the part file is renamed away"
    );
}

#[test]
fn own_media_becomes_the_shot_list() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    let (code, v) = e.make(
        "First shot here. Second shot now. Third one comes. And the last.",
        "ok",
        &["--media", "media"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["metrics"]["shots"], 4,
        "every file appears: {}",
        v["metrics"]
    );
}

/// security-5: logo, overlay, inset, font and narration are read from inside the project only.
#[test]
fn a_brief_cannot_pull_a_file_from_outside_the_project() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let out = tempfile::tempdir().unwrap();
    std::fs::write(out.path().join("a.png"), b"x").unwrap();
    let a = out.path().join("a.png");
    let a = a.to_str().unwrap();
    for field in [
        format!(r#""logo": {{"file": "{a}"}}"#),
        format!(r#""narration": "{a}""#),
        format!(r#""scenes": [{{"script": "Hello there.", "overlay": "{a}"}}]"#),
        format!(r#""scenes": [{{"script": "Hello there.", "inset": {{"file": "{a}"}}}}]"#),
    ] {
        let script = if field.contains("scenes") {
            ""
        } else {
            r#""script": "Hello there.","#
        };
        std::fs::write(e.dir.path().join("b.json"), format!("{{{script} {field}}}")).unwrap();
        let o = e
            .cmd("flaky")
            .args(["make", "--brief", "b.json", "--plan", "--json"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["error"]["code"], "SVX_PATH_OUTSIDE_ROOT", "{field}: {v}");
    }
}

#[test]
fn media_outside_the_project_is_refused() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("a.mp4"), b"x").unwrap();
    let (code, v) = e.make(
        "Hello.",
        "flaky",
        &["--media", outside.path().to_str().unwrap()],
    );
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["error"]["code"], "SVX_PATH_OUTSIDE_ROOT");
}

#[test]
fn missing_or_empty_media_folder_has_a_fix() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::create_dir_all(e.dir.path().join("empty")).unwrap();
    for dir in ["nope", "empty"] {
        // STUB_MODE=flaky: the check must fail before TTS (else exit 5).
        let (code, v) = e.make("Hello.", "flaky", &["--media", dir]);
        assert_eq!(code, 1, "{dir}: {v}");
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
}

#[test]
fn media_add_refuses_outside_and_indexes_inside() {
    let d = tempfile::tempdir().unwrap();
    let proj = d.path().join("proj");
    std::fs::create_dir_all(proj.join("clips")).unwrap();
    std::fs::write(proj.join("clips/a.jpg"), b"x").unwrap();
    std::fs::write(d.path().join("b.mp4"), b"x").unwrap();
    let run = |arg: &str| {
        let o = common::bin()
            .current_dir(&proj)
            .args(["media", "add", arg, "--json"])
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run("..");
    assert_eq!(
        (code, v["error"]["code"].as_str()),
        (1, Some("SVX_PATH_OUTSIDE_ROOT"))
    );
    let (code, v) = run("clips");
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"][0]["kind"], "image");
    assert!(proj.join(".scriptvox/media.json").is_file());
}

#[test]
fn cached_tts_rerun_makes_no_worker_call() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Cache me once. Then replay me.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    // A worker call now would fail (exit 5): exit 0 proves the cache answered.
    let (code, v) = e.make("Cache me once. Then replay me.", "flaky", &[]);
    assert_eq!(code, 0, "{v}");
    let (code, _) = e.make("A different script.", "flaky", &[]);
    assert_eq!(code, 5, "a new script still calls the worker");
}

#[test]
fn brief_file_drives_make_and_flags_override_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    std::fs::write(
        p.join("b.json"),
        r#"{"script": "A brief makes this video. Flags may override it.", "aspect": "1:1"}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "b.json", "--aspect", "16:9", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let probe = std::process::Command::new(Path::new(&ff).with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(v["artifacts"]["video"].as_str().unwrap())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&probe.stdout).trim(),
        "1920,1080",
        "--aspect overrides the brief"
    );
}

#[test]
fn topic_only_asks_the_agent_for_a_script() {
    let d = tempfile::tempdir().unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .args(["make", "dates in the Gulf", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(10), "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "script");
    assert!(v["needs_input"][0]["human_action"]
        .as_str()
        .unwrap()
        .contains("dates in the Gulf"));
}

#[test]
fn scenes_put_each_visual_on_its_own_line() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    let p = e.dir.path();
    // The stub voice is 3 s with words 0.35 s apart: 7 words fit inside it.
    std::fs::write(
        p.join("b.json"),
        r#"{"scenes": [
            {"script": "One test card.", "media": "media/01-testcard.mp4"},
            {"script": "Two fractal.", "media": "media/02-fractal.jpg"},
            {"script": "Three none."}
        ]}"#,
    )
    .unwrap();
    let o = e
        .cmd("ok")
        .args(["make", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["shots"], 3);
    // A scene file outside the project is refused before any voice call.
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("x.mp4"), b"x").unwrap();
    std::fs::write(
        p.join("bad.json"),
        format!(
            r#"{{"scenes": [{{"script": "Hi.", "media": "{}"}}]}}"#,
            outside.path().join("x.mp4").display()
        ),
    )
    .unwrap();
    let o = e
        .cmd("flaky")
        .args(["make", "--brief", "bad.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["error"]["code"], "SVX_PATH_OUTSIDE_ROOT", "{v}");
}

#[test]
fn no_voice_has_no_voice_metrics_and_check_of_a_file_names_the_next_step() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make(
        "Text on screen only, read at a calm pace.",
        "ok",
        &["--tts", "none"],
    );
    assert_eq!(code, 0, "{v}");
    assert!(v["metrics"].get("voice_tempo").is_none(), "{v}");
    assert!(v["metrics"].get("voice_rate_pct").is_none(), "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap();
    let o = common::bin()
        .current_dir(e.dir.path())
        .args(["check", video, "--json"])
        .output()
        .unwrap();
    let c: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(c["next"][0].as_str().unwrap().contains("snapshot"), "{c}");
}

#[test]
fn a_draft_is_half_size_and_export_refuses_it_until_final() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make(
        "A quick look first, the full video later.",
        "ok",
        &["--draft"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["draft"], true, "{v}");
    let run = v["run_id"].as_str().unwrap();
    assert!(v["next"][1].as_str().unwrap().contains("--final"), "{v}");
    // 15 fps against the usual 30.
    let full = Env::new(&ff);
    let (_, w) = full.make("A quick look first, the full video later.", "ok", &[]);
    let (d, f) = (frames(&v) as f64, frames(&w) as f64);
    assert!((f / d - 2.0).abs() < 0.1, "{d} and {f} frames");
    let cmd = |args: &[&str]| {
        let p = e.dir.path();
        let mut c = common::bin();
        common::shared_uv(&mut c);
        let o = c
            .current_dir(p)
            .env("SCRIPTVOX_WORKER_SRC", p.join("worker"))
            .env("SCRIPTVOX_DATA", p.join("data"))
            // As `Env::cmd`: a user `stt` row would hear the stub tone and fail gate `words`.
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("STUB_AUDIO", p.join("voice.mp3"))
            .env("STUB_MODE", "ok")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, x) = cmd(&["export", run]);
    assert_eq!(code, 2, "{x}");
    assert!(
        x["error"]["fix"].as_str().unwrap().contains("--final"),
        "{x}"
    );
    // By its file too, not only by its run.
    let (code, x) = cmd(&["export", v["artifacts"]["video"].as_str().unwrap()]);
    assert_eq!(code, 2, "{x}");
    let (code, r) = cmd(&["resume", run, "--final", "--wait"]);
    assert_eq!(code, 0, "{r}");
    assert!(r["metrics"].get("draft").is_none(), "{r}");
    assert!(frames(&r).abs_diff(frames(&w)) <= 1, "{r}");
}

#[test]
fn a_change_of_emphasis_or_card_changes_the_plan_id() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let plan = |brief: &str| {
        std::fs::write(e.dir.path().join("b.json"), brief).unwrap();
        let o = common::bin()
            .current_dir(e.dir.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["plan", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        v["plan_id"].as_str().unwrap().to_string()
    };
    let base = plan(
        r#"{"lang": "en", "tts": "edge", "scenes": [{"script": "One two three.", "card": {"kind": "title", "text": "A"}}]}"#,
    );
    let card = plan(
        r#"{"lang": "en", "tts": "edge", "scenes": [{"script": "One two three.", "card": {"kind": "title", "text": "B"}}]}"#,
    );
    let mark = plan(
        r#"{"lang": "en", "tts": "edge", "emphasis": ["two"], "scenes": [{"script": "One two three.", "card": {"kind": "title", "text": "A"}}]}"#,
    );
    assert_ne!(base, card);
    assert_ne!(base, mark);
}

// Only a run id becomes a path under `.scriptvox/runs`: a result file in another folder is not read.
#[test]
fn export_does_not_leave_the_runs_folder() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let d = e.dir.path();
    std::fs::create_dir_all(d.join(".scriptvox/runs")).unwrap();
    std::fs::create_dir_all(d.join("other")).unwrap();
    std::fs::write(
        d.join("other/result.json"),
        r#"{"artifacts": {"video": "other/v.mp4"}}"#,
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d)
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args(["export", "../../other", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not a finished run"),
        "{v}"
    );
}

// Skills test, 11 Nov 2026: the plan of a `behind` title listed no matte step, and the run then called the row.
#[test]
fn a_title_behind_the_subject_is_a_step_of_the_plan() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    std::fs::write(
        e.dir.path().join("b.json"),
        r#"{"lang": "en", "tts": "edge", "scenes": [{"script": "One two three.", "card": {"kind": "title", "text": "A", "behind": true}}]}"#,
    )
    .unwrap();
    let o = common::bin()
        .current_dir(e.dir.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .env("SCRIPTVOX_SELFHOST_KEY", "test-key")
        .args(["plan", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert!(
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|st| st["stage"] == "behind" && st["tool"].as_str().unwrap().starts_with("matte.")),
        "{v}"
    );
}

#[test]
fn export_writes_more_formats_and_a_transparent_overlay() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make(
        "A short line for the export formats, with captions.",
        "ok",
        &[],
    );
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap();
    let p = e.dir.path();
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .args(["export", run, "--format", "hls,webm", "--overlay", "--json"])
        .output()
        .unwrap();
    let x: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{x}");
    let art = &x["artifacts"];
    for k in ["video_hls", "video_webm", "overlay_mov", "overlay_webm"] {
        assert!(
            std::path::Path::new(art[k].as_str().unwrap()).is_file(),
            "{k}: {x}"
        );
    }
    // The overlay's first frame is empty and transparent: the alpha plane reads 0 in the corner.
    let shot = p.join("a.gray");
    let s = std::process::Command::new(&ff)
        .args(["-v", "error", "-y", "-c:v", "libvpx-vp9", "-i"])
        .arg(art["overlay_webm"].as_str().unwrap())
        .args([
            "-vf",
            "alphaextract,crop=8:8:0:0",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
        ])
        .arg(&shot)
        .status()
        .unwrap();
    assert!(s.success());
    assert!(std::fs::read(&shot).unwrap().iter().all(|b| *b == 0));
}

/// A silent video with music: a bed that stops before the end is reported (11 Nov 2026: the bed of a 15 s video
/// ended at 9.3 s and `make` dropped the silence finding, because the video had no voice).
#[test]
fn music_that_stops_before_the_end_of_a_silent_video_is_reported() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    assert!(std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=2,apad=whole_dur=30",
            "-ac",
            "2",
            "-y",
            "bed.wav"
        ])
        .status()
        .unwrap()
        .success());
    std::fs::write(
        d.path().join("b.json"),
        r#"{"tts": "none", "music": "bed.wav", "script": "One two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty."}"#,
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args([
            "make",
            "--brief",
            "b.json",
            "--local-only",
            "--draft",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    let silence = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "silence")
        .unwrap_or_else(|| panic!("no silence finding: {v}"));
    assert_eq!(silence["status"], "warn", "{v}");
}

#[test]
fn a_file_named_as_the_topic_is_the_brief() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("b.json"),
        r#"{"script": "Hello there.", "tts": "none"}"#,
    )
    .unwrap();
    let plan = |name: &str| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_DATA", d.path().join("data"))
            .args(["plan", name, "--draft", "--json"])
            .output()
            .unwrap();
        serde_json::from_slice::<Value>(&o.stdout).unwrap()
    };
    let v = plan("b.json");
    assert_eq!(v["ok"], true, "{v}");
    assert!(
        v["draft_leaves_out"][0]
            .as_str()
            .unwrap()
            .contains("15 fps"),
        "{v}"
    );
    std::fs::write(d.path().join("notes"), "not a script").unwrap();
    assert!(plan("notes")["needs_input"][0]["human_action"]
        .as_str()
        .unwrap()
        .contains("about \"notes\""));
    let v = plan("gone.json");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no such file"),
        "{v}"
    );
}

#[test]
fn hls_parts_keep_their_promise_and_a_percent_in_the_folder_stays() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let script = "Words go on here for a good while. ".repeat(6);
    let (code, v) = e.make(&script, "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let run = v["run_id"].as_str().unwrap();
    let p = e.dir.path();
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .args(["export", run, "--format", "hls", "--dir", "out%d", "--json"])
        .output()
        .unwrap();
    let x: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{x}");
    assert!(
        p.join("out%d/metadata.json").is_file() && p.join("out%d/cover.jpg").is_file(),
        "{x}"
    );
    let list = std::fs::read_to_string(p.join("out%d/hls/index.m3u8")).unwrap();
    assert!(p.join("out%d/hls/000.ts").is_file(), "{list}");
    for l in list.lines().filter_map(|l| l.strip_prefix("#EXTINF:")) {
        let d: f64 = l.trim_end_matches(',').parse().unwrap();
        assert!(d <= 6.5, "{list}");
    }
}

#[test]
fn a_recorded_narration_is_the_voice_and_sets_the_length() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=220",
            "-t",
            "4",
        ])
        .arg(p.join("me.wav"))
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(
        p.join("b.json"),
        r#"{"narration": "me.wav", "script": "These words were said by a person in a room."}"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_DATA", p.join("data"))
            .env("SCRIPTVOX_CONFIG", p.join("cfg"))
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(args)
            .args(["--json"])
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (code, v) = run(&["make", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    let secs = v["metrics"]["duration_s"].as_f64().unwrap();
    assert!((4.0..5.0).contains(&secs), "{v}");
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(srt.contains("These words"), "{srt}");
    // No such file, and a duration with a recording, stop at the plan.
    std::fs::write(
        p.join("c.json"),
        r#"{"narration": "gone.wav", "script": "Some words here."}"#,
    )
    .unwrap();
    assert_eq!(run(&["plan", "c.json"]).0, Some(2));
    std::fs::write(
        p.join("e.json"),
        r#"{"narration": "me.wav", "duration": 9, "script": "Some words here."}"#,
    )
    .unwrap();
    assert_eq!(run(&["plan", "e.json"]).0, Some(2));
}

/// G048: a scene `overlay` with no alpha is refused before any voice; one with alpha plans.
#[test]
fn an_overlay_needs_alpha() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let mk = |src: &str, out: &str| {
        let st = std::process::Command::new(&ff)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                src,
                "-frames:v",
                "1",
            ])
            .arg(p.join(out))
            .status()
            .unwrap();
        assert!(st.success());
    };
    mk("color=c=black@0.0:s=64x64,format=rgba", "alpha.png");
    mk("color=c=red:s=64x64", "solid.jpg");
    let plan = |file: &str| {
        let b = serde_json::json!({"tts": "none", "scenes": [{"script": "One two three.", "overlay": file}]});
        std::fs::write(p.join("b.json"), b.to_string()).unwrap();
        let o = common::bin()
            .current_dir(p)
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["plan", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
        (o.status.code().unwrap_or(-1), v)
    };
    let (code, v) = plan("solid.jpg");
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("no alpha"),
        "{v}"
    );
    let (code, v) = plan("alpha.png");
    assert_eq!(code, 0, "{v}");
    let (code, _) = plan("missing.webm");
    assert_eq!(code, 2);
}

#[test]
fn make_keeps_a_subtitle_file_it_did_not_write_beside_out() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let mine = e.dir.path().join("a.srt");
    std::fs::write(&mine, "my hand fixed subtitles").unwrap();
    let (code, v) = e.make("A short line for the subtitles.", "ok", &["--out", "a.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        std::fs::read_to_string(&mine).unwrap(),
        "my hand fixed subtitles"
    );
    let new = v["artifacts"]["sidecar_srt"].as_str().unwrap();
    assert!(new.ends_with("a-2.srt") && Path::new(new).is_file(), "{v}");
    assert!(
        e.dir.path().join("a.vtt").is_file(),
        "no file there, so the plain name"
    );
}

/// make-11: a duration that is no length is a usage error, not a script length problem.
#[test]
fn a_duration_that_is_no_length_is_a_usage_error() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    for d in ["NaN", "inf", "0", "-5"] {
        let flag = format!("--duration={d}");
        let (code, v) = e.make("A short script.", "ok", &["--plan", &flag]);
        assert_eq!(code, 2, "{d}: {v}");
        assert_eq!(v["error"]["code"], "SVX_USAGE", "{d}: {v}");
    }
}

/// ScriptVox's own earlier subtitle is replaced by a rerun, so `snapshot` and `judge` read the new words.
#[test]
fn a_rerun_replaces_its_own_subtitle_beside_out() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("A small test of the script.", "ok", &["--out", "r.mp4"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = e.make(
        "Totally different words are here.",
        "ok",
        &["--out", "r.mp4"],
    );
    assert_eq!(code, 0, "{v}");
    let srt = std::fs::read_to_string(e.dir.path().join("r.srt")).unwrap();
    assert!(srt.contains("different"), "{srt}");
    assert!(!e.dir.path().join("r-2.srt").exists(), "{v}");
}

#[test]
fn make_never_writes_over_a_clip_it_reads() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    let clip = std::fs::read_dir(e.dir.path().join("media"))
        .unwrap()
        .map(|f| f.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "mp4"))
        .unwrap();
    let before = std::fs::read(&clip).unwrap();
    let (code, v) = e.make(
        "A short line over a clip.",
        "ok",
        &["--media", "media", "--out", clip.to_str().unwrap()],
    );
    assert_eq!(code, 2, "{v}");
    assert_eq!(std::fs::read(&clip).unwrap(), before, "{v}");
}

/// A video cut short since the run is no finished run: the same key makes it again.
#[test]
fn a_damaged_video_is_not_replayed_as_a_finished_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let args = ["--idempotency-key", "K1", "--out", "kk.mp4"];
    let (code, v) = e.make("A short line for the key.", "ok", &args);
    assert_eq!(code, 0, "{v}");
    let path = e.dir.path().join("kk.mp4");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    let (code, v) = e.make("A short line for the key.", "ok", &args);
    assert_eq!(code, 0, "{v}");
    assert!(v["idempotent"].is_null(), "{v}");
    assert_eq!(std::fs::read(&path).unwrap().len(), bytes.len(), "{v}");
}

/// security-5: a brief's script and cutaway files stay inside the project.
#[test]
fn a_brief_cannot_read_a_script_or_a_cutaway_from_outside_the_project() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let far = tempfile::tempdir().unwrap();
    let (secret, clip) = (far.path().join("secret.txt"), far.path().join("clip.mp4"));
    std::fs::write(&secret, "stolen words").unwrap();
    std::fs::write(&clip, "x").unwrap();
    let briefs = [
        serde_json::json!({"script_file": secret}),
        serde_json::json!({"base_video": "base.mp4", "cutaways": [{"at": 1, "seconds": 2, "file": clip}]}),
    ];
    std::fs::write(e.dir.path().join("base.mp4"), "x").unwrap();
    for b in briefs {
        std::fs::write(e.dir.path().join("b.json"), b.to_string()).unwrap();
        let o = e
            .cmd("ok")
            .args(["make", "--brief", "b.json", "--json"])
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["error"]["code"], "SVX_PATH_OUTSIDE_ROOT", "{v}");
    }
}

#[test]
fn sources_of_the_brief_reach_the_metadata_and_the_credits() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let run = |args: &[&str]| {
        let o = e.cmd("ok").args(args).output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    // A source with no url and no title stops before any work.
    std::fs::write(
        p.join("bad.json"),
        r#"{"script": "Revenue grew by forty two percent.", "sources": [{"text": "Revenue grew 42 percent"}]}"#,
    )
    .unwrap();
    let (code, v) = run(&["make", "--brief", "bad.json", "--json"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("sources"),
        "{v}"
    );
    std::fs::write(
        p.join("b.json"),
        r#"{"script": "Revenue grew by forty two percent.", "sources": [
            {"text": "Revenue grew 42 percent", "title": "Annual report 2025", "url": "https://example.com/report"}]}"#,
    )
    .unwrap();
    let (code, v) = run(&["make", "--brief", "b.json", "--json"]);
    assert_eq!(code, 0, "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    let (code, x) = run(&["export", &id, "--json"]);
    assert_eq!(code, 0, "{x}");
    let meta: Value = serde_json::from_str(
        &std::fs::read_to_string(x["artifacts"]["metadata"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        meta["sources"][0]["url"], "https://example.com/report",
        "{meta}"
    );
    let credits = std::fs::read_to_string(x["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.contains(
            "Source: Revenue grew 42 percent (Annual report 2025, https://example.com/report)"
        ),
        "{credits}"
    );
    // The recipe in the MP4 carries them too.
    let (code, r) = run(&[
        "explain",
        v["artifacts"]["video"].as_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0, "{r}");
    assert!(r.to_string().contains("Annual report 2025"), "{r}");
}

#[test]
fn batch_describe_names_the_keys_and_makes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let run = |brief: &str, extra: &[&str]| {
        std::fs::write(d.path().join("b.json"), brief).unwrap();
        let o = common::bin()
            .current_dir(d.path())
            .args(["batch", "--brief", "b.json", "--json"])
            .args(extra)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    let (code, v) = run(
        r#"{"script": "Welcome to {{city}}, known for {{thing}}.", "fps": {{fps}}}"#,
        &["--describe"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["keys"],
        serde_json::json!(["city", "thing", "fps"]),
        "{v}"
    );
    assert_eq!(
        v["needs_a_number_or_true_false"],
        serde_json::json!(["fps"]),
        "{v}"
    );
    assert_eq!(v["csv_header"], "city,thing,fps", "{v}");
    assert_eq!(v["valid_when_filled"], true, "{v}");
    assert_eq!(v["sample_rows"][0]["fps"], 1, "{v}");
    assert!(!d.path().join(".scriptvox").exists(), "nothing is made");
    // A wrong field name shows before any row is written.
    let (code, v) = run(r#"{"scirpt": "Hi {{name}}"}"#, &["--describe"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["valid_when_filled"], false, "{v}");
    assert!(
        v["findings"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("scirpt"),
        "{v}"
    );
    // Without --describe, --rows is still required; with both, the call is refused.
    let (code, _) = run(r#"{"script": "Hi {{name}}"}"#, &[]);
    assert_eq!(code, 2);
    let (code, _) = run(
        r#"{"script": "Hi {{name}}"}"#,
        &["--describe", "--rows", "r.json"],
    );
    assert_eq!(code, 2);
}
