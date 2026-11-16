//! Text rows (`docs/specs/write.md`): a `[[text]]` block in the user's `generators.toml` names a service with the
//! OpenAI chat API. No row is built in. Only `research` and `write` call one; a paid row goes through the plan, the
//! spend cap and the approval.

use crate::envelope::Envelope;
use crate::exit::Exit;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRow {
    pub name: String,
    pub base: String,
    pub model: String,
    #[serde(default = "none")]
    pub key: String,
    #[serde(default)]
    pub usd_per_1m_in: f64,
    #[serde(default)]
    pub usd_per_1m_out: f64,
}

fn none() -> String {
    "none".into()
}

#[derive(Deserialize)]
struct Overlay {
    #[serde(default)]
    text: Vec<TextRow>,
}

/// The one command that sets a row up.
pub const SETUP: &str = "scriptvox connect local --url http://127.0.0.1:11434 --text --model <id>";

impl TextRow {
    pub fn paid(&self) -> bool {
        self.usd_per_1m_in > 0.0 || self.usd_per_1m_out > 0.0
    }
    /// On this machine: `--local-only` allows it.
    pub fn local(&self) -> bool {
        crate::http_rows::local_host(&self.base)
    }
    pub fn host(&self) -> &str {
        let rest = self.base.split_once("://").map_or("", |x| x.1);
        rest.split(['/', '?', '#']).next().unwrap_or("")
    }
    fn problem(&self) -> Option<String> {
        let name_ok = !self.name.is_empty()
            && self.name.len() <= 48
            && self
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
        if !name_ok {
            return Some("name must be 1 to 48 of a-z, 0-9, '.', '_' or '-'".into());
        }
        if !crate::generators::speech_base_ok(&self.base) {
            return Some("base must be https://, or http:// only on 127.0.0.1 or localhost".into());
        }
        if self.model.trim().is_empty() {
            return Some("model is empty".into());
        }
        if self.key != "none" && crate::keys::env_var(&self.key).is_none() {
            return Some(format!(
                "key {:?} is not declared; add a [[key]] block or use connect --key-name",
                self.key
            ));
        }
        let price = |p: f64| p.is_finite() && (0.0..=10_000.0).contains(&p);
        if !price(self.usd_per_1m_in) || !price(self.usd_per_1m_out) {
            return Some("a price is 0 to 10000 USD for one million tokens".into());
        }
        None
    }
    /// USD for one call with these token counts.
    fn cost(&self, tokens_in: f64, tokens_out: f64) -> f64 {
        (tokens_in * self.usd_per_1m_in + tokens_out * self.usd_per_1m_out) / 1e6
    }
    /// The most one call can cost: 2 characters counted as 1 token in, and all of `max_tokens` out.
    fn worst(&self, in_chars: usize, max_tokens: u32) -> f64 {
        self.cost(in_chars as f64 / 2.0, max_tokens as f64)
    }
}

/// The good rows of the user's file, and one line for each bad block.
pub fn rows() -> (Vec<TextRow>, Vec<String>) {
    let file = crate::keys::overlay_file();
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let (mut good, mut bad): (Vec<TextRow>, Vec<String>) = (Vec::new(), Vec::new());
    for block in crate::generators::blocks(&text) {
        if !crate::generators::block_is(block, "text") {
            continue;
        }
        match toml::from_str::<Overlay>(block) {
            Ok(o) => {
                for r in o.text {
                    match r.problem() {
                        Some(e) => bad.push(format!("{}: text {}: {e}", file.display(), r.name)),
                        None if good.iter().any(|g| g.name == r.name) => bad.push(format!(
                            "{}: text {}: the name is used twice",
                            file.display(),
                            r.name
                        )),
                        None => good.push(r),
                    }
                }
            }
            Err(e) => bad.push(format!("{}: [[text]]: {}", file.display(), e.message())),
        }
    }
    (good, bad)
}

/// For `providers`: the text rows (no key value) and what is wrong with a bad `[[text]]` block.
pub fn listing() -> Value {
    let (rows, errors) = rows();
    json!({
        "rows": rows.iter().map(|r| json!({
            "name": r.name, "host": r.host(), "model": r.model, "key": r.key,
            "ready": r.key == "none" || crate::keys::resolve(&r.key).is_some(),
            "local": r.local(), "paid": r.paid(),
            "usd_per_1m_in": r.usd_per_1m_in, "usd_per_1m_out": r.usd_per_1m_out,
        })).collect::<Vec<_>>(),
        "errors": errors,
        "add": SETUP,
    })
}

/// The row to use: the named one, else the first. No row: exit 10 with the one command that sets a row up.
pub fn pick(command: &str, name: Option<&str>) -> Result<TextRow, Box<Envelope>> {
    let (rows, bad) = rows();
    let row = match name {
        Some(n) => rows.iter().find(|r| r.name == n).cloned().ok_or_else(|| {
            Envelope::fail(
                command,
                Exit::Usage,
                format!("no text row named {n}"),
                Some(if rows.is_empty() {
                    format!("Add one: `{SETUP}`.")
                } else {
                    format!(
                        "The text rows: {}.",
                        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
                    )
                }),
            )
        })?,
        None => rows.first().cloned().ok_or_else(|| {
            let mut env = Envelope::fail(
                command,
                Exit::NeedsInput,
                format!(
                    "{command} needs a text model, and no text row is set up. ScriptVox calls no text model until you add one."
                ),
                Some(format!(
                    "Run `{SETUP}` in your terminal (a local server with the OpenAI chat API, such as Ollama or llama.cpp), or give the https address of a service and `--key-name <name>`."
                )),
            );
            env.next = vec![SETUP.to_string()];
            if !bad.is_empty() {
                env = env.with("text_row_errors", json!(bad));
            }
            env
        })?,
    };
    if row.key != "none" && crate::keys::resolve(&row.key).is_none() {
        let fix = format!("Run `scriptvox keys set {}` in your terminal.", row.key);
        let mut env = Envelope::fail(
            command,
            Exit::NeedsInput,
            format!(
                "the text row {} needs the key {}, and it is not set",
                row.name, row.key
            ),
            Some(fix.clone()),
        );
        env.needs_input = vec![crate::voices::needs_input(&row.key, "secret", &fix, vec![])];
        return Err(Box::new(env));
    }
    Ok(row)
}

/// The money one command may spend on a row, and what it spent.
pub struct Budget {
    pub row: TextRow,
    pub plan_id: String,
    cap: f64,
    pub spent: f64,
    /// Every request that was sent, also one that the service refused for its shape (it costs nothing).
    pub calls: u32,
    /// The service refused `max_tokens` one time: each later call of this command starts with the newer shape.
    newer: bool,
}

/// The plan of a command's calls: `--plan` prints it, a paid row stops for the cap or the approval. `inputs` are what
/// sets the size of the work; no page text is in them, so the id of the stop is the id of the next run.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    command: &str,
    row: TextRow,
    inputs: Value,
    max_calls: u32,
    in_chars: usize,
    max_tokens: u32,
    max_spend: Option<f64>,
    plan_only: bool,
) -> Result<Budget, Box<Envelope>> {
    let cost = row.worst(in_chars, max_tokens) * max_calls as f64;
    let id = crate::plan::plan_id(&json!({
        "command": command, "row": row.name, "base": row.base, "model": row.model,
        "usd_per_1m_in": row.usd_per_1m_in, "usd_per_1m_out": row.usd_per_1m_out, "inputs": inputs,
    }));
    if plan_only {
        let mut env = Envelope::ok(command).with(
            "plan",
            json!({"text_row": row.name, "model": row.model, "host": row.host(), "paid": row.paid(),
                   "calls_at_most": max_calls, "characters_in_at_most": in_chars, "max_tokens": max_tokens,
                   "cost_is": "an upper limit from the row's own prices"}),
        );
        env.plan_id = Some(id.clone());
        env.metrics.insert("cost_usd".into(), json!(cost));
        let unknown = !row.paid() && !row.local();
        if unknown {
            env = env.with(
                "price",
                json!("not known: the row is on a network service and has no price"),
            );
        }
        if cost > 0.0 || unknown {
            env.next = vec![format!(
                "Run the same command without --plan and with --approve {id}"
            )];
        }
        return Err(Box::new(env));
    }
    // A row on a network service with no price: the cost is not known, so the person decides each time. Without
    // this, a second row at the same service with the price left out would run with no approval.
    if !row.paid() && !row.local() && crate::plan::approved() != Some(id.as_str()) {
        return Err(Box::new(crate::plan::approval_fail(
            command,
            format!(
                "approval needed: the text row {} is on a network service ({}) and has no price, so the cost of this command is not known. It sends at most {max_calls} calls. plan_id {id}",
                row.name,
                row.host()
            ),
        )));
    }
    if let Some(env) = crate::plan::over_cap(command, &id, cost, max_spend, vec![]) {
        return Err(Box::new(env));
    }
    if crate::plan::needs_approval(&id, cost, crate::plan::approved()).is_some() {
        let mut env = crate::plan::approval_fail(
            command,
            format!(
                "approval needed: {} is a paid text row, an external API on your own key. This command sends at most {max_calls} calls for at most ${cost:.2} by the row's prices. plan_id {id}",
                row.name
            ),
        );
        env.metrics.insert("cost_usd".into(), json!(cost));
        return Err(Box::new(env));
    }
    Ok(Budget {
        row,
        plan_id: id,
        cap: cost,
        spent: 0.0,
        calls: 0,
        newer: false,
    })
}

/// Why a call failed: (exit, message, fix). The message holds the host and the HTTP code, never a key or a body.
pub type ChatErr = (Exit, String, String);

impl Budget {
    /// One more call of this size stays inside the approved cost.
    pub fn can_call(&self, in_chars: usize, max_tokens: u32) -> bool {
        self.spent + self.row.worst(in_chars, max_tokens) <= self.cap + 1e-9
    }

    /// One chat call. The answer text, or the failure.
    pub fn chat(&mut self, system: &str, user: &str, max_tokens: u32) -> Result<String, ChatErr> {
        let row = &self.row;
        let retry =
            "Start the server or check the row, then run the same command again.".to_string();
        let agent = crate::http_rows::agent_ca(180, None)
            .map_err(|e| (Exit::Error, e, "Check the CA file.".to_string()))?;
        let messages =
            json!([{"role": "system", "content": system}, {"role": "user", "content": user}]);
        // Two request shapes. Older servers (and most local ones) take `max_tokens` and `temperature`. Newer hosted
        // models refuse both and take `max_completion_tokens`. A refused request (HTTP 400 that names the field) costs
        // nothing, so the second shape is tried one time, and the command keeps it for its later calls.
        let mut newer = self.newer;
        let mut resp = loop {
            let body = if newer {
                json!({"model": row.model, "max_completion_tokens": max_tokens, "stream": false, "messages": messages})
            } else {
                json!({"model": row.model, "temperature": 0, "max_tokens": max_tokens, "stream": false, "messages": messages})
            }
            .to_string();
            let mut req = agent
                .post(&format!(
                    "{}/v1/chat/completions",
                    row.base.trim_end_matches('/')
                ))
                .header("Content-Type", "application/json")
                .header("User-Agent", crate::http_rows::USER_AGENT);
            if row.key != "none" {
                if let Some((k, _)) = crate::keys::resolve(&row.key) {
                    req = req.header("Authorization", &format!("Bearer {k}"));
                }
            }
            self.calls += 1;
            let mut resp = req.send(body.as_str()).map_err(|e| {
                // The kind of the failure only (timeout, connection, TLS): no address, no key, no body.
                let kind = match e {
                    ureq::Error::Timeout(_) => "timeout",
                    ureq::Error::Io(_) | ureq::Error::ConnectionFailed => "connection",
                    ureq::Error::Tls(_) => "TLS",
                    _ => "request",
                };
                (
                    Exit::ProviderRetryable,
                    format!(
                        "no answer from the text row {} at {} ({kind})",
                        row.name,
                        row.host()
                    ),
                    retry.clone(),
                )
            })?;
            if resp.status().as_u16() == 400 && !newer {
                let why = resp.body_mut().read_to_string().unwrap_or_default();
                if why.contains("max_tokens") || why.contains("temperature") {
                    newer = true;
                    self.newer = true;
                    continue;
                }
                return Err((
                    Exit::ProviderRetryable,
                    format!(
                        "the text row {} at {} answered HTTP 400",
                        row.name,
                        row.host()
                    ),
                    retry,
                ));
            }
            break resp;
        };
        let code = resp.status().as_u16();
        if code != 200 {
            let (exit, fix) = match code {
                401 | 403 => (
                    Exit::NeedsInput,
                    format!("Run `scriptvox keys set {}` in your terminal with the right key.", row.key),
                ),
                404 => (
                    Exit::Usage,
                    "Check the row's base and model in generators.toml: the base is the address without /v1.".to_string(),
                ),
                _ => (Exit::ProviderRetryable, retry),
            };
            return Err((
                exit,
                format!(
                    "the text row {} at {} answered HTTP {code}",
                    row.name,
                    row.host()
                ),
                fix,
            ));
        }
        let v: Value = resp
            .body_mut()
            .read_to_string()
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let content = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        // The service's own count, else the characters, 2 for 1 token.
        let tokens =
            |field: &str, chars: usize| v["usage"][field].as_f64().unwrap_or(chars as f64 / 2.0);
        self.spent += row.cost(
            tokens("prompt_tokens", system.len() + user.len()),
            tokens("completion_tokens", content.len()),
        );
        Ok(content)
    }
}

/// The first complete JSON object in a model's answer. A code fence or a thinking part around it is allowed.
pub fn first_json(answer: &str) -> Option<Value> {
    // After a closed thinking part, when there is one.
    let answer = answer.rsplit_once("</think>").map_or(answer, |x| x.1);
    let mut from = 0;
    while let Some(at) = answer[from..].find('{') {
        let start = from + at;
        let mut de = serde_json::Deserializer::from_str(&answer[start..]).into_iter::<Value>();
        if let Some(Ok(v)) = de.next() {
            if v.is_object() {
                return Some(v);
            }
        }
        from = start + 1;
    }
    None
}

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// `connect <name> --url <base> --text --model <id>`: one `[[text]]` block. `connect` has checked the address and the
/// key name before this.
pub fn connect(
    a: &crate::connect::Args,
    base: &str,
    listed: Vec<String>,
) -> Result<Envelope, (Exit, String, String)> {
    let usage = |why: String, fix: String| (Exit::Usage, why, fix);
    let some = listed
        .iter()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let [model] = a.models.as_slice() else {
        return Err(usage(
            "a text row needs one --model".into(),
            format!("Name one model of the service with --model: {some}."),
        ));
    };
    if !listed.contains(model) {
        return Err(usage(
            format!("{base} does not list {model}"),
            format!("It lists: {some}."),
        ));
    }
    let row = TextRow {
        name: a.name.clone(),
        base: base.to_string(),
        model: model.clone(),
        key: a.key_name.clone().unwrap_or_else(none),
        usd_per_1m_in: a.usd_in.unwrap_or(0.0),
        usd_per_1m_out: a.usd_out.unwrap_or(0.0),
    };
    // On a dry run a new key name is not declared yet: that is not a fault of the row.
    if let Some(e) = row
        .problem()
        .filter(|e| !(a.dry_run && e.starts_with("key ")))
    {
        return Err(usage(
            format!("text row {}: {e}", row.name),
            "Correct the flag and run again.".into(),
        ));
    }
    let (existing, _) = rows();
    if existing.iter().any(|r| r.name == row.name) {
        return Err(usage(
            format!("a text row named {} exists already", row.name),
            "Give another name, or remove its [[text]] block from generators.toml.".into(),
        ));
    }
    if existing
        .iter()
        .any(|r| r.key != "none" && r.key == row.key && r.base != row.base)
    {
        return Err(usage(
            format!(
                "the key {} belongs to another service, so it is not sent to {base}",
                row.key
            ),
            "Give this service its own key name with --key-name.".into(),
        ));
    }
    let block = format!(
        "[[text]]\nname = {}\nbase = {}\nmodel = {}\nkey = {}\nusd_per_1m_in = {}\nusd_per_1m_out = {}\n",
        q(&row.name),
        q(&row.base),
        q(&row.model),
        q(&row.key),
        row.usd_per_1m_in,
        row.usd_per_1m_out
    );
    let file = crate::keys::overlay_file();
    let shape = json!({"name": row.name, "base": row.base, "model": row.model, "key": row.key, "paid": row.paid()});
    if a.dry_run {
        let mut env = Envelope::ok("connect")
            .with("would_add", json!([row.name]))
            .with("kind", json!("text"))
            .with("row", shape)
            .with("file", json!(file));
        env.next
            .push("Run the same command without --dry-run to add this row.".into());
        return Ok(env);
    }
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(&block);
    file.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&file, text))
        .map_err(|e| {
            (
                Exit::Error,
                format!("{}: {e}", file.display()),
                "Check the config folder is writable.".to_string(),
            )
        })?;
    let mut env = Envelope::ok("connect")
        .with("added", json!([row.name]))
        .with("kind", json!("text"))
        .with("row", shape)
        .with("file", json!(file));
    if row.key != "none" && crate::keys::resolve(&row.key).is_none() {
        env.next.push(format!("scriptvox keys set {}", row.key));
    }
    env.next
        .push("scriptvox research \"<topic>\" --json".into());
    env.next
        .push("scriptvox write --from research.json --json".into());
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::first_json;

    #[test]
    fn a_row_on_a_network_service_with_no_price_stops_for_approval() {
        let row = |base: &str| super::TextRow {
            name: "r".into(),
            base: base.into(),
            model: "m".into(),
            key: "none".into(),
            usd_per_1m_in: 0.0,
            usd_per_1m_out: 0.0,
        };
        let plan = |base: &str| {
            super::plan(
                "write",
                row(base),
                serde_json::json!({}),
                2,
                1000,
                100,
                None,
                false,
            )
        };
        // A public address (a literal, so no name is resolved): the cost is not known.
        let stop = plan("https://93.184.216.34").err().unwrap();
        assert_eq!(stop.exit, crate::exit::Exit::ApprovalNeeded);
        assert!(stop.plan_id.is_some());
        // A row on this machine with no price is free.
        assert!(plan("http://127.0.0.1:11434").is_ok());
    }

    #[test]
    fn the_json_of_an_answer_is_found_inside_other_text() {
        let v = first_json(
            "<think>{not json</think>Here:\n```json\n{\"facts\": [{\"a\": \"}\"}]}\n``` done",
        )
        .unwrap();
        assert_eq!(v["facts"][0]["a"], "}");
        assert!(first_json("no object here [1, 2]").is_none());
    }
}
