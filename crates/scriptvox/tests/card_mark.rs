//! A mark on a phrase of a text card (docs/specs/card-mark.md), end to end with no voice: the drawings in
//! `captions.ass`, their times against the card's, and the refusal by `plan`.
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

fn seconds(t: &str) -> f64 {
    t.split(':')
        .fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap())
}

/// Makes the brief; returns the mark drawings (layer 3 drawings of the `Card` style) as (start, end, line), and the
/// run's `cards.json`.
fn marks(p: &Path, ff: &str, body: &str) -> (Vec<(f64, f64, String)>, Value) {
    std::fs::write(p.join("b.json"), body).unwrap();
    let (code, v) = run(p, ff, &["make", "--brief", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    for gate in ["text_bounds", "contrast"] {
        let f = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["gate"] == gate)
            .unwrap();
        assert_eq!(f["status"], "pass", "{f}");
    }
    let file = v["artifacts"]["captions_ass"].as_str().unwrap();
    let ass = std::fs::read_to_string(file).unwrap();
    let cards = std::fs::read_to_string(Path::new(file).with_file_name("cards.json")).unwrap();
    let found = ass
        .lines()
        .filter(|l| l.starts_with("Dialogue: 3,") && l.contains(",Card,") && l.contains("\\p1}"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (seconds(f[1]), seconds(f[2]), l.to_string())
        })
        .collect();
    (found, serde_json::from_str(&cards).unwrap())
}

#[test]
fn marks_are_drawn_on_text_cards_in_english_and_arabic() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (found, cards) = marks(
        p,
        &ff,
        r#"{"tts": "none", "aspect": "16:9", "scenes": [
        {"script": "Revenue grew by forty two percent in the first year alone.",
         "card": {"kind": "stat", "text": "42%", "label": "revenue growth in year one",
          "mark": [{"part": "42%", "on": "forty", "style": "ring"},
                   {"part": "year one", "on": "year", "style": "highlight"}]}},
        {"script": "We want faster delivery, lower prices and happy customers.",
         "card": {"kind": "list", "items": ["Faster delivery", "Lower prices", "Happy customers"],
          "mark": [{"part": "customers", "on": "customers"}]}}]}"#,
    );
    assert_eq!(found.len(), 3, "{found:?}");
    // In the order of the brief: a ring (a clear fill with a border), a band, a line.
    assert!(
        found[0].2.contains("\\bord") && found[0].2.contains(" b "),
        "{}",
        found[0].2
    );
    assert!(found[1].2.contains("\\1a&HA8&"), "{}", found[1].2);
    assert!(found[2].2.contains("\\1a&H00&"), "{}", found[2].2);
    // Each mark lives inside its card's time, and the frame for `snapshot --cards` comes after it has grown.
    for (i, scene) in [0usize, 0, 1].into_iter().enumerate() {
        let c = &cards[scene];
        let (start, end) = (c["start"].as_f64().unwrap(), c["end"].as_f64().unwrap());
        let (a, b, _) = &found[i];
        assert!(
            *a >= start + 0.29 && *b <= end + 0.02 && b - a >= 0.5,
            "{found:?} {c}"
        );
        assert!(c["landed"].as_f64().unwrap() >= a + 0.25, "{found:?} {c}");
    }
    // Arabic: the line is drawn from its right corner to the left.
    let (found, _) = marks(
        p,
        &ff,
        r#"{"tts": "none", "lang": "ar", "aspect": "16:9", "scenes": [
        {"script": "ارتفع عدد الزوار بنسبة أربعين بالمئة في السنة الأولى.",
         "card": {"kind": "stat", "text": "40%", "label": "نمو عدد الزوار",
          "mark": [{"part": "عدد الزوار", "on": "السنة"}]}}]}"#,
    );
    assert_eq!(found.len(), 1);
    assert!(found[0].2.contains("}m 0 0 l -"), "{}", found[0].2);
}

#[test]
fn plan_refuses_a_part_the_card_does_not_have_with_the_scene_number() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let plan = |mark: &str| {
        std::fs::write(
            p.join("b.json"),
            format!(
                r#"{{"tts": "none", "scenes": [{{"script": "First we say hello."}},
                {{"script": "Revenue grew by forty percent.",
                  "card": {{"kind": "title", "text": "Revenue grew", "mark": [{mark}]}}}}]}}"#
            ),
        )
        .unwrap();
        run(p, &ff, &["plan", "--brief", "b.json"])
    };
    for (mark, word) in [
        (r#"{"part": "profit", "on": "forty"}"#, "profit"),
        (r#"{"part": "grew", "on": "never"}"#, "never"),
        (
            r#"{"part": "grew", "on": "forty", "style": "circle"}"#,
            "circle",
        ),
    ] {
        let (code, v) = plan(mark);
        assert_eq!(code, Some(2), "{v}");
        let said = v["error"]["message"].as_str().unwrap();
        assert!(said.contains(word), "{said}");
        assert!(
            said.contains("scene 2") || said.contains("scenes[1]"),
            "{said}"
        );
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
    let (code, v) = plan(r#"{"part": "grew", "on": "forty", "style": "ring"}"#);
    assert_eq!(code, Some(0), "{v}");
}
