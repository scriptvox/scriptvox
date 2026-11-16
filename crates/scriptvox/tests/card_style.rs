//! Brief `card_style` (docs/specs/card-style.md), end to end with no voice and no network: the plan id of an old
//! brief, the refusals, and a `paper` video with a hook, several cards in a scene, marks and an end card, in English
//! and Arabic, through the tool's own gates. The pictures of each card kind are in `cards_golden.rs`.
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

/// A brief on a light page with a dark accent, as the manual video of 14 Nov 2026 had it.
fn brief(more: &str) -> String {
    format!(
        r##"{{"lang": "en", "tts": "none", "aspect": "16:9", "background": "#f6f4ee,#ebe7db", "accent": "#b3421f",
        "scenes": [{{"script": "One plan.", "card": {{"kind": "steps", "items": ["Plan", "Make"]}}}},
        {{"script": "Two parts.", "card": {{"kind": "title", "text": "Two parts"}}}}]{more}}}"##
    )
}

#[test]
fn an_old_brief_keeps_its_plan_id_and_only_paper_changes_it() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let plan = |body: String| {
        std::fs::write(p.join("b.json"), body).unwrap();
        run(p, &ff, &["plan", "--brief", "b.json"])
    };
    let id = |more: &str| {
        let (code, v) = plan(brief(more));
        assert_eq!(code, Some(0), "{v}");
        v["plan_id"].as_str().unwrap().to_string()
    };
    // The id that the build before `card_style` gave this brief (main at a19f295, 14 Nov 2026).
    let old = id("");
    // This value is for Linux: the id holds the pinned FFmpeg and uv of the system, and macOS has no such pins.
    if cfg!(target_os = "linux") {
        assert_eq!(old, "p_8c6b22b9997c1bae");
    }
    assert_eq!(old, id(r#", "card_style": "dark""#));
    assert_ne!(old, id(r#", "card_style": "paper""#));
    // Refusals, each with what to do: a value that is not a style, a page that is not light, an accent that cannot
    // be read on paper.
    for (body, word) in [
        (brief(r#", "card_style": "auto""#), "dark or paper"),
        (
            brief(r#", "card_style": "paper""#).replace("#f6f4ee,#ebe7db", "night"),
            "is not light",
        ),
        (
            brief(r#", "card_style": "paper""#).replace("#b3421f", "#FFD040"),
            "too light on a paper card",
        ),
    ] {
        let (code, v) = plan(body);
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(word),
            "{v}"
        );
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
}

/// The lines of the run's `captions.ass`.
fn ass_of(v: &Value) -> Vec<String> {
    std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap())
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn a_paper_video_has_ink_cards_the_same_captions_and_passes_the_gates() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let en = r##"{"lang": "en", "tts": "none", "aspect": "16:9", "size": 720, "background": "#f6f4ee,#ebe7db",
      "accent": "#b3421f", "caption_box": 0.9, "hook": "Three steps", "outro": {"text": "Made with care", "seconds": 1.5},
      "scenes": [
        {"script": "Revenue grew by forty two percent in one year. Three things made it happen: speed, price and care.",
         "cards": [
           {"kind": "stat", "text": "42%", "label": "revenue growth", "on": "forty", "hold": 2.0},
           {"kind": "list", "items": ["Speed", "Price", "Care"], "on": "three",
            "mark": [{"part": "Price", "on": "price", "style": "highlight"}]}]},
        {"script": "Make a plan first. Then make the video and check the result.",
         "card": {"kind": "steps", "items": ["Plan", "Make", "Check"]}},
        {"script": "The form is one binary and the licence is open.",
         "card": {"kind": "table", "label": "Data", "rows": [["Item", "Data"], ["Form", "One binary"], ["Licence", "Open"]],
                  "mark": [{"part": "Form", "on": "binary"}]}}]STYLE}"##;
    let ar = r##"{"lang": "ar", "tts": "none", "aspect": "16:9", "size": 720, "background": "#f6f4ee,#ebe7db",
      "accent": "#b3421f", "caption_box": 0.9, "hook": "ثلاث خطوات", "outro": {"text": "صنع بعناية", "seconds": 1.5},
      "scenes": [
        {"script": "ارتفع عدد الزوار بنسبة أربعين بالمئة في السنة الأولى وهذا رقم كبير جدا للمدينة.",
         "cards": [
           {"kind": "title", "text": "عدد الزوار"},
           {"kind": "stat", "text": "40%", "label": "نمو عدد الزوار", "at": 3.5}]},
        {"script": "اكتب الملخص أولا ثم راجع الخطة ثم اصنع الفيديو.",
         "card": {"kind": "steps", "items": ["اكتب الملخص", "راجع الخطة", "اصنع الفيديو"]}},
        {"script": "الخطة المجانية فيها مقعد واحد وخطة الفريق فيها عشرة مقاعد.",
         "card": {"kind": "table", "label": "الخطط", "rows": [["الخطة", "المقاعد"], ["مجانية", "١"], ["الفريق", "١٠"]]}}]STYLE}"##;
    // `caption_box` 0.9, as in the manual video: on a light page the default caption box (65 percent black) is grey,
    // and this accent has only 3.0 to 1 on it. The captions are not part of `card_style`.
    for body in [en, ar] {
        let make = |style: &str| {
            std::fs::write(p.join("b.json"), body.replace("STYLE", style)).unwrap();
            let (code, v) = run(p, &ff, &["make", "--brief", "b.json"]);
            assert_eq!(code, Some(0), "{v}");
            v
        };
        let (dark, paper) = (make(""), make(r#", "card_style": "paper""#));
        let (d, l) = (ass_of(&dark), ass_of(&paper));
        // The same lines at the same places: only colours differ. The captions are the same lines.
        assert_eq!(d.len(), l.len());
        let cap = |x: &[String]| -> Vec<String> {
            x.iter().filter(|s| s.contains("Cap,")).cloned().collect()
        };
        assert_eq!(cap(&d), cap(&l));
        // Ink letters and a light box for the cards, the hook and the end card.
        for style in ["Style: Card,", "Style: Hook,"] {
            let s = l.iter().find(|s| s.starts_with(style)).unwrap();
            assert!(s.contains(",&H001A1816,&H001A1816,&H"), "{s}");
            assert!(s.contains("FAFCFD,1,0,0,0,"), "{s}");
        }
        for s in l.iter().filter(|s| s.contains(",Card,,")) {
            assert!(
                !s.contains("&HFFFFFF&") && !s.contains("&H1E1A17&") && !s.contains("&H0000E5FF"),
                "{s}"
            );
        }
        // The tool's own gates on the light page.
        for gate in ["contrast", "text_bounds"] {
            let f = paper["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["gate"] == gate)
                .unwrap();
            assert_eq!(f["status"], "pass", "{f}");
        }
        // The end card: the page is not made dark under it. The dark style makes it dark, as before.
        let corner = |v: &Value| {
            let video = Path::new(v["artifacts"]["video"].as_str().unwrap());
            let at = v["metrics"]["duration_s"].as_f64().unwrap() - 0.3;
            frame(&ff, video, at, (1280, 720))[20 * 1280 + 20]
        };
        assert!(corner(&paper)[1] > 215, "{:?}", corner(&paper));
        assert!(corner(&dark)[1] < 200, "{:?}", corner(&dark));
        // `resume --patch` on the dark run gives the paper cards, and a style that is not one is refused.
        let id = dark["run_id"].as_str().unwrap();
        std::fs::write(p.join("patch.json"), r#"{"card_style": "paper"}"#).unwrap();
        let (code, v) = run(p, &ff, &["resume", id, "--patch", "patch.json", "--wait"]);
        assert_eq!(code, Some(0), "{v}");
        assert_eq!(ass_of(&v), l);
        std::fs::write(p.join("patch.json"), r#"{"card_style": "auto"}"#).unwrap();
        let (code, v) = run(p, &ff, &["resume", id, "--patch", "patch.json", "--wait"]);
        assert_eq!(code, Some(2), "{v}");
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
}
