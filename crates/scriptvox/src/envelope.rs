//! The `scriptvox.result/v1` envelope (PLAN-R1 §2.3). With `--json` (or no TTY)
//! stdout holds exactly this one document. Logs go to stderr.

use crate::exit::Exit;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

pub const SCHEMA: &str = "scriptvox.result/v1";

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub fix: Option<String>,
    pub retryable: bool,
    pub retry_after_s: Option<u64>,
}

/// `scriptvox.result/v1`: with `--json` (or no TTY) stdout holds exactly this document.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Envelope {
    pub schema: &'static str,
    pub ok: bool,
    pub status: &'static str,
    pub command: String,
    pub run_id: Option<String>,
    pub plan_id: Option<String>,
    pub needs_input: Vec<Value>,
    pub findings: Vec<Value>,
    pub next: Vec<String>,
    pub error: Option<ErrorInfo>,
    pub artifacts: Map<String, Value>,
    pub metrics: Map<String, Value>,
    pub events_file: Option<String>,
    /// Command-specific top-level fields, for example `proto` on `--version`.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
    #[serde(skip)]
    #[schemars(skip)]
    pub exit: Exit,
}

impl Envelope {
    /// What a person at a terminal sees. Agents get the JSON: this never changes that. Errors print first, a failed
    /// check next, then what to do, and the terms note last. An error always ends with a next step.
    pub fn human_lines(&self) -> Vec<String> {
        let mut l: Vec<String> = Vec::new();
        if let Some(e) = &self.error {
            l.push(format!("error: {} ({})", e.message, e.code));
            if let Some(fix) = &e.fix {
                l.push(format!("fix: {fix}"));
            }
        } else if let Some(v) = self.artifacts.get("video").and_then(|v| v.as_str()) {
            let m = &self.metrics;
            let secs = m
                .get("duration_s")
                .and_then(|d| d.as_f64())
                .map_or(String::new(), |d| format!(" · {d:.1} s"));
            let cost = m.get("cost_usd").and_then(|c| c.as_f64()).unwrap_or(0.0);
            let checks = if self.findings.iter().any(|f| f["status"] == "fail") {
                "a check failed"
            } else {
                "checks passed"
            };
            l.push(format!("result: {v}{secs} · ${cost:.2} · {checks}"));
        } else if let Some(a) = self.artifacts.get("audio").and_then(|a| a.as_str()) {
            let secs = self.metrics.get("duration_s").and_then(|d| d.as_f64());
            l.push(format!("result: {a} · {:.1} s", secs.unwrap_or(0.0)));
        }
        if let Some(p) = &self.plan_id {
            l.push(format!("plan: {p}"));
        }
        for n in &self.needs_input {
            // The item made from the fix is already on the `fix:` line.
            if let Some(a) = n["human_action"]
                .as_str()
                .filter(|a| self.error.as_ref().and_then(|e| e.fix.as_deref()) != Some(*a))
            {
                l.push(format!("needs: {a}"));
            }
        }
        for f in self
            .findings
            .iter()
            .filter(|f| f["status"] == "fail" || f["status"] == "warn")
        {
            l.push(format!(
                "{}: {}: {}",
                f["status"].as_str().unwrap_or(""),
                // A gate row has `gate`; a doctor row has `name`.
                f["gate"].as_str().or(f["name"].as_str()).unwrap_or(""),
                f["detail"].as_str().unwrap_or("")
            ));
        }
        for n in &self.next {
            l.push(format!("next: {n}"));
        }
        if self.error.is_some()
            && self.error.as_ref().is_some_and(|e| e.fix.is_none())
            && self.needs_input.is_empty()
        {
            l.push("next: `scriptvox doctor` checks this machine; `--help` on the command shows its usage.".to_string());
        }
        if l.is_empty() {
            l.push(serde_json::to_string_pretty(&self.extra).unwrap_or_default());
        }
        l
    }

    /// A paid plan, or a stop for approval or for the spend cap: the answer tells an agent to ask the person, in a
    /// `needs_input` item of kind `approval` and in the `next` line before any command. Called once, where every answer is printed, so no command
    /// can leave it out (13 Nov 2026: a trial agent approved a $0.56 plan by itself, because `needs_input` was empty
    /// and `next` held only the `--approve` command). A free plan, a finished run and a call that carries this
    /// plan's own `--approve` stay as they are. This only words the answer: it blocks nothing.
    pub fn ask_before_paying(&mut self) {
        let cost = self
            .metrics
            .get("cost_usd")
            .and_then(|c| c.as_f64())
            .filter(|c| *c > 0.0);
        let stop = self.status == "approval_needed";
        let plan = self.ok && self.run_id.is_none() && self.plan_id.is_some() && cost.is_some();
        // A plan on a run (`export <run> --plan`) has a run id: there the sign is a `next` that names the flag.
        let tells = self.next.iter().any(|n| n.contains("--approve"));
        let approved = self.plan_id.is_some() && self.plan_id.as_deref() == crate::plan::approved();
        if !(stop || (plan || tells) && !approved)
            || self.needs_input.iter().any(|n| n["kind"] == "approval")
        {
            return;
        }
        let price = cost.map_or("money".to_string(), |c| format!("${c:.2}"));
        // Over the spend cap an approval of this id would be refused, so the item names the cap, not the id.
        let over_cap = self
            .error
            .as_ref()
            .is_some_and(|e| e.code == "SVX_SPEND_CAP");
        let then = match (&self.plan_id, over_cap) {
            (_, true) => "Raise --max-spend only after the person says yes.".to_string(),
            (Some(id), _) => format!("Give --approve {id} only after the person says yes."),
            (None, _) => "Give --approve only after the person says yes.".to_string(),
        };
        let mut item = crate::voices::needs_input(
            "approve",
            "approval",
            &format!(
                "This plan costs {price}. Show the price to the person and ask. {then} An agent must not \
                 approve, or raise --max-spend, by its own decision."
            ),
            vec![],
        );
        item["cost_usd"] = serde_json::json!(cost);
        item["plan_id"] = serde_json::json!(self.plan_id);
        // A step that blocks before the money (a missing key) keeps its first place; the ask comes before any
        // command.
        let at = self
            .next
            .iter()
            .take_while(|n| self.needs_input.iter().any(|i| i["human_action"] == **n))
            .count();
        self.needs_input.push(item);
        self.next.insert(
            at,
            format!("Ask the person to approve {price}. Go on only after the person says yes."),
        );
    }

    pub fn ok(command: &str) -> Self {
        Self {
            schema: SCHEMA,
            ok: true,
            status: "ok",
            command: command.to_string(),
            run_id: None,
            plan_id: None,
            needs_input: vec![],
            findings: vec![],
            next: vec![],
            error: None,
            artifacts: Map::new(),
            metrics: Map::new(),
            events_file: None,
            extra: Map::new(),
            exit: Exit::Ok,
        }
    }

    pub fn fail(
        command: &str,
        exit: Exit,
        message: impl Into<String>,
        fix: Option<String>,
    ) -> Self {
        let mut e = Self::ok(command);
        e.ok = false;
        e.status = "error";
        e.exit = exit;
        let message = message.into();
        // Exit 10 has one shape. A caller that knows more replaces this item.
        if exit == Exit::NeedsInput {
            e.status = "needs_input";
            let action = fix.as_deref().unwrap_or(&message);
            e.needs_input
                .push(crate::voices::needs_input(command, "input", action, vec![]));
        }
        e.error = Some(ErrorInfo {
            code: exit.error_code(),
            message,
            fix,
            retryable: exit == Exit::ProviderRetryable,
            retry_after_s: None,
        });
        e
    }

    /// A failure whose error code is more specific than the exit name,
    /// for example `SVX_PROTO_MISMATCH` with exit 1.
    pub fn fail_code(
        command: &str,
        exit: Exit,
        code: &str,
        message: impl Into<String>,
        fix: Option<String>,
    ) -> Self {
        let mut e = Self::fail(command, exit, message, fix);
        if let Some(err) = e.error.as_mut() {
            err.code = code.to_string();
        }
        e
    }

    pub fn with(mut self, key: &str, value: Value) -> Self {
        self.extra.insert(key.to_string(), value);
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("envelope serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_10_has_one_shape() {
        let e = Envelope::fail(
            "judge",
            Exit::NeedsInput,
            "none set",
            Some("Set it.".into()),
        );
        assert_eq!(e.status, "needs_input");
        assert_eq!(e.needs_input.len(), 1);
        assert_eq!(e.needs_input[0]["human_action"], "Set it.");
        assert_eq!(e.needs_input[0]["field"], "judge");
        let e = Envelope::fail("x", Exit::NeedsInput, "do this", None);
        assert_eq!(e.needs_input[0]["human_action"], "do this");
        assert_eq!(Envelope::fail("x", Exit::Error, "b", None).status, "error");
        assert!(Envelope::fail("x", Exit::Error, "b", None)
            .needs_input
            .is_empty());
    }

    #[test]
    fn a_doctor_row_shows_its_name_on_a_terminal() {
        let mut e = Envelope::ok("doctor");
        e.findings.push(serde_json::json!({"name": "ffmpeg", "status": "fail", "detail": "FFmpeg was not found"}));
        assert!(e
            .human_lines()
            .contains(&"fail: ffmpeg: FFmpeg was not found".to_string()));
    }

    #[test]
    fn a_terminal_error_always_says_what_to_do_next() {
        let mut e = Envelope::fail("x", Exit::Error, "boom", None);
        e.next.clear();
        assert!(e.human_lines().iter().any(|l| l.starts_with("next: ")));
        let mut ok = Envelope::ok("make");
        ok.artifacts
            .insert("video".into(), serde_json::json!("out.mp4"));
        ok.metrics
            .insert("duration_s".into(), serde_json::json!(30.0));
        ok.next = vec!["scriptvox judge".into()];
        let lines = ok.human_lines();
        assert!(
            lines[0].starts_with("result: out.mp4 · 30.0 s"),
            "{lines:?}"
        );
        assert!(lines.contains(&"next: scriptvox judge".to_string()));
    }
}
