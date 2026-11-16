//! Generative video rows (phase 2, P2-001): `registry/generators.toml`, checked here. A row is data: a gateway
//! (fal, selfhost), the model endpoints, what it accepts, and a price read from the provider's own page.
//! The job runner (submit, poll, download) is P2-002; this module only loads, validates and prices rows.

use crate::http_rows::Terms;
use serde::{Deserialize, Serialize};

const REGISTRY: &str = include_str!("../../../registry/generators.toml");
/// `selfhost`: a self-hosted queue that speaks the fal protocol at the row's own `base`.
const GATEWAYS: [&str; 7] = [
    "fal",
    "selfhost",
    "openai-speech",
    "local",
    OPENAI_IMAGES,
    COMFYUI,
    DATA_FILE,
];
/// `price` of a user's row whose price nobody has read yet.
pub const UNKNOWN: &str = "unknown";

/// A row on a speech service with the OpenAI speech API (`POST <base>/v1/audio/speech`), such as a local Kokoro or
/// Piper server (G049). `t2v` is the model name it asks for; `key = "none"` sends no Authorization header.
pub const OPENAI_SPEECH: &str = "openai-speech";
/// A row on an OpenAI-compatible image service (`POST <base>/v1/images/generations`), added by `scriptvox connect`.
/// `t2v` is the model name it asks for.
pub const OPENAI_IMAGES: &str = "openai-images";
/// A ComfyUI server (`connect --workflow`): the row's graph is in `<config>/comfy/<row>.json`.
pub const COMFYUI: &str = "comfyui";
/// A service described by a data file (`connect --spec`): the row's file is in `<config>/spec/<row>.json`.
pub const DATA_FILE: &str = "data-file";
/// What a row makes. Every result is one file behind a URL: video, image, audio, or the JSON of a transcript.
pub const KINDS: [&str; 11] = [
    "video.gen",
    "image.gen",
    "music.gen",
    "sfx.gen",
    "tts",
    "stt",
    "upscale",
    "interpolate",
    "matte",
    "stems",
    "lipsync",
];
/// The frame shapes scriptvox renders; a visual row must offer at least one.
const ASPECTS: [&str; 3] = ["9:16", "16:9", "1:1"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    /// Measured seconds for one clip on a free row: an agent chooses a row by it before it starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes_s: Option<f64>,
    /// An image row that takes reference pictures (`image_urls`): the brief's `character`.
    #[serde(default)]
    pub takes_reference: bool,
    pub gateway: String,
    pub kind: String,
    pub key: String,
    /// Text to video endpoint, image to video endpoint (at least one).
    pub t2v: Option<String>,
    pub i2v: Option<String>,
    pub durations: Vec<f64>,
    pub aspects: Vec<String>,
    pub usd_per_s: f64,
    /// The price of one picture, on an image row (a picture has no length). `usd_per_s` stays 0 there. On a speech to text
    /// row `usd_per_s` is per second of the audio sent.
    #[serde(default)]
    pub usd_each: f64,
    /// Price shapes for rows that do not bill by output length alone (G014). The smallest charge of one call.
    #[serde(default)]
    pub usd_min: f64,
    /// Per second of the source sent, such as a video to upscale.
    #[serde(default)]
    pub usd_per_input_s: f64,
    /// Per reference picture sent.
    #[serde(default)]
    pub usd_per_reference: f64,
    /// On top of `usd_per_s` when the row's `extra` turns sound on with `generate_audio = true`.
    #[serde(default)]
    pub usd_per_s_sound: f64,
    /// Nobody has run this row against the real service yet: `service` says so.
    #[serde(default)]
    pub unverified: bool,
    /// "exact" or "base" (see the registry header).
    pub price: String,
    pub price_source: String,
    pub price_checked: String,
    /// Where the file's URL sits in the result JSON, as an `http_rows::get` path. `inline`: the result JSON is the file.
    pub output: String,
    pub hosts: Vec<String>,
    pub terms: Terms,
    /// The model takes `duration` as a string ("5"), not a number.
    #[serde(default)]
    pub duration_string: bool,
    /// Goes after the seconds in the request (Luma takes "5s"). The value is then a string.
    #[serde(default)]
    pub duration_suffix: Option<String>,
    /// Fixed inputs sent with every request (for example `generate_audio = false`: scriptvox adds its own voice).
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// The queue URL for a self-hosted gateway (https). Hosted gateways use their own.
    #[serde(default)]
    pub base: Option<String>,
    /// A CA name: this row's requests trust only `<config>/ca/<name>.pem` (a private PKI), not the system roots.
    #[serde(default)]
    pub ca: Option<String>,
    /// The provider took the model away: when and why, in plain words (G082). The row stays listed, never runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<String>,
    /// The row to use instead of a retired one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instead: Option<String>,
    /// The row takes `seed` and gives the same picture for the same seed and input (checked on this row, G075).
    #[serde(default)]
    pub takes_seed: bool,
    /// The request field for a last-frame picture (G074): the clip ends on it. Only on a row checked against the
    /// service's own docs; none of the built-in rows is yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes_last_frame: Option<String>,
    /// The request field for the first picture, when the row does not call it `image_url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_field: Option<String>,
    /// The request field for the language, when the row does not call it `language` (ElevenLabs: `language_code`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang_field: Option<String>,
    /// The file behind a row added with `models add`: where it came from, its revision, size, hash and licence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::model_add::Source>,
}

/// Why a row cannot run. `kind` is a `needs_input` kind (`secret`, `dependency`), or `retired`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhyNot {
    pub kind: &'static str,
    pub why: String,
    pub fix: String,
}

/// What a caller asks a row for; each kind reads only the fields it needs.
#[derive(Debug, Clone, Default)]
pub struct GenInput {
    pub prompt: String,
    pub seconds: f64,
    pub aspect: String,
    /// Data URI or URL of an image (image to video, image to image).
    pub image: Option<String>,
    /// Data URI of audio (speech to text input, or a voice sample to clone).
    pub audio: Option<String>,
    /// Data URI of a video (the input of an upscale row).
    pub video: Option<String>,
    pub voice: Option<String>,
    pub lang: Option<String>,
}

#[derive(Deserialize)]
struct Registry {
    // An overlay may hold only `[[key]]` blocks.
    #[serde(default)]
    generator: Vec<Generator>,
}

/// Parses rows from TOML text.
pub fn parse(text: &str) -> Result<Vec<Generator>, String> {
    toml::from_str::<Registry>(text)
        .map(|r| r.generator)
        .map_err(|e| format!("generators.toml: {e}"))
}

/// The `[[...]]` blocks of a TOML text, each as its own text, so one mistake costs only its own block.
// ponytail: a line that starts with `[[` inside a multi-line string or array splits there; that block then reports an error.
pub fn blocks(text: &str) -> Vec<&str> {
    let (mut at, mut pos) = (vec![0], 0);
    for l in text.split_inclusive('\n') {
        if pos > 0 && l.trim_start().starts_with("[[") {
            at.push(pos);
        }
        pos += l.len();
    }
    at.push(text.len());
    at.windows(2).map(|w| &text[w[0]..w[1]]).collect()
}

/// The block opens with `[[kind]]`.
pub fn block_is(block: &str, kind: &str) -> bool {
    block
        .trim_start()
        .strip_prefix("[[")
        .and_then(|r| r.split_once("]]"))
        .is_some_and(|(n, _)| n.trim() == kind)
}

/// The tables of the `[[generator]]` blocks in `text`, with one line for each block that is not TOML.
fn tables(text: &str, file: &str) -> (Vec<toml::Table>, Vec<String>) {
    let (mut good, mut bad) = (Vec::new(), Vec::new());
    for (i, block) in blocks(text).into_iter().enumerate() {
        if block_is(block, "key") {
            continue;
        }
        match toml::from_str::<toml::Table>(block) {
            Ok(mut t) => {
                if let Some(toml::Value::Array(rows)) = t.remove("generator") {
                    good.extend(rows.into_iter().filter_map(|r| match r {
                        toml::Value::Table(t) => Some(t),
                        _ => None,
                    }));
                }
            }
            Err(e) => {
                let name = block.lines().find_map(|l| {
                    let v = l
                        .trim()
                        .strip_prefix("name")?
                        .trim_start()
                        .strip_prefix('=')?;
                    Some(v.trim().trim_matches('"').to_string())
                });
                bad.push(format!(
                    "generator {}: {} (in {file}); correct or remove this block",
                    name.unwrap_or_else(|| format!("block {}", i + 1)),
                    e.message()
                ));
            }
        }
    }
    (good, bad)
}

/// Adds the blocks of `text` to `rows`. A known name replaces only the fields the block sets, so `name` and `base` are
/// enough for a built-in row. A block that does not make a row leaves `rows` as it was and gives one line in `errs`.
fn overlay(
    text: &str,
    file: &str,
    stock: &[toml::Table],
    rows: &mut Vec<Generator>,
    errs: &mut Vec<String>,
) {
    let (blocks, bad) = tables(text, file);
    errs.extend(bad);
    for block in blocks {
        let name = block
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string();
        let mut row = stock
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
            .cloned()
            .unwrap_or_default();
        let is_new = row.is_empty();
        for (k, v) in block {
            row.insert(k, v);
        }
        if is_new {
            // A new row of the user's: the fields only the user can know, all named at once; the rest get defaults.
            let missing: Vec<&str> = ["gateway", "kind"]
                .into_iter()
                .filter(|k| !row.contains_key(*k))
                .chain(
                    (!row.contains_key("t2v") && !row.contains_key("i2v")).then_some("t2v or i2v"),
                )
                .collect();
            if !missing.is_empty() {
                errs.push(format!(
                    "generator {name}: missing {} (in {file}); add them to this block",
                    missing.join(", ")
                ));
                continue;
            }
            fill_user_defaults(&mut row);
        }
        match toml::Value::Table(row).try_into::<Generator>() {
            Ok(r) => match rows.iter().position(|x| x.name == r.name) {
                Some(i) => rows[i] = r,
                None => rows.push(r),
            },
            Err(e) => errs.push(format!(
                "generator {name}: {} (in {file}); correct or remove this block",
                e.message()
            )),
        }
    }
}

/// A new row's price and terms are not known until someone reads them: the row says so, and `generate` needs
/// `--max-spend`. The built-in rows never take this route.
fn fill_user_defaults(row: &mut toml::Table) {
    let gateway = row
        .get("gateway")
        .and_then(|g| g.as_str())
        .unwrap_or("")
        .to_string();
    let host = row
        .get("base")
        .and_then(|b| b.as_str())
        .and_then(|b| b.strip_prefix("https://"))
        .and_then(|b| b.split('/').next())
        .map(String::from)
        .unwrap_or_else(|| "fal.media".to_string());
    let key = if gateway == "selfhost" {
        "selfhost"
    } else if gateway == "fal" {
        "fal"
    } else {
        "none"
    };
    let output = match row.get("kind").and_then(|k| k.as_str()) {
        Some("image.gen") => "images[*].url",
        Some("tts" | "music.gen" | "sfx.gen") => "audio.url",
        _ => "video.url",
    };
    let terms: toml::Table = [
        (
            "attribution_required".to_string(),
            toml::Value::Boolean(false),
        ),
        ("redistribution".to_string(), toml::Value::Boolean(false)),
        (
            "note".to_string(),
            toml::Value::String(
                "Terms not checked: read the model's page before business use.".into(),
            ),
        ),
    ]
    .into_iter()
    .collect();
    for (k, v) in [
        ("key", toml::Value::String(key.into())),
        ("durations", toml::Value::Array(vec![])),
        (
            "aspects",
            toml::Value::Array(
                ["9:16", "16:9", "1:1"]
                    .map(|a| toml::Value::String(a.into()))
                    .to_vec(),
            ),
        ),
        ("usd_per_s", toml::Value::Float(0.0)),
        ("price", toml::Value::String(UNKNOWN.into())),
        ("price_source", toml::Value::String(String::new())),
        ("price_checked", toml::Value::String(String::new())),
        ("output", toml::Value::String(output.into())),
        ("hosts", toml::Value::Array(vec![toml::Value::String(host)])),
        ("terms", toml::Value::Table(terms)),
    ] {
        row.entry(k.to_string()).or_insert(v);
    }
}

/// The rows, and one line for each block that made no row. `SCRIPTVOX_GENERATORS` points at a file used instead of the
/// built-in rows (tests, mirrors). Else the built-in rows always stay, and generators.toml in the config folder goes
/// over them block by block.
pub fn load() -> (Vec<Generator>, Vec<String>) {
    let (mut rows, mut errs) = (Vec::new(), Vec::new());
    let file = crate::keys::overlay_file();
    let shown = file.display().to_string();
    if crate::provision::user_env("SCRIPTVOX_GENERATORS").is_some() {
        match std::fs::read_to_string(&file) {
            Ok(text) => overlay(&text, &shown, &[], &mut rows, &mut errs),
            Err(e) => errs.push(format!("{shown}: {e}")),
        }
        return (rows, errs);
    }
    match parse(REGISTRY) {
        Ok(r) => rows = r,
        Err(e) => errs.push(e),
    }
    if let Ok(text) = std::fs::read_to_string(&file) {
        overlay(&text, &shown, &tables(REGISTRY, "").0, &mut rows, &mut errs);
    }
    (rows, errs)
}

/// The row for a word: `free`, `cheapest` or `best` for one kind of output, among the rows that can run now. None when
/// `word` is a row name. `best` means the highest price here: scriptvox has no quality score, so it says that.
pub fn choose_by_word(word: &str, kind: &str) -> Option<Result<(Generator, String), String>> {
    if !["free", "cheapest", "best"].contains(&word) {
        return None;
    }
    let full = if kind.contains('.') || matches!(kind, "tts" | "stt") {
        kind.to_string()
    } else {
        format!("{kind}.gen")
    };
    let mut rows: Vec<Generator> = all()
        .into_iter()
        .filter(|g| g.kind == full && g.ready() && !g.unknown_price())
        .collect();
    if word == "free" {
        rows.retain(|g| !g.paid());
    }
    rows.sort_by(|a, b| a.cost(5.0).total_cmp(&b.cost(5.0)));
    let picked = if word == "best" {
        rows.pop()
    } else {
        rows.into_iter().next()
    };
    Some(match picked {
        Some(g) => {
            let why = match word {
                "free" => format!("the first {full} row that runs at no charge"),
                "cheapest" => format!("the lowest cost among the {full} rows that can run now"),
                _ => format!("the highest price among the {full} rows that can run now; scriptvox has no quality score"),
            };
            Ok((g, why))
        }
        None => Err(format!(
            "no {full} row can run now for `{word}`. `scriptvox providers --kind {kind} --ready` lists them; a free row needs your own GPU service or a setup target."
        )),
    })
}

/// Up to 3 row names nearest to `bad`, of this kind when one is given.
pub fn nearest(bad: &str, rows: &[Generator], kind: Option<&str>) -> Vec<String> {
    let names = rows
        .iter()
        .filter(|g| kind.is_none_or(|k| g.kind.starts_with(k)))
        .map(|g| g.name.as_str());
    nearest_names(bad, names, 3)
}

/// Up to `n` of `names` nearest to `bad`, the nearest first.
pub fn nearest_names<'a>(bad: &str, names: impl Iterator<Item = &'a str>, n: usize) -> Vec<String> {
    let mut scored: Vec<(f64, &str)> = names.map(|s| (strsim::jaro_winkler(bad, s), s)).collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(n)
        .map(|(_, s)| s.to_string())
        .collect()
}

/// A TOML block to copy into generators.toml for a gateway, with a comment for each field to change.
pub fn template(gateway: &str) -> Option<String> {
    let (key, extra) = match gateway {
        "fal" => ("fal", "t2v = \"fal-ai/your-model\"  # the endpoint on fal.ai\n"),
        "selfhost" => ("selfhost", "base = \"https://your-host/scriptvox-gen\"  # your GPU service\nt2v = \"your-model/t2v\"\n"),
        "openai-speech" => ("none", "base = \"https://your-host\"  # OpenAI speech API\nt2v = \"tts-1\"  # the model name\n"),
        _ => return None,
    };
    Some(format!(
        "[[generator]]\nname = \"my-row\"\ngateway = \"{gateway}\"\nkind = \"video.gen\"  # or image.gen, music.gen, tts, stt\nkey = \"{key}\"\n{extra}# Then add the real price to spend without --max-spend: usd_per_s, price = \"exact\", price_source, price_checked.\n"
    ))
}

/// Adds one `[[generator]]` block to the user's file, after checking it alone. Every missing or wrong field is named at
/// once, and nothing is written when a field is wrong.
pub fn add(block: &str) -> Result<std::path::PathBuf, Vec<String>> {
    let file = crate::keys::overlay_file();
    let shown = file.display().to_string();
    let stock = tables(REGISTRY, "").0;
    let (mut rows, mut errs) = (parse(REGISTRY).unwrap_or_default(), Vec::new());
    overlay(block, &shown, &stock, &mut rows, &mut errs);
    let name = block
        .lines()
        .find_map(|l| l.trim().strip_prefix("name = \"")?.strip_suffix('"'))
        .unwrap_or_default()
        .to_string();
    errs.extend(
        validate(&rows)
            .into_iter()
            .filter(|e| e.starts_with(&format!("generator {name}:"))),
    );
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(block);
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&file, text).map_err(|e| vec![e.to_string()])?;
    Ok(file)
}

/// Every problem with the rows and the `[[key]]` blocks, one line each, worded as the fix.
pub fn problems() -> Vec<String> {
    let (rows, errs) = load();
    errs.into_iter()
        .chain(validate(&rows))
        .chain(crate::keys::declared_errors())
        .collect()
}

/// Why a row named by the user cannot be used as written, and where to correct it. None for a good row or an unknown name.
pub fn problem(name: &str) -> Option<(String, String)> {
    let at = format!("generator {name}:");
    problems()
        .into_iter()
        .find(|e| e.starts_with(&at))
        .map(|e| {
            (
                e,
                format!(
                    "Correct or remove this row in {}.",
                    crate::keys::overlay_file().display()
                ),
            )
        })
}

/// A warning for a plan when the user's rows have problems. It stops nothing: the other rows work.
pub fn plan_finding() -> Option<serde_json::Value> {
    let p = problems();
    (!p.is_empty()).then(|| serde_json::json!({"gate": "generators", "status": "warn", "detail": p.join("; "),
        "fix": format!("Correct or remove these blocks in {}. `scriptvox providers --json` lists them in `generator_errors`.", crate::keys::overlay_file().display())}))
}

/// The valid rows only: a row that fails `validate` is never used. A built-in row stays when the block over it fails.
pub fn all() -> Vec<Generator> {
    let rows = load().0;
    let bad = validate(&rows);
    let builtin = |name: &str| {
        crate::provision::user_env("SCRIPTVOX_GENERATORS")
            .is_none()
            .then(|| parse(REGISTRY).unwrap_or_default())?
            .into_iter()
            .find(|g| g.name == name)
    };
    rows.into_iter()
        .filter_map(|g| {
            if bad
                .iter()
                .any(|e| e.starts_with(&format!("generator {}:", g.name)))
            {
                builtin(&g.name)
            } else {
                Some(g)
            }
        })
        .chain(crate::provision::image_ready().then(local_image))
        .collect()
}

/// The local image model as a row (`scriptvox setup image-local`). Like `local_stt` it is not in the registry and sends
/// nothing: `genjob` runs one sd.cpp process for each picture. `takes_s` is the only measure there is: one H100, own
/// CUDA build, G006. `service` says so.
pub fn local_image() -> Generator {
    let row = r#"[[generator]]
name = "flux2-klein-4b-local"
takes_s = 44
takes_reference = true
takes_seed = true
gateway = "local"
kind = "image.gen"
key = "selfhost"
t2v = "local"
durations = []
aspects = ["9:16", "16:9", "1:1"]
usd_per_s = 0.0
price = "exact"
price_source = "https://huggingface.co/black-forest-labs/FLUX.2-klein-4B"
price_checked = "2026-11-06"
output = "inline"
hosts = ["local"]
terms = { commercial = true, attribution_required = false, redistribution = true, note = "FLUX.2 klein 4B, Apache-2.0 (model card read 6 Nov 2026), Q4 weights, 4 steps, on this machine." }
"#;
    parse(row).expect("the local row parses").remove(0)
}

/// The local speech to text plugin as a row, so every caller of a speech row can use it. Not in the registry and not
/// sent anywhere: `genjob` runs it as a process (`scriptvox setup speech-local`).
pub fn local_stt() -> Generator {
    let mut g = local_stt_row();
    if crate::provision::whisper_row_name() != g.name {
        g.name = crate::provision::whisper_row_name().into();
        g.price_source = "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2".into();
        g.terms.note = "faster-whisper large-v3-turbo, MIT, on this machine's CPU.".into();
    }
    g
}

fn local_stt_row() -> Generator {
    let row = r#"[[generator]]
name = "whisper-small-local"
gateway = "local"
kind = "stt"
key = "selfhost"
t2v = "local"
durations = []
aspects = []
usd_per_s = 0.0
price = "exact"
price_source = "https://huggingface.co/Systran/faster-whisper-small"
price_checked = "2026-11-05"
output = "inline"
hosts = ["local"]
terms = { commercial = true, attribution_required = false, redistribution = true, note = "faster-whisper small, MIT, on this machine's CPU." }
"#;
    parse(row).expect("the local row parses").remove(0)
}

/// A price older than this is flagged in `providers` and in plans.
pub const PRICE_STALE_DAYS: i64 = 30;

/// Days from a `YYYY-MM-DD` date to today, by the system clock.
pub fn days_since(day: &str) -> Option<i64> {
    let b = day.as_bytes();
    let num = |r: std::ops::Range<usize>| std::str::from_utf8(b.get(r)?).ok()?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64
        / 86_400;
    Some(today - days_from_civil(y, m, d))
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn date_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && s.chars()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// https anywhere; plain http only to this machine, where nothing crosses a network. A user part before `@` is refused:
/// `http://127.0.0.1:80@host/` goes to `host`.
pub fn speech_base_ok(b: &str) -> bool {
    let Some((scheme, rest)) = b.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.split(':').next().unwrap_or("");
    !host.is_empty()
        && !authority.contains('@')
        && (scheme == "https" || (scheme == "http" && (host == "127.0.0.1" || host == "localhost")))
}

/// A test server's base URL that replaces a real service: plain http on 127.0.0.1 only, with a port and no user part.
pub fn loopback_base_ok(b: &str) -> bool {
    b.starts_with("http://127.0.0.1:") && speech_base_ok(b)
}

fn endpoint_ok(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/._-".contains(c))
}

/// Every problem with the rows, one line each, worded as the fix. Empty means the rows are usable.
/// A row name: it also names files and folders, so it can never be a path.
pub fn name_ok(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 48
        && !n.contains("..")
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
}

pub fn validate(rows: &[Generator]) -> Vec<String> {
    let mut errs = Vec::new();
    for (i, g) in rows.iter().enumerate() {
        let at = format!(
            "generator {}",
            if g.name.is_empty() {
                i.to_string()
            } else {
                g.name.clone()
            }
        );
        if !name_ok(&g.name) {
            errs.push(format!(
                "{at}: name must be 1 to 48 of a-z, 0-9, '.', '_' or '-'"
            ));
        }
        if rows[..i].iter().any(|o| o.name == g.name) {
            errs.push(format!(
                "{at}: name is used twice; give each row its own name"
            ));
        }
        if !GATEWAYS.contains(&g.gateway.as_str()) {
            errs.push(format!("{at}: gateway must be one of {GATEWAYS:?}"));
        }
        if !KINDS.contains(&g.kind.as_str()) {
            errs.push(format!("{at}: kind must be one of {KINDS:?}"));
        }
        // ponytail: price units are per second (output, or audio sent to stt) and per image. Other kinds must be free
        // until they get a unit (per 1k characters for tts).
        if !g.timed() && g.kind != "stt" && g.usd_per_s != 0.0 {
            errs.push(format!(
                "{at}: a {} row must have usd_per_s = 0 (no per-second price for it)",
                g.kind
            ));
        }
        if !(0.0..10.0).contains(&g.usd_each) || (g.usd_each > 0.0 && g.kind != "image.gen") {
            errs.push(format!(
                "{at}: usd_each is the price of one picture: below 10, and only on an image.gen row"
            ));
        }
        if [
            g.usd_min,
            g.usd_per_input_s,
            g.usd_per_reference,
            g.usd_per_s_sound,
        ]
        .iter()
        .any(|p| !(0.0..10.0).contains(p))
        {
            errs.push(format!(
                "{at}: usd_min, usd_per_input_s, usd_per_reference and usd_per_s_sound must be from 0 to below 10"
            ));
        }
        if g.base.as_deref().is_some_and(|b| {
            b.chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '"')
        }) {
            errs.push(format!(
                "{at}: base must be one URL, with no spaces or quotes"
            ));
        }
        // A `base` is the user's host: a built-in key must not go there. The self-hosted key is made for that host.
        if g.base.is_some()
            && g.key != "selfhost"
            && crate::keys::PROVIDERS.iter().any(|(p, _)| *p == g.key)
        {
            errs.push(format!(
                "{at}: key {} is a built-in provider's key and must not go to `base`: declare your own key name",
                g.key
            ));
        }
        if g.gateway == OPENAI_IMAGES
            && (g.kind != "image.gen" || !g.base.as_deref().is_some_and(speech_base_ok))
        {
            errs.push(format!(
                "{at}: an {OPENAI_IMAGES} row is an image row: kind = \"image.gen\", and base = \"https://...\", or http:// on 127.0.0.1 or localhost"
            ));
        }
        if (g.gateway == COMFYUI || g.gateway == DATA_FILE)
            && (!matches!(g.kind.as_str(), "image.gen" | "video.gen")
                || !g.base.as_deref().is_some_and(speech_base_ok))
        {
            errs.push(format!(
                "{at}: a {COMFYUI} or {DATA_FILE} row makes a picture or a clip: kind = \"image.gen\" or \"video.gen\", and base = \"https://...\", or http:// on 127.0.0.1 or localhost"
            ));
        }
        if g.gateway == OPENAI_SPEECH {
            if g.kind != "tts" {
                errs.push(format!(
                    "{at}: an {OPENAI_SPEECH} row is a voice: kind = \"tts\""
                ));
            }
            if !g.base.as_deref().is_some_and(speech_base_ok) {
                errs.push(format!(
                    "{at}: an {OPENAI_SPEECH} row needs base = \"https://...\", or http:// on 127.0.0.1 or localhost"
                ));
            }
        }
        if g.gateway == "local" && (g.source.is_none() || g.kind != "image.gen") {
            errs.push(format!(
                "{at}: a local row comes from `scriptvox models add`"
            ));
        }
        if let Some(r) = &g.instead {
            if !rows.iter().any(|o| &o.name == r && o.retired.is_none()) {
                errs.push(format!(
                    "{at}: instead names {r}, which is not a running row"
                ));
            }
        }
        if g.key != "none" && crate::keys::env_var(&g.key).is_none() {
            errs.push(format!(
                "{at}: key {:?} is not known; add a [[key]] block with name and env to generators.toml",
                g.key
            ));
        }
        match (&g.t2v, &g.i2v) {
            (None, None) => errs.push(format!("{at}: set t2v, i2v or both")),
            _ => {
                for e in [&g.t2v, &g.i2v].into_iter().flatten() {
                    // A service names its own models, such as `flux:Latest`. It goes only into the JSON body.
                    let free_name = (g.gateway == OPENAI_IMAGES
                        || g.gateway == COMFYUI
                        || g.gateway == DATA_FILE)
                        && !e.is_empty()
                        && !e.chars().any(char::is_control);
                    if !free_name && !endpoint_ok(e) {
                        errs.push(format!(
                            "{at}: endpoint {e:?} may hold only a-z, 0-9, '/', '.', '_', '-'"
                        ));
                    }
                }
            }
        }
        if (g.timed() && g.durations.is_empty() && !g.unknown_price())
            || g.durations.iter().any(|d| !(*d > 0.0 && *d <= 120.0))
        {
            errs.push(format!(
                "{at}: durations must list seconds between 0 and 120, as the provider page states"
            ));
        }
        if g.visual() && !g.aspects.iter().any(|a| ASPECTS.contains(&a.as_str())) {
            errs.push(format!(
                "{at}: aspects must include at least one of {ASPECTS:?}"
            ));
        }
        // A self-hosted row may be free; a hosted row never is (a 0 there is a missing price).
        let free_ok =
            ((g.gateway == "selfhost" || g.gateway == OPENAI_SPEECH || g.gateway == "local")
                && g.usd_per_s == 0.0)
                || g.unknown_price();
        if !(free_ok
            || g.usd_each > 0.0
            || g.usd_per_input_s > 0.0
            || (g.usd_per_s > 0.0 && g.usd_per_s < 10.0))
        {
            errs.push(format!(
                "{at}: usd_per_s must be above 0 and below 10 (a per-second price; 0 only on a selfhost row, or with usd_each)"
            ));
        }
        if g.gateway != OPENAI_SPEECH
            && g.gateway != OPENAI_IMAGES
            && g.gateway != COMFYUI
            && g.gateway != DATA_FILE
            && (g.gateway == "selfhost")
                != g.base.as_deref().is_some_and(|b| b.starts_with("https://"))
        {
            errs.push(format!(
                "{at}: a selfhost row needs base = \"https://...\" and other rows must not set it"
            ));
        }
        if let Some(c) = &g.ca {
            if c.is_empty()
                || !c
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                errs.push(format!(
                    "{at}: ca must be a name of a-z, 0-9 and '-' (the file <config>/ca/<name>.pem)"
                ));
            }
        }
        if !["exact", "base", UNKNOWN].contains(&g.price.as_str()) {
            errs.push(format!(
                "{at}: price must be \"exact\", \"base\" or \"unknown\""
            ));
        }
        if g.unknown_price() || g.gateway == "local" {
            // Nothing else to prove: `generate` needs --max-spend for an unknown price, and a local row sends nothing.
        } else if !g.price_source.starts_with("https://") {
            errs.push(format!(
                "{at}: price_source must be the https page the price was read from"
            ));
        }
        if !g.unknown_price() && g.gateway != "local" && !date_ok(&g.price_checked) {
            errs.push(format!(
                "{at}: price_checked must be the day the price was read, YYYY-MM-DD"
            ));
        }
        if g.output.is_empty() {
            errs.push(format!(
                "{at}: output must name the result field with the video URL"
            ));
        }
        if g.hosts.is_empty() || g.hosts.iter().any(|h| h.is_empty() || h.contains('/')) {
            errs.push(format!(
                "{at}: hosts must list the download hosts (names only, no paths)"
            ));
        }
        if g.terms.note.trim().is_empty() {
            errs.push(format!(
                "{at}: terms.note must say what the licence allows, or that it is not checked"
            ));
        }
    }
    errs
}

impl Generator {
    /// The clip length to ask for: the shortest allowed duration that covers `want`, else the longest allowed.
    pub fn duration_for(&self, want: f64) -> f64 {
        let mut d = self.durations.clone();
        d.sort_by(f64::total_cmp);
        // A row with no listed durations takes any length.
        d.iter()
            .copied()
            .find(|&x| x + 1e-9 >= want)
            .unwrap_or(*d.last().unwrap_or(&want))
    }

    /// Cost in USD of one clip of `seconds`, rounded up to the cent (a plan never shows less than the bill).
    pub fn cost(&self, seconds: f64) -> f64 {
        self.cost_for(seconds, 0.0, 0)
    }

    /// As `cost`, with the source length and the reference pictures that a call sends.
    pub fn cost_for(&self, seconds: f64, input_s: f64, references: usize) -> f64 {
        let sound = if self.extra.get("generate_audio") == Some(&serde_json::Value::Bool(true)) {
            self.usd_per_s_sound * seconds
        } else {
            0.0
        };
        let sum = self.usd_each
            + self.usd_per_s * seconds
            + sound
            + self.usd_per_input_s * input_s
            + self.usd_per_reference * references as f64;
        // `+ 0.0`: a free row gives ceil(-1e-9) = -0.0, which would print as "-0.0".
        (sum.max(self.usd_min) * 100.0 - 1e-9).ceil() / 100.0 + 0.0
    }

    /// The parts of the price beyond the main unit, for a plan line and `providers`.
    pub fn shape_words(&self) -> String {
        let mut w = String::new();
        if self.usd_per_input_s > 0.0 && !self.priced_by_source() {
            w += &format!(", plus ${}/s of the source", self.usd_per_input_s);
        }
        if self.usd_per_reference > 0.0 {
            w += &format!(", plus ${} a reference picture", self.usd_per_reference);
        }
        if self.usd_per_s_sound > 0.0 {
            w += &format!(", plus ${}/s with sound", self.usd_per_s_sound);
        }
        if self.usd_min > 0.0 {
            w += &format!(", at least ${} a call", self.usd_min);
        }
        w
    }

    /// A user's row whose price nobody has read: `generate` needs `--max-spend`, and no plan picks it alone.
    pub fn unknown_price(&self) -> bool {
        self.price == UNKNOWN
    }

    /// A call to this row costs money.
    pub fn paid(&self) -> bool {
        self.unknown_price()
            || self.usd_per_s > 0.0
            || self.usd_each > 0.0
            || self.usd_min > 0.0
            || self.usd_per_input_s > 0.0
            || self.usd_per_reference > 0.0
            || self.usd_per_s_sound > 0.0
    }

    /// All of the price comes from the length of the user's own file.
    fn priced_by_source(&self) -> bool {
        self.usd_per_s == 0.0 && self.usd_each == 0.0 && self.usd_per_input_s > 0.0
    }

    /// The price in plain words.
    fn price_words(&self) -> String {
        if self.unknown_price() {
            "price not known".to_string()
        } else if self.priced_by_source() {
            format!("${}/s of the source", self.usd_per_input_s)
        } else if self.usd_each > 0.0 || self.kind == "image.gen" {
            format!("${} an image", self.usd_each)
        } else if self.kind == "stt" {
            format!("${}/s of audio", self.usd_per_s)
        } else {
            format!("${}/s", self.usd_per_s)
        }
    }

    /// What this call costs, in words, for a plan line.
    pub fn cost_words(&self, seconds: f64) -> String {
        if self.unknown_price() {
            self.price_words()
        } else if self.priced_by_source() {
            format!("{}{}", self.price_words(), self.shape_words())
        } else if self.usd_each > 0.0 || self.kind == "image.gen" {
            format!("one image at {}{}", self.price_words(), self.shape_words())
        } else {
            format!(
                "{seconds} s at {}{}",
                self.price_words(),
                self.shape_words()
            )
        }
    }

    /// The output length is priced and asked for in seconds (video, music, sound effects).
    pub fn timed(&self) -> bool {
        matches!(self.kind.as_str(), "video.gen" | "music.gen" | "sfx.gen")
    }

    /// Who runs this row and who pays, in plain words: a user sees it without reading a licence.
    pub fn service(&self) -> String {
        if self.gateway == OPENAI_SPEECH {
            return format!(
                "no charge, on your own speech service at {} (OpenAI speech API)",
                self.base.as_deref().unwrap_or("?")
            );
        }
        if self.gateway == OPENAI_IMAGES {
            return format!(
                "your own image service at {} (OpenAI images API), price not known",
                self.base.as_deref().unwrap_or("?")
            );
        }
        if self.gateway == COMFYUI {
            return format!(
                "your own ComfyUI at {}, price not known",
                self.base.as_deref().unwrap_or("?")
            );
        }
        if self.gateway == DATA_FILE {
            return format!(
                "your own service at {}, described by a data file, price not known",
                self.base.as_deref().unwrap_or("?")
            );
        }
        if self.gateway == "selfhost" {
            return "no charge, on your own GPU service (`services/gen`), your own key".into();
        }
        if self.source.is_some() {
            return "no charge, on this machine (sd.cpp), your own model file. The time is not measured".into();
        }
        if self.gateway == "local" {
            return if self.kind == "image.gen" {
                "no charge, on this machine (`scriptvox setup image-local`). The time was measured on one H100 with another build. This build took 446 s at 512x512 on 2 CPU cores, and on a card it is not measured".into()
            } else {
                "no charge, on this machine (`scriptvox setup speech-local`)".into()
            };
        }
        let key = crate::keys::env_var(&self.key).unwrap_or("not set up".into());
        if self.paid() {
            // The price stays last: `external_note` cuts it.
            let untried = if self.unverified {
                "not run live yet, "
            } else {
                ""
            };
            format!(
                "paid external API through {}, {untried}your own key {key}, {}",
                self.gateway,
                self.price_words()
            )
        } else {
            format!("external API through {}, your own key {key}", self.gateway)
        }
    }

    /// `; <service>` for the plan line: who runs the row and on whose key. The line has the price already.
    pub fn external_note(&self) -> String {
        let s = self.service();
        let s = s.rsplit_once(", $").map_or(s.as_str(), |x| x.0);
        if self.undeployed() {
            format!("; {s}; not deployed yet: {}", self.deploy_step())
        } else {
            format!("; {s}")
        }
    }

    /// A selfhost row whose host is still the `.invalid` placeholder: no service answers there, a key cannot help.
    /// `SCRIPTVOX_FAL_BASE` (loopback test servers only) replaces the host.
    pub fn undeployed(&self) -> bool {
        self.gateway == "selfhost"
            && std::env::var_os("SCRIPTVOX_FAL_BASE").is_none()
            && self.base.as_deref().is_some_and(|b| {
                b.trim_start_matches("https://")
                    .split(['/', ':'])
                    .next()
                    .is_some_and(|h| h.ends_with(".invalid"))
            })
    }

    /// Whether the user's own GPU service of this row answers now: one TCP connect to its host, 1.5 s at most, asked
    /// one time for each host in a process. `None`: the row is not on a service of the user's own (a paid API, a
    /// local row), or the network goes through a proxy, so it is not asked. Nothing of the brief is sent.
    pub fn answers_now(&self) -> Option<bool> {
        use std::net::{TcpStream, ToSocketAddrs};
        static SEEN: std::sync::Mutex<Vec<(String, bool)>> = std::sync::Mutex::new(Vec::new());
        let proxied = ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
            .iter()
            .any(|v| std::env::var_os(v).is_some());
        if self.gateway != "selfhost" || self.undeployed() || proxied {
            return None;
        }
        let base = std::env::var("SCRIPTVOX_FAL_BASE")
            .ok()
            .or_else(|| self.base.clone())?;
        let (scheme, rest) = base.split_once("://")?;
        let host = rest.split('/').next()?;
        let addr = match host.contains(':') {
            true => host.to_string(),
            false => format!("{host}:{}", if scheme == "http" { 80 } else { 443 }),
        };
        let mut seen = SEEN.lock().ok()?;
        if let Some((_, up)) = seen.iter().find(|(a, _)| *a == addr) {
            return Some(*up);
        }
        let up = addr.to_socket_addrs().ok().is_some_and(|mut list| {
            list.any(|sa| {
                TcpStream::connect_timeout(&sa, std::time::Duration::from_millis(1500)).is_ok()
            })
        });
        seen.push((addr, up));
        Some(up)
    }

    /// What to do before an undeployed row can run.
    pub fn deploy_step(&self) -> String {
        // ponytail: drop the video line when G010 ships local video.
        let free = match self.kind.as_str() {
            "video.gen" => {
                ". Free video needs a server like this for now: scriptvox has no local video yet"
            }
            "image.gen" => ". A free still needs no server: `scriptvox setup image-local`",
            _ => "",
        };
        format!(
            "deploy `services/gen` on your own GPU (services/gen/README.md), add a row named {} with your host as `base` in {}, then run `scriptvox keys set {}`{free}",
            self.name,
            crate::keys::config_dir().join("generators.toml").display(),
            self.key
        )
    }

    /// This row can run now.
    pub fn ready(&self) -> bool {
        self.why_not().is_none()
    }

    /// Why this row cannot run now, and the one step that changes that. None when it can.
    pub fn why_not(&self) -> Option<WhyNot> {
        if let Some((why, fix)) = self.retired_why() {
            return Some(WhyNot {
                kind: "retired",
                why,
                fix,
            });
        }
        if let Some(s) = &self.source {
            return s.missing().map(|why| WhyNot {
                kind: "dependency",
                why,
                fix: s.add_again(),
            });
        }
        if self.gateway == "local" {
            let (there, target) = if self.kind == "image.gen" {
                (crate::provision::image_ready(), "image-local")
            } else {
                (
                    crate::provision::whisper_ready() || crate::provision::whisper_large_ready(),
                    "speech-local",
                )
            };
            return (!there).then(|| WhyNot {
                kind: "dependency",
                why: format!("{} is not installed", self.name),
                fix: format!("Run `scriptvox setup {target}`."),
            });
        }
        if self.gateway == DATA_FILE && !crate::data_row::file_for(&self.name).is_file() {
            return Some(WhyNot {
                kind: "dependency",
                why: format!("{} has no spec file", self.name),
                fix: format!(
                    "Run `scriptvox connect {} --url <base> --spec <file>` again.",
                    self.name
                ),
            });
        }
        if self.gateway == COMFYUI && !crate::comfy::file_for(&self.name).is_file() {
            return Some(WhyNot {
                kind: "dependency",
                why: format!("{} has no workflow file", self.name),
                fix: format!(
                    "Run `scriptvox connect {} --url <base> --workflow <file>` again.",
                    self.name
                ),
            });
        }
        if self.undeployed() {
            return Some(WhyNot {
                kind: "dependency",
                why: format!(
                    "{} has no service yet: its host is a placeholder that no server answers",
                    self.name
                ),
                fix: format!("To run it, {}.", self.deploy_step()),
            });
        }
        (!self.has_key()).then(|| WhyNot {
            kind: "secret",
            why: format!("no {} key", self.key),
            fix: format!("Run `scriptvox keys set {}` in your terminal.", self.key),
        })
    }

    /// The one admission check: what stops a call to this row. `max_spend` is what the caller allows on a row with no
    /// known price; a whole plan passes None, because it cannot price the row. Test, generate and make all ask here.
    pub fn admission(&self, max_spend: Option<f64>) -> Option<WhyNot> {
        if let Some((why, fix)) = self.retired_why() {
            return Some(WhyNot {
                kind: "retired",
                why,
                fix,
            });
        }
        if self.unknown_price() && !max_spend.is_some_and(|m| m.is_finite() && m > 0.0) {
            return Some(WhyNot {
                kind: "price",
                why: format!("{} has no known price", self.name),
                fix: format!("Give --max-spend with an amount above 0, or add usd_per_s, price, price_source and price_checked to its row in {}.", crate::keys::overlay_file().display()),
            });
        }
        self.why_not()
    }

    /// Why this row cannot run any more, and what to use: None while it runs.
    pub fn retired_why(&self) -> Option<(String, String)> {
        let why = self.retired.as_ref()?;
        let fix = match &self.instead {
            Some(r) => format!("Use row {r} instead (`--model {r}`, or `model` in the brief)."),
            None => "Pick another row from `scriptvox providers`.".into(),
        };
        Some((format!("{} is retired: {why}", self.name), fix))
    }

    /// Its key is set, or it needs none (`key = "none"`).
    pub fn has_key(&self) -> bool {
        self.key == "none" || crate::keys::resolve(&self.key).is_some()
    }

    /// A speech row that tells speakers apart: its `extra` has `diarize`, which a speakers request turns on (G020).
    pub fn gives_speakers(&self) -> bool {
        self.kind == "stt" && self.extra.contains_key("diarize")
    }

    /// The output is a picture or a clip, so it has an aspect.
    pub fn visual(&self) -> bool {
        matches!(self.kind.as_str(), "video.gen" | "image.gen")
    }

    /// File extension of the result.
    pub fn ext(&self) -> &'static str {
        match self.kind.as_str() {
            "video.gen" | "upscale" | "interpolate" | "matte" | "lipsync" => "mp4",
            "image.gen" => "png",
            "stt" => "json",
            _ => "wav",
        }
    }

    /// The request body for any kind. Video keeps `input`'s shape; the others send only what they use.
    pub fn request(&self, r: &GenInput) -> serde_json::Value {
        if self.kind == "video.gen" {
            return self.input(&r.prompt, r.seconds, &r.aspect, r.image.as_deref());
        }
        let mut v = self.extra.clone();
        let mut put = |k: &str, x: Option<serde_json::Value>| {
            if let Some(x) = x {
                v.insert(k.into(), x);
            }
        };
        let text = (!r.prompt.is_empty()).then(|| r.prompt.clone().into());
        match self.kind.as_str() {
            "image.gen" => {
                put("prompt", text);
                put("aspect_ratio", Some(r.aspect.clone().into()));
                // A reference picture, on the rows that take one.
                put("image_urls", r.image.clone().map(|u| vec![u].into()));
            }
            "tts" => {
                put("text", text);
                put("voice", r.voice.clone().map(Into::into));
                put("language", r.lang.clone().map(Into::into));
                // A short sample of a voice to clone, on rows that can.
                put("reference_audio_url", r.audio.clone().map(Into::into));
            }
            "upscale" | "interpolate" | "matte" => {
                put("video_url", r.video.clone().map(Into::into))
            }
            // A video and the voice its mouth must follow.
            "lipsync" => {
                put("video_url", r.video.clone().map(Into::into));
                put("audio_url", r.audio.clone().map(Into::into));
            }
            "stt" => {
                put("audio_url", r.audio.clone().map(Into::into));
                put(
                    self.lang_field.as_deref().unwrap_or("language"),
                    r.lang.clone().map(Into::into),
                );
                // Whisper's initial prompt: its style is copied (punctuation, or "um" and "uh" written out).
                put("prompt", text);
            }
            // A track split into voice and the rest; the row returns the rest (music, ambience).
            "stems" => put("audio_url", r.audio.clone().map(Into::into)),
            _ => {
                put("prompt", text);
                put(
                    "duration",
                    Some(serde_json::json!(self.duration_for(r.seconds))),
                );
                // Sound that follows a picture (MMAudio v2a): the clip to score.
                put("video_url", r.video.clone().map(Into::into));
            }
        }
        serde_json::Value::Object(v)
    }

    /// The request body for one clip. `image_url` (a URL or a data URI) selects image to video.
    pub fn input(
        &self,
        prompt: &str,
        seconds: f64,
        aspect: &str,
        image_url: Option<&str>,
    ) -> serde_json::Value {
        let d = self.duration_for(seconds);
        let mut v = self.extra.clone();
        v.insert("prompt".into(), prompt.into());
        v.insert("aspect_ratio".into(), aspect.into());
        v.insert(
            "duration".into(),
            if self.duration_string || self.duration_suffix.is_some() {
                format!("{d}{}", self.duration_suffix.as_deref().unwrap_or("")).into()
            } else {
                serde_json::json!(d)
            },
        );
        if let Some(u) = image_url {
            v.insert(self.image_key().into(), u.into());
        }
        serde_json::Value::Object(v)
    }

    /// The request field that carries the first picture.
    pub fn image_key(&self) -> &str {
        self.image_field.as_deref().unwrap_or("image_url")
    }

    /// The last-frame picture (a URL or a data URI) under the row's own field. None when the row takes none.
    pub fn with_last_frame(
        &self,
        mut v: serde_json::Value,
        uri: Option<&str>,
    ) -> Option<serde_json::Value> {
        if let Some(u) = uri {
            let field = self
                .takes_last_frame
                .as_deref()
                .filter(|_| self.i2v.is_some())?;
            v.as_object_mut()?.insert(field.into(), u.into());
        }
        Some(v)
    }

    /// `seed` in the request when the row takes one; a row that does not would ignore it or refuse it.
    pub fn with_seed(&self, mut v: serde_json::Value, seed: Option<u64>) -> serde_json::Value {
        if let (true, Some(s), Some(o)) = (self.takes_seed, seed, v.as_object_mut()) {
            o.insert("seed".into(), s.into());
        }
        v
    }

    /// The plan's words on a seed: kept, or not taken by this row.
    pub fn seed_note(&self, seed: Option<u64>) -> String {
        match (seed, self.takes_seed) {
            (None, _) => String::new(),
            (Some(s), true) => format!("; seed {s}"),
            (Some(_), false) => "; this row takes no seed, so each run gives a new picture".into(),
        }
    }

    /// Days since the price was read, by the system clock. Over `PRICE_STALE_DAYS` the price may have changed.
    pub fn price_age_days(&self) -> Option<i64> {
        days_since(&self.price_checked)
    }

    /// A note for the plan when the price is old, else empty.
    pub fn stale_note(&self) -> String {
        match self.price_age_days() {
            Some(a) if a > PRICE_STALE_DAYS => {
                format!(
                    "; the price is {a} days old: check {} before you approve",
                    self.price_source
                )
            }
            _ => String::new(),
        }
    }

    /// "t2v" and "i2v", for the modes this row has.
    pub fn modes(&self) -> Vec<&'static str> {
        [(self.t2v.is_some(), "t2v"), (self.i2v.is_some(), "i2v")]
            .into_iter()
            .filter(|m| m.0)
            .map(|m| m.1)
            .collect()
    }

    pub fn endpoint(&self, image: bool) -> Option<&str> {
        if image {
            self.i2v.as_deref()
        } else {
            self.t2v.as_deref()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_retired_row_never_runs_and_names_its_successor() {
        let mut g = super::local_image();
        assert!(g.retired_why().is_none());
        g.retired = Some("gone on 1 Nov".into());
        g.instead = Some("x-row".into());
        assert!(!g.ready());
        let (why, fix) = g.retired_why().unwrap();
        assert!(why.contains("is retired: gone on 1 Nov"), "{why}");
        assert!(fix.contains("--model x-row"), "{fix}");
    }

    #[test]
    fn a_last_frame_goes_only_to_a_row_that_names_its_field() {
        let mut g = super::local_image();
        let v = serde_json::json!({"prompt": "p"});
        assert_eq!(g.with_last_frame(v.clone(), None), Some(v.clone()));
        assert_eq!(g.with_last_frame(v.clone(), Some("data:x")), None);
        g.takes_last_frame = Some("tail_image_url".into());
        assert_eq!(
            g.with_last_frame(v.clone(), Some("data:x")),
            None,
            "no i2v endpoint"
        );
        g.i2v = Some("x/i2v".into());
        assert_eq!(
            g.with_last_frame(v, Some("data:x")).unwrap()["tail_image_url"],
            "data:x"
        );
    }

    #[test]
    fn a_seed_goes_only_to_a_row_that_takes_it() {
        let local = super::local_image();
        let v = local.with_seed(serde_json::json!({"prompt": "p"}), Some(7));
        assert_eq!(v["seed"], 7);
        assert_eq!(local.seed_note(Some(7)), "; seed 7");
        let mut other = local.clone();
        other.takes_seed = false;
        assert!(other
            .with_seed(serde_json::json!({}), Some(7))
            .get("seed")
            .is_none());
        assert!(other.seed_note(Some(7)).contains("takes no seed"));
        assert_eq!(other.seed_note(None), "");
    }

    use super::*;

    #[test]
    fn a_row_with_a_duration_suffix_sends_a_string() {
        let g = all()
            .into_iter()
            .find(|g| g.name == "luma-ray-3.2-fal")
            .unwrap();
        let v = g.request(&GenInput {
            prompt: "x".into(),
            seconds: 5.0,
            aspect: "16:9".into(),
            ..Default::default()
        });
        assert_eq!(v["duration"], "5s", "{v}");
        assert_eq!(v["resolution"], "720p", "{v}");
    }

    #[test]
    fn a_row_priced_by_the_source_never_says_zero_per_second() {
        let g = all()
            .into_iter()
            .find(|g| g.name == "sync-lipsync-2-fal")
            .unwrap();
        assert_eq!(g.cost_words(0.0), "$0.05/s of the source");
        assert!(!g.service().contains("$0/s"));
    }

    #[test]
    fn a_new_users_row_gets_defaults_and_its_missing_fields_named_together() {
        let (mut rows, mut errs) = (Vec::new(), Vec::new());
        overlay(
            "[[generator]]\nname = \"mine\"\n",
            "f",
            &[],
            &mut rows,
            &mut errs,
        );
        assert!(
            errs[0].contains("missing gateway, kind, t2v or i2v"),
            "{errs:?}"
        );
        errs.clear();
        overlay(
            "[[generator]]\nname = \"mine\"\ngateway = \"fal\"\nkind = \"video.gen\"\nt2v = \"x/y\"\n",
            "f", &[], &mut rows, &mut errs,
        );
        assert!(errs.is_empty(), "{errs:?}");
        let g = rows.iter().find(|g| g.name == "mine").unwrap();
        assert!(g.unknown_price() && g.paid() && g.terms.commercial.is_none());
        assert!(g.cost_words(5.0).contains("not known"));
        assert!(validate(&rows).is_empty(), "{:?}", validate(&rows));
    }

    #[test]
    fn built_in_rows_never_have_an_unknown_price() {
        assert!(parse(REGISTRY).unwrap().iter().all(|g| !g.unknown_price()));
    }

    #[test]
    fn a_word_picks_a_row_and_says_why() {
        assert!(choose_by_word("kling-2.6-pro", "video").is_none());
        let names: Vec<String> = parse(REGISTRY)
            .unwrap()
            .iter()
            .map(|g| g.name.clone())
            .collect();
        let n = nearest("klng-2.6-pro", &parse(REGISTRY).unwrap(), Some("video"));
        assert_eq!(n[0], "kling-2.6-pro", "{n:?} {names:?}");
        assert!(n.len() == 3);
    }

    #[test]
    fn price_shapes_reach_the_bill() {
        let mut g = local_stt();
        assert_eq!(g.cost_for(5.0, 0.0, 0), 0.0);
        g.usd_per_s = 0.1;
        g.usd_min = 1.0;
        assert_eq!(g.cost(5.0), 1.0, "minimum charge");
        assert_eq!(g.cost(20.0), 2.0, "above the minimum");
        g.usd_min = 0.0;
        g.usd_per_input_s = 0.02;
        g.usd_per_reference = 0.05;
        assert_eq!(g.cost_for(10.0, 30.0, 2), 1.7);
        g.usd_per_s_sound = 0.1;
        assert_eq!(g.cost(10.0), 1.0, "sound is off");
        g.extra
            .insert("generate_audio".into(), serde_json::json!(true));
        assert_eq!(g.cost(10.0), 2.0, "sound is on");
        assert!(g.shape_words().contains("reference"));
        for set in [
            |g: &mut Generator| g.usd_min = 0.5,
            |g: &mut Generator| g.usd_per_input_s = 0.5,
            |g: &mut Generator| g.usd_per_reference = 0.5,
            |g: &mut Generator| g.usd_per_s_sound = 0.5,
        ] {
            let mut row = local_stt();
            set(&mut row);
            assert!(row.paid());
        }
    }

    #[test]
    fn shipped_rows_are_valid() {
        let rows = parse(REGISTRY).unwrap();
        assert!(rows.len() >= 3);
        assert_eq!(validate(&rows), Vec::<String>::new());
    }

    #[test]
    fn a_gateway_with_no_runner_is_refused_when_the_row_loads() {
        let mut rows = parse(REGISTRY).unwrap();
        rows[0].gateway = "replicate".into();
        let errs = validate(&rows);
        assert!(
            errs.iter().any(|e| e.contains("gateway must be")),
            "{errs:?}"
        );
    }

    #[test]
    fn bad_rows_are_refused_with_a_fix_line() {
        let mut rows = parse(REGISTRY).unwrap();
        let mut g = rows[0].clone();
        g.gateway = "acme".into();
        g.t2v = Some("../etc/passwd".into());
        g.usd_per_s = 0.0;
        g.price_checked = "yesterday".into();
        g.hosts = vec!["https://x/y".into()];
        g.durations = vec![];
        rows.push(g);
        let errs = validate(&rows);
        for want in [
            "used twice",
            "gateway",
            "endpoint",
            "usd_per_s",
            "price_checked",
            "hosts",
            "durations",
        ] {
            assert!(
                errs.iter().any(|e| e.contains(want)),
                "no {want} error in {errs:?}"
            );
        }
    }

    #[test]
    fn image_and_transcript_rows_have_their_own_price() {
        let rows = parse(REGISTRY).unwrap();
        let image = rows
            .iter()
            .find(|g| g.name == "flux-pro-ultra-fal")
            .unwrap();
        assert_eq!((image.cost(1.0), image.cost(0.0)), (0.06, 0.06));
        assert!(image.paid() && image.service().contains("$0.06 an image"));
        assert!(image.service().contains("not run live yet"));
        let stt = rows.iter().find(|g| g.name == "scribe-fal").unwrap();
        assert_eq!(stt.cost(120.0), 0.06);
        assert!(stt.service().contains("/s of audio"));
        // A hosted row with no price is refused, and so is a price on the wrong kind.
        let mut free = image.clone();
        free.usd_each = 0.0;
        assert!(!validate(&[free]).is_empty());
        let mut wrong = stt.clone();
        wrong.usd_each = 0.1;
        assert!(!validate(&[wrong]).is_empty());
    }

    #[test]
    fn a_test_base_is_loopback_with_no_user_part() {
        assert!(loopback_base_ok("http://127.0.0.1:8080"));
        assert!(!loopback_base_ok("http://127.0.0.1:1@127.0.0.2:18942"));
        assert!(!loopback_base_ok("https://127.0.0.1:8080"));
        assert!(!loopback_base_ok("http://127.0.0.1.example.com:80"));
    }

    /// fal's pages, read 9 Nov 2026: the worst bill of a clip these rows ask for, at the resolution they ask for.
    #[test]
    fn the_plan_of_a_fal_row_is_never_below_the_bill() {
        let rows = parse(REGISTRY).unwrap();
        let row = |n: &str| rows.iter().find(|g| g.name == n).unwrap();
        let grok = row("grok-imagine");
        assert_eq!(grok.extra["resolution"], "720p");
        assert!(grok.cost(6.0) >= 6.0 * 0.07 + 0.002, "{}", grok.cost(6.0));
        let minimax = row("minimax-h3-max-turbo");
        assert_eq!(minimax.extra["resolution"], "768P");
        assert!(minimax.cost(5.0) >= 5.7 * 0.04, "{}", minimax.cost(5.0));
    }

    #[test]
    fn kling_takes_its_first_picture_as_start_image_url() {
        let rows = parse(REGISTRY).unwrap();
        let kling = rows.iter().find(|g| g.name == "kling-2.6-pro").unwrap();
        let v = kling.input("p", 5.0, "16:9", Some("data:x"));
        assert_eq!(v["start_image_url"], "data:x");
        assert!(v.get("image_url").is_none(), "{v}");
    }

    #[test]
    fn duration_and_cost() {
        let g = parse(REGISTRY)
            .unwrap()
            .into_iter()
            .find(|g| g.name == "kling-2.6-pro")
            .unwrap();
        assert_eq!(g.duration_for(3.0), 5.0);
        assert_eq!(g.duration_for(7.0), 10.0);
        assert_eq!(g.duration_for(30.0), 10.0);
        assert_eq!(g.cost(5.0), 0.35);
        assert_eq!(g.cost(10.0), 0.70);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 11, 2), 20_759);
        assert!(g.price_age_days().unwrap() >= 0);
        assert_eq!(
            g.endpoint(true),
            Some("fal-ai/kling-video/v2.6/pro/image-to-video")
        );
    }

    #[test]
    fn upscale_row_sends_the_video_and_its_fixed_resolution() {
        let g = parse(REGISTRY)
            .unwrap()
            .into_iter()
            .find(|g| g.name == "seedvr2-3b-self")
            .unwrap();
        let body = g.request(&GenInput {
            video: Some("data:video/mp4;base64,AAAA".into()),
            ..Default::default()
        });
        assert_eq!(
            body,
            serde_json::json!({"video_url": "data:video/mp4;base64,AAAA", "resolution": 1080, "batch_size": 121})
        );
        assert_eq!((g.ext(), g.cost(5.0)), ("mp4", 0.0));
    }

    #[test]
    fn the_public_rows_name_no_real_self_hosted_host() {
        for g in parse(REGISTRY).unwrap() {
            if let Some(b) = g.base.as_deref() {
                assert!(b.contains("example.invalid"), "{}: {b}", g.name);
            }
        }
    }

    #[test]
    fn one_bad_block_costs_only_itself_and_a_known_name_merges() {
        let mut rows = parse(REGISTRY).unwrap();
        let n = rows.len();
        let own = rows
            .iter()
            .find(|g| g.gateway == "selfhost")
            .unwrap()
            .name
            .clone();
        let text = format!(
            "# mine\n[[generator]]\nname = {own:?}\nbase = \"https://gpu.example/q\"\n\n[[generator]]\nname = \"typo\"\nkey = = 1\n\n[[generator]]\nname = \"half-row\"\ngateway = \"fal\"\n"
        );
        let mut errs = Vec::new();
        overlay(
            &text,
            "f.toml",
            &tables(REGISTRY, "").0,
            &mut rows,
            &mut errs,
        );
        assert_eq!(rows.len(), n);
        let g = rows.iter().find(|g| g.name == own).unwrap();
        assert_eq!(g.base.as_deref(), Some("https://gpu.example/q"));
        assert_eq!(
            g.kind,
            parse(REGISTRY)
                .unwrap()
                .iter()
                .find(|g| g.name == own)
                .unwrap()
                .kind
        );
        assert_eq!(errs.len(), 2, "{errs:?}");
        assert!(
            errs[0].starts_with("generator typo:") && errs[0].contains("f.toml"),
            "{errs:?}"
        );
        assert!(errs[1].starts_with("generator half-row:"), "{errs:?}");
    }

    #[test]
    fn why_not_names_a_placeholder_host_before_any_key() {
        let rows = parse(REGISTRY).unwrap();
        let g = rows.iter().find(|g| g.undeployed()).unwrap();
        let b = g.why_not().unwrap();
        assert_eq!(b.kind, "dependency");
        assert!(b.fix.contains("services/gen"), "{b:?}");
        assert!(!g.ready());
        assert!(b.fix.contains("Free video needs a server"), "{b:?}");
        let still = rows
            .iter()
            .find(|g| g.undeployed() && g.kind == "image.gen")
            .unwrap();
        assert!(still.deploy_step().contains("setup image-local"));
    }

    #[test]
    fn lipsync_row_sends_video_and_voice_and_is_not_commercial() {
        let g = parse(REGISTRY)
            .unwrap()
            .into_iter()
            .find(|g| g.kind == "lipsync")
            .unwrap();
        let body = g.request(&GenInput {
            video: Some("data:video/mp4;base64,AA".into()),
            audio: Some("data:audio/wav;base64,BB".into()),
            ..Default::default()
        });
        assert_eq!(
            body,
            serde_json::json!({"video_url": "data:video/mp4;base64,AA", "audio_url": "data:audio/wav;base64,BB"})
        );
        assert_eq!(g.ext(), "mp4");
        assert_eq!(g.terms.commercial, Some(false));
    }

    #[test]
    fn interpolate_row_sends_the_video_and_doubles_the_frames() {
        let g = parse(REGISTRY)
            .unwrap()
            .into_iter()
            .find(|g| g.kind == "interpolate")
            .unwrap();
        let body = g.request(&GenInput {
            video: Some("data:video/mp4;base64,AAAA".into()),
            ..Default::default()
        });
        assert_eq!(
            body,
            serde_json::json!({"video_url": "data:video/mp4;base64,AAAA", "multi": 2})
        );
        assert_eq!(
            (g.name.as_str(), g.ext(), g.cost(5.0)),
            ("rife426-self", "mp4", 0.0)
        );
    }

    #[test]
    fn sound_from_video_row_sends_the_clip_prompt_and_length() {
        let g = parse(REGISTRY)
            .unwrap()
            .into_iter()
            .find(|g| g.name == "mmaudio-v2a-self")
            .unwrap();
        let body = g.request(&GenInput {
            prompt: "waves on rocks".into(),
            seconds: 5.0,
            video: Some("data:video/mp4;base64,AAAA".into()),
            ..Default::default()
        });
        assert_eq!(
            body,
            serde_json::json!({"prompt": "waves on rocks", "duration": 5.0, "video_url": "data:video/mp4;base64,AAAA"})
        );
        assert_eq!(
            (g.ext(), g.cost(8.0), g.terms.commercial),
            ("wav", 0.0, Some(false))
        );
    }
}
