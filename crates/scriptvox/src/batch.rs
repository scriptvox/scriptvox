//! `scriptvox batch --brief b.json --rows rows.json`: one video per row. The brief holds `{{key}}` placeholders in its
//! text; each row (a JSON object) fills them. Every row runs as its own `scriptvox make` with the same money rules, so
//! a paid row still stops at its own plan. The filled brief is written next to the original (hidden), so paths in it
//! stay relative to the same folder.

use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub struct Args {
    pub brief: PathBuf,
    pub rows: PathBuf,
    pub local_only: bool,
    /// Rows at the same time, 1 or 2 (the load limit of a shared machine).
    pub jobs: usize,
}

/// A CSV file as rows: the first line names the columns. Quoted fields may hold commas, line breaks and `""`.
pub fn csv_rows(text: &str) -> Result<Vec<Map<String, Value>>, String> {
    let mut recs: Vec<Vec<String>> = vec![];
    let (mut rec, mut field, mut quoted, mut chars) = (
        vec![],
        String::new(),
        false,
        text.trim_start_matches('\u{feff}').chars().peekable(),
    );
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => rec.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                rec.push(std::mem::take(&mut field));
                recs.push(std::mem::take(&mut rec));
            }
            (c, _) => field.push(c),
        }
    }
    if quoted {
        return Err("a quote is not closed".into());
    }
    if !field.is_empty() || !rec.is_empty() {
        rec.push(field);
        recs.push(rec);
    }
    recs.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    let Some((head, body)) = recs.split_first() else {
        return Err("the file is empty".into());
    };
    body.iter()
        .enumerate()
        .map(|(i, r)| {
            if r.len() != head.len() {
                return Err(format!(
                    "line {} has {} fields, the header has {}",
                    i + 2,
                    r.len(),
                    head.len()
                ));
            }
            Ok(head
                .iter()
                .map(|h| h.trim().to_string())
                .zip(r.iter().map(|v| json!(v)))
                .collect())
        })
        .collect()
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("batch", exit, msg.into(), Some(fix.into()))
}

/// The brief text with every `{{key}}` filled from `row`, each value escaped for a JSON string. Err names a
/// placeholder the row does not fill.
pub fn fill(text: &str, row: &Map<String, Value>) -> Result<String, String> {
    let mut out = text.to_string();
    for (k, v) in row {
        let s = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let escaped = serde_json::to_string(&s).unwrap_or_default();
        out = out.replace(&format!("{{{{{k}}}}}"), &escaped[1..escaped.len() - 1]);
    }
    if let Some(i) = out.find("{{") {
        let name: String = out[i + 2..].chars().take_while(|c| *c != '}').collect();
        return Err(format!("no value for {{{{{name}}}}}"));
    }
    Ok(out)
}

/// The `{{key}}` names of a brief, in the order they first appear, each with `true` when it stands outside a JSON
/// string at least once (`"duration": {{secs}}`), so its rows need a number or true/false, not text. A name is
/// letters, digits, `_` and `-`.
pub fn placeholders(text: &str) -> Vec<(String, bool)> {
    let mut keys: Vec<(String, bool)> = vec![];
    let (mut in_string, mut escaped) = (false, false);
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            _ if escaped => escaped = false,
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            b'{' if b.get(i + 1) == Some(&b'{') => {
                if let Some(j) = text[i + 2..].find("}}") {
                    let name = text[i + 2..i + 2 + j].trim();
                    if !name.is_empty()
                        && name
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    {
                        match keys.iter_mut().find(|k| k.0 == name) {
                            Some(k) => k.1 |= !in_string,
                            None => keys.push((name.to_string(), !in_string)),
                        }
                        i += j + 4;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    keys
}

/// `batch --describe`: what a batch brief needs, before any row exists (G115). The keys, one sample row as JSON and
/// as a CSV header, and whether the brief is a valid brief once every key has a value. Nothing is made.
pub fn describe(brief: &std::path::Path) -> Envelope {
    let text = match std::fs::read_to_string(brief) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                Exit::Usage,
                format!("{}: {e}", brief.display()),
                "Give a brief file.",
            )
        }
    };
    let found = placeholders(&text);
    let keys: Vec<&String> = found.iter().map(|k| &k.0).collect();
    let bare: Vec<&String> = found.iter().filter(|k| k.1).map(|k| &k.0).collect();
    // A key inside a text value gets text; a bare key gets a number, because text there would not be JSON.
    let sample: Map<String, Value> = found
        .iter()
        .map(|(k, bare)| {
            (
                k.clone(),
                if *bare {
                    json!(1)
                } else {
                    json!(format!("<{k}>"))
                },
            )
        })
        .collect();
    // The brief with every key filled must be a brief: a placeholder in a number field, or a typo in a field name,
    // shows here and not on row 1 of 200.
    let problem = match fill(&text, &sample) {
        Err(e) => Some(e),
        Ok(filled) => serde_json::from_str::<crate::brief::Brief>(&filled)
            .err()
            .map(|e| e.to_string()),
    };
    let mut env = Envelope::ok("batch")
        .with("keys", json!(keys))
        .with("sample_rows", json!([sample]))
        .with("needs_a_number_or_true_false", json!(bare))
        .with(
            "csv_header",
            json!(keys
                .iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(",")),
        )
        .with("valid_when_filled", json!(problem.is_none()));
    if let Some(p) = &problem {
        env.findings.push(json!({"gate": "brief", "status": "warn",
            "detail": format!("with the sample row the brief is not valid: {p}"),
            "fix": "Check the field names and values with `scriptvox schema brief`."}));
    }
    if keys.is_empty() {
        env.findings.push(json!({"gate": "keys", "status": "warn",
            "detail": "the brief has no {{key}}: every row would make the same video",
            "fix": "Put {{name}} where the rows differ, for example in `script`."}));
    }
    env.next = vec![format!(
        "scriptvox batch --brief {} --rows <rows.json or rows.csv>",
        brief.display()
    )];
    env
}

pub fn run(a: Args) -> Envelope {
    let text = match std::fs::read_to_string(&a.brief) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                Exit::Usage,
                format!("{}: {e}", a.brief.display()),
                "Give a brief file.",
            )
        }
    };
    let is_csv = a
        .rows
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    let parsed: Option<Vec<Value>> = if is_csv {
        match std::fs::read_to_string(&a.rows)
            .map_err(|e| e.to_string())
            .and_then(|t| csv_rows(&t))
        {
            Ok(r) => Some(r.into_iter().map(Value::Object).collect()),
            Err(e) => {
                return fail(
                    Exit::Usage,
                    format!("{}: {e}", a.rows.display()),
                    "Write a CSV with a header line, one video per line.",
                )
            }
        }
    } else {
        std::fs::read_to_string(&a.rows)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok())
    };
    if !(1..=2).contains(&a.jobs) {
        return fail(
            Exit::Usage,
            "--jobs is 1 or 2",
            "More at once would load a shared machine; run a second batch later.",
        );
    }
    let rows: Vec<Map<String, Value>> = match parsed {
        Some(r) if !r.is_empty() && r.iter().all(Value::is_object) => r
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .collect(),
        _ => {
            return fail(
                Exit::Usage,
                format!("{} is not a JSON list of objects", a.rows.display()),
                "Write rows like [{\"name\": \"Al Ain\"}, {\"name\": \"Al Dhafra\"}].",
            )
        }
    };
    if rows.len() > 200 {
        return fail(
            Exit::Usage,
            format!("{} rows; the limit is 200", rows.len()),
            "Split the rows.",
        );
    }
    // Every row must fill every placeholder: check them all before the first video.
    let mut filled = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match fill(&text, row) {
            Ok(t) => filled.push(t),
            Err(e) => {
                return fail(
                    Exit::Usage,
                    format!("row {}: {e}", i + 1),
                    "Add the value to the row.",
                )
            }
        }
    }
    let dir = a
        .brief
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = a
        .brief
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("brief")
        .to_string();
    let exe = std::env::current_exe().unwrap_or_else(|_| "scriptvox".into());
    // One row: skipped when its video exists from the same filled brief (the hash beside it), else one `make`.
    let one = |i: usize| -> Value {
        let n = i + 1;
        let t = &filled[i];
        let out = PathBuf::from("scriptvox-out").join(format!("{stem}-{n}.mp4"));
        let mark = out.with_extension("mp4.brief-hash");
        let hash = blake3::hash(t.as_bytes()).to_hex().to_string();
        if out.is_file() && std::fs::read_to_string(&mark).is_ok_and(|h| h.trim() == hash) {
            return json!({"row": n, "values": rows[i], "exit": 0, "status": "skipped", "video": std::path::absolute(&out).unwrap_or(out.clone()),
                "note": "made before from the same row; delete the video to make it again"});
        }
        let row_brief = dir.join(format!(".{stem}.row{n}.json"));
        if let Err(e) = std::fs::write(&row_brief, t) {
            return json!({"row": n, "values": rows[i], "exit": 1, "error": format!("{}: {e}", row_brief.display())});
        }
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("make")
            .arg("--brief")
            .arg(&row_brief)
            .arg("--out")
            .arg(&out)
            .arg("--json")
            .stdin(std::process::Stdio::null());
        if a.local_only {
            cmd.arg("--local-only");
        }
        let o = cmd.output();
        let _ = std::fs::remove_file(&row_brief);
        let (code, env) = match o {
            Ok(o) => (
                o.status.code().unwrap_or(1),
                serde_json::from_slice::<Value>(
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .last()
                        .unwrap_or("")
                        .as_bytes(),
                )
                .unwrap_or_default(),
            ),
            Err(e) => (1, json!({"error": {"message": e.to_string()}})),
        };
        if code == 0 {
            let _ = std::fs::write(&mark, &hash);
        }
        json!({
            "row": n, "values": rows[i], "exit": code, "status": env["status"], "run_id": env["run_id"],
            "video": env["artifacts"]["video"], "error": env["error"]["message"],
            "findings": env["findings"].as_array().map(|f| f.iter().filter(|x| x["status"] != "pass").cloned().collect::<Vec<_>>()),
        })
    };
    // ponytail: rows go to `jobs` threads by index; each runs its own `make` process.
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::Mutex::new(vec![Value::Null; filled.len()]);
    std::thread::scope(|sc| {
        for _ in 0..a.jobs {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= filled.len() {
                    break;
                }
                let r = one(i);
                done.lock().unwrap()[i] = r;
            });
        }
    });
    let results = done.into_inner().unwrap_or_default();
    let worst = results
        .iter()
        .filter_map(|r| r["exit"].as_i64())
        .find(|c| *c != 0)
        .unwrap_or(0) as i32;
    let mut env = if worst == 0 {
        Envelope::ok("batch")
    } else {
        Envelope::fail(
            "batch",
            Exit::ALL
                .iter()
                .copied()
                .find(|e| i32::from(e.code()) == worst)
                .unwrap_or(Exit::Error),
            format!(
                "{} of {} rows did not finish ok",
                results.iter().filter(|r| r["exit"] != 0).count(),
                results.len()
            ),
            Some("Read each row's error; rerun only those rows.".into()),
        )
    };
    env.metrics.insert("rows".into(), json!(results.len()));
    env.with("rows", json!(results))
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholders_are_found_in_order_with_their_place() {
        let t = r#"{"script": "Hi {{name}}, from {{city}}. \"{{name}}\"", "duration": {{secs}}, "x": "{{ not a key }} {{}}"}"#;
        assert_eq!(
            super::placeholders(t),
            vec![
                ("name".to_string(), false),
                ("city".to_string(), false),
                ("secs".to_string(), true)
            ]
        );
        assert!(super::placeholders("no keys here {").is_empty());
    }

    use super::*;

    #[test]
    fn csv_rows_read_quotes_and_refuse_ragged_lines() {
        let r =
            csv_rows("name,line\r\n\"Al Ain, East\",\"Say \"\"hi\"\"\"\nDhafra,two\n\n").unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["name"], "Al Ain, East");
        assert_eq!(r[0]["line"], "Say \"hi\"");
        assert_eq!(r[1]["line"], "two");
        assert!(csv_rows("a,b\n1\n").unwrap_err().contains("line 2"));
        assert!(csv_rows("a\n\"open\n").is_err());
    }

    #[test]
    fn placeholders_fill_escaped_and_missing_ones_are_named() {
        let row: Map<String, Value> =
            serde_json::from_str(r#"{"name": "Al \"Ain\"", "n": 3}"#).unwrap();
        let t = fill(
            r#"{"script": "Welcome to {{name}}, district {{n}}."}"#,
            &row,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["script"], "Welcome to Al \"Ain\", district 3.");
        assert!(fill("{{name}} {{city}}", &row)
            .unwrap_err()
            .contains("{{city}}"));
    }
}
