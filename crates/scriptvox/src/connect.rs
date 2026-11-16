//! `scriptvox connect <name> --url <base>` (section 6 row 25, E5; docs/specs/connect.md): rows for an OpenAI-compatible
//! image service, read from its `GET /v1/models`, or with `--workflow` one row for a ComfyUI graph.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::generators::{COMFYUI, DATA_FILE, OPENAI_IMAGES};
use serde_json::{json, Value};

pub struct Args {
    pub name: String,
    pub url: String,
    pub key_name: Option<String>,
    pub models: Vec<String>,
    /// A ComfyUI graph in API format: the row runs it, with the fields found by role.
    pub workflow: Option<std::path::PathBuf>,
    pub spec: Option<std::path::PathBuf>,
    /// `role=node.input`, over what is found.
    pub maps: Vec<String>,
    pub dry_run: bool,
    /// A text row for `research` and `write` (`docs/specs/write.md`), with the user's prices.
    pub text: bool,
    pub usd_in: Option<f64>,
    pub usd_out: Option<f64>,
}

/// (exit, what is wrong, the fix)
type Refusal = (Exit, String, String);

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

fn key_name_ok(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 32
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))
}

/// An image model, going by the words in its id. `--model` names any other.
fn looks_like_images(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    [
        "image",
        "dall-e",
        "dalle",
        "sdxl",
        "sd1",
        "sd3",
        "stable-diffusion",
        "flux",
        "kolors",
        "imagen",
    ]
    .iter()
    .any(|w| id.contains(w))
}

/// JSON from a service's `path`, and whether it took the key. With a key: tried with it, and the answer says so.
fn get_json(base: &str, path: &str, key_name: Option<&str>) -> Result<Value, Refusal> {
    let key = key_name.and_then(crate::keys::resolve).map(|(v, _)| v);
    let agent = crate::http_rows::agent_ca(30, None)
        .map_err(|e| (Exit::Error, e, "Check the CA file.".to_string()))?;
    let mut req = agent.get(&format!("{base}{path}"));
    if let Some(k) = &key {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|e| {
        (
            Exit::ProviderRetryable,
            format!("no answer from {base}: {e}"),
            "Start the service, or check --url.".to_string(),
        )
    })?;
    let code = resp.status().as_u16();
    if code == 401 || code == 403 {
        return Err(match (key_name, &key) {
            (None, _) => (
                Exit::NeedsInput,
                format!("{base} asks for a key"),
                "Give the key a name with `--key-name <name>`, then follow the next step."
                    .to_string(),
            ),
            (Some(n), None) => (
                Exit::NeedsInput,
                format!("{base} asks for a key and {n} has none"),
                format!(
                    "Run `scriptvox keys set {n}` in your terminal, then this same command again."
                ),
            ),
            (Some(n), Some(_)) => (
                Exit::NeedsInput,
                format!("{base} refused the {n} key (HTTP {code})"),
                format!("Run `scriptvox keys set {n}` in your terminal with the right key."),
            ),
        });
    }
    if code >= 400 {
        return Err((
            Exit::Usage,
            format!("{base}{path} answered HTTP {code}"),
            "Check --url: it is the base of the service, without /v1.".to_string(),
        ));
    }
    // ComfyUI's /object_info is larger than the default limit.
    resp.body_mut()
        .with_config()
        .limit(64 << 20)
        .read_to_string()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| {
            (
                Exit::Usage,
                format!("{base}{path} gave no JSON"),
                "Check --url.".to_string(),
            )
        })
}

/// The model ids a service lists.
fn list_models(base: &str, key_name: Option<&str>) -> Result<Vec<String>, Refusal> {
    let body = get_json(base, "/v1/models", key_name)?;
    let ids: Vec<String> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(String::from))
        .collect();
    if ids.is_empty() {
        return Err((
            Exit::Usage,
            format!("{base}/v1/models lists no models"),
            "Load a model in the service and run this again.".to_string(),
        ));
    }
    Ok(ids)
}

fn slug(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn block(row: &str, key: &str, model: &str, base: &str, host: &str) -> String {
    format!(
        "[[generator]]\nname = {}\ngateway = \"{OPENAI_IMAGES}\"\nkind = \"image.gen\"\nkey = {}\nt2v = {}\nbase = {}\noutput = \"inline\"\nhosts = [{}]\n",
        q(row), q(key), q(model), q(base), q(host)
    )
}

fn usage(why: impl Into<String>, fix: &str) -> Refusal {
    (Exit::Usage, why.into(), fix.to_string())
}

/// One row for a ComfyUI graph. The fields are found by role and the server must have every node class.
fn connect_comfy(
    a: &Args,
    file: &std::path::Path,
    base: &str,
    host: &str,
) -> Result<Envelope, Refusal> {
    const EXPORT: &str = "In ComfyUI use Workflow, Export (API), and give that file.";
    let bytes = std::fs::read(file).map_err(|e| {
        usage(
            format!("{}: {e}", file.display()),
            "Give the path of the graph file.",
        )
    })?;
    if bytes.len() > crate::comfy::MAX_GRAPH {
        return Err(usage(
            format!("{} is larger than 1 MB", file.display()),
            EXPORT,
        ));
    }
    let graph: Value = serde_json::from_slice(&bytes)
        .map_err(|e| usage(format!("{}: {e}", file.display()), EXPORT))?;
    let t = crate::comfy::template(graph, &a.maps).map_err(|e| usage(e, EXPORT))?;
    let installed = get_json(base, "/object_info", a.key_name.as_deref())?;
    let missing = crate::comfy::missing_classes(&t, &installed);
    if !missing.is_empty() {
        return Err(usage(
            format!("{base} does not have these nodes: {}", missing.join(", ")),
            "Install those custom nodes in ComfyUI and restart it, or use another graph.",
        ));
    }
    let (bad, checked, skipped) = crate::comfy::choices(&t, &installed);
    if !bad.is_empty() {
        // ponytail: the first 5 are named; the rest show on the next run.
        let shown = || bad.iter().take(5);
        let what: Vec<String> = shown()
            .map(|b| format!("node {} ({}) {} = {:?}", b.node, b.class, b.input, b.value))
            .collect();
        let near: Vec<String> = shown()
            .map(|b| {
                let n = crate::generators::nearest_names(
                    &b.value,
                    b.allowed.iter().map(String::as_str),
                    5,
                );
                if n.is_empty() {
                    format!("The server lists no value for {}.", b.input)
                } else {
                    format!("Nearest for {}: {}.", b.input, n.join(", "))
                }
            })
            .collect();
        return Err(usage(
            format!("{base} does not list these values: {}", what.join("; ")),
            &format!(
                "{} Pick one of the server's values in the graph, or install the missing model file on the server, then run this again.",
                near.join(" ")
            ),
        ));
    }
    let choices = json!({"checked": checked, "skipped": skipped});
    let existing = crate::generators::load().0;
    let old = existing.iter().find(|g| g.name == a.name);
    if old.is_some_and(|g| g.base.as_deref() != Some(base)) {
        return Err(usage(
            format!("a row named {} exists already", a.name),
            "Give another name.",
        ));
    }
    let kind = if t.video { "video.gen" } else { "image.gen" };
    let roles = json!({"prompt": t.prompt, "seed": t.seed, "width": t.width, "height": t.height, "length": t.length, "image": t.image, "output": t.output});
    if a.dry_run {
        let mut env = Envelope::ok("connect")
            .with("would_add", json!([a.name]))
            .with("kind", json!(kind))
            .with("roles", roles)
            .with("choices", choices)
            .with("base", json!(base));
        env.next
            .push("Run the same command without --dry-run to add this row.".into());
        return Ok(env);
    }
    let key = a.key_name.as_deref().unwrap_or("none");
    let mut block = format!(
        "[[generator]]\nname = {}\ngateway = \"{COMFYUI}\"\nkind = \"{kind}\"\nkey = {}\nt2v = {}\nbase = {}\noutput = \"inline\"\nhosts = [{}]\n",
        q(&a.name), q(key), q(&a.name), q(base), q(host)
    );
    if !t.seed.is_empty() {
        block += "takes_seed = true\n";
    }
    if t.video && t.image.is_some() {
        block += &format!("i2v = {}\n", q(&a.name));
    }
    let mut written = crate::keys::overlay_file();
    if old.is_none() {
        written = crate::generators::add(&block)
            .map_err(|e| usage(e.join("; "), "Correct these and run again."))?;
    }
    // After the row: `add` has checked the name that becomes this file's name.
    let path = crate::comfy::file_for(&a.name);
    let text = serde_json::to_string(&t).map_err(|e| usage(e.to_string(), "Check the graph."))?;
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text))
        .map_err(|e| {
            (
                Exit::Error,
                format!("{}: {e}", path.display()),
                "Check the config folder is writable.".to_string(),
            )
        })?;
    let mut env = Envelope::ok("connect")
        .with("added", json!([a.name]))
        .with("kind", json!(kind))
        .with("roles", roles)
        .with("choices", choices)
        .with("base", json!(base))
        .with("file", json!(written))
        .with("workflow", json!(path));
    env.next
        .push(format!("scriptvox providers test {} --max-spend 1", a.name));
    env.next.push(format!(
        "scriptvox generate --model {} --prompt \"...\" --max-spend 1",
        a.name
    ));
    Ok(env)
}

/// The host of a row's base, for the row's `hosts` list.
fn service_host(base: &str) -> String {
    base.split_once("://")
        .map_or("", |x| x.1)
        .split(['/', ':'])
        .next()
        .unwrap_or("")
        .to_string()
}

fn connect_spec(
    a: &Args,
    file: &std::path::Path,
    base: &str,
    host: &str,
) -> Result<Envelope, Refusal> {
    const SEE: &str = "Fix the file. docs/specs/connect.md lists every field.";
    let bytes = std::fs::read(file).map_err(|e| {
        usage(
            format!("{}: {e}", file.display()),
            "Give the path of the data file.",
        )
    })?;
    let spec = crate::data_row::parse(&bytes).map_err(|e| usage(e, SEE))?;
    let existing = crate::generators::load().0;
    let old = existing.iter().find(|g| g.name == a.name);
    if old.is_some_and(|g| g.base.as_deref() != Some(base)) {
        return Err(usage(
            format!("a row named {} exists already", a.name),
            "Give another name.",
        ));
    }
    let i2v = crate::data_row::uses(&spec, "image");
    if a.dry_run {
        let mut env = Envelope::ok("connect")
            .with("would_add", json!([a.name]))
            .with("kind", json!(spec.kind))
            .with("takes_image", json!(i2v))
            .with("base", json!(base));
        env.next
            .push("Run the same command without --dry-run to add this row.".into());
        return Ok(env);
    }
    let key = a.key_name.as_deref().unwrap_or("none");
    let mut block = format!(
        "[[generator]]\nname = {}\ngateway = \"{DATA_FILE}\"\nkind = \"{}\"\nkey = {}\nt2v = {}\nbase = {}\noutput = \"inline\"\nhosts = [{}]\n",
        q(&a.name), spec.kind, q(key), q(&a.name), q(base), q(host)
    );
    if i2v {
        block += &format!("i2v = {}\n", q(&a.name));
    }
    if crate::data_row::uses(&spec, "seed") {
        block += "takes_seed = true\n";
    }
    let mut written = crate::keys::overlay_file();
    if old.is_none() {
        written = crate::generators::add(&block)
            .map_err(|e| usage(e.join("; "), "Correct these and run again."))?;
    }
    // After the row: `add` has checked the name that becomes this file's name.
    let path = crate::data_row::file_for(&a.name);
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, &bytes))
        .map_err(|e| {
            (
                Exit::Error,
                format!("{}: {e}", path.display()),
                "Check the config folder is writable.".to_string(),
            )
        })?;
    let mut env = Envelope::ok("connect")
        .with("added", json!([a.name]))
        .with("kind", json!(spec.kind))
        .with("takes_image", json!(i2v))
        .with("base", json!(base))
        .with("file", json!(written))
        .with("spec", json!(path));
    env.next
        .push(format!("scriptvox providers test {} --max-spend 1", a.name));
    Ok(env)
}

fn connect(a: &Args) -> Result<Envelope, Refusal> {
    let base = a
        .url
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string();
    if !crate::generators::speech_base_ok(&base) {
        return Err((Exit::Usage, format!("{base} is not allowed"), "Use https://, or http:// only on 127.0.0.1 or localhost. A key must not cross a network in clear text.".to_string()));
    }
    if let Some(n) = &a.key_name {
        if !key_name_ok(n) {
            return Err((
                Exit::Usage,
                format!("key name {n:?} is not valid"),
                "Use 1 to 32 of a-z, 0-9, '_' or '-'.".to_string(),
            ));
        }
    }
    // A key goes only to the service it was made for: a built-in key, or one a row with another base uses, never to this URL.
    if let Some(n) = &a.key_name {
        let foreign = crate::keys::PROVIDERS.iter().any(|(p, _)| p == n)
            || crate::generators::load()
                .0
                .iter()
                .any(|g| &g.key == n && g.base.as_deref() != Some(&base));
        if foreign {
            return Err((
                Exit::Usage,
                format!("the key {n} belongs to another service, so it is not sent to {base}"),
                "Give this service its own key name with --key-name.".to_string(),
            ));
        }
    }
    // A new key name goes in first: `keys set` needs it, also when the listing below stops for the missing key.
    let new_key = a
        .key_name
        .as_deref()
        .filter(|n| crate::keys::env_var(n).is_none());
    if let (Some(n), false) = (new_key, a.dry_run) {
        let env = format!("{}_API_KEY", n.to_ascii_uppercase().replace('-', "_"));
        crate::generators::add(&format!("[[key]]\nname = {}\nenv = {}\n", q(n), q(&env))).map_err(
            |e| {
                (
                    Exit::Usage,
                    e.join("; "),
                    "Use another key name.".to_string(),
                )
            },
        )?;
    }
    if a.text {
        let listed = list_models(&base, a.key_name.as_deref())?;
        return crate::text_rows::connect(a, &base, listed);
    }
    if let Some(file) = &a.spec {
        return connect_spec(a, file, &base, &service_host(&base));
    }
    if let Some(file) = &a.workflow {
        return connect_comfy(a, file, &base, &service_host(&base));
    }
    let listed = list_models(&base, a.key_name.as_deref())?;
    let chosen: Vec<&String> = if a.models.is_empty() {
        listed.iter().filter(|m| looks_like_images(m)).collect()
    } else {
        for m in &a.models {
            if !listed.contains(m) {
                return Err((
                    Exit::Usage,
                    format!("{base} does not list {m}"),
                    format!(
                        "It lists: {}.",
                        listed
                            .iter()
                            .take(20)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        a.models.iter().collect()
    };
    if chosen.is_empty() {
        return Err((
            Exit::Usage,
            format!(
                "no image model among the {} that {base} lists",
                listed.len()
            ),
            format!(
                "Name it with --model: {}.",
                listed
                    .iter()
                    .take(20)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    let key = a.key_name.as_deref().unwrap_or("none");
    let host = base
        .split_once("://")
        .map_or("", |x| x.1)
        .split(['/', ':'])
        .next()
        .unwrap_or("")
        .to_string();
    let existing = crate::generators::load().0;
    let mut rows: Vec<(String, String)> = Vec::new();
    for m in &chosen {
        let row = if chosen.len() == 1 {
            a.name.clone()
        } else {
            format!("{}-{}", a.name, slug(m)).chars().take(48).collect()
        };
        rows.push((row, m.to_string()));
    }
    for (row, _) in &rows {
        if existing
            .iter()
            .any(|g| &g.name == row && g.base.as_deref() != Some(&base))
        {
            return Err((
                Exit::Usage,
                format!("a row named {row} exists already"),
                "Give another name.".to_string(),
            ));
        }
    }
    let names: Vec<&String> = rows.iter().map(|r| &r.0).collect();
    if a.dry_run {
        let mut env = Envelope::ok("connect")
            .with("would_add", json!(names))
            .with("listed", json!(listed))
            .with("base", json!(base));
        env.next
            .push("Run the same command without --dry-run to add these rows.".into());
        return Ok(env);
    }
    let mut file = crate::keys::overlay_file();
    for (row, model) in &rows {
        if existing.iter().any(|g| &g.name == row) {
            continue;
        }
        file = crate::generators::add(&block(row, key, model, &base, &host)).map_err(|e| {
            (
                Exit::Usage,
                e.join("; "),
                "Correct these and run again.".to_string(),
            )
        })?;
    }
    let mut env = Envelope::ok("connect")
        .with("added", json!(names))
        .with("listed", json!(listed))
        .with("base", json!(base))
        .with("file", json!(file));
    let first = names.first().map(|n| n.as_str()).unwrap_or_default();
    env.next
        .push(format!("scriptvox providers test {first} --max-spend 1"));
    env.next.push(format!(
        "scriptvox generate --model {first} --prompt \"...\" --max-spend 1"
    ));
    Ok(env)
}

pub fn run(a: Args) -> Envelope {
    connect(&a).unwrap_or_else(|(exit, why, fix)| Envelope::fail("connect", exit, why, Some(fix)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn image_models_are_told_apart_by_their_ids() {
        assert!(super::looks_like_images("flux.1-dev"));
        assert!(super::looks_like_images("gpt-image-1"));
        assert!(!super::looks_like_images("gpt-4o"));
    }
}
