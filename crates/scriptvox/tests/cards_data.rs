//! The data cards (docs/specs/cards-data.md) end to end with no voice: one `make` per language with all six kinds,
//! the gates `text_bounds` and `contrast` on the burned video, the lit band of a table and the lit step from the
//! words, and the refusals by `plan` with the scene number and a fix.
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

/// Makes the brief and returns the card events of `captions.ass` and the run's `cards.json`.
fn made(p: &Path, ff: &str, body: &str) -> (Vec<String>, Value) {
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
    (
        ass.lines()
            .filter(|l| l.contains(",Card,"))
            .map(String::from)
            .collect(),
        serde_json::from_str(&cards).unwrap(),
    )
}

#[test]
fn the_six_data_cards_are_made_and_pass_the_text_gates_in_english_and_arabic() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let (ev, cards) = made(
        p,
        &ff,
        r#"{"tts": "none", "aspect": "9:16", "scenes": [
        {"script": "The team plan costs twelve dollars.",
         "card": {"kind": "table", "label": "Plans", "rows": [["Plan", "Price"], ["Free", "$0"], ["Team", "$12"]],
          "mark": [{"part": "Team", "on": "team"}]}},
        {"script": "The score moved from twenty to eighty.",
         "card": {"kind": "number_line", "values": [0, 100], "points": [80], "from": 20}},
        {"script": "A brief becomes a plan and then a video.",
         "card": {"kind": "diagram", "nodes": ["Brief", "Plan", "Video"], "edges": [["Brief", "Plan"], ["Plan", "Video"]]}},
        {"script": "First write it. Then check the plan. Then render the video.",
         "card": {"kind": "steps", "items": ["Write the brief", "Check the plan", "Render the video"]}},
        {"script": "The battery is at seventy two.", "card": {"kind": "gauge", "values": [72], "label": "Battery"}},
        {"script": "Three of five are done.", "card": {"kind": "progress", "values": [3, 5], "label": "Steps done"}}]}"#,
    );
    let kinds: Vec<&str> = cards
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "table",
            "number_line",
            "diagram",
            "steps",
            "gauge",
            "progress"
        ]
    );
    let window = |i: usize| {
        (
            cards[i]["start"].as_f64().unwrap(),
            cards[i]["end"].as_f64().unwrap(),
        )
    };
    let times = |l: &str| {
        let f: Vec<&str> = l.split(',').collect();
        (seconds(f[1]), seconds(f[2]))
    };
    // The table's lit band: one, inside the card's time, after the card's start (the word "team" is the second).
    let bands: Vec<&String> = ev.iter().filter(|l| l.contains("\\1a&HA8&")).collect();
    assert_eq!(bands.len(), 1, "{bands:?}");
    let (a, b) = times(bands[0]);
    assert!(
        a >= window(0).0 + 0.29 && b <= window(0).1 + 0.02,
        "{a} {b} {:?}",
        window(0)
    );
    // The traveller: 12 moving value texts inside the number line's time.
    let moving: Vec<&String> = ev
        .iter()
        .filter(|l| l.contains("\\move(") && !l.contains("\\p1"))
        .collect();
    assert_eq!(moving.len(), 12);
    assert!(moving
        .iter()
        .all(|l| times(l).0 >= window(1).0 && times(l).1 <= window(1).1));
    // The diagram: two arrows and three boxes.
    assert_eq!(ev.iter().filter(|l| l.contains("\\3a&H40&")).count(), 2);
    assert_eq!(ev.iter().filter(|l| l.contains("\\3a&H70&")).count(), 3);
    // The steps: "Check the plan" is lit (white, not grey) from the word "check", after the card's start and
    // before the third step's word.
    let lit: Vec<&String> = ev
        .iter()
        .filter(|l| l.ends_with("Check the plan") && !l.contains("&HB4B4B4&"))
        .collect();
    assert_eq!(lit.len(), 1, "{lit:?}");
    let (a, b) = times(lit[0]);
    assert!(
        a > window(3).0 + 0.3 && b < window(3).1 - 0.3,
        "{a} {b} {:?}",
        window(3)
    );
    // The gauge and the bar end on their numbers.
    assert!(ev.iter().any(|l| l.ends_with("72%")) && ev.iter().any(|l| l.ends_with("60%")));
    // Arabic: the same kinds; the table's first column and the steps stand at the right.
    let (ev, _) = made(
        p,
        &ff,
        r#"{"tts": "none", "lang": "ar", "aspect": "16:9", "scenes": [
        {"script": "خطة الفريق سعرها اثنا عشر.",
         "card": {"kind": "table", "rows": [["الخطة", "السعر"], ["مجانية", "٠"], ["الفريق", "١٢"]],
          "mark": [{"part": "الفريق", "on": "الفريق"}]}},
        {"script": "ارتفعت النتيجة من عشرين إلى ثمانين.",
         "card": {"kind": "number_line", "values": [0, 100], "points": [80], "items": ["الآن"], "from": 20}},
        {"script": "الملخص يصبح خطة ثم فيديو.",
         "card": {"kind": "diagram", "nodes": ["الملخص", "الخطة", "الفيديو"], "edges": [["الملخص", "الخطة"], ["الخطة", "الفيديو"]]}},
        {"script": "أولا اكتب الملخص. ثم راجع الخطة.",
         "card": {"kind": "steps", "items": ["اكتب الملخص", "راجع الخطة"]}},
        {"script": "البطارية عند اثنين وسبعين.", "card": {"kind": "gauge", "values": [72], "label": "البطارية"}},
        {"script": "ثلاث من خمس انتهت.", "card": {"kind": "progress", "values": [3, 5], "label": "الخطوات المنجزة"}}]}"#,
    );
    assert!(ev
        .iter()
        .any(|l| l.contains("\\an6") && l.ends_with("الخطة")));
    assert!(ev
        .iter()
        .any(|l| l.contains("\\an6") && l.ends_with("اكتب الملخص")));
    // The bar's fill is drawn to the left of its origin.
    assert!(
        ev.iter()
            .any(|l| l.contains("\\fscx0") && l.contains("}m -")),
        "{ev:#?}"
    );
}

#[test]
fn plan_refuses_a_bad_data_card_with_the_scene_number_and_a_fix() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let plan = |card: &str| {
        std::fs::write(
            p.join("b.json"),
            format!(
                r#"{{"tts": "none", "scenes": [{{"script": "First we say hello."}},
                {{"script": "Then the team plan.", "card": {card}}}]}}"#
            ),
        )
        .unwrap();
        let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
        assert_eq!(code, Some(2), "{v}");
        v["error"]["message"].as_str().unwrap().to_string()
    };
    for (card, want) in [
        (
            r#"{"kind": "table", "text": "Plans", "rows": [["A", "B"], ["1", "2"]]}"#,
            "a table card does not draw `text`: remove it",
        ),
        (
            r#"{"kind": "table", "rows": [["A", "B"], ["1", "2"]], "mark": [{"part": "1", "on": "missing"}]}"#,
            "is not in this scene's script",
        ),
        (
            r#"{"kind": "diagram", "nodes": ["A", "B", "C"], "edges": [["A", "B"], ["B", "C"], ["C", "A"]]}"#,
            "remove one arrow of the circle",
        ),
        (
            r#"{"kind": "number_line", "values": [0, 10], "points": [12]}"#,
            "widen `values` or move the point",
        ),
        (
            r#"{"kind": "steps", "items": ["Only one"]}"#,
            "2 to 5 steps",
        ),
        (
            r#"{"kind": "gauge", "values": [120]}"#,
            "a gauge card needs `values`",
        ),
        (
            r#"{"kind": "stat", "text": "4", "nodes": ["A", "B"]}"#,
            "only a diagram card takes `nodes`",
        ),
    ] {
        let m = plan(card);
        assert!(m.contains("scene 2") && m.contains(want), "{m}");
    }
}
