//! ComfyUI as a row (`scriptvox connect --workflow`, section 6 row 25, G047; docs/specs/connect.md). ComfyUI is a separate
//! program, reached over HTTP. A user's API-format graph becomes a template: each field ScriptVox changes is found by its
//! role (prompt, seed, size, length, image, output), and nothing else in the graph changes.
//! `safe_name`, `safe_id` and `encode` come from an MIT OR Apache-2.0 crate, used as Apache-2.0: docs/provenance/comfyui-bridge.md.

use crate::exit::Exit;
use crate::generators::Generator;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::Read;

/// A field of a node: the node's id and the input's name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Field {
    pub node: String,
    pub input: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Template {
    pub graph: Value,
    pub prompt: Field,
    pub seed: Vec<Field>,
    pub width: Option<Field>,
    pub height: Option<Field>,
    pub length: Option<Field>,
    /// Where a first picture goes: the graph's one `LoadImage`, or `--map image=`. Older saved rows have none.
    #[serde(default)]
    pub image: Option<Field>,
    pub output: String,
    pub video: bool,
}

/// Why a call produced no file. Only `Refused` is known to have cost nothing.
pub enum Failure {
    Refused(Exit, String, String),
    /// A refusal that names nodes: the same, with one entry for each node error.
    RefusedNodes(Exit, String, String, Value),
    Unconfirmed(String),
}

pub const MAX_GRAPH: usize = 1 << 20;
/// ponytail: the result is held in memory, so 512 MB is the ceiling; stream to the part file if longer clips need it.
const MAX_RESULT: u64 = 512 << 20;

fn class(n: &Value) -> &str {
    n["class_type"].as_str().unwrap_or("")
}

fn literal(n: &Value, input: &str) -> bool {
    !n["inputs"].get(input).is_none_or(Value::is_array)
}

fn numeric(n: &Value, input: &str) -> bool {
    n["inputs"][input].is_number()
}

/// The node with the prompt's words, followed back along the conditioning links from `start`.
fn text_node(graph: &Value, start: &str, steps: usize) -> Option<Field> {
    let mut id = start.to_string();
    for _ in 0..steps {
        let n = graph.get(&id)?;
        if let Some(input) = ["text", "prompt"]
            .into_iter()
            .find(|k| n["inputs"][*k].is_string())
        {
            return Some(Field {
                node: id,
                input: input.into(),
            });
        }
        id = ["positive", "conditioning"]
            .iter()
            .find_map(|k| n["inputs"][*k][0].as_str())?
            .to_string();
    }
    None
}

/// Each prompt node that reaches a sampler as its positive side: never one on the negative side.
fn prompt_nodes(graph: &Value) -> Vec<Field> {
    let Some(nodes) = graph.as_object() else {
        return vec![];
    };
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for (id, n) in nodes {
        let starts = n["inputs"]["positive"][0].is_string()
            || (class(n).contains("Guider") && n["inputs"]["conditioning"][0].is_string());
        if !starts {
            continue;
        }
        if let Some(f) = text_node(graph, id, nodes.len() + 1) {
            if seen.insert(f.node.clone()) {
                out.push(f);
            }
        }
    }
    out
}

fn parse_field(role: &str, v: &str, graph: &Value) -> Result<Field, String> {
    let (node, input) = v
        .split_once('.')
        .ok_or_else(|| format!("--map {role}={v}: use {role}=<node id>.<input>"))?;
    let n = graph
        .get(node)
        .ok_or_else(|| format!("--map {role}={v}: the graph has no node {node}"))?;
    if !literal(n, input) {
        return Err(format!(
            "--map {role}={v}: that input is linked to another node or is missing, so a value cannot go there"
        ));
    }
    Ok(Field {
        node: node.into(),
        input: input.into(),
    })
}

/// A template from an API-format graph. `maps` are `role=node.input` (`output=node`) and win over what is found.
pub fn template(graph: Value, maps: &[String]) -> Result<Template, String> {
    if graph.get("nodes").is_some_and(Value::is_array) {
        return Err("this is ComfyUI's editor format, not its API format".into());
    }
    let Some(nodes) = graph.as_object().filter(|n| !n.is_empty()) else {
        return Err("the workflow is not a map of nodes".into());
    };
    if let Some((id, _)) = nodes
        .iter()
        .find(|(_, n)| n["class_type"].as_str().is_none() || !n["inputs"].is_object())
    {
        return Err(format!("node {id} has no class_type or inputs"));
    }
    let map = |role: &str| {
        maps.iter()
            .find_map(|m| m.strip_prefix(role)?.strip_prefix('='))
    };
    for m in maps {
        let role = m.split('=').next().unwrap_or("");
        if ![
            "prompt", "seed", "width", "height", "length", "image", "output",
        ]
        .contains(&role)
        {
            return Err(format!(
                "--map {m}: roles are prompt, seed, width, height, length, image and output"
            ));
        }
    }
    let prompt = match map("prompt") {
        Some(v) => parse_field("prompt", v, &graph)?,
        None => {
            let found = prompt_nodes(&graph);
            match found.as_slice() {
                [one] => one.clone(),
                [] => {
                    return Err(
                        "no prompt node was found: give --map prompt=<node id>.<input>".into(),
                    )
                }
                many => {
                    let ids: Vec<&str> = many.iter().map(|f| f.node.as_str()).collect();
                    return Err(format!(
                        "more than one prompt node ({}): choose with --map prompt=<node id>.<input>",
                        ids.join(", ")
                    ));
                }
            }
        }
    };
    if !graph[&prompt.node]["inputs"][&prompt.input].is_string() {
        return Err(format!(
            "the prompt input {}.{} does not hold text",
            prompt.node, prompt.input
        ));
    }
    let output = match map("output") {
        Some(id) => {
            if !nodes.contains_key(id) {
                return Err(format!("--map output={id}: the graph has no node {id}"));
            }
            id.to_string()
        }
        None => {
            let found: Vec<&String> = nodes
                .iter()
                .filter(|(_, n)| ["SaveImage", "SaveVideo", "VHS_VideoCombine"].contains(&class(n)))
                .map(|(id, _)| id)
                .collect();
            match found.as_slice() {
                [one] => (*one).clone(),
                [] => {
                    return Err(
                        "no SaveImage or SaveVideo node: give --map output=<node id>".into(),
                    )
                }
                many => {
                    let ids: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
                    return Err(format!(
                        "more than one output node ({}): choose with --map output=<node id>",
                        ids.join(", ")
                    ));
                }
            }
        }
    };
    let video = {
        let c = class(&nodes[&output]).to_ascii_lowercase();
        c.contains("video") || c.contains("webm")
    };
    let seed = match map("seed") {
        Some(v) => vec![parse_field("seed", v, &graph)?],
        None => nodes
            .iter()
            .flat_map(|(id, n)| {
                ["seed", "noise_seed"]
                    .into_iter()
                    .filter(|k| numeric(n, k))
                    .map(|k| Field {
                        node: id.clone(),
                        input: k.into(),
                    })
            })
            .collect(),
    };
    let latent = nodes
        .iter()
        .find(|(_, n)| class(n).starts_with("Empty") && numeric(n, "width") && numeric(n, "height"))
        .map(|(id, _)| id.clone());
    let at = |id: &Option<String>, input: &str| {
        id.clone().map(|node| Field {
            node,
            input: input.into(),
        })
    };
    let pick = |role: &str, found: Option<Field>| match map(role) {
        Some(v) => parse_field(role, v, &graph).map(Some),
        None => Ok(found),
    };
    let width = pick("width", at(&latent, "width"))?;
    let height = pick("height", at(&latent, "height"))?;
    let frames = nodes
        .iter()
        .find(|(id, n)| *id != &output && numeric(n, "length"))
        .map(|(id, _)| id.clone());
    let length = pick("length", if video { at(&frames, "length") } else { None })?;
    // ponytail: only one LoadImage is found by itself; a graph with more names its first picture with --map image=.
    let loaders: Vec<&String> = nodes
        .iter()
        .filter(|(_, n)| class(n) == "LoadImage" && literal(n, "image"))
        .map(|(id, _)| id)
        .collect();
    let loader = (loaders.len() == 1).then(|| loaders[0].clone());
    let image = pick("image", at(&loader, "image"))?;
    Ok(Template {
        prompt,
        seed,
        width,
        height,
        length,
        image,
        output,
        video,
        graph,
    })
}

/// The classes the graph uses that the server does not have.
pub fn missing_classes(t: &Template, installed: &Value) -> Vec<String> {
    let used: BTreeSet<&str> = t
        .graph
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, n)| class(n))
        .collect();
    used.into_iter()
        .filter(|c| installed.get(*c).is_none())
        .map(String::from)
        .collect()
}

/// A literal text input whose value the server does not list for that slot.
#[derive(Debug, PartialEq)]
pub struct BadChoice {
    pub node: String,
    pub class: String,
    pub input: String,
    pub value: String,
    pub allowed: Vec<String>,
}

/// The graph's literal text inputs against the server's node catalogue: the values that are not in the slot's list, how
/// many were checked, and how many were skipped because the slot's shape is not known here. A slot is a choice when it
/// starts with a list of texts (`[["a", "b"], {...}]`). A slot that starts with a type name (`["STRING", {...}]`) is not
/// a choice. A link is not a value. The picture role is not checked: a run uploads its own file there.
pub fn choices(t: &Template, installed: &Value) -> (Vec<BadChoice>, usize, usize) {
    let (mut bad, mut checked, mut skipped) = (vec![], 0, 0);
    for (id, n) in t.graph.as_object().into_iter().flatten() {
        let slots = &installed[class(n)]["input"];
        for (input, v) in n["inputs"].as_object().into_iter().flatten() {
            let Some(value) = v.as_str() else { continue };
            if t.image
                .as_ref()
                .is_some_and(|f| &f.node == id && &f.input == input)
            {
                continue;
            }
            let slot = ["required", "optional"]
                .iter()
                .find_map(|k| slots[*k].get(input));
            let first = slot.map_or(&Value::Null, |s| &s[0]);
            let list: Option<Vec<&str>> = first
                .as_array()
                .and_then(|l| l.iter().map(Value::as_str).collect());
            match list {
                Some(l) => {
                    checked += 1;
                    if !l.contains(&value) {
                        bad.push(BadChoice {
                            node: id.clone(),
                            class: class(n).to_string(),
                            input: input.clone(),
                            value: value.to_string(),
                            allowed: l.into_iter().map(String::from).collect(),
                        });
                    }
                }
                // ponytail: the `["COMBO", {"options": [...]}]` form is in no page read, so it is skipped, not judged.
                None if first.as_str().is_some_and(|ty| ty != "COMBO") => {}
                None => skipped += 1,
            }
        }
    }
    (bad, checked, skipped)
}

/// A server's refusal of a queued graph, when its answer has `node_errors`: what to say, the fix that fits the cause,
/// and one entry for each node error. None for any other shape. The input comes from `extra_info.input_name`, or is the
/// input of that node in `graph` that the message names.
fn refusal(text: &str, graph: &Value, again: &str) -> Option<(String, String, Value)> {
    let doc: Value = serde_json::from_str(text).ok()?;
    let short = |v: &Value, n: usize| -> String {
        let s = v.as_str().unwrap_or("");
        s.chars().filter(|c| !c.is_control()).take(n).collect()
    };
    let (mut rows, mut lines) = (vec![], vec![]);
    let (mut wrong, mut missing) = (BTreeSet::new(), BTreeSet::new());
    for (id, n) in doc["node_errors"].as_object()?.iter().take(8) {
        let id: String = id.chars().filter(|c| !c.is_control()).take(40).collect();
        let class = short(&n["class_type"], 80);
        for e in n["errors"].as_array().into_iter().flatten().take(4) {
            let kind = short(&e["type"], 60);
            let mut msg = short(&e["message"], 200);
            let details = short(&e["details"], 200);
            if !details.is_empty() && !msg.contains(&details) {
                msg = format!("{msg}: {details}");
            }
            let named = |k: &str| {
                msg.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == k)
            };
            let input = Some(short(&e["extra_info"]["input_name"], 80))
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let keys = graph[&id]["inputs"].as_object()?;
                    keys.keys().find(|k| named(k)).cloned()
                });
            let low = msg.to_lowercase();
            let label = input.clone().unwrap_or_else(|| format!("node {id}"));
            if kind == "value_not_in_list" || low.contains("not in list") {
                wrong.insert(label);
            } else if kind.contains("required") || low.contains("required input") {
                missing.insert(label);
            }
            lines.push(match &input {
                Some(i) => format!("node {id} ({class}), input {i}: {msg}"),
                None => format!("node {id} ({class}): {msg}"),
            });
            rows.push(
                json!({"node": id, "class": class, "input": input, "type": kind, "message": msg}),
            );
        }
    }
    if rows.is_empty() {
        return None;
    }
    let top = match &doc["error"] {
        Value::String(_) => short(&doc["error"], 200),
        e => short(&e["message"], 200),
    };
    let list = |s: BTreeSet<String>| s.into_iter().collect::<Vec<_>>().join(", ");
    let mut fix = vec![];
    if !wrong.is_empty() {
        fix.push(format!(
            "{}: the value is not in the server's list. Pick one of the server's values in the graph, or install the missing model file on the server.",
            list(wrong)
        ));
    }
    if !missing.is_empty() {
        fix.push(format!(
            "{}: a required input has no value. Set it in the graph, or give its place with `--map`.",
            list(missing)
        ));
    }
    if fix.is_empty() {
        fix.push("Fix the graph in ComfyUI.".into());
    }
    Some((
        format!("{top}: {}", lines.join("; ")),
        format!("{} Then: {again}", fix.join(" ")),
        json!(rows),
    ))
}

/// The size for an aspect: the graph's long side, the other side from the aspect, both multiples of 16.
fn sized(w: u64, h: u64, aspect: &str) -> (u64, u64) {
    let parsed = aspect
        .split_once(':')
        .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))
        .filter(|(a, b)| *a > 0 && *b > 0);
    let Some((a, b)) = parsed else {
        return (w, h);
    };
    let long = w.max(h);
    let (x, y) = if a >= b {
        (long, long * b / a)
    } else {
        (long * a / b, long)
    };
    ((x / 16).max(1) * 16, (y / 16).max(1) * 16)
}

/// The frames for `seconds`: 8n+1, a count both Wan and LTX accept.
fn frames(seconds: f64, fps: f64) -> u64 {
    (seconds * fps / 8.0).round().max(1.0) as u64 * 8 + 1
}

/// The graph with this request's values in its role fields.
pub fn render(t: &Template, prompt: &str, seed: u64, aspect: &str, seconds: Option<f64>) -> Value {
    let mut g = t.graph.clone();
    let mut put = |f: &Field, v: Value| g[&f.node]["inputs"][&f.input] = v;
    put(&t.prompt, json!(prompt));
    t.seed.iter().for_each(|f| put(f, json!(seed)));
    if let (Some(wf), Some(hf)) = (&t.width, &t.height) {
        let at = |f: &Field| t.graph[&f.node]["inputs"][&f.input].as_u64();
        if let (Some(w), Some(h)) = (at(wf), at(hf)) {
            let (w, h) = sized(w, h, aspect);
            put(wf, json!(w));
            put(hf, json!(h));
        }
    }
    if let (Some(lf), Some(s)) = (&t.length, seconds) {
        let out = &t.graph[&t.output]["inputs"];
        let fps = ["fps", "frame_rate"]
            .iter()
            .find_map(|k| out[*k].as_f64())
            .unwrap_or(16.0);
        put(lf, json!(frames(s, fps)));
    }
    g
}

pub fn file_for(row: &str) -> std::path::PathBuf {
    crate::keys::config_dir()
        .join("comfy")
        .join(format!("{row}.json"))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn safe_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && !s.contains("..")
        && !s.contains(['/', '\\', ':'])
        && !s.chars().any(char::is_control)
}

pub(crate) fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}

/// The output file's `/view` query, refused if its name or folder could leave ComfyUI's output folders.
fn view_query(node: &Value, video: bool) -> Result<String, String> {
    let keys: &[&str] = if video {
        &["videos", "gifs", "images"]
    } else {
        &["images"]
    };
    let file = keys
        .iter()
        .find_map(|k| node[*k][0].as_object())
        .ok_or("the workflow gave no file at its output node")?;
    let name = file.get("filename").and_then(Value::as_str).unwrap_or("");
    let sub = file.get("subfolder").and_then(Value::as_str).unwrap_or("");
    let kind = file.get("type").and_then(Value::as_str).unwrap_or("output");
    if !matches!(kind, "output" | "temp")
        || !safe_name(name)
        || (!sub.is_empty() && !safe_name(sub))
    {
        return Err(format!(
            "the output path is unsafe: {name} in {sub} ({kind})"
        ));
    }
    let mut q = format!("filename={}&type={kind}", encode(name));
    if !sub.is_empty() {
        q += &format!("&subfolder={}", encode(sub));
    }
    Ok(q)
}

/// Queues the row's graph and waits for its one file. `wait_s` bounds the wait.
/// The bytes and file extension of a picture in a data URI.
fn picture_bytes(uri: &str) -> Option<(Vec<u8>, &'static str)> {
    use base64::Engine;
    let (head, b64) = uri.strip_prefix("data:")?.split_once(";base64,")?;
    let ext = match head {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => return None,
    };
    Some((
        base64::engine::general_purpose::STANDARD.decode(b64).ok()?,
        ext,
    ))
}

/// Sends the first picture to ComfyUI's input folder and returns the name the graph must use. The name comes from the
/// bytes, so a retry overwrites the same file. Nothing is queued yet, so every failure here cost nothing.
fn upload(
    agent: &ureq::Agent,
    base: &str,
    auth: Option<&str>,
    bytes: &[u8],
    ext: &str,
    row: &str,
) -> Result<String, Failure> {
    use sha2::{Digest, Sha256};
    use Failure::Refused;
    let hex: String = Sha256::digest(bytes)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    let file = format!("scriptvox-{hex}.{ext}");
    const B: &str = "scriptvox-upload-boundary";
    let mut body = format!(
        "--{B}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{file}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(
        format!("\r\n--{B}\r\nContent-Disposition: form-data; name=\"overwrite\"\r\n\r\ntrue\r\n--{B}--\r\n").as_bytes(),
    );
    let mut req = agent.post(&format!("{base}/upload/image")).header(
        "Content-Type",
        &format!("multipart/form-data; boundary={B}"),
    );
    if let Some(a) = auth {
        req = req.header("Authorization", a);
    }
    let retry = "Check that ComfyUI runs, then try again.".to_string();
    let mut resp = req.send(&body[..]).map_err(|e| {
        Refused(
            Exit::ProviderRetryable,
            format!("{row}: the picture upload failed: {e}"),
            retry.clone(),
        )
    })?;
    let code = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if code >= 400 {
        let exit = match code {
            401 | 403 => Exit::NeedsInput,
            c if c >= 500 => Exit::ProviderRetryable,
            _ => Exit::Error,
        };
        let short: String = text.chars().take(300).collect();
        return Err(Refused(
            exit,
            format!("{row} refused the picture: HTTP {code} {short}"),
            retry,
        ));
    }
    let doc: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let got = doc["name"].as_str().unwrap_or("");
    let sub = doc["subfolder"].as_str().unwrap_or("");
    if !safe_name(got) || !(sub.is_empty() || safe_name(sub)) {
        return Err(Refused(
            Exit::Error,
            format!("{row} gave no usable name for the picture"),
            retry,
        ));
    }
    Ok(if sub.is_empty() {
        got.to_string()
    } else {
        format!("{sub}/{got}")
    })
}

pub fn run(
    g: &Generator,
    input: &Value,
    key: Option<&str>,
    wait_s: u64,
) -> Result<Vec<u8>, Failure> {
    use Failure::{Refused, Unconfirmed};
    let name = g.name.as_str();
    let path = file_for(name);
    let again = format!("Run `scriptvox connect {name} --url <base> --workflow <file>` again.");
    let t: Template = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| {
            Refused(
                Exit::NeedsInput,
                format!("{} is missing or broken", path.display()),
                again.clone(),
            )
        })?;
    let base = g.base.as_deref().unwrap_or("").trim_end_matches('/');
    let seed = input["seed"].as_u64().unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| u64::from(d.subsec_nanos()) + d.as_secs())
    });
    let seconds = input["duration"].as_f64();
    let mut graph = render(
        &t,
        input["prompt"].as_str().unwrap_or(""),
        seed,
        input["aspect_ratio"].as_str().unwrap_or(""),
        seconds,
    );
    let agent: ureq::Agent = crate::http_rows::agent_ca(600, g.ca.as_deref())
        .map_err(|e| Refused(Exit::Error, e, "Check the CA file.".into()))?;
    let auth = key.map(|k| format!("Bearer {k}"));
    if let (Some(f), Some(uri)) = (&t.image, input[g.image_key()].as_str()) {
        let (bytes, ext) = picture_bytes(uri).ok_or_else(|| {
            Refused(
                Exit::Usage,
                "the first picture is not a PNG, JPEG or WebP data URI".into(),
                "Give --image a local picture file.".into(),
            )
        })?;
        let name = upload(&agent, base, auth.as_deref(), &bytes, ext, name)?;
        graph[&f.node]["inputs"][&f.input] = json!(name);
    }
    let body = json!({"prompt": graph}).to_string();
    let mut req = agent
        .post(&format!("{base}/prompt"))
        .header("Content-Type", "application/json");
    if let Some(a) = &auth {
        req = req.header("Authorization", a);
    }
    let mut resp = req
        .send(body.as_bytes())
        .map_err(|e| Unconfirmed(format!("{name} did not answer: {e}")))?;
    let code = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if code >= 500 {
        return Err(Unconfirmed(format!("{name} answered HTTP {code}")));
    }
    if code >= 400 {
        let exit = if code == 401 || code == 403 {
            Exit::NeedsInput
        } else {
            Exit::Error
        };
        if let Some((what, fix, nodes)) = refusal(&text, &graph, &again) {
            return Err(Failure::RefusedNodes(
                exit,
                format!("{name} refused the workflow: HTTP {code} {what}"),
                fix,
                nodes,
            ));
        }
        let short: String = text.chars().take(400).collect();
        return Err(Refused(
            exit,
            format!("{name} refused the workflow: HTTP {code} {short}"),
            format!("Fix the graph in ComfyUI, then: {again}"),
        ));
    }
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Unconfirmed(format!("{name} gave no JSON: {e}")))?;
    let id = doc["prompt_id"]
        .as_str()
        .filter(|s| safe_id(s))
        .ok_or_else(|| Unconfirmed(format!("{name} gave no usable prompt id")))?
        .to_string();
    let get = |url: String| {
        let mut r = agent.get(&url);
        if let Some(a) = &auth {
            r = r.header("Authorization", a);
        }
        r.config().max_redirects(0).build().call()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_s.max(1));
    let record = loop {
        // A lost poll is not a lost job: the deadline decides.
        if let Ok(mut r) = get(format!("{base}/history/{id}")) {
            let h: Value = r
                .body_mut()
                .with_config()
                .limit(32 << 20)
                .read_to_string()
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let rec = &h[&id];
            if rec["status"]["status_str"] == "error" {
                let says = rec["status"]["messages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|m| m[0] == "execution_error")
                    .and_then(|m| m[1]["exception_message"].as_str())
                    .unwrap_or("no reason given");
                let short: String = says.chars().take(300).collect();
                return Err(Unconfirmed(format!("{name} failed the workflow: {short}")));
            }
            if rec["status"]["completed"] == true {
                break rec.clone();
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(Unconfirmed(format!(
                "{name} did not finish {id} in {wait_s} s"
            )));
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    };
    let q = view_query(&record["outputs"][&t.output], t.video).map_err(Unconfirmed)?;
    let mut r = get(format!("{base}/view?{q}"))
        .map_err(|e| Unconfirmed(format!("the result download failed: {e}")))?;
    if r.status().as_u16() >= 300 {
        return Err(Unconfirmed(format!(
            "the result download answered HTTP {}",
            r.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    r.body_mut()
        .as_reader()
        .take(MAX_RESULT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Unconfirmed(format!("the result download failed: {e}")))?;
    if bytes.len() as u64 > MAX_RESULT {
        return Err(Unconfirmed("the result is larger than 512 MB".into()));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SD_GRAPH: &str = r#"{
      "3": {"class_type": "KSampler", "inputs": {"seed": 1, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0]}},
      "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "a.safetensors"}},
      "5": {"class_type": "EmptyLatentImage", "inputs": {"width": 1024, "height": 1024, "batch_size": 1}},
      "6": {"class_type": "CLIPTextEncode", "inputs": {"text": "old", "clip": ["4", 1]}},
      "7": {"class_type": "CLIPTextEncode", "inputs": {"text": "bad hands", "clip": ["4", 1]}},
      "9": {"class_type": "SaveImage", "inputs": {"images": ["3", 0], "filename_prefix": "x"}}
    }"#;

    fn graph() -> Value {
        serde_json::from_str(SD_GRAPH).unwrap()
    }

    #[test]
    fn roles_are_found_and_only_they_change() {
        let t = template(graph(), &[]).unwrap();
        assert_eq!(
            t.prompt,
            Field {
                node: "6".into(),
                input: "text".into()
            }
        );
        assert_eq!(t.output, "9");
        assert!(!t.video);
        let g = render(&t, "rain", 7, "16:9", None);
        assert_eq!(g["6"]["inputs"]["text"], "rain");
        assert_eq!(
            g["7"]["inputs"]["text"], "bad hands",
            "the negative side stays"
        );
        assert_eq!(g["3"]["inputs"]["seed"], 7);
        assert_eq!(
            (&g["5"]["inputs"]["width"], &g["5"]["inputs"]["height"]),
            (&json!(1024), &json!(576))
        );
    }

    #[test]
    fn a_video_graph_gets_its_length_and_conditioning_is_followed() {
        let g: Value = serde_json::from_str(r#"{
          "1": {"class_type": "CLIPTextEncode", "inputs": {"text": "a"}},
          "2": {"class_type": "WanImageToVideo", "inputs": {"positive": ["1", 0], "width": 832, "height": 480, "length": 33}},
          "3": {"class_type": "KSampler", "inputs": {"seed": 1, "positive": ["2", 0]}},
          "4": {"class_type": "SaveVideo", "inputs": {"video": ["3", 0], "fps": 16}}
        }"#).unwrap();
        let t = template(g, &["width=2.width".into(), "height=2.height".into()]).unwrap();
        assert!(t.video);
        assert_eq!(t.prompt.node, "1");
        let out = render(&t, "x", 1, "9:16", Some(5.0));
        assert_eq!(out["2"]["inputs"]["length"], 81);
        assert_eq!(
            out["2"]["inputs"]["width"], 464,
            "832 x 9/16 = 468, down to a multiple of 16"
        );
        assert_eq!(out["2"]["inputs"]["height"], 832);
    }

    #[test]
    fn a_wrong_workflow_is_refused_with_the_way_out() {
        let e = template(json!({"nodes": []}), &[]).unwrap_err();
        assert!(e.contains("API format"), "{e}");
        let mut two = graph();
        two["3"]["inputs"]["positive"] = json!(["6", 0]);
        two["8"] = json!({"class_type": "KSampler", "inputs": {"positive": ["7", 0]}});
        let e = template(two.clone(), &[]).unwrap_err();
        assert!(e.contains("6, 7") && e.contains("--map prompt"), "{e}");
        assert_eq!(
            template(two, &["prompt=7.text".into()])
                .unwrap()
                .prompt
                .node,
            "7"
        );
        let e = template(graph(), &["prompt=3.positive".into()]).unwrap_err();
        assert!(e.contains("linked"), "{e}");
        let e = template(graph(), &["size=5.width".into()]).unwrap_err();
        assert!(e.contains("roles are"), "{e}");
    }

    #[test]
    fn output_paths_cannot_leave_the_output_folder() {
        let at = |name: &str, sub: &str, kind: &str| {
            view_query(
                &json!({"images": [{"filename": name, "subfolder": sub, "type": kind}]}),
                false,
            )
        };
        assert_eq!(
            at("a b.png", "", "output").unwrap(),
            "filename=a%20b.png&type=output"
        );
        assert!(at("../x.png", "", "output").is_err());
        assert!(at("x.png", "../up", "output").is_err());
        assert!(at("x.png", "", "input").is_err());
    }

    #[test]
    fn one_load_image_is_the_picture_role_and_more_need_a_map() {
        let g = |n: usize| {
            let mut v: Value = serde_json::from_str(SD_GRAPH).unwrap();
            for i in 0..n {
                v[format!("2{i}")] =
                    json!({"class_type": "LoadImage", "inputs": {"image": "a.png"}});
            }
            v
        };
        assert_eq!(template(g(0), &[]).unwrap().image, None);
        let one = template(g(1), &[]).unwrap().image.unwrap();
        assert_eq!((one.node.as_str(), one.input.as_str()), ("20", "image"));
        assert_eq!(template(g(2), &[]).unwrap().image, None);
        let m = template(g(2), &["image=21.image".to_string()])
            .unwrap()
            .image
            .unwrap();
        assert_eq!(m.node, "21");
        assert!(picture_bytes("data:image/png;base64,QUJD").is_some());
        assert!(picture_bytes("https://x/a.png").is_none());
    }

    #[test]
    fn only_a_list_of_texts_is_a_choice_and_other_answers_are_not_read() {
        let t = template(graph(), &[]).unwrap();
        let slot =
            |s: Value| json!({"CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": s}}}});
        let counts = |s: Value| {
            let (bad, checked, skipped) = choices(&t, &slot(s));
            (bad.len(), checked, skipped)
        };
        // The two prompt texts and the file prefix have no slot here: 3 skipped each time.
        assert_eq!(counts(json!([["a.safetensors", "b"]])), (0, 1, 3));
        assert_eq!(counts(json!([["b"]])), (1, 1, 3));
        assert_eq!(counts(json!([[]])), (1, 1, 3), "no file on the server");
        assert_eq!(counts(json!(["STRING", {}])), (0, 0, 3), "not a choice");
        assert_eq!(counts(json!(["COMBO", {"options": ["b"]}])), (0, 0, 4));
        assert_eq!(counts(json!([["b", 1]])), (0, 0, 4), "a mixed list");
        assert_eq!(counts(json!("odd")), (0, 0, 4));

        let g = graph();
        assert!(refusal("not json", &g, "again").is_none());
        assert!(refusal(r#"{"error": "x", "node_errors": {}}"#, &g, "again").is_none());
        assert!(refusal(r#"{"error": "x", "node_errors": []}"#, &g, "again").is_none());
        let (what, fix, rows) = refusal(
            r#"{"error": "bad", "node_errors": {"9": {"errors": [{"message": "odd\nthing"}]}}}"#,
            &g,
            "again",
        )
        .unwrap();
        assert_eq!(what, "bad: node 9 (): oddthing");
        assert_eq!(fix, "Fix the graph in ComfyUI. Then: again");
        assert_eq!(rows[0]["input"], Value::Null);
    }

    #[test]
    fn a_missing_class_is_named() {
        let t = template(graph(), &[]).unwrap();
        let installed = json!({"KSampler": {}, "CheckpointLoaderSimple": {}, "EmptyLatentImage": {}, "SaveImage": {}});
        assert_eq!(missing_classes(&t, &installed), ["CLIPTextEncode"]);
    }
}
