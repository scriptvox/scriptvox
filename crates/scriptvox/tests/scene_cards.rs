//! Several cards in one scene (docs/specs/scene-cards.md), end to end with no voice: the cards take turns in
//! `captions.ass` and `cards.json`, `snapshot --cards` gives one frame per card, and `plan` and `make` refuse.
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

/// Makes the brief with all gates passing; returns the run id, `captions.ass` and the rows of `cards.json`.
fn made(p: &Path, ff: &str, body: &str) -> (String, String, Vec<Value>) {
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
    let cards = std::fs::read_to_string(Path::new(file).with_file_name("cards.json")).unwrap();
    (
        v["run_id"].as_str().unwrap().to_string(),
        std::fs::read_to_string(file).unwrap(),
        serde_json::from_str(&cards).unwrap(),
    )
}

/// The cards of one scene take turns: the rows are in order with a 0.2 s gap, each row has card events inside it,
/// and no card event is on screen in the middle of a gap.
fn take_turns(ass: &str, rows: &[Value], scene: u64, n: usize) {
    let rows: Vec<&Value> = rows.iter().filter(|r| r["scene"] == scene).collect();
    assert_eq!(rows.len(), n, "{rows:?}");
    let events: Vec<(f64, f64)> = ass
        .lines()
        .filter(|l| l.starts_with("Dialogue:") && l.contains(",Card,"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (seconds(f[1]), seconds(f[2]))
        })
        .collect();
    let t = |r: &Value, k: &str| r[k].as_f64().unwrap();
    for (j, r) in rows.iter().enumerate() {
        assert_eq!(r["card"], j as u64 + 1, "{r}");
        let (start, end) = (t(r, "start"), t(r, "end"));
        assert!(end - start >= 0.9, "{r}");
        assert!(t(r, "landed") > start && t(r, "landed") < end, "{r}");
        assert!(
            events
                .iter()
                .any(|e| e.0 >= start - 0.01 && e.1 <= end + 0.02),
            "no event inside {r}"
        );
        if let Some(next) = rows.get(j + 1) {
            let gap = t(next, "start") - end;
            assert!(gap >= 0.19, "{r} {next}");
            let mid = end + gap / 2.0;
            assert!(
                !events.iter().any(|e| e.0 < mid && e.1 > mid),
                "a card event is on screen at {mid}, between two cards"
            );
        }
    }
}

#[test]
fn cards_of_one_scene_take_turns_in_english_and_arabic() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (id, ass, rows) = made(
        p,
        &ff,
        r#"{"tts": "none", "aspect": "16:9", "scenes": [
        {"script": "This year was a good year for the whole company."},
        {"script": "Revenue grew by forty two percent in one year. Three things made it happen: faster delivery, lower prices and happy customers.",
         "cards": [
          {"kind": "title", "text": "Revenue", "hold": 1.2},
          {"kind": "stat", "text": "42%", "label": "revenue growth", "on": "forty", "hold": 2.0},
          {"kind": "list", "items": ["Faster delivery", "Lower prices", "Happy customers"], "on": "three"}]}]}"#,
    );
    take_turns(&ass, &rows, 2, 3);
    assert_eq!(rows[1]["kind"], "stat");
    // One frame per card, each after its card has landed.
    let (code, v) = run(p, &ff, &["snapshot", &id, "--cards"]);
    assert_eq!(code, Some(0), "{v}");
    let frames = v["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3, "{v}");
    let kinds: Vec<&str> = frames.iter().map(|f| f["card"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["title", "stat", "list"]);
    // Arabic: `at` from the scene's start for the second card; the first has no times.
    let (_, ass, rows) = made(
        p,
        &ff,
        r#"{"tts": "none", "lang": "ar", "scenes": [
        {"script": "كانت هذه السنة سنة جيدة للشركة كلها."},
        {"script": "ارتفع عدد الزوار بنسبة أربعين بالمئة في السنة الأولى وهذا رقم كبير جدا للمدينة.",
         "cards": [
          {"kind": "title", "text": "عدد الزوار"},
          {"kind": "stat", "text": "40%", "label": "نمو عدد الزوار", "at": 2.5}]}]}"#,
    );
    take_turns(&ass, &rows, 2, 2);
    let (first, second) = (&rows[0], &rows[1]);
    let scene_start = first["start"].as_f64().unwrap();
    assert!(
        (second["start"].as_f64().unwrap() - scene_start - 2.5).abs() < 0.01,
        "{rows:?}"
    );
}

#[test]
fn one_entry_in_cards_draws_what_card_draws() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let brief = |field: &str, open: &str, close: &str| {
        format!(
            r#"{{"tts": "none", "scenes": [{{"script": "Revenue grew by forty two percent in the first year alone.",
            "{field}": {open}{{"kind": "stat", "text": "42%", "label": "revenue growth", "on": "forty", "hold": 2.0}}{close}}}]}}"#
        )
    };
    let (_, one, rows_one) = made(p, &ff, &brief("card", "", ""));
    let (_, list, rows_list) = made(p, &ff, &brief("cards", "[", "]"));
    assert_eq!(one, list);
    assert!(
        rows_one[0].get("card").is_none(),
        "a single card's row is as before"
    );
    assert_eq!(rows_list[0]["card"], 1);
    assert_eq!(rows_one[0]["start"], rows_list[0]["start"]);
}

#[test]
fn plan_and_make_refuse_with_the_scene_and_the_card() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let go = |cmd: &str, scene: &str| {
        std::fs::write(
            p.join("b.json"),
            format!(
                r#"{{"tts": "none", "scenes": [{{"script": "First we say hello to all."}},
                {{"script": "Revenue grew by forty two percent in the first year alone.", {scene}}}]}}"#
            ),
        )
        .unwrap();
        run(p, &ff, &[cmd, "--brief", "b.json"])
    };
    let title = r#"{"kind": "title", "text": "T"}"#;
    for (scene, word) in [
        (format!(r#""card": {title}, "cards": [{title}]"#), "`card` and `cards`"),
        (r#""cards": []"#.to_string(), "1 to 4"),
        (
            r#""cards": [{"kind": "title", "text": "A", "on": "year"}, {"kind": "title", "text": "B", "on": "forty"}]"#.to_string(),
            "card 2 of `cards`",
        ),
        (
            r#""cards": [{"kind": "title", "text": "A", "on": "forty", "at": 1}]"#.to_string(),
            "not both",
        ),
        (r#""cards": [{"kind": "titel", "text": "A"}]"#.to_string(), "scenes[1].cards[0]"),
    ] {
        let (code, v) = go("plan", &scene);
        assert_eq!(code, Some(2), "{v}");
        let said = v["error"]["message"].as_str().unwrap();
        assert!(said.contains(word), "{said}");
        assert!(said.contains("scene 2") || said.contains("scenes[1]"), "{said}");
        assert!(v["error"]["fix"].is_string(), "{v}");
    }
    // The captions at the top and a card use the same space, with `cards` as with `card`.
    std::fs::write(
        p.join("b.json"),
        r#"{"tts": "none", "caption_at": "top", "scenes": [{"script": "One two three.", "cards": [{"kind": "title", "text": "A"}]}]}"#,
    )
    .unwrap();
    let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("caption_at"),
        "{v}"
    );
    // Known only after the voice: two words 0.4 s apart. `plan` passes, `make` stops with exit 2 and a fix.
    let close = r#""cards": [{"kind": "title", "text": "A", "on": "forty"}, {"kind": "title", "text": "B", "on": "two"}]"#;
    assert_eq!(go("plan", close).0, Some(0));
    let (code, v) = go("make", close);
    assert_eq!(code, Some(2), "{v}");
    let said = v["error"]["message"].as_str().unwrap();
    assert!(
        said.contains("scene 2: card 1 starts at") && said.contains("less than 0.9 s"),
        "{said}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("earlier `on` word"),
        "{v}"
    );
}
