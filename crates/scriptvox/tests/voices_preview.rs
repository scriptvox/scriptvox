//! `voices --preview`: one short audio file of a voice, through the stub worker (docs/specs/voices-preview.md).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};

const VOICE: &str = "en-US-AndrewNeural";

fn go(e: &Env, args: &[&str]) -> (i32, Value) {
    let o = e
        .cmd("ok")
        .env_remove("ELEVENLABS_API_KEY")
        .arg("voices")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

#[test]
fn a_free_voice_gives_one_audio_file_and_never_writes_over_another() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let (code, v) = go(&e, &["--preview", VOICE, "--text", "Hello there."]);
    assert_eq!(code, 0, "{v}");
    let audio = v["artifacts"]["audio"].as_str().unwrap().to_string();
    assert!(
        audio.ends_with("scriptvox-out/voice-en-US-AndrewNeural.mp3"),
        "{audio}"
    );
    assert!(std::fs::metadata(&audio).unwrap().len() > 1000);
    assert_eq!(v["metrics"]["chars"], 12, "{v}");
    assert!(v["metrics"]["duration_s"].as_f64().unwrap() > 0.5, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    assert_eq!(v["text"], "Hello there.", "{v}");
    assert_eq!(v["voice"]["engine"], "edge", "{v}");
    // No line about the terms in the output (owner, 13 Nov 2026): they are data in credits.txt and metadata.json.
    assert!(v.get("terms_note").is_none(), "{v}");
    let next = v["next"].to_string();
    assert!(
        next.contains("scriptvox make") && next.contains(&format!("--voice {VOICE}")),
        "{next}"
    );
    // The same sound again is the same file.
    let (_, v) = go(&e, &["--preview", VOICE, "--text", "Hello there."]);
    assert_eq!(v["artifacts"]["audio"], audio.as_str(), "{v}");
    // Another file at that name stays, and the preview gets a free name.
    std::fs::write(&audio, "my own file").unwrap();
    let (code, v) = go(&e, &["--preview", VOICE, "--text", "Hello there."]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["artifacts"]["audio"]
            .as_str()
            .unwrap()
            .ends_with("voice-en-US-AndrewNeural-2.mp3"),
        "{v}"
    );
    assert_eq!(std::fs::read_to_string(&audio).unwrap(), "my own file");
    // A named --out with other content is refused, not replaced.
    std::fs::write(p.join("mine.mp3"), "keep me").unwrap();
    let (code, v) = go(&e, &["--preview", VOICE, "--out", "mine.mp3"]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(
        std::fs::read_to_string(p.join("mine.mp3")).unwrap(),
        "keep me"
    );
    let (code, v) = go(&e, &["--preview", VOICE, "--out", "sub/new.mp3"]);
    assert_eq!(code, 0, "{v}");
    assert!(p.join("sub/new.mp3").exists());
}

#[test]
fn the_text_is_the_first_sentence_of_the_brief_as_make_reads_it() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    std::fs::write(
        p.join("b.json"),
        json!({"script": "Welcome [pause 0.5] to ACME today. A second sentence here.", "say": {"ACME": "Akmee"}})
            .to_string(),
    )
    .unwrap();
    let (code, v) = go(&e, &["--preview", VOICE, "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["text"], "Welcome to ACME today.", "{v}");
    // The stub writes the words it was given: the voice got the brief's `say` form, as in `make`.
    let words = std::fs::read_to_string(p.join(".scriptvox/preview/words.json")).unwrap();
    assert!(
        words.contains("Akmee") && !words.contains("second"),
        "{words}"
    );
    // No text and no brief: a sentence of the voice's own language.
    let (code, v) = go(&e, &["--preview", "ar-AE-FatimaNeural"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["text"]
            .as_str()
            .unwrap()
            .chars()
            .any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)),
        "{v}"
    );
    let (_, en) = go(&e, &["--preview", VOICE]);
    assert!(en["text"].as_str().unwrap().is_ascii(), "{en}");
}

#[test]
fn wrong_input_is_a_usage_error_and_writes_nothing() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let long = "word ".repeat(41);
    let (code, v) = go(&e, &["--preview", VOICE, "--text", &long]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("200"),
        "{v}"
    );
    let (code, v) = go(&e, &["--preview", "no-such-voice"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("scriptvox voices"),
        "{v}"
    );
    // The flags belong to --preview.
    let (code, _) = go(&e, &["--text", "Hello."]);
    assert_eq!(code, 2);
    // The silent row has nothing to hear.
    let (code, v) = go(&e, &["--preview", "no-voice", "--tts", "none"]);
    assert_ne!(code, 0, "{v}");
    assert!(!e.dir.path().join("scriptvox-out").exists());
    // --plan runs nothing.
    let (code, v) = go(&e, &["--preview", VOICE, "--plan"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["plan_id"].as_str().is_some(), "{v}");
    assert!(!e.dir.path().join("scriptvox-out").exists());
}

#[test]
fn a_brief_cannot_make_the_preview_read_a_file_outside_the_project() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "The vault code is nine nine nine.").unwrap();
    for file in [
        secret.display().to_string(),
        "../../etc/hostname".to_string(),
    ] {
        std::fs::write(p.join("b.json"), json!({ "script_file": file }).to_string()).unwrap();
        let (code, v) = go(&e, &["--preview", VOICE, "--brief", "b.json"]);
        assert_eq!(code, 2, "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains("outside"),
            "{v}"
        );
        assert!(!v.to_string().contains("vault code"), "{v}");
    }
    assert!(!p.join("scriptvox-out").exists(), "nothing is written");
}
