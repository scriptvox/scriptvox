//! Delivery cues, IPA in `say` and SSML in a script (docs/specs/delivery-cues.md) with the stub worker: no network and
//! no key. The stub voice (`edge`) takes no cue and no IPA, so these tests prove the drop, the respelled form, the
//! break tag as a pause and the one warning for each. The row that takes IPA runs a model: the spec has its real run.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};

const PLAIN: &str = "Come closer and listen. Today Nguyen works at the lab with the team.";
const CUED: &str =
    "[whisper] Come closer and listen. [excited] Today Nguyen works at the lab with the team.";

fn go(e: &Env, brief: Value, args: &[&str]) -> (i32, Value) {
    std::fs::write(e.dir.path().join("b.json"), brief.to_string()).unwrap();
    let o = e.cmd("ok").args(args).arg("--json").output().unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

fn warns<'a>(v: &'a Value, gate: &str) -> Vec<&'a Value> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["gate"] == gate)
        .collect()
}

/// Every text.txt under the run folder: the text the stub voice got in each call.
fn engine_text(dir: &std::path::Path) -> String {
    let mut out = String::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.push_str(&engine_text(&p));
        } else if p.file_name().is_some_and(|n| n == "text.txt") {
            out.push_str(&std::fs::read_to_string(&p).unwrap());
        }
    }
    out
}

#[test]
fn a_voice_that_takes_no_cue_drops_it_with_one_warning_and_captions_never_show_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let plan = |script: &str| {
        let (code, v) = go(
            &e,
            json!({"script": script, "aspect": "16:9"}),
            &["plan", "--brief", "b.json"],
        );
        assert_eq!(code, 0, "{v}");
        v
    };
    let (plain, cued) = (plan(PLAIN), plan(CUED));
    // The same words reach the voice, so it is the same plan; a script with no cue has no warning.
    assert_eq!(plain["plan_id"], cued["plan_id"]);
    assert!(warns(&plain, "cues").is_empty(), "{plain}");
    let w = warns(&cued, "cues");
    assert_eq!(w.len(), 1, "{cued}");
    let detail = w[0]["detail"].as_str().unwrap();
    assert!(detail.contains("[whisper], [excited]"), "{detail}");
    assert!(w[0]["fix"].as_str().unwrap().contains("takes_tags"));

    let (code, v) = go(
        &e,
        json!({"script": CUED, "aspect": "16:9"}),
        &["make", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(warns(&v, "cues").len(), 1, "{v}");
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(
        srt.contains("Come closer") && srt.contains("Today Nguyen"),
        "{srt}"
    );
    assert!(
        !srt.contains('[') && !srt.contains("whisper") && !srt.contains("excited"),
        "{srt}"
    );
    let run = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    let got = engine_text(&run);
    assert!(got.contains("closer") && !got.contains("whisper"), "{got}");
}

#[test]
fn an_ipa_form_is_read_as_its_respelled_form_with_one_warning() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |say: Value| json!({"script": PLAIN, "aspect": "16:9", "say": say});
    let (code, v) = go(
        &e,
        brief(json!({"Nguyen": "/ŋwiən/ win", "lab": "/læb/"})),
        &["make", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{v}");
    let w = warns(&v, "say");
    assert_eq!(w.len(), 1, "{v}");
    let detail = w[0]["detail"].as_str().unwrap();
    assert!(
        detail.contains("Nguyen is read as \"win\"") && detail.contains("lab is read as written"),
        "{detail}"
    );
    // The voice got the respelled form and no IPA; the captions keep the written word.
    let run = e
        .dir
        .path()
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    let got = engine_text(&run);
    assert!(
        got.contains("Today win works at the lab") && !got.contains('ŋ') && !got.contains('/'),
        "{got}"
    );
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(srt.contains("Nguyen") && !srt.contains("win "), "{srt}");
    // A respelled form with no IPA is the old `say`: no warning.
    let (code, v) = go(
        &e,
        brief(json!({"Nguyen": "win"})),
        &["plan", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{v}");
    assert!(warns(&v, "say").is_empty(), "{v}");
}

#[test]
fn an_ssml_break_becomes_a_pause_with_a_warning_and_other_ssml_stops_with_a_fix() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |script: &str| json!({"script": script, "aspect": "16:9"});
    let (code, tag) = go(
        &e,
        brief("One two three. <break time=\"4s\"/> Four five six."),
        &["plan", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{tag}");
    let w = warns(&tag, "ssml");
    assert_eq!(w.len(), 1, "{tag}");
    assert!(w[0]["fix"].as_str().unwrap().contains("[pause 2]"));
    // The same plan as the pause tag written by hand.
    let (_, pause) = go(
        &e,
        brief("One two three. [pause 4] Four five six."),
        &["plan", "--brief", "b.json"],
    );
    assert!(warns(&pause, "ssml").is_empty());
    assert_eq!(tag["plan_id"], pause["plan_id"]);
    let (code, v) = go(
        &e,
        brief("One two three. <break time=\"4s\"/> Four five six."),
        &["make", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(warns(&v, "ssml").len(), 1, "{v}");
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(
        !srt.contains('<') && !srt.contains("break") && !srt.contains("pause"),
        "{srt}"
    );
    // The stub voice is 3 s a part: two parts and a 4 s pause.
    assert!(v["metrics"]["duration_s"].as_f64().unwrap() > 9.0, "{v}");

    let (code, v) = go(
        &e,
        brief("This is <emphasis level=\"strong\">big</emphasis> news."),
        &["plan", "--brief", "b.json"],
    );
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("<emphasis>"),
        "{v}"
    );
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("[pause 2]"),
        "{v}"
    );
    // A "<" that is not a tag is text.
    let (code, v) = go(
        &e,
        brief("Speed < 40 and height > 2 is the rule here."),
        &["plan", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{v}");
    assert!(warns(&v, "ssml").is_empty(), "{v}");
}

/// A brief with scenes: the cue and the break tag are in a scene. One warning each, and clean captions.
#[test]
fn scenes_get_one_warning_each_and_clean_captions() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let brief = |one: &str, two: &str| json!({"aspect": "16:9", "scenes": [{"script": one}, {"script": two}]});
    let (code, clean) = go(
        &e,
        brief(
            "Come closer and listen now.",
            "Today the team works. [pause 2] Then it rests.",
        ),
        &["plan", "--brief", "b.json"],
    );
    assert_eq!(code, 0, "{clean}");
    assert!(
        warns(&clean, "cues").is_empty() && warns(&clean, "ssml").is_empty(),
        "{clean}"
    );
    let cued = brief(
        "[whisper] Come closer and listen now.",
        "Today the team works. <break time=\"2s\"/> Then it rests.",
    );
    let (code, v) = go(&e, cued.clone(), &["plan", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    // The plan id holds the scenes as the brief wrote them, so it is not the id of the clean brief.
    assert_eq!(warns(&v, "cues").len(), 1, "{v}");
    let ssml = warns(&v, "ssml");
    assert_eq!(ssml.len(), 1, "{v}");
    assert!(
        ssml[0]["detail"].as_str().unwrap().starts_with("1 SSML"),
        "{v}"
    );
    let (code, v) = go(&e, cued, &["make", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        (warns(&v, "cues").len(), warns(&v, "ssml").len()),
        (1, 1),
        "{v}"
    );
    let srt = std::fs::read_to_string(v["artifacts"]["captions_srt"].as_str().unwrap()).unwrap();
    assert!(
        srt.contains("Come closer") && srt.contains("Then it rests"),
        "{srt}"
    );
    assert!(
        !srt.contains('[') && !srt.contains('<') && !srt.contains("whisper"),
        "{srt}"
    );
}
