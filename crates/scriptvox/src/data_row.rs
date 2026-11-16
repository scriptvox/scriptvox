//! A video or image service described by a small data file (`scriptvox connect --spec`, section 6 row 25, third runner;
//! docs/specs/connect.md). One queue flow: submit, poll, take the result. A spec is data: no code, no conditions.
//! The idea and its limits come from an Apache-2.0 design: docs/provenance/data-file-adapter.md. This code is ours.

use crate::comfy::Failure;
use crate::exit::Exit;
use crate::generators::Generator;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_SPEC: usize = 32 << 10;
const MAX_REPLY: u64 = 32 << 20;
/// ponytail: a fixed poll pace; a field in the spec when a service rate-limits this.
const POLL_S: u64 = 2;
const HOLES: [&str; 5] = ["prompt", "aspect", "seconds", "seed", "image"];
/// Headers the transport or the key rule own. `auth` must not name them.
const RESERVED: [&str; 8] = [
    "authorization",
    "host",
    "cookie",
    "content-length",
    "content-type",
    "connection",
    "transfer-encoding",
    "proxy-authorization",
];

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub kind: String,
    #[serde(default)]
    pub auth: Auth,
    pub submit: Submit,
    pub poll: Poll,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields, rename_all = "lowercase", tag = "kind")]
pub enum Auth {
    #[default]
    Bearer,
    Header {
        name: String,
    },
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    pub path: String,
    pub body: Value,
    pub job_id_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    pub path: String,
    pub status_at: String,
    /// Each word the service can say, as `running`, `succeeded` or `failed`.
    pub states: BTreeMap<String, String>,
    pub result_at: String,
    pub result_as: String,
}

fn holes(s: &str) -> Vec<&str> {
    s.split('{')
        .skip(1)
        .map(|p| p.split('}').next().unwrap_or(p))
        .collect()
}

fn leaves<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| leaves(x, out)),
        Value::Object(o) => o.values().for_each(|x| leaves(x, out)),
        _ => {}
    }
}

fn path_ok(p: &str) -> bool {
    p.starts_with('/')
        && !p.contains("..")
        && !p.contains("//")
        && !p.contains("://")
        && !p.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn pointer_ok(p: &str) -> bool {
    p.starts_with('/') && p.split('/').count() <= 9
}

/// Every rule of the file, so a bad one stops at `connect` and never at a paid call.
pub fn parse(bytes: &[u8]) -> Result<Spec, String> {
    if bytes.len() > MAX_SPEC {
        return Err("the spec is larger than 32 KB".into());
    }
    let s: Spec = serde_json::from_slice(bytes).map_err(|e| format!("the spec: {e}"))?;
    if !matches!(s.kind.as_str(), "video.gen" | "image.gen") {
        return Err("kind must be video.gen or image.gen".into());
    }
    if let Auth::Header { name } = &s.auth {
        let good = !name.is_empty()
            && name.len() <= 64
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !RESERVED.contains(&name.to_ascii_lowercase().as_str());
        if !good {
            return Err(format!("auth header {name:?} is not allowed"));
        }
    }
    if !path_ok(&s.submit.path) || s.submit.path.contains('{') {
        return Err(format!(
            "submit path {:?} is not allowed: start with /, no host, no .., no placeholder",
            s.submit.path
        ));
    }
    let job = holes(&s.poll.path);
    if !path_ok(&s.poll.path) || job != ["job_id"] {
        return Err(format!(
            "poll path {:?} is not allowed: start with /, no host, no .., and {{job_id}} once",
            s.poll.path
        ));
    }
    for p in [&s.submit.job_id_at, &s.poll.status_at, &s.poll.result_at] {
        if !pointer_ok(p) {
            return Err(format!("{p:?} is not a JSON pointer of at most 8 levels"));
        }
    }
    if s.poll.states.is_empty() || s.poll.states.len() > 16 {
        return Err("poll states needs 1 to 16 words".into());
    }
    if let Some((w, v)) = s
        .poll
        .states
        .iter()
        .find(|(_, v)| !matches!(v.as_str(), "running" | "succeeded" | "failed"))
    {
        return Err(format!(
            "state {w:?} is {v:?}: use running, succeeded or failed"
        ));
    }
    if !matches!(s.poll.result_as.as_str(), "url" | "base64") {
        return Err("result_as must be url or base64".into());
    }
    let mut text = vec![];
    leaves(&s.submit.body, &mut text);
    if let Some(bad) = text
        .iter()
        .flat_map(|l| holes(l))
        .find(|h| !HOLES.contains(h))
    {
        return Err(format!("the body has {{{bad}}}: use {}", HOLES.join(", ")));
    }
    if !text.iter().any(|l| holes(l).contains(&"prompt")) {
        return Err("the body needs {prompt}".into());
    }
    Ok(s)
}

pub fn uses(s: &Spec, hole: &str) -> bool {
    let mut text = vec![];
    leaves(&s.submit.body, &mut text);
    text.iter().any(|l| holes(l).contains(&hole))
}

/// The body for one request. A leaf that is only `{seconds}` or `{seed}` stays a number. A leaf that needs a value this
/// request does not have goes away. The structure comes only from the spec, so a prompt cannot add a field.
fn fill(v: &Value, input: &Value, image: Option<&str>) -> Option<Value> {
    Some(match v {
        Value::String(s) => {
            let number = |k: &str| match k {
                "{seconds}" => Some(input["duration"].clone()),
                "{seed}" => Some(input["seed"].clone()),
                _ => None,
            };
            if let Some(n) = number(s) {
                return (!n.is_null()).then_some(n);
            }
            let pairs: [(&str, Option<String>); 5] = [
                ("prompt", input["prompt"].as_str().map(String::from)),
                ("aspect", input["aspect_ratio"].as_str().map(String::from)),
                ("seconds", input["duration"].as_f64().map(|d| d.to_string())),
                ("seed", input["seed"].as_u64().map(|d| d.to_string())),
                ("image", image.map(String::from)),
            ];
            // One pass from left to right: a value that holds `{image}` is not filled again.
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(i) = rest.find('{') {
                out.push_str(&rest[..i]);
                let tail = &rest[i + 1..];
                let name = tail.split('}').next().unwrap_or("");
                let value = pairs.iter().find(|(k, _)| *k == name)?.1.clone()?;
                out.push_str(&value);
                rest = &tail[name.len() + 1..];
            }
            out.push_str(rest);
            json!(out)
        }
        Value::Array(a) => Value::Array(a.iter().filter_map(|x| fill(x, input, image)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .filter_map(|(k, x)| Some((k.clone(), fill(x, input, image)?)))
                .collect(),
        ),
        other => other.clone(),
    })
}

pub fn file_for(row: &str) -> PathBuf {
    crate::keys::config_dir()
        .join("spec")
        .join(format!("{row}.json"))
}

/// Submit, poll, and return the result's bytes. Only a refusal before the job exists is known to have cost nothing.
pub fn run(
    g: &Generator,
    input: &Value,
    key: Option<&str>,
    wait_s: u64,
    dir: &Path,
    part: &Path,
) -> Result<Vec<u8>, Failure> {
    use Failure::{Refused, Unconfirmed};
    let name = g.name.as_str();
    let path = file_for(name);
    let again = format!("Run `scriptvox connect {name} --url <base> --spec <file>` again.");
    let s = std::fs::read(&path)
        .ok()
        .and_then(|b| parse(&b).ok())
        .ok_or_else(|| {
            Refused(
                Exit::NeedsInput,
                format!("{} is missing or broken", path.display()),
                again.clone(),
            )
        })?;
    let base = g.base.as_deref().unwrap_or("").trim_end_matches('/');
    let agent: ureq::Agent = crate::http_rows::agent_ca(600, g.ca.as_deref())
        .map_err(|e| Refused(Exit::Error, e, "Check the CA file.".into()))?;
    let auth: Option<(String, String)> = match (&s.auth, key) {
        (Auth::None, _) | (_, None) => None,
        (Auth::Bearer, Some(k)) => Some(("Authorization".into(), format!("Bearer {k}"))),
        (Auth::Header { name }, Some(k)) => Some((name.clone(), k.to_string())),
    };
    let image = input[g.image_key()].as_str();
    let body = fill(&s.submit.body, input, image).unwrap_or(Value::Null);
    let mut req = agent
        .post(&format!("{base}{}", s.submit.path))
        .header("Content-Type", "application/json");
    if let Some((h, v)) = &auth {
        req = req.header(h.as_str(), v.as_str());
    }
    let mut resp = req
        .config()
        .max_redirects(0)
        .build()
        .send(body.to_string().as_bytes())
        .map_err(|e| Unconfirmed(format!("{name} did not answer: {e}")))?;
    let code = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if code >= 500 || (300..400).contains(&code) {
        return Err(Unconfirmed(format!("{name} answered HTTP {code}")));
    }
    if code >= 400 {
        let exit = if code == 401 || code == 403 {
            Exit::NeedsInput
        } else {
            Exit::Error
        };
        let short: String = text.chars().take(300).collect();
        return Err(Refused(
            exit,
            format!("{name} refused the request: HTTP {code} {short}"),
            format!(
                "Check the spec file, or the key with `scriptvox keys set {}`.",
                g.key
            ),
        ));
    }
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Unconfirmed(format!("{name} gave no JSON: {e}")))?;
    let job = match doc.pointer(&s.submit.job_id_at) {
        Some(Value::String(j)) => j.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    if !crate::comfy::safe_id(&job) {
        return Err(Unconfirmed(format!("{name} gave no usable job id")));
    }
    let url = format!("{base}{}", s.poll.path.replace("{job_id}", &job));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_s.max(1));
    let done = loop {
        let mut r = agent.get(&url);
        if let Some((h, v)) = &auth {
            r = r.header(h.as_str(), v.as_str());
        }
        // A lost poll is not a lost job: the deadline decides.
        if let Ok(mut resp) = r.config().max_redirects(0).build().call() {
            let doc: Value = resp
                .body_mut()
                .with_config()
                .limit(MAX_REPLY)
                .read_to_string()
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            if let Some(word) = doc.pointer(&s.poll.status_at).and_then(Value::as_str) {
                match s.poll.states.get(word).map(String::as_str) {
                    Some("succeeded") => break doc,
                    Some("failed") => {
                        return Err(Unconfirmed(format!("{name} says the job failed: {word}")))
                    }
                    Some(_) => {}
                    // Never a guess: a word the file does not list stops the run.
                    None => {
                        return Err(Unconfirmed(format!(
                            "{name} said {word:?}, which the spec does not list"
                        )))
                    }
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(Unconfirmed(format!(
                "{name} did not finish job {job} in {wait_s} s"
            )));
        }
        std::thread::sleep(std::time::Duration::from_secs(POLL_S));
    };
    let Some(result) = done.pointer(&s.poll.result_at).and_then(Value::as_str) else {
        return Err(Unconfirmed(format!(
            "{name} finished, with nothing at {}",
            s.poll.result_at
        )));
    };
    if s.poll.result_as == "base64" {
        use base64::Engine;
        return base64::engine::general_purpose::STANDARD
            .decode(result.trim())
            .map_err(|e| Unconfirmed(format!("{name} gave bad base64: {e}")));
    }
    crate::genjob::fetch_result(g, result, dir, part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Value {
        json!({"kind": "video.gen",
            "submit": {"path": "/v1/videos", "job_id_at": "/id",
                "body": {"prompt": "{prompt}", "ratio": "{aspect}", "len": "{seconds}", "seed": "{seed}", "image": "{image}"}},
            "poll": {"path": "/v1/videos/{job_id}", "status_at": "/status", "result_at": "/url", "result_as": "url",
                "states": {"queued": "running", "done": "succeeded", "bad": "failed"}}})
    }

    fn bad(edit: impl Fn(&mut Value)) -> String {
        let mut v = good();
        edit(&mut v);
        parse(v.to_string().as_bytes()).unwrap_err()
    }

    #[test]
    fn every_unsafe_field_stops_at_connect() {
        assert!(parse(good().to_string().as_bytes()).is_ok());
        assert!(
            bad(|v| v["submit"]["path"] = json!("https://evil.example/x")).contains("not allowed")
        );
        assert!(bad(|v| v["submit"]["path"] = json!("/a/../b")).contains("not allowed"));
        assert!(bad(|v| v["poll"]["path"] = json!("/v1/videos")).contains("job_id"));
        assert!(bad(|v| v["webhook"] = json!(1)).contains("unknown field"));
        assert!(
            bad(|v| v["auth"] = json!({"kind": "header", "name": "Authorization"}))
                .contains("not allowed")
        );
        assert!(bad(|v| v["submit"]["body"]["x"] = json!("{key}")).contains("{key}"));
        assert!(bad(|v| v["submit"]["body"] = json!({"a": 1})).contains("{prompt}"));
        assert!(bad(|v| v["poll"]["states"]["x"] = json!("maybe")).contains("maybe"));
    }

    #[test]
    fn a_prompt_cannot_add_a_field_and_a_missing_value_goes_away() {
        let s = parse(good().to_string().as_bytes()).unwrap();
        let input = json!({"prompt": "a\", \"admin\": \"1 {image}", "aspect_ratio": "16:9", "duration": 5.0});
        let v = fill(&s.submit.body, &input, None).unwrap();
        assert_eq!(v["prompt"], input["prompt"], "{v}");
        assert_eq!(v["len"], 5.0, "{v}");
        assert!(
            v.get("admin").is_none() && v.get("image").is_none() && v.get("seed").is_none(),
            "{v}"
        );
        let v = fill(&s.submit.body, &input, Some("data:x")).unwrap();
        assert_eq!(v["image"], "data:x", "{v}");
    }
}
