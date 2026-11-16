//! Brief `size` (docs/specs/output-size.md), end to end with no voice and no network: the frame of the file, the
//! caption size that follows it, the plan line, the plan id, the brand file, a draft and its final.
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

fn run(p: &Path, ff: &str, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("SCRIPTVOX_FFMPEG", ff)
        .args(args)
        .args(["--json"])
        .output()
        .unwrap();
    (
        o.status.code(),
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
    )
}

fn brief(p: &Path, more: &str) {
    let body = format!(
        r#"{{"scenes": [{{"script": "A short line for the size test.", "card": {{"kind": "title", "text": "Size"}}}}, {{"script": "And one more line."}}], "lang": "en", "tts": "none", "aspect": "16:9"{more}}}"#
    );
    std::fs::write(p.join("b.json"), body).unwrap();
}

/// The frame of the made file: `make` names it in `artifacts.video`, `resume` in `out` (relative to the project).
fn frame_of(ff: &str, p: &Path, v: &Value) -> String {
    let out = v["artifacts"]["video"]
        .as_str()
        .or(v["out"].as_str())
        .unwrap_or_else(|| panic!("{v}"));
    probe(ff, &p.join(out), "v:0", "stream=width,height").replace('\n', "x")
}

#[test]
fn size_sets_the_frame_and_the_text_follows_it() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    brief(p, r#", "size": 720"#);
    let (code, v) = run(p, &ff, &["make", "--brief", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(frame_of(&ff, p, &v), "1280x720", "{v}");
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    assert!(ass.contains("PlayResX: 1280\nPlayResY: 720"), "{ass}");
    // 84 px on a 1080 px short side is 56 px on 720.
    let cap = ass.lines().find(|l| l.starts_with("Style: Cap,")).unwrap();
    assert_eq!(cap.split(',').nth(2), Some("56"), "{cap}");
    let gate = |name: &str| {
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["gate"] == name)
            .map(|f| f["status"].as_str().unwrap().to_string())
    };
    assert_eq!(gate("text_bounds").as_deref(), Some("pass"), "{v}");
    assert_eq!(gate("captions").as_deref(), Some("pass"), "{v}");

    // A draft is half of the asked size, and its final has the asked size again.
    let (code, v) = run(p, &ff, &["make", "--brief", "b.json", "--draft"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(frame_of(&ff, p, &v), "640x360", "{v}");
    let id = v["run_id"].as_str().unwrap().to_string();
    let (code, v) = run(p, &ff, &["resume", &id, "--final"]);
    assert_eq!(code, Some(0), "{v}");
    let child = v["run_id"].as_str().unwrap().to_string();
    let (code, done) = run(p, &ff, &["wait", &child]);
    assert_eq!(code, Some(0), "{done}");
    assert_eq!(frame_of(&ff, p, &v), "1280x720", "{done}");
}

#[test]
fn the_plan_states_the_size_and_an_old_brief_keeps_its_id() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    brief(p, r#", "size": 1000"#);
    let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
    assert_eq!(code, Some(2), "{v}");
    let said = v["error"].to_string();
    assert!(said.contains("720") && said.contains("2160"), "{said}");
    let plan = |more: &str| {
        brief(p, more);
        let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
        assert_eq!(code, Some(0), "{v}");
        v
    };
    let id = |v: &Value| v["plan_id"].as_str().unwrap().to_string();
    let render = |v: &Value| {
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["stage"] == "render")
            .unwrap()["explain"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let none = plan("");
    // The id of this brief before `size` existed (13 Nov 2026, main at 0a399c3): a brief with no size keeps it.
    assert_eq!(id(&none), id(&plan(r#", "size": 1080"#)));
    assert!(render(&none).starts_with("1920x1080 at 30 fps, chunked, resumable; about "));
    assert!(!render(&none).contains("scaled up"));
    let big = plan(r#", "size": 2160"#);
    assert_ne!(id(&none), id(&big));
    let line = render(&big);
    for word in [
        "3840x2160",
        "s to render",
        "MB",
        "GB of memory",
        "scaled up",
    ] {
        assert!(line.contains(word), "{line}");
    }
    assert!(!big["findings"].to_string().contains("\"size\""), "{big}");
    // 21:9 at 2160 with two scenes is 5040x2160.
    assert!(render(&plan(r#", "size": 2160, "aspect": "21:9""#)).starts_with("5040x2160"));
    // A brand file may carry it; the brief wins.
    std::fs::write(p.join("brand.json"), r#"{"size": 2160}"#).unwrap();
    assert_eq!(id(&plan(r#", "brand": "brand.json""#)), id(&big));
    assert_eq!(
        id(&plan(r#", "brand": "brand.json", "size": 1080"#)),
        id(&none)
    );
    // Many scenes on a large frame: the picture is made in groups of scenes (docs/specs/render-split.md), so the
    // plan says that and does not warn. With the split off, the plan warns about memory, with a fix.
    let scenes: Vec<String> = (0..12)
        .map(|i| format!(r#"{{"script": "Scene number {i} has a few words."}}"#))
        .collect();
    std::fs::write(
        p.join("b.json"),
        format!(
            r#"{{"scenes": [{}], "lang": "en", "tts": "none", "aspect": "16:9", "size": 2160}}"#,
            scenes.join(",")
        ),
    )
    .unwrap();
    let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    assert!(!v["findings"].to_string().contains("\"size\""), "{v}");
    assert!(v.to_string().contains("made in 4 groups of scenes"), "{v}");
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("SCRIPTVOX_FFMPEG", &ff)
        .env("SCRIPTVOX_SPLIT_SCENES", "0")
        .args(["plan", "--brief", "b.json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let warn = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "size")
        .unwrap_or_else(|| panic!("{v}"));
    assert_eq!(warn["status"], "warn");
    assert!(warn["fix"].as_str().unwrap().contains("join"), "{warn}");
}
