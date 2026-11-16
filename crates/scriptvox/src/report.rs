//! `scriptvox report`: a text about this machine and one run, for the user to read and then send with a problem
//! (docs/specs/report.md, G126). It is made here and written to a file. Nothing leaves the machine.
//! What is taken out: every key value, the home folder and the user name, and the words of the user's own script,
//! prompts, cards and sources in the request (their lengths stay). A gate message can still quote a few words: the
//! text says so at its top.
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Args {
    /// The run to describe. Default: the newest run that failed, else the newest run.
    pub run: Option<String>,
    pub out: Option<PathBuf>,
    pub use_system: bool,
}

/// Request fields that hold the user's own words. Their text is replaced by its length.
const WORDS: [&str; 13] = [
    "script",
    "prompt",
    "hook",
    "text",
    "label",
    "items",
    "music_prompt",
    "first_frame",
    "say",
    "sources",
    "lines",
    "title",
    "captions_second",
];

/// `v` with the user's words replaced by their size, at any depth.
pub fn without_words(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| {
                    let hidden = WORDS.contains(&k.as_str())
                        && !x.is_null()
                        && !x.is_boolean()
                        && !x.is_number();
                    let y = if hidden {
                        match x {
                            Value::String(s) => {
                                json!(format!("<{} characters>", s.chars().count()))
                            }
                            Value::Array(a) => json!(format!("<{} entries>", a.len())),
                            _ => json!("<taken out>"),
                        }
                    } else {
                        without_words(x)
                    };
                    (k.clone(), y)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(without_words).collect()),
        other => other.clone(),
    }
}

fn tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

pub fn run(a: Args) -> Envelope {
    let project = Path::new(".");
    let runs = crate::run::list(project, 5);
    let id = match &a.run {
        Some(r) => Some(r.clone()),
        None => runs
            .iter()
            .find(|r| r["exit"].as_i64().is_some_and(|e| e != 0) || r["status"] == "died")
            .or(runs.first())
            .and_then(|r| r["id"].as_str().map(String::from)),
    };
    // A run id is letters, digits, `_` and `.` (r_ab12cd.1): nothing else may reach a path.
    if let Some(bad) = id.as_deref().filter(|i| {
        i.is_empty()
            || !i
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            || i.contains("..")
    }) {
        return Envelope::fail(
            "report",
            Exit::Usage,
            format!("{bad} is not a run id"),
            Some("Give a run id from `scriptvox status`.".into()),
        );
    }
    let dir = id.as_ref().map(|i| project.join(".scriptvox/runs").join(i));
    if let (Some(i), Some(d)) = (&a.run, &dir) {
        if !d.is_dir() {
            return Envelope::fail(
                "report",
                Exit::Usage,
                format!("no run {i} in this project"),
                Some("Run this in the project folder of the run, or leave the id out.".into()),
            );
        }
    }
    let (checks, extra) = crate::doctor::run(a.use_system, false);
    let mut t = String::new();
    t.push_str("# ScriptVox report\n\n");
    t.push_str("Made by `scriptvox report` on the user's machine. Nothing was sent. Key values, the home folder and the user name are taken out, and so are the script, prompt and card texts of the request. An error or a finding can still quote a few of your words, for example a title that left the frame: read this file before you send it.\n\n");
    t.push_str(&format!(
        "- Version: {}\n- System: {} {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    t.push_str("\n## Machine check (`doctor --offline`)\n\n");
    for c in &checks {
        t.push_str(&format!("- {}: {}, {}\n", c.name, c.status, c.detail));
    }
    if let Some(keys) = extra["keys"].as_array() {
        let set: Vec<&str> = keys
            .iter()
            .filter(|k| k["source"] != "none")
            .filter_map(|k| k["provider"].as_str())
            .collect();
        t.push_str(&format!(
            "- keys set (names only): {}\n",
            if set.is_empty() {
                "none".into()
            } else {
                set.join(", ")
            }
        ));
    }
    t.push_str("\n## Latest runs\n\n");
    if runs.is_empty() {
        t.push_str("No run in this project folder.\n");
    }
    for r in &runs {
        t.push_str(&format!(
            "- {} {} {} exit {}\n",
            r["id"].as_str().unwrap_or("?"),
            r["command"].as_str().unwrap_or("?"),
            r["status"].as_str().unwrap_or("?"),
            r["exit"].as_i64().map_or("none".into(), |e| e.to_string())
        ));
    }
    if let (Some(i), Some(d)) = (&id, dir.as_ref().filter(|d| d.is_dir())) {
        t.push_str(&format!("\n## Run {i}\n\n"));
        let result = read_json(&d.join("result.json"));
        if result.is_null() {
            t.push_str("No result file: the run did not finish.\n");
        } else {
            let part = json!({"status": result["status"], "error": result["error"], "findings": result["findings"],
                "metrics": result["metrics"], "next": result["next"], "not_checked": result["not_checked"]});
            t.push_str(&format!(
                "Result:\n\n```json\n{}\n```\n",
                serde_json::to_string_pretty(&part).unwrap_or_default()
            ));
        }
        let request = read_json(&d.join("request.json"));
        if !request.is_null() {
            t.push_str(&format!(
                "\nRequest, without the words:\n\n```json\n{}\n```\n",
                serde_json::to_string_pretty(&without_words(&request)).unwrap_or_default()
            ));
        }
        for (name, n) in [("events.jsonl", 40), ("supervisor.log", 20)] {
            let body = tail(&d.join(name), n);
            if !body.is_empty() {
                t.push_str(&format!("\nLast lines of {name}:\n\n```\n{body}\n```\n"));
            }
        }
    }
    let text = crate::doctor::redact_paths(&crate::redact::Redactor::from_env().apply(&t));
    let out = a.out.clone().unwrap_or_else(|| {
        let stem = id.clone().unwrap_or_else(|| "machine".into());
        let base = Path::new("scriptvox-out");
        // Never over an earlier report: the next free number.
        (1..)
            .map(|n| {
                base.join(if n == 1 {
                    format!("report-{stem}.md")
                } else {
                    format!("report-{stem}-{n}.md")
                })
            })
            .find(|p| !p.exists())
            .unwrap_or_else(|| base.join("report.md"))
    });
    if a.out.is_some() && out.exists() {
        return Envelope::fail(
            "report",
            Exit::Usage,
            format!("{} exists", out.display()),
            Some("Give a new file name: a report does not write over a file.".into()),
        );
    }
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(p) {
            return Envelope::fail("report", Exit::Error, format!("{}: {e}", p.display()), None);
        }
    }
    if let Err(e) = std::fs::write(&out, &text) {
        return Envelope::fail(
            "report",
            Exit::Error,
            format!("{}: {e}", out.display()),
            None,
        );
    }
    let mut env = Envelope::ok("report");
    env.run_id = id;
    env.artifacts.insert(
        "report".into(),
        json!(std::path::absolute(&out).unwrap_or(out)),
    );
    env.metrics
        .insert("lines".into(), json!(text.lines().count()));
    env.next = vec![
        "Read the file before you send it: it is yours to send or not. Nothing was sent.".into(),
    ];
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_of_a_request_are_replaced_by_their_size() {
        let v = json!({"script": "A secret launch on Friday.", "aspect": "9:16", "duration": 12,
            "scenes": [{"script": "Scene words", "card": {"kind": "stat", "text": "42%", "label": "growth"},
                        "generate": {"prompt": "a harbour", "seconds": 5}}],
            "sources": [{"text": "x", "url": "https://example.com"}], "captions": "karaoke", "hook": null});
        let w = without_words(&v);
        let s = w.to_string();
        for gone in [
            "secret",
            "Scene words",
            "42%",
            "growth",
            "harbour",
            "example.com",
        ] {
            assert!(!s.contains(gone), "{gone} in {s}");
        }
        assert_eq!(w["script"], "<26 characters>");
        assert_eq!(w["aspect"], "9:16");
        assert_eq!(w["duration"], 12);
        assert_eq!(w["scenes"][0]["card"]["kind"], "stat");
        assert_eq!(w["scenes"][0]["generate"]["seconds"], 5);
        assert_eq!(w["sources"], "<1 entries>");
        assert_eq!(w["captions"], "karaoke");
        assert!(w["hook"].is_null());
    }
}
