//! `make --topic "<text>"` (`docs/specs/make-topic.md`): one command with no agent. `research`, then `write`, then
//! the normal `make` with its plan and its approval. This file gives `make` the brief. Opt-in as `research` and
//! `write` are: with no text row it stops with exit 10 and the one `connect` command.

use crate::envelope::Envelope;
use serde_json::json;
use std::path::{Path, PathBuf};

pub struct Args {
    pub topic: String,
    pub lang: Option<String>,
    pub duration: Option<f64>,
    /// `make --tts`: the voice whose rate sets the word target of the script.
    pub tts: Option<String>,
    pub plan_only: bool,
    pub max_spend: Option<f64>,
    pub local_only: bool,
}

/// The folder of one topic: `scriptvox-out/topic-<8 hex>`, from the topic, the language and the duration. The second
/// call of the same command (with `--approve`, or the child of `--detach`) finds the same brief there. A model does
/// not give the same words two times, and other words would be another plan id.
pub fn folder(topic: &str, lang: Option<&str>, duration: Option<f64>) -> PathBuf {
    let topic = topic.split_whitespace().collect::<Vec<_>>().join(" ");
    let key = json!({"topic": topic, "lang": lang, "duration": duration}).to_string();
    Path::new("scriptvox-out").join(format!(
        "topic-{}",
        &blake3::hash(key.as_bytes()).to_hex()[..8]
    ))
}

/// A stop of `research` or `write` is a stop of `make`: the person runs the same `make --topic` again.
fn as_make(mut env: Envelope) -> Box<Envelope> {
    env.command = "make".into();
    Box::new(env)
}

/// The brief of the topic. An `Err` is the answer of `make`: a stop for input or approval, an error, or the plan
/// of the text steps.
pub fn brief(a: Args) -> Result<PathBuf, Box<Envelope>> {
    let dir = folder(&a.topic, a.lang.as_deref(), a.duration);
    let (research, brief) = (dir.join("research.json"), dir.join("brief.json"));
    if brief.is_file() {
        return Ok(brief);
    }
    // Before any request and any folder: no row, or a row with no key, is exit 10.
    let row = crate::text_rows::pick("make", None)?;
    if !research.is_file() {
        if !a.plan_only {
            std::fs::create_dir_all(&dir).map_err(|e| {
                Box::new(Envelope::fail(
                    "make",
                    crate::exit::Exit::Error,
                    format!("cannot make {}: {e}", dir.display()),
                    Some("Run the command in a folder that you can write to.".into()),
                ))
            })?;
        }
        let mut env = crate::research::run(crate::research::Args {
            topic: a.topic.clone(),
            lang: a.lang.clone(),
            depth: "normal".into(),
            out: Some(research.clone()),
            urls: vec![],
            text_row: None,
            pages_only: false,
            plan_only: a.plan_only,
            max_spend: a.max_spend,
            local_only: a.local_only,
        });
        if a.plan_only {
            // `--plan` sends nothing, and there is no script yet: this is the plan of the first text step.
            env = env.with(
                "topic_steps",
                json!(["research", "write", "the plan of the video"]),
            );
            env.next.push(format!(
                "Run the same command without --plan: the text row {} reads the pages and writes the script, then the plan of the video is shown and a paid plan stops for approval.",
                row.name
            ));
            return Err(as_make(env));
        }
        if !env.ok {
            // A research file with no fact must not stay: the next run would take it as done.
            let _ = std::fs::remove_file(&research);
            // Only when it is empty: a stop before the first request leaves nothing.
            let _ = std::fs::remove_dir(&dir);
            if env.exit == crate::exit::Exit::Error {
                if let Some(e) = env.error.as_mut() {
                    // `make` has no --url: the way with a named page goes through `research`.
                    let fix = e.fix.clone().unwrap_or_default().replace(
                        "add --url <address of a page about the topic>",
                        "run `scriptvox research \"<topic>\" --url <address of a page about the topic>`, then `scriptvox write --from research.json` and `scriptvox make --brief brief.json`",
                    );
                    e.fix = Some(format!(
                        "{fix} Or write the script without checked facts: `scriptvox write --topic \"<topic>\"`, then `scriptvox make --brief brief.json`."
                    ));
                }
            }
            return Err(as_make(env));
        }
    }
    let env = crate::write::run(crate::write::Args {
        from: Some(research.clone()),
        topic: None,
        duration: a.duration,
        lang: a.lang.clone(),
        out: Some(brief.clone()),
        text_row: None,
        tts: a.tts.clone(),
        sources_card: false,
        plan_only: a.plan_only,
        max_spend: a.max_spend,
        local_only: a.local_only,
    });
    if !env.ok || a.plan_only {
        return Err(as_make(env));
    }
    // With no `--duration` the video is as long as the script that the model wrote. `write` aims at 45 s, and a
    // shorter answer must not stop `make` at its "script too short for the duration" check.
    // The voice is the one of each `make` call (`--tts`), not the one of the call that wrote the script.
    let text = std::fs::read_to_string(&brief).unwrap_or_default();
    if let Ok(serde_json::Value::Object(mut b)) = serde_json::from_str(&text) {
        b.remove("tts");
        if a.duration.is_none() {
            b.remove("duration");
        }
        let _ = std::fs::write(&brief, serde_json::Value::Object(b).to_string());
    }
    // Logs go to stderr: the person can read and change the script, then `make --brief`.
    eprintln!(
        "topic: facts in {}, script in {}",
        research.display(),
        brief.display()
    );
    Ok(brief)
}
