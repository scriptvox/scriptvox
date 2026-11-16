//! Brief `caption_outline`, `caption_box` and a free `caption_size` (docs/specs/caption-style.md), end to end with no
//! voice: the `Cap` style in `captions.ass`, the plan id, the brand file, `resume --patch` and the refusals.
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

const EN: &str = "Most people think a good video needs a studio. You need a clear idea.";
const AR: &str = "يظن معظم الناس أن الفيديو الجيد يحتاج إلى استوديو كبير. تحتاج إلى فكرة واضحة.";

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

/// The run's `captions.ass`: its `Cap` style from the colours on, and the dialogue lines.
fn made(p: &Path, ff: &str, args: &[&str]) -> (String, Vec<String>, Value) {
    let (code, v) = run(p, ff, args);
    assert_eq!(code, Some(0), "{v}");
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    let style = ass.lines().find(|l| l.starts_with("Style: Cap,")).unwrap();
    let at = style.find(",&H").unwrap();
    let lines = ass
        .lines()
        .filter(|l| l.starts_with("Dialogue:"))
        .map(String::from)
        .collect();
    (style[at + 1..].to_string(), lines, v)
}

fn make(p: &Path, ff: &str, body: &str) -> (String, Vec<String>, Value) {
    std::fs::write(p.join("b.json"), body).unwrap();
    made(p, ff, &["make", "--brief", "b.json"])
}

#[test]
fn the_outline_and_the_box_reach_the_caption_style_in_english_and_arabic() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let en = |more: &str| format!(r#"{{"script": "{EN}", "aspect": "16:9", "tts": "none"{more}}}"#);
    // No field: the style of every earlier version.
    let (old, old_lines, _) = make(p, &ff, &en(""));
    assert_eq!(
        old,
        "&H0000E5FF,&H00FFFFFF,&H59000000,&H59000000,1,0,0,0,100,100,0,0,4,14,0,2,60,60,97,-1"
    );
    // No box, a 5 px outline, 115 percent: an outline style at 96 px. The words are the same.
    let (s, lines, v) = make(
        p,
        &ff,
        &en(r#", "caption_box": 0, "caption_outline": 5, "caption_size": "115%""#),
    );
    assert_eq!(
        s,
        "&H0000E5FF,&H00FFFFFF,&H00000000,&H59000000,1,0,0,0,100,100,0,0,1,5,0,2,60,60,97,-1"
    );
    assert!(v["artifacts"]["captions_ass"].is_string());
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    assert!(ass.contains("Style: Cap,Noto Sans,96,"), "{ass}");
    assert!(!lines.is_empty() && !old_lines.is_empty());
    // A lighter box with an outline inside it: the padding stays 14 (3 + 11).
    let (s, _, _) = make(p, &ff, &en(r#", "caption_box": 0.3, "caption_outline": 3"#));
    assert!(
        s.contains(",&H00000000,&HB3000000,") && s.contains(",4,3,11,2,"),
        "{s}"
    );
    // `resume --patch` takes the fields and reuses the voice.
    let run_id = v["run_id"].as_str().unwrap().to_string();
    std::fs::write(
        p.join("patch.json"),
        r#"{"caption_box": 1, "caption_outline": null}"#,
    )
    .unwrap();
    let (s, _, v) = made(
        p,
        &ff,
        &["resume", &run_id, "--patch", "patch.json", "--wait"],
    );
    // The parent's 115 percent stays: 96 px, so the box's own padding is 16.
    assert!(
        s.contains(",&H00000000,&H00000000,") && s.contains(",4,16,0,2,"),
        "{s}"
    );
    assert_ne!(v["run_id"].as_str().unwrap(), run_id);
    // Arabic: the same style fields, and every caption line still starts with the right-to-left mark.
    let (s, lines, _) = make(
        p,
        &ff,
        &format!(
            r#"{{"script": "{AR}", "lang": "ar", "aspect": "16:9", "tts": "none", "caption_box": 0, "caption_outline": 5}}"#
        ),
    );
    assert!(s.contains(",1,5,0,2,"), "{s}");
    let caps: Vec<_> = lines.iter().filter(|l| l.contains(",Cap,,")).collect();
    assert!(!caps.is_empty());
    assert!(caps
        .iter()
        .all(|l| l.splitn(10, ',').nth(9).unwrap().starts_with('\u{200F}')));
}

#[test]
fn bad_values_are_usage_errors_and_only_set_fields_change_the_plan_id() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let plan = |more: &str| {
        std::fs::write(
            p.join("b.json"),
            format!(r#"{{"script": "Hi there.", "tts": "none"{more}}}"#),
        )
        .unwrap();
        run(p, &ff, &["plan", "--brief", "b.json"])
    };
    for (bad, word) in [
        (r#", "caption_outline": 40"#, "caption_outline"),
        (r#", "caption_box": 2"#, "caption_box"),
        (r#", "caption_box": 0.5, "captions": "reveal""#, "reveal"),
        (r#", "caption_size": "300%""#, "200%"),
        (r#", "caption_size": "big""#, "percent"),
    ] {
        let (code, v) = plan(bad);
        assert_eq!(code, Some(2), "{bad}: {v}");
        assert!(v["error"].to_string().contains(word), "{bad}: {v}");
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
    let id = |more: &str| {
        let (code, v) = plan(more);
        assert_eq!(code, Some(0), "{v}");
        v["plan_id"].as_str().unwrap().to_string()
    };
    let none = id("");
    assert_eq!(none, id(r#", "loop": true"#));
    assert_eq!(none, id(r#", "caption_size": "normal""#));
    let boxed = id(r#", "caption_box": 0"#);
    assert_ne!(none, boxed);
    assert_ne!(none, id(r#", "caption_outline": 3"#));
    assert_ne!(none, id(r#", "caption_size": "115%""#));
    // A bare number is the same percent.
    assert_eq!(
        id(r#", "caption_size": 115"#),
        id(r#", "caption_size": "115%""#)
    );
    // A brand file may carry the look; the brief wins.
    std::fs::write(p.join("brand.json"), r#"{"caption_box": 0}"#).unwrap();
    assert_eq!(boxed, id(r#", "brand": "brand.json""#));
    assert_ne!(boxed, id(r#", "brand": "brand.json", "caption_box": 0.5"#));
}

/// `edit --captions` takes the same three fields as flags, through the same function: the same `Cap` style as `make`
/// gives for the same values. The captions come from an SRT file, so no speech row is needed.
#[test]
fn edit_takes_the_caption_style_flags_and_writes_the_style_of_make() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let st = std::process::Command::new(&ff)
        .current_dir(p)
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=1920x1080:r=25:d=3")
        .args(["-f", "lavfi", "-i", "sine=f=300:d=3"])
        .args([
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "in.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::write(
        p.join("c.srt"),
        "1\n00:00:00,200 --> 00:00:02,600\nA clear idea wins\n",
    )
    .unwrap();
    let style = |args: &[&str]| {
        let mut all = vec![
            "edit",
            "in.mp4",
            "--captions-file",
            "c.srt",
            "--out",
            "o.mp4",
        ];
        all.extend(args);
        let (code, v) = run(p, &ff, &all);
        assert_eq!(code, Some(0), "{v}");
        let ass = std::fs::read_to_string(p.join(".scriptvox/edit/in/captions.ass")).unwrap();
        ass.lines()
            .find(|l| l.starts_with("Style: Cap,"))
            .unwrap()
            .to_string()
    };
    // No flag: the style of every earlier version (84 px, a box).
    let old = style(&[]);
    assert!(
        old.starts_with("Style: Cap,Noto Sans,84,") && old.contains(",4,14,0,2,"),
        "{old}"
    );
    // The values of the `make` test above give the same numbers: 96 px, no box, a 5 px outline.
    let new = style(&[
        "--caption-box",
        "0",
        "--caption-outline",
        "5",
        "--caption-size",
        "115%",
    ]);
    assert!(new.starts_with("Style: Cap,Noto Sans,96,"), "{new}");
    assert!(
        new.contains(",&H00000000,&H59000000,") && new.contains(",1,5,0,2,"),
        "{new}"
    );
    // The refusals of the one check, with a fix line that names the flag; and the flags need captions.
    for (args, want) in [
        (
            vec!["--captions", "pop", "--caption-size", "huge"],
            "caption_size is",
        ),
        (
            vec!["--captions", "pop", "--caption-outline", "40"],
            "caption_outline is 0 to 12",
        ),
        (
            vec!["--captions", "reveal", "--caption-box", "0.5"],
            "reveal has no box",
        ),
        (vec!["--from", "0", "--caption-box", "0"], "need --captions"),
    ] {
        let mut all = vec!["edit", "in.mp4", "--out", "x.mp4"];
        all.extend(args);
        let (code, v) = run(p, &ff, &all);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(want),
            "{v}"
        );
        assert!(
            v["error"]["fix"].as_str().unwrap().contains("--caption"),
            "{v}"
        );
    }
}
