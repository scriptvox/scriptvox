//! `scriptvox recipes` (docs/specs/recipes.md): every recipe is a valid brief, plans with no warning, and is named
//! in the skill `scriptvox-genres`. No render here: the real runs are in the spec's daily note.
mod common;
use scriptvox::brief::Brief;
use scriptvox::recipes::ALL;
use serde_json::Value;

fn run(dir: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let o = common::bin()
        .current_dir(dir)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

/// The brief with no made voice: what a machine without the local voice can still plan.
fn without_voice(mut b: Value) -> Value {
    b["tts"] = "none".into();
    b.as_object_mut().unwrap().remove("voice");
    // The words are sized for the voice, not for the reading pace of text.
    b.as_object_mut().unwrap().remove("duration");
    for s in b["scenes"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("voice");
    }
    b
}

#[test]
fn every_recipe_loads_and_plans_local_only_with_no_warning() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(ALL.len(), 12);
    for r in &ALL {
        let p = d.path().join(format!("{}.json", r.name));
        std::fs::write(&p, r.brief).unwrap();
        let brief = Brief::load(&p).unwrap_or_else(|e| panic!("{}: {e}", r.name));
        assert!(brief.scenes.is_some(), "{}: a recipe has scenes", r.name);
        let file = p.to_str().unwrap();
        let (mut code, mut v) = run(d.path(), &["plan", "--brief", file, "--local-only"]);
        // No local voice on this machine (the test's data folder is empty): plan the same scenes and cards with no voice.
        if v["needs_input"][0]["field"] == "tts.engine" {
            let action = v["needs_input"][0]["human_action"].as_str().unwrap();
            assert!(action.contains("setup voice-rust"), "{}: {action}", r.name);
            let b = without_voice(serde_json::from_str(r.brief).unwrap());
            std::fs::write(&p, b.to_string()).unwrap();
            (code, v) = run(d.path(), &["plan", "--brief", file, "--local-only"]);
        }
        assert_eq!(code, 0, "{}: {v}", r.name);
        assert!(v["plan_id"].is_string(), "{}: {v}", r.name);
        let bad: Vec<&Value> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["status"] != "pass")
            .collect();
        assert!(bad.is_empty(), "{}: {bad:?}", r.name);
        let stages = v["stages"].to_string();
        assert!(!stages.contains("\"network\":true"), "{}: {stages}", r.name);
    }
}

#[test]
fn the_list_and_show_give_the_brief_and_the_notes() {
    let d = tempfile::tempdir().unwrap();
    let (code, v) = run(d.path(), &["recipes"]);
    assert_eq!(code, 0);
    let rows = v["recipes"].as_array().unwrap();
    assert_eq!(rows.len(), 12);
    for row in rows {
        for k in [
            "name", "kind", "seconds", "aspect", "lang", "voice", "scenes", "use_for", "command",
        ] {
            assert!(!row[k].is_null(), "{row}: {k}");
        }
    }
    let (code, v) = run(d.path(), &["recipes", "show", "news-brief"]);
    assert_eq!(code, 0);
    let r = &v["recipe"];
    assert_eq!(r["brief"]["sources_card"], true);
    assert_eq!(r["aspect"], "9:16");
    assert!(r["shape"]["script"].as_str().unwrap().contains("Ending"));
    assert!(r["adapt"][0].as_str().unwrap().contains("invented"));
    assert!(v["next"][0].as_str().unwrap().contains("scriptvox plan"));
    let (code, v) = run(d.path(), &["recipes", "show", "vlog"]);
    assert_eq!(code, 2);
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("explainer"));
    assert!(v["error"]["fix"]
        .as_str()
        .unwrap()
        .contains("scriptvox recipes"));
}

#[test]
fn the_kinds_the_item_names_are_there_and_free() {
    let has = |n: &str| {
        ALL.iter()
            .find(|r| r.name == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    for (name, secs, aspect) in [
        ("explainer", 60, "16:9"),
        ("explainer-short", 30, "9:16"),
        ("ad", 15, "9:16"),
    ] {
        let b: Value = serde_json::from_str(has(name).brief).unwrap();
        assert_eq!(
            (has(name).seconds, b["aspect"].as_str().unwrap()),
            (secs, aspect)
        );
    }
    for r in &ALL {
        let b: Value = serde_json::from_str(r.brief).unwrap();
        // Free and local: the local voice or none, and nothing that needs a file, a key or a network.
        assert!(
            matches!(b["tts"].as_str(), Some("kokoro-rust" | "none")),
            "{}",
            r.name
        );
        for s in b["scenes"].as_array().unwrap() {
            for k in [
                "media",
                "stock",
                "open_image",
                "generate",
                "overlay",
                "inset",
            ] {
                assert!(s[k].is_null(), "{}: {k}", r.name);
            }
        }
        for k in [
            "logo",
            "music_prompt",
            "music_search",
            "brand",
            "font",
            "base_video",
        ] {
            assert!(b[k].is_null(), "{}: {k}", r.name);
        }
        assert!(
            matches!(b["music"].as_str(), Some("calm" | "pulse" | "none")),
            "{}",
            r.name
        );
        if r.name.ends_with("-ar") {
            assert_eq!(b["lang"], "ar");
        }
    }
    // News and data: facts with sources, shown on the Sources card.
    for name in ["news-brief", "data-story"] {
        let b: Value = serde_json::from_str(has(name).brief).unwrap();
        assert_eq!(b["sources_card"], true, "{name}");
        for s in b["sources"].as_array().unwrap() {
            assert!(
                s["title"].as_str().unwrap().starts_with("PLACEHOLDER"),
                "{name}"
            );
        }
    }
    // The dialogue has two voices.
    let b: Value = serde_json::from_str(has("micro-drama").brief).unwrap();
    assert!(b["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["voice"].is_string()));
}

#[test]
fn the_genres_skill_names_every_recipe_and_is_installed() {
    let (_, text) = scriptvox::skill::all()
        .into_iter()
        .find(|(n, _)| n == "scriptvox-genres")
        .expect("the skill is one of the installed ones");
    for r in &ALL {
        assert!(text.contains(&format!("`{}`", r.name)), "{}", r.name);
    }
    let home = tempfile::tempdir().unwrap();
    let paths = scriptvox::skill::install_at(home.path().join(".claude/skills/scriptvox")).unwrap();
    assert!(home
        .path()
        .join(".claude/skills/scriptvox-genres/SKILL.md")
        .is_file());
    assert_eq!(paths.len(), scriptvox::skill::all().len());
}
