//! `scriptvox models add <hf id, local file or fal endpoint>` (section 6 row 24, E4; docs/specs/models-add.md).
//! A row needs a real file to run: the revision, size and sha256 are pinned where the host publishes them. The licence
//! stays on the row and refuses nothing. A file format that can run code gets a warning.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::provision as p;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The families sd.cpp runs from one file. Others need separate VAE and text encoder files.
const RUNNABLE: [&str; 3] = ["sd1", "sd2", "sdxl"];
const WEIGHTS: [&str; 7] = [
    ".safetensors",
    ".gguf",
    ".ckpt",
    ".pt",
    ".pth",
    ".bin",
    ".pkl",
];
/// Enough for the header of any checkpoint seen so far. A longer header gives an unknown family: `--family` says it.
const HEAD: u64 = 4 << 20;
const OBSERVED: &str = "observed, not verified";

/// Where a local row's file came from, on the row itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    /// `huggingface` or `file`.
    pub origin: String,
    /// The model id on the host. Empty for a local file.
    #[serde(default)]
    pub id: String,
    /// The commit the file is pinned to. Empty for a local file.
    #[serde(default)]
    pub revision: String,
    /// The file on this machine.
    pub file: String,
    #[serde(default)]
    pub url: String,
    pub bytes: u64,
    #[serde(default)]
    pub sha256: String,
    /// `published` by the host and checked at download, or `observed, not verified`.
    pub hash: String,
    /// As the host states it, or `not published`.
    pub licence: String,
    /// sd1, sd2 or sdxl.
    pub family: String,
    pub format: String,
}

impl Source {
    /// Why this row's engine or file is not here, or None.
    pub fn missing(&self) -> Option<String> {
        if p::pin_file("sd-cli").is_none() {
            return Some("the sd.cpp engine (sd-cli) is not installed".into());
        }
        match std::fs::metadata(&self.file) {
            Ok(m) if m.len() == self.bytes => None,
            Ok(_) => Some(format!("{} changed size since it was added", self.file)),
            Err(_) => Some(format!("{} is not there", self.file)),
        }
    }

    /// The command that brings this row back.
    pub fn add_again(&self) -> String {
        let from = if self.origin == "huggingface" {
            self.id.as_str()
        } else {
            self.file.as_str()
        };
        format!(
            "Run `scriptvox models add {from} --family {}` again, with the same --name.",
            self.family
        )
    }
}

pub struct Args {
    pub source: String,
    pub name: Option<String>,
    pub file: Option<String>,
    pub family: Option<String>,
    pub kind: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, PartialEq)]
enum From_ {
    Hf(String),
    Path(PathBuf),
    Fal(String),
}

/// (exit, what is wrong, the fix)
type Refusal = (Exit, String, String);

fn usage(why: impl Into<String>, fix: impl Into<String>) -> Refusal {
    (Exit::Usage, why.into(), fix.into())
}

fn is_hf_id(s: &str) -> bool {
    let mut parts = s.split('/');
    let ok = |p: Option<&str>| {
        p.is_some_and(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

fn parse_source(s: &str) -> Result<From_, Refusal> {
    let s = s.trim().trim_end_matches('/');
    for pre in ["https://huggingface.co/", "hf:"] {
        if let Some(rest) = s.strip_prefix(pre) {
            let id = rest.split('/').take(2).collect::<Vec<_>>().join("/");
            return if is_hf_id(&id) {
                Ok(From_::Hf(id))
            } else {
                Err(usage(
                    format!("{s} holds no Hugging Face id"),
                    "Use the form org/model.",
                ))
            };
        }
    }
    for pre in ["https://fal.ai/models/", "fal:"] {
        if let Some(rest) = s.strip_prefix(pre) {
            return Ok(From_::Fal(rest.to_string()));
        }
    }
    if s.starts_with("fal-ai/") {
        return Ok(From_::Fal(s.to_string()));
    }
    let path = Path::new(s);
    let weight = WEIGHTS.iter().any(|w| s.to_ascii_lowercase().ends_with(w));
    if path.is_file() || s.starts_with(['/', '.']) || weight {
        return Ok(From_::Path(path.to_path_buf()));
    }
    if is_hf_id(s) {
        return Ok(From_::Hf(s.to_string()));
    }
    Err(usage(
        format!("{s} is not a Hugging Face id, a local file or a fal endpoint"),
        "Examples: `org/model`, `./model.safetensors`, `fal-ai/some-model`.",
    ))
}

/// What the file is. A pickle name comes first: first bytes must not hide a file that can run code.
fn format_of(head: &[u8], file: &str) -> &'static str {
    let low = file.to_ascii_lowercase();
    if [".ckpt", ".pt", ".pth", ".bin", ".pkl"]
        .iter()
        .any(|e| low.ends_with(e))
        || head.starts_with(b"PK\x03\x04")
        || head.first() == Some(&0x80)
    {
        return "pickle";
    }
    if head.starts_with(b"GGUF") {
        return "gguf";
    }
    if header_of(head).is_some() {
        return "safetensors";
    }
    "unknown"
}

/// The JSON header of a safetensors file. Only JSON is read, nothing in the file can run.
fn header_of(head: &[u8]) -> Option<serde_json::Map<String, Value>> {
    let n = u64::from_le_bytes(head.get(..8)?.try_into().ok()?) as usize;
    match serde_json::from_slice(head.get(8..8usize.checked_add(n)?)?).ok()? {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

/// The family from the names in a safetensors header. Only JSON is read, nothing in the file can run.
fn family_of(head: &[u8]) -> Option<&'static str> {
    let keys = header_of(head)?;
    let starts = |p: &str| keys.keys().any(|k| k.starts_with(p));
    let inside = |p: &str| keys.keys().any(|k| k.contains(p));
    let whole = starts("first_stage_model.");
    Some(if inside("lora_") {
        "lora"
    } else if inside("double_blocks.") {
        "flux"
    } else if inside("joint_blocks.") {
        "sd3"
    } else if whole && starts("conditioner.embedders.1.") {
        "sdxl"
    } else if whole && starts("cond_stage_model.transformer.") {
        "sd1"
    } else if whole && starts("cond_stage_model.model.") {
        "sd2"
    } else {
        "other"
    })
}

/// Stops a family this install cannot run, and says what is missing.
fn runnable_family(
    found: Option<&str>,
    given: Option<&str>,
    format: &str,
) -> Result<String, Refusal> {
    let needs = "Use one file that holds a whole SD 1.x, SD 2.x or SDXL model, or use a service: `scriptvox models add fal-ai/<endpoint>`.";
    let family = match (given, found) {
        (Some(f), _) | (None, Some(f)) => f,
        (None, None) => {
            return Err(usage(
                format!("scriptvox cannot read the model family from a {format} file"),
                "Say which one: `--family sd1`, `--family sd2` or `--family sdxl`.",
            ));
        }
    };
    if RUNNABLE.contains(&family) {
        return Ok(family.to_string());
    }
    let (why, missing) = match family {
        "lora" => ("a LoRA, not a whole model", "a base model"),
        "flux" | "sd3" => (
            "a family that sd.cpp runs only with separate files",
            "the VAE and text encoder files, which scriptvox fetches only for `setup image-local`",
        ),
        "other" => (
            "not a whole checkpoint that scriptvox knows",
            "one file with the model, the VAE and the text encoder",
        ),
        f => {
            return Err(usage(
                format!("family {f} is not known"),
                "Use sd1, sd2 or sdxl.",
            ))
        }
    };
    Err((
        Exit::NeedsInput,
        format!("no local engine for this file: it is {why} ({family}). Missing: {missing}"),
        needs.to_string(),
    ))
}

fn hf_base() -> String {
    std::env::var("SCRIPTVOX_HF_BASE")
        .ok()
        .filter(|b| crate::generators::loopback_base_ok(b))
        .unwrap_or_else(|| "https://huggingface.co".into())
}

fn hf_call(url: &str, range: bool) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let once = || {
        let agent = crate::http_rows::agent_ca(60, None).expect("no CA file to read");
        let mut req = agent.get(url);
        if url.starts_with("https://huggingface.co/") {
            if let Some((token, _)) = crate::keys::resolve("hf") {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
        }
        if range {
            req = req.header("Range", format!("bytes=0-{}", HEAD - 1));
        }
        req.call()
    };
    let resp = match once() {
        Err(_) if crate::http_rows::force_ipv4() => once(),
        r => r,
    }?;
    match resp.status().as_u16() {
        c @ 400.. => Err(ureq::Error::StatusCode(c)),
        _ => Ok(resp),
    }
}

/// The file's first bytes, without fetching the rest.
fn hf_head(url: &str, page: &str) -> Result<Vec<u8>, Refusal> {
    let mut resp = hf_call(url, true).map_err(|e| match e {
        ureq::Error::StatusCode(401 | 403) => {
            let text = p::gated(page, crate::keys::resolve("hf").is_some());
            let (_, action) = text
                .strip_prefix(p::GATED)
                .and_then(|r| r.split_once(' '))
                .unwrap_or(("", ""));
            (Exit::NeedsInput, text.clone(), action.to_string())
        }
        e => (
            Exit::ProviderRetryable,
            format!("cannot read {url}: {e}"),
            "Check the network and run again.".to_string(),
        ),
    })?;
    let mut head = Vec::new();
    resp.body_mut()
        .with_config()
        .reader()
        .take(HEAD)
        .read_to_end(&mut head)
        .map_err(|e| {
            (
                Exit::ProviderRetryable,
                e.to_string(),
                "Run again.".to_string(),
            )
        })?;
    Ok(head)
}

struct Sib {
    file: String,
    bytes: u64,
    sha256: Option<String>,
}

fn siblings(info: &Value) -> Vec<Sib> {
    info["siblings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some(Sib {
                file: s["rfilename"].as_str()?.to_string(),
                bytes: s["size"].as_u64().or(s["lfs"]["size"].as_u64())?,
                sha256: s["lfs"]["sha256"].as_str().map(String::from),
            })
        })
        .collect()
}

/// The one file to fetch: the named one, else the only whole-file candidate at the top of the repo.
fn pick_file(all: Vec<Sib>, want: Option<&str>, id: &str) -> Result<Sib, Refusal> {
    let weight = |f: &str| WEIGHTS.iter().any(|w| f.ends_with(w));
    let top = |all: &[Sib]| {
        all.iter()
            .filter(|s| !s.file.contains('/') && weight(&s.file))
            .map(|s| format!("{} ({} MB)", s.file, s.bytes / 1_000_000))
            .collect::<Vec<_>>()
    };
    if let Some(w) = want {
        let list = top(&all);
        return all.into_iter().find(|s| s.file == w).ok_or_else(|| {
            usage(
                format!("{id} has no file {w}"),
                format!("Its model files: {}.", list.join(", ")),
            )
        });
    }
    let list = top(&all);
    let mut c: Vec<Sib> = all
        .into_iter()
        .filter(|s| !s.file.contains('/') && weight(&s.file))
        .collect();
    if c.iter()
        .any(|s| s.file.ends_with(".safetensors") || s.file.ends_with(".gguf"))
    {
        c.retain(|s| s.file.ends_with(".safetensors") || s.file.ends_with(".gguf"));
    }
    match c.len() {
        1 => Ok(c.remove(0)),
        0 => Err((Exit::NeedsInput, format!("{id} has no single model file at its top level (only folders of parts). Missing: one file with the whole model"), "Use a repo that has one .safetensors or .gguf file, or a fal endpoint.".to_string())),
        _ => Err(usage(format!("{id} has more than one model file"), format!("Pick one with --file: {}.", list.join(", ")))),
    }
}

/// (the row's source, the file's first bytes)
fn from_hf(id: &str, a: &Args) -> Result<(Source, Vec<u8>), Refusal> {
    let base = hf_base();
    let page = format!("https://huggingface.co/{id}");
    let mut resp =
        hf_call(&format!("{base}/api/models/{id}?blobs=true"), false).map_err(|e| match e {
            ureq::Error::StatusCode(401 | 403 | 404) => usage(
                format!("no public model {id} on Hugging Face"),
                "Check the id on the model's page. For a private model, run `scriptvox keys set hf`.",
            ),
            e => (
                Exit::ProviderRetryable,
                format!("cannot reach Hugging Face: {e}"),
                "Check the network and run again.".to_string(),
            ),
        })?;
    let info: Value = resp
        .body_mut()
        .read_to_string()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| {
            (
                Exit::ProviderRetryable,
                "Hugging Face gave no readable answer".to_string(),
                "Run again.".to_string(),
            )
        })?;
    let revision = info["sha"]
        .as_str()
        .ok_or_else(|| usage(format!("{id} has no revision"), "Check the model's page."))?
        .to_string();
    let chosen = pick_file(siblings(&info), a.file.as_deref(), id)?;
    let url = format!(
        "{base}/{id}/resolve/{revision}/{}",
        crate::http_rows::enc_path(&chosen.file)
    );
    let head = hf_head(&url, &page)?;
    let licence = info["cardData"]["license"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            info["tags"]
                .as_array()?
                .iter()
                .find_map(|t| t.as_str()?.strip_prefix("license:").map(String::from))
        })
        .unwrap_or_else(|| "not published".into());
    let hash = if chosen.sha256.is_some() {
        "published"
    } else {
        OBSERVED
    };
    Ok((
        Source {
            origin: "huggingface".into(),
            id: id.into(),
            revision,
            file: chosen.file.clone(),
            url,
            bytes: chosen.bytes,
            sha256: chosen.sha256.unwrap_or_default(),
            hash: hash.into(),
            licence,
            family: String::new(),
            format: String::new(),
        },
        head,
    ))
}

fn from_path(path: &Path, dry_run: bool) -> Result<(Source, Vec<u8>), Refusal> {
    let abs = std::fs::canonicalize(path).map_err(|e| {
        usage(
            format!("cannot read {}: {e}", path.display()),
            "Give the path of one model file.",
        )
    })?;
    let meta = std::fs::metadata(&abs)
        .map_err(|e| usage(e.to_string(), "Give the path of one model file."))?;
    if !meta.is_file() {
        return Err(usage(
            format!("{} is a folder", abs.display()),
            "Give the path of one model file.",
        ));
    }
    let mut head = Vec::new();
    std::fs::File::open(&abs)
        .and_then(|f| f.take(HEAD).read_to_end(&mut head))
        .map_err(|e| usage(e.to_string(), "Check that the file can be read."))?;
    // ponytail: a dry run does not hash; a 7 GB file costs a minute, and nothing is recorded.
    let sha256 = if dry_run {
        String::new()
    } else {
        p::sha256_of(&abs).map_err(|e| usage(e.to_string(), "Check that the file can be read."))?
    };
    Ok((
        Source {
            origin: "file".into(),
            id: String::new(),
            revision: String::new(),
            file: abs.display().to_string(),
            url: String::new(),
            bytes: meta.len(),
            sha256,
            hash: OBSERVED.into(),
            licence: "not published".into(),
            family: String::new(),
            format: String::new(),
        },
        head,
    ))
}

fn slug(s: &str) -> String {
    let stem = s.rsplit('/').next().unwrap_or(s);
    let stem = WEIGHTS
        .iter()
        .find_map(|w| stem.strip_suffix(w))
        .unwrap_or(stem);
    let t: String = stem
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    t.trim_matches('-').chars().take(48).collect()
}

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// What the licence name alone settles. Anything else stays not confirmed, and refuses nothing.
fn business(licence: &str) -> Option<bool> {
    let l = licence.to_ascii_lowercase();
    if l.contains("non-commercial") || l.contains("noncommercial") || l.contains("-nc") {
        Some(false)
    } else if [
        "apache-2.0",
        "mit",
        "bsd-2-clause",
        "bsd-3-clause",
        "cc0-1.0",
        "cc-by-4.0",
    ]
    .contains(&l.as_str())
    {
        Some(true)
    } else {
        None
    }
}

fn local_block(name: &str, s: &Source) -> String {
    let note = if s.licence == "not published" {
        "No licence published with this file: read its page before business use.".to_string()
    } else {
        format!("Licence as published on the model page: {}.", s.licence)
    };
    let commercial = business(&s.licence).map_or(String::new(), |b| format!("commercial = {b}, "));
    let attribution = s.licence.to_ascii_lowercase().starts_with("cc-by");
    let mut b = format!(
        "[[generator]]\nname = {}\ngateway = \"local\"\nkind = \"image.gen\"\nkey = \"none\"\nt2v = \"local\"\nprice = \"exact\"\noutput = \"inline\"\nhosts = [\"local\"]\nterms = {{ {commercial}attribution_required = {attribution}, redistribution = false, note = {} }}\n\n[generator.source]\n",
        q(name),
        q(&note)
    );
    for (k, v) in [
        ("origin", &s.origin),
        ("id", &s.id),
        ("revision", &s.revision),
        ("file", &s.file),
        ("url", &s.url),
        ("sha256", &s.sha256),
        ("hash", &s.hash),
        ("licence", &s.licence),
        ("family", &s.family),
        ("format", &s.format),
    ] {
        b.push_str(&format!("{k} = {}\n", q(v)));
    }
    b.push_str(&format!("bytes = {}\n", s.bytes));
    b
}

/// (kind, is the endpoint image to video)
fn fal_kind(endpoint: &str, given: Option<&str>) -> Result<(String, bool), Refusal> {
    let i2v = endpoint.contains("image-to-video") || endpoint.ends_with("/i2v");
    if let Some(k) = given {
        return Ok((k.to_string(), i2v));
    }
    let has = |w: &str| endpoint.contains(w);
    let kind = if has("video") || endpoint.ends_with("/t2v") || i2v {
        "video.gen"
    } else if has("speech") || has("tts") {
        "tts"
    } else if has("music") {
        "music.gen"
    } else if has("image") {
        "image.gen"
    } else {
        return Err(usage(format!("scriptvox cannot tell what {endpoint} makes"), "Say it: `--kind video.gen`, `image.gen`, `music.gen`, `tts` or another kind from `scriptvox providers`."));
    };
    Ok((kind.to_string(), i2v))
}

fn add_fal(endpoint: &str, a: &Args) -> Result<Envelope, Refusal> {
    let (kind, i2v) = fal_kind(endpoint, a.kind.as_deref())?;
    let name = a.name.clone().unwrap_or_else(|| {
        slug(
            endpoint
                .trim_start_matches("fal-ai/")
                .replace('/', "-")
                .as_str(),
        )
    });
    let field = if i2v { "i2v" } else { "t2v" };
    let block = format!(
        "[[generator]]\nname = {}\ngateway = \"fal\"\nkind = {}\n{field} = {}\n",
        q(&name),
        q(&kind),
        q(endpoint)
    );
    let page = format!("https://fal.ai/models/{endpoint}");
    if a.dry_run {
        let mut env = Envelope::ok("models").with("would_add", json!({"name": name, "gateway": "fal", "kind": kind, "endpoint": endpoint, "page": page}));
        env.next
            .push("Run the same command without --dry-run to add the row.".into());
        return Ok(env);
    }
    let file = crate::generators::add(&block).map_err(|e| {
        usage(
            e.join("; "),
            "Correct these and run again; nothing was written.",
        )
    })?;
    let mut env = Envelope::ok("models")
        .with("added", json!(name))
        .with("gateway", json!("fal"))
        .with("kind", json!(kind))
        .with("page", json!(page))
        .with("file", json!(file));
    env.next.push(format!(
        "scriptvox generate --model {name} --prompt \"...\" --max-spend 1"
    ));
    env.next.push(format!(
        "Its price and terms are on {page}. Write them into {} to run it without --max-spend.",
        file.display()
    ));
    Ok(env)
}

fn add_local(mut s: Source, head: Vec<u8>, a: &Args, what: &str) -> Result<Envelope, Refusal> {
    let name = a.name.clone().unwrap_or_else(|| slug(what));
    if !crate::generators::name_ok(&name) {
        return Err(usage(
            format!("{name:?} is not a row name"),
            "Give --name with 1 to 48 of a-z, 0-9, '.', '_' or '-'.",
        ));
    }
    s.format = format_of(&head, &s.file).to_string();
    let found = (s.format == "safetensors")
        .then(|| family_of(&head))
        .flatten();
    s.family = runnable_family(found, a.family.as_deref(), &s.format)?;
    let warn = (s.format == "pickle").then(|| json!({
        "gate": "format", "status": "warn",
        "detail": format!("{} is a pickle file. A pickle file can run code when it is opened.", what),
        "fix": "Use the .safetensors or .gguf form of this model if there is one, and keep this file only if you trust its source.",
    }));
    let same = crate::generators::load()
        .0
        .into_iter()
        .find(|g| g.name == name);
    if let Some(g) = &same {
        if !g.source.as_ref().is_some_and(|o| {
            o.origin == s.origin && o.id == s.id && (o.url == s.url || o.file == s.file)
        }) {
            return Err(usage(
                format!("a row named {name} exists already"),
                "Give another name with --name.",
            ));
        }
    }
    let Some(engine) = p::pin("sd-cli") else {
        return Err((
            Exit::NeedsInput,
            format!(
                "no sd.cpp build is pinned for this machine ({})",
                p::target()
            ),
            "Use a service row: `scriptvox models add fal-ai/<endpoint>`.".to_string(),
        ));
    };
    let model_pin = (s.origin == "huggingface").then(|| p::Pin {
        name: name.clone(),
        version: s.revision.clone(),
        target: "any".into(),
        url: s.url.clone(),
        sha256: s.sha256.clone(),
        size: s.bytes,
        licence: s.licence.clone(),
        strip: 0,
        bins: vec![s.file.rsplit('/').next().unwrap_or(&s.file).to_string()],
        raw: true,
        mirror: String::new(),
    });
    if a.dry_run {
        let pins: Vec<p::Pin> = p::installed(&engine)
            .is_none()
            .then_some(engine)
            .into_iter()
            .chain(model_pin)
            .collect();
        let mut env = crate::doctor::setup_dry_run(&pins)
            .with("would_add", json!(name))
            .with("source", json!(s));
        env.command = "models".into();
        env.findings.extend(warn);
        env.next
            .push("Run the same command without --dry-run to download and add it.".into());
        return Ok(env);
    }
    let say = |m: &str| eprintln!("scriptvox: {m}");
    let net = |e: String| {
        let env = crate::doctor::setup_failure(e);
        (
            env.exit,
            env.error
                .as_ref()
                .map_or(String::new(), |x| x.message.clone()),
            env.error.and_then(|x| x.fix).unwrap_or_default(),
        )
    };
    let bin = p::install(&engine, &say).map_err(net)?.remove(0);
    p::sd_cli_starts(&bin).map_err(|e| {
        (
            Exit::NeedsInput,
            format!("the pinned sd.cpp build does not start on this machine: {e}"),
            "Use a service row: `scriptvox models add fal-ai/<endpoint>`.".to_string(),
        )
    })?;
    if let Some(pin) = &model_pin {
        let got = p::install(pin, &say).map_err(net)?.remove(0);
        s.file = got.display().to_string();
        if s.sha256.is_empty() {
            s.sha256 =
                p::sha256_of(&got).map_err(|e| usage(e.to_string(), "Check the data folder."))?;
        }
    }
    let file = if same.is_some() {
        crate::keys::overlay_file()
    } else {
        crate::generators::add(&local_block(&name, &s)).map_err(|e| {
            usage(
                e.join("; "),
                "Correct these and run again; nothing was written.",
            )
        })?
    };
    let mut env = Envelope::ok("models")
        .with("added", json!(name))
        .with("gateway", json!("local"))
        .with("source", json!(s))
        .with("file", json!(file));
    env.findings.extend(warn);
    env.next.push(format!(
        "scriptvox generate --model {name} --prompt \"...\""
    ));
    Ok(env)
}

pub fn add(a: Args) -> Envelope {
    let done = parse_source(&a.source).and_then(|from| match from {
        From_::Fal(e) => add_fal(&e, &a),
        From_::Hf(id) => {
            let (s, head) = from_hf(&id, &a)?;
            let label = a.file.clone().unwrap_or_else(|| id.clone());
            add_local(s, head, &a, &label)
        }
        From_::Path(path) => {
            let (s, head) = from_path(&path, a.dry_run)?;
            add_local(s, head, &a, &path.display().to_string())
        }
    });
    done.unwrap_or_else(|(exit, why, fix)| Envelope::fail("models", exit, why, Some(fix)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn safetensors(keys: &[&str]) -> Vec<u8> {
        let m: serde_json::Map<String, Value> =
            keys.iter().map(|k| (k.to_string(), json!({}))).collect();
        let body = serde_json::to_vec(&m).unwrap();
        let mut h = (body.len() as u64).to_le_bytes().to_vec();
        h.extend(body);
        h
    }

    #[test]
    fn the_source_is_read_by_its_form() {
        assert_eq!(
            parse_source("org/model").unwrap(),
            From_::Hf("org/model".into())
        );
        assert_eq!(
            parse_source("https://huggingface.co/org/model/tree/main").unwrap(),
            From_::Hf("org/model".into())
        );
        assert_eq!(
            parse_source("fal-ai/x/text-to-video").unwrap(),
            From_::Fal("fal-ai/x/text-to-video".into())
        );
        assert_eq!(
            parse_source("https://fal.ai/models/fal-ai/x").unwrap(),
            From_::Fal("fal-ai/x".into())
        );
        assert_eq!(
            parse_source("./a.safetensors").unwrap(),
            From_::Path("./a.safetensors".into())
        );
        assert!(parse_source("a b c").is_err());
    }

    #[test]
    fn the_first_bytes_decide_the_format_and_the_header_the_family() {
        assert_eq!(
            format_of(b"PK\x03\x04aaaa", "m.safetensors"),
            "pickle",
            "a name does not make a safe file"
        );
        assert_eq!(format_of(&[0x80, 2, 3], "m"), "pickle");
        assert_eq!(format_of(b"GGUF\x03", "m"), "gguf");
        // The name wins for pickle: first bytes cannot hide a file that runs code.
        assert_eq!(format_of(b"12345678{not json", "m.ckpt"), "pickle");
        assert_eq!(format_of(b"GGUF\x03", "m.ckpt"), "pickle");
        assert_eq!(format_of(b"12345678{not json", "m"), "unknown");
        let sd1 = safetensors(&[
            "first_stage_model.a",
            "cond_stage_model.transformer.b",
            "model.diffusion_model.c",
        ]);
        assert_eq!(format_of(&sd1, "m"), "safetensors");
        assert_eq!(family_of(&sd1), Some("sd1"));
        assert_eq!(
            family_of(&safetensors(&[
                "first_stage_model.a",
                "conditioner.embedders.1.b"
            ])),
            Some("sdxl")
        );
        assert_eq!(
            family_of(&safetensors(&["model.diffusion_model.double_blocks.0.x"])),
            Some("flux")
        );
    }

    #[test]
    fn a_family_with_no_engine_names_what_is_missing() {
        let e = runnable_family(Some("flux"), None, "safetensors").unwrap_err();
        assert_eq!(e.0, Exit::NeedsInput);
        assert!(e.1.contains("Missing: the VAE"), "{e:?}");
        assert!(runnable_family(None, None, "pickle")
            .unwrap_err()
            .2
            .contains("--family"));
        assert_eq!(
            runnable_family(None, Some("sdxl"), "pickle").unwrap(),
            "sdxl"
        );
    }

    #[test]
    fn the_licence_name_settles_only_what_it_says() {
        assert_eq!(business("apache-2.0"), Some(true));
        assert_eq!(business("cc-by-nc-4.0"), Some(false));
        assert_eq!(business("creativeml-openrail-m"), None);
    }

    #[test]
    fn a_fal_endpoint_says_what_it_makes_or_asks() {
        assert_eq!(
            fal_kind("fal-ai/x/image-to-video", None).unwrap(),
            ("video.gen".into(), true)
        );
        assert_eq!(
            fal_kind("fal-ai/x/text-to-image", None).unwrap(),
            ("image.gen".into(), false)
        );
        assert!(fal_kind("fal-ai/x", None).is_err());
        assert_eq!(fal_kind("fal-ai/x", Some("tts")).unwrap().0, "tts");
    }
}
