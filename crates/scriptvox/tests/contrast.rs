//! Gates `contrast` and `loop_seam` (docs/specs/gate-contrast-loop.md) on real renders with no voice and no network:
//! the letters of the burned captions against the picture around them, and the last frame against the first.
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

/// Makes the brief and returns the finding of `gate`: its status and its detail. A warning is exit 0.
fn finding(p: &Path, ff: &str, body: &str, gate: &str) -> Option<(String, String)> {
    std::fs::write(p.join("b.json"), body).unwrap();
    let (code, v) = run(p, ff, &["make", "--brief", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == gate)
        .map(|f| {
            (
                f["status"].as_str().unwrap().to_string(),
                f["detail"].as_str().unwrap().to_string(),
            )
        })
}

/// The ratio in a pass detail ("lowest contrast 9.5 to 1") or a warning ("a contrast of 1.0 to 1").
fn ratio(detail: &str) -> f64 {
    let at = detail.find("contrast").unwrap();
    detail[at..]
        .split_whitespace()
        .find_map(|w| w.parse::<f64>().ok())
        .unwrap()
}

#[test]
fn white_captions_on_a_light_picture_warn_and_a_box_or_an_outline_makes_them_pass() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (light, dark) = ("#f4f4f4,#d8e8ff", "#101020,#202840");
    let brief = |script: &str, lang: &str, back: &str, more: &str| {
        format!(
            r#"{{"script": "{script}", "lang": "{lang}", "tts": "none", "aspect": "16:9", "background": "{back}"{more}}}"#
        )
    };
    let en = "You need a clear idea and captions.";
    let get = |body: String| finding(p, &ff, &body, "contrast").unwrap();
    // The default box: the letters stand on 65 percent black.
    let (status, detail) = get(brief(en, "en", light, ""));
    assert_eq!(status, "pass", "{detail}");
    assert!(ratio(&detail) > 7.0, "{detail}");
    // No box and no outline, white on light: it cannot be read, and the finding says where and how to fix it.
    let (status, detail) = get(brief(
        en,
        "en",
        light,
        r#", "caption_box": 0, "caption_outline": 0"#,
    ));
    assert_eq!(status, "warn", "{detail}");
    assert!(ratio(&detail) < 1.5, "{detail}");
    assert!(
        detail.contains(" s has a contrast of ") && detail.contains("under 3 to 1"),
        "{detail}"
    );
    // The same letters on a dark picture read.
    let (status, detail) = get(brief(
        en,
        "en",
        dark,
        r#", "caption_box": 0, "caption_outline": 0"#,
    ));
    assert_eq!(status, "pass", "{detail}");
    // The outline case: the default outline of a caption with no box (a tenth of the size) is wider than the ring
    // the gate reads, so it passes on the light picture. A 1 px outline is not enough.
    let (status, detail) = get(brief(en, "en", light, r#", "caption_box": 0"#));
    assert_eq!(status, "pass", "{detail}");
    let (status, detail) = get(brief(
        en,
        "en",
        light,
        r#", "caption_box": 0, "caption_outline": 1"#,
    ));
    assert_eq!(status, "warn", "{detail}");
    // Arabic: the same measure on joined letters.
    let ar = "تحتاج إلى فكرة واضحة ونص قصير.";
    let (status, detail) = get(brief(
        ar,
        "ar",
        light,
        r#", "caption_box": 0, "caption_outline": 0"#,
    ));
    assert_eq!(status, "warn", "{detail}");
    let (status, detail) = get(brief(ar, "ar", light, ""));
    assert_eq!(status, "pass", "{detail}");
}

#[test]
fn the_loop_seam_is_checked_only_when_the_brief_asks_for_a_loop() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    // Two pictures that differ a lot: colour bars, and the same with every colour turned over.
    for (name, source) in [
        ("a.png", "testsrc2=s=640x360:d=1"),
        ("b.png", "testsrc2=s=640x360:d=1,negate"),
    ] {
        let ok = std::process::Command::new(&ff)
            .current_dir(p)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", source])
            .args(["-frames:v", "1", name])
            .status()
            .unwrap();
        assert!(ok.success());
    }
    let brief = |last: &str, more: &str| {
        format!(
            r#"{{"tts": "none", "aspect": "16:9"{more}, "scenes": [
            {{"script": "This is the first picture.", "media": "a.png"}},
            {{"script": "This is the last picture.", "media": "{last}"}}]}}"#
        )
    };
    // No `loop`: no finding at all.
    assert!(finding(p, &ff, &brief("b.png", ""), "loop_seam").is_none());
    // It ends on another picture: the loop jumps.
    let (status, detail) =
        finding(p, &ff, &brief("b.png", r#", "loop": true"#), "loop_seam").unwrap();
    assert_eq!(status, "warn", "{detail}");
    assert!(detail.contains("percent"), "{detail}");
    // It ends on the picture it starts on: only the caption and the slow zoom differ.
    let (status, detail) =
        finding(p, &ff, &brief("a.png", r#", "loop": true"#), "loop_seam").unwrap();
    assert_eq!(status, "pass", "{detail}");
}
