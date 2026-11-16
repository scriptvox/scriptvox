//! Voice registry and TTS policy (PLAN-R1 §5, RVX-010).
//!
//! Rules: `--local-only` never picks a network engine. Arabic never falls back
//! to an English-only engine: when Edge refuses an Arabic voice, the only
//! alternatives offered are engines that speak Arabic.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const REGISTRY: &str = include_str!("../../../registry/voices.toml");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Voice {
    pub engine: String,
    pub id: String,
    pub lang: String,
    pub gender: String,
    pub network: bool,
    pub cost: String,
    pub timing: String,
    pub terms: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// False when the output terms limit commercial use. Data only (`voices`, `metadata.json`): the user decides.
    #[serde(default = "yes")]
    pub commercial: bool,
    /// Engine model, when the engine has several (ElevenLabs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Price estimate for paid engines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_per_1k_chars: Option<f64>,
    /// The day the price was read from `price_source`, on a paid voice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_checked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_source: Option<String>,
    /// The engine takes an IPA form of a `say` term. Other rows read the respelled form (docs/specs/delivery-cues.md).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub takes_ipa: bool,
    /// The delivery cues the engine takes in the script, such as `whisper`. Other cues are dropped with a warning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub takes_tags: Vec<String>,
    /// The brief's `say`, for the voice's read of the text.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub say: std::collections::BTreeMap<String, String>,
}

/// The consent records: in the config folder, as a voice is used across projects.
fn consent_file() -> std::path::PathBuf {
    crate::keys::config_dir().join("consent.json")
}

fn consent_records() -> Vec<serde_json::Value> {
    std::fs::read_to_string(consent_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The statement `voices add --own` records (by hash) in place of a proof file.
pub const OWN_STATEMENT: &str = "I state that this sample is my own voice.";

/// Records a sample and its proof by hash. Only hashes stay: no voice and no form is copied. With no proof the
/// sample is the user's own voice: the record holds the hash of `OWN_STATEMENT` and `own: true`.
pub fn add_consent(
    sample: &std::path::Path,
    proof: Option<&std::path::Path>,
    name: Option<&str>,
) -> Result<serde_json::Value, String> {
    let hash = |p: &std::path::Path| {
        crate::store::hash_file(p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let sample_hash = hash(sample)?;
    let (proof_hash, statement_hash) = match proof {
        Some(p) => (Some(hash(p)?), None),
        None => (
            None,
            Some(blake3::hash(OWN_STATEMENT.as_bytes()).to_hex().to_string()),
        ),
    };
    let id = format!("c_{}", &sample_hash[..12]);
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let record = serde_json::json!({"id": id, "sample_hash": sample_hash, "proof_hash": proof_hash,
        "own": proof.is_none(), "statement_hash": statement_hash, "name": name, "added_unix": at});
    let mut all = consent_records();
    all.retain(|r| r["id"] != record["id"]);
    all.push(record.clone());
    let text = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(crate::keys::config_dir()).map_err(|e| e.to_string())?;
    std::fs::write(consent_file(), text).map_err(|e| e.to_string())?;
    Ok(record)
}

/// The record id of a sample, if it has one.
pub fn consent_id(sample: &std::path::Path) -> Option<String> {
    let h = crate::store::hash_file(sample).ok()?;
    consent_records()
        .iter()
        .find(|r| r["sample_hash"] == h.as_str())
        .and_then(|r| r["id"].as_str().map(String::from))
}

fn yes() -> bool {
    true
}

impl Voice {
    pub fn speaks(&self, lang: &str) -> bool {
        self.lang.split(',').any(|l| l.trim() == lang)
    }

    /// The `explain` line for this voice. The terms are data (`voices`, the receipt, `metadata.json`), not a line here
    /// (owner, 13 Nov 2026).
    pub fn explain(&self) -> String {
        format!(
            "TTS {} {} ({}){}{}",
            self.engine,
            self.id,
            self.service(),
            self.sends(),
            self.stale_note()
        )
    }

    /// What leaves this machine, and to whom, when the voice needs the network (audit M4). Empty for a local voice.
    pub fn sends(&self) -> String {
        if !self.network {
            return String::new();
        }
        let to = match self.engine.as_str() {
            "edge" => "Microsoft's Edge read-aloud service (speech.platform.bing.com)",
            "elevenlabs" => "ElevenLabs (api.elevenlabs.io)",
            "selfhost" => "your own GPU service",
            _ => "the voice's service",
        };
        format!("; the script text goes to {to}")
    }

    /// A note for the plan when the price is old, else empty. Same limit as a generator row.
    fn stale_note(&self) -> String {
        let (Some(day), Some(source)) = (&self.price_checked, &self.price_source) else {
            return String::new();
        };
        match crate::generators::days_since(day) {
            Some(a) if a > crate::generators::PRICE_STALE_DAYS => {
                format!("; the price is {a} days old: check {source} before you approve")
            }
            _ => String::new(),
        }
    }

    /// Who runs this voice and who pays, in plain words: a user sees it without reading a licence.
    pub fn service(&self) -> String {
        match (self.cost.as_str(), &self.key, self.network) {
            ("paid", Some(_), _) => format!(
                "paid external API, your own key {}{}",
                crate::keys::env_var(&self.engine).unwrap_or("not set up".into()),
                self.usd_per_1k_chars
                    .map_or(String::new(), |p| format!(", ${p:.2} per 1000 characters"))
            ),
            (_, None, true) => format!("{}, external service, no key", self.cost),
            (_, Some(_), true) => {
                "no charge, on your own GPU service (`services/gen`), your own key".to_string()
            }
            _ => format!("{}, local", self.cost),
        }
    }

    /// Callable today: free, or paid with its key set.
    pub fn available(&self) -> bool {
        // The open voice speaks through a row: one on your GPU service, or your own OpenAI speech service (G049).
        if self.engine == "selfhost" {
            return crate::opentts::rows("tts").is_some();
        }
        (self.engine != "kokoro" || crate::provision::kokoro_ready())
            && (self.engine != crate::kokoro_rust::ENGINE || crate::kokoro_rust::ready())
            && (self.key.is_none() || crate::keys::resolve(&self.engine).is_some())
    }
}

#[derive(Deserialize)]
struct Registry {
    voice: Vec<Voice>,
}

pub fn all() -> Vec<Voice> {
    toml::from_str::<Registry>(REGISTRY)
        .expect("registry/voices.toml parses")
        .voice
}

/// A NEEDS_INPUT item (PLAN-R1 §2.3 envelope).
pub fn needs_input(field: &str, kind: &str, human_action: &str, alternatives: Vec<Value>) -> Value {
    json!({"field": field, "kind": kind, "human_action": human_action, "alternatives": alternatives})
}

fn alternative(v: &Voice, drop_local_only: bool) -> Value {
    let mut note = v.cost.to_string();
    if drop_local_only {
        note.push_str(", network: runs without --local-only");
    }
    json!({"args": ["--tts", v.engine, "--voice", v.id], "cost_usd": (v.cost == "free").then_some(0), "paid": v.cost != "free", "note": note})
}

/// An installed local voice (no network) wins over a network one; among equals, registry order (`min_by_key` keeps the first).
fn prefer_local<'a>(available: impl Iterator<Item = &'a Voice>) -> Option<&'a Voice> {
    available.min_by_key(|v| v.network)
}

/// Picks a voice, or says what input is needed.
pub fn choose(
    lang: &str,
    engine: Option<&str>,
    voice: Option<&str>,
    local_only: bool,
) -> Result<Voice, Value> {
    let fits: Vec<Voice> = all()
        .into_iter()
        // No voice is a choice, never a fallback. The Rust voice is a choice too, until it measures equal or better.
        .filter(|v| {
            !matches!(v.engine.as_str(), "none" | crate::kokoro_rust::ENGINE)
                || engine == Some(v.engine.as_str())
        })
        .filter(|v| {
            v.speaks(lang)
                && engine.is_none_or(|e| v.engine == e)
                && voice.is_none_or(|id| v.id == id)
        })
        .collect();
    let usable: Vec<&Voice> = fits.iter().filter(|v| !(local_only && v.network)).collect();
    if let Some(v) = prefer_local(usable.iter().copied().filter(|v| v.available())) {
        return Ok(v.clone());
    }
    if local_only && !fits.is_empty() {
        let alts = fits
            .iter()
            .filter(|v| v.available())
            .take(2)
            .map(|v| alternative(v, true))
            .collect();
        // A local engine exists for this language but is not installed yet.
        // The Rust voice was asked by name: its own setup, not the plugin's.
        let action = if engine == Some(crate::kokoro_rust::ENGINE) {
            "Install the Rust voice once: `scriptvox setup voice-rust` (about 332 MB, no GPL part)."
                .to_string()
        } else if fits.iter().any(|v| !v.network) {
            "Install the local voice once: `scriptvox setup voice-local` (about 180 MB; includes GPL-3.0 parts in a separate process). Or run without --local-only (Edge is a network option).".to_string()
        } else {
            format!("No local voice for '{lang}' in v0.1. Edge is a network option: run without --local-only.")
        };
        return Err(needs_input("tts.engine", "dependency", &action, alts));
    }
    if let Some(action) = engine.and_then(|e| engine_cause(e, lang, voice, &fits)) {
        return Err(needs_input(
            "tts.engine",
            "dependency",
            &action,
            refused_alternatives(lang, engine.unwrap_or_default()),
        ));
    }
    if let Some(paid) = fits.iter().find(|v| v.key.is_some()) {
        return Err(needs_input(
            &format!("tts.{}.api_key", paid.engine),
            "secret",
            &format!(
                "Set {}, or run `scriptvox keys set {}` in your terminal",
                paid.key.as_deref().unwrap(),
                paid.engine
            ),
            refused_alternatives(lang, &paid.engine),
        ));
    }
    Err(needs_input(
        "tts.voice",
        "dependency",
        &match voice {
            Some(id) => format!("Voice {id} does not speak lang '{lang}' or is not in this install. Run `scriptvox voices --lang {lang}`, or give the script's language with --lang."),
            None => format!("No voice matches lang '{lang}'. Languages that exist: {}.", languages().join(", ")),
        },
        vec![],
    ))
}

/// Why a named engine cannot speak here, or None when the next check (a key) is the real cause. Order: the name,
/// the language, then what to install.
fn engine_cause(engine: &str, lang: &str, voice: Option<&str>, fits: &[Voice]) -> Option<String> {
    let rows = all();
    if !rows.iter().any(|v| v.engine == engine) {
        let mut names: Vec<&str> = rows.iter().map(|v| v.engine.as_str()).collect();
        names.dedup();
        return Some(format!(
            "Unknown TTS engine '{engine}'. Valid engines: {}.",
            names.join(", ")
        ));
    }
    if fits.is_empty() {
        // The engine speaks this language and only the named voice is wrong: the caller's line about the voice is true.
        if voice.is_some() && rows.iter().any(|v| v.engine == engine && v.speaks(lang)) {
            return None;
        }
        return Some(format!(
            "The {engine} engine has no voice for lang '{lang}'. Run `scriptvox voices --lang {lang}`."
        ));
    }
    match engine {
        "kokoro" => Some(
            "Install the local voice once: `scriptvox setup voice-local` (about 180 MB; includes GPL-3.0 parts in a separate process)."
                .to_string(),
        ),
        crate::kokoro_rust::ENGINE => Some(
            "Install the Rust voice once: `scriptvox setup voice-rust` (about 332 MB, no GPL part).".to_string(),
        ),
        "selfhost" => Some(
            "This voice needs your own GPU service: deploy `services/gen` (services/gen/README.md), add a `selfhost` row with your host in generators.toml, then run `scriptvox keys set selfhost`."
                .to_string(),
        ),
        _ => None,
    }
}

/// The language codes that have a voice.
fn languages() -> Vec<String> {
    let mut l: Vec<String> = all()
        .iter()
        .filter(|v| v.engine != "none")
        .flat_map(|v| {
            v.lang
                .split(',')
                .map(|x| x.trim().split('-').next().unwrap_or("").to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    l.sort();
    l.dedup();
    l
}

/// A language as a person says it, as a code: `arabic`, `العربية` and `ar` give `ar`.
pub fn lang_code(name: &str) -> String {
    let n = name.trim();
    match n.to_lowercase().as_str() {
        "arabic" | "العربية" | "عربي" | "عربية" | "arab" => "ar".to_string(),
        "english" | "الإنجليزية" | "الانجليزية" | "إنجليزي" | "انجليزي" | "إنجليزية" | "انجليزية" => {
            "en".to_string()
        }
        "french" | "français" | "الفرنسية" | "فرنسي" => "fr".to_string(),
        "spanish" | "español" | "الإسبانية" | "إسباني" => "es".to_string(),
        "german" | "deutsch" | "الألمانية" | "ألماني" => "de".to_string(),
        "portuguese" | "português" | "البرتغالية" | "برتغالي" => "pt".to_string(),
        "indonesian" | "bahasa indonesia" | "indonesia" | "الإندونيسية" | "إندونيسي" => {
            "id".to_string()
        }
        "hindi" | "हिन्दी" | "الهندية" | "هندي" => "hi".to_string(),
        "urdu" | "الأردية" | "أردو" => "ur".to_string(),
        _ => n.to_string(),
    }
}

/// Alternatives after an engine refused: only engines that speak `lang`, never
/// an English-only engine for Arabic.
pub fn refused_alternatives(lang: &str, refused_engine: &str) -> Vec<Value> {
    let mut seen = Vec::new();
    all()
        .into_iter()
        .filter(|v| v.speaks(lang) && v.engine != refused_engine)
        .filter(|v| {
            let first = !seen.contains(&v.engine);
            seen.push(v.engine.clone());
            first
        })
        .map(|v| alternative(&v, false))
        .collect()
}

/// `voices --preview`: what the command line gave.
pub struct Preview<'a> {
    pub id: &'a str,
    pub text: Option<&'a str>,
    pub brief: Option<&'a std::path::Path>,
    pub lang: Option<&'a str>,
    pub tts: Option<&'a str>,
    pub out: Option<std::path::PathBuf>,
    pub approve: Option<&'a str>,
    pub max_spend: Option<f64>,
    pub plan_only: bool,
    pub local_only: bool,
    pub use_system: bool,
}

/// The most characters a preview speaks: a line, not a script.
pub const PREVIEW_MAX_CHARS: usize = 200;

/// The sentence a preview speaks when the user gave no text. A language with no sentence here needs `--text`.
fn preview_default(lang: &str) -> Option<&'static str> {
    match lang.split('-').next() {
        Some("en") => Some("Hello. This is how this voice will sound in your video."),
        Some("ar") => Some("مرحبا. هكذا سيبدو هذا الصوت في الفيديو."),
        Some("es") => Some("Hola. Así va a sonar esta voz en tu vídeo."),
        Some("fr") => Some("Bonjour. Voici comment cette voix sonnera dans votre vidéo."),
        Some("de") => Some("Hallo. So klingt diese Stimme in deinem Video."),
        Some("pt") => Some("Olá. É assim que esta voz vai soar no seu vídeo."),
        Some("id") => Some("Halo. Seperti inilah suara ini akan terdengar di video Anda."),
        _ => None,
    }
}

/// One short audio file of a voice, so a person or an agent hears it before a video (docs/specs/voices-preview.md).
/// It speaks through `make::synthesize`, so the cache, the worker and the ledger are the ones `make` uses. A paid
/// voice stops at the plan, before the cache, the worker and FFmpeg.
pub fn preview(a: &Preview) -> crate::envelope::Envelope {
    use crate::envelope::Envelope;
    use crate::exit::Exit;
    use crate::{make, plan};
    let usage =
        |msg: String, fix: &str| Envelope::fail("voices", Exit::Usage, msg, Some(fix.to_string()));
    let needs = |item: Value| {
        let action = item["human_action"].as_str().unwrap_or("input needed");
        let mut env = Envelope::fail("voices", Exit::NeedsInput, action, None);
        env.needs_input = vec![item];
        env
    };
    let Some(row) = all().into_iter().find(|v| v.id == a.id) else {
        return usage(
            format!("no voice has the id {}", a.id),
            "Run `scriptvox voices --json` and give one of its ids.",
        );
    };
    let brief = match a.brief.map(crate::brief::Brief::load) {
        Some(Err(e)) => {
            let fix = crate::brief::load_fix(&e, a.brief.unwrap_or(std::path::Path::new("")));
            return usage(e, &fix);
        }
        Some(Ok(b)) => b,
        None => crate::brief::Brief::default(),
    };
    // The same gate as `make`: a brief cannot make this command read a file outside the project (its `script_file`
    // would be read here, and its first sentence sent to a voice service).
    if let Some(p) = crate::brief::file_outside_project(&brief) {
        return usage(
            format!("{} is outside this project folder", p.display()),
            "Use files inside the folder you run scriptvox in.",
        );
    }
    // The brief's first sentence, as `make` gets its text. `--text` is said whole.
    let given = match a.text {
        Some(t) => Some(crate::pauses::strip(t)),
        None => match crate::brief::script_text(brief.script_file.as_deref(), &brief) {
            Err(e) => return usage(e, "Give a brief with a script, or use --text."),
            Ok(script) => script.map(|s| {
                let spoken = crate::pauses::strip(&s);
                let words: Vec<&str> = spoken.split_whitespace().collect();
                let first = crate::captions::sentences(&words).into_iter().next();
                first.map_or(String::new(), |r| words[r].join(" "))
            }),
        },
    }
    .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
    if a.brief.is_some() && given.is_none() {
        return usage(
            "the brief has no script".into(),
            "Give a brief with a script, or use --text.",
        );
    }
    let lang = a
        .lang
        .map(lang_code)
        .or(brief.lang.as_deref().map(lang_code))
        .or(given
            .as_deref()
            .map(|t| make::detect_lang(t).to_string())
            .filter(|l| row.speaks(l)))
        .unwrap_or(
            row.lang
                .split(',')
                .next()
                .unwrap_or("en")
                .trim()
                .to_string(),
        );
    let Some(text) = given.or(preview_default(&lang).map(String::from)) else {
        return usage(
            format!("there is no built-in sentence for lang '{lang}'"),
            "Give the words with --text.",
        );
    };
    let chars = text.chars().count();
    if chars == 0 || chars > PREVIEW_MAX_CHARS {
        return usage(
            format!("the preview text has {chars} characters; a preview speaks 1 to {PREVIEW_MAX_CHARS}"),
            "Give one short line with --text. A whole script is heard with `scriptvox make`.",
        );
    }
    let voice = match choose(&lang, a.tts, Some(a.id), a.local_only) {
        // The brief's `say` forms ride on the voice, as in `make`.
        Ok(v) => Voice {
            say: brief.say.clone(),
            ..v
        },
        Err(item) => return needs(item),
    };
    if voice.engine == "none" {
        return usage(
            format!(
                "{} is the row for no voice: there is nothing to hear",
                voice.id
            ),
            "Pick a voice from `scriptvox voices --json`.",
        );
    }
    // The plan: the same price rule as the voice stage of `make`, on what the voice says.
    let voiced = crate::say::apply(&text, &voice.say);
    let listed = voice.usd_per_1k_chars.map_or(0.0, |p| {
        (voiced.chars().count() as f64 / 1000.0 * p * 100.0).ceil() / 100.0
    });
    // A preview that is in the voice cache is no call and no charge, as in `make` (docs/specs/voice-plan-cache.md).
    let segs = [crate::pauses::Seg::Speech(text.clone())];
    let made = listed > 0.0
        && make::voice_parts(&voice, &[], &segs, None)
            .iter()
            .all(|p| p.cached);
    let cost = if made { 0.0 } else { listed };
    if voice.cost != "free" && listed <= 0.0 {
        return Envelope::fail(
            "voices",
            Exit::Error,
            format!(
                "the paid voice {} has no price in the registry, so a preview cannot plan it",
                voice.id
            ),
            Some("Preview a free voice: `scriptvox voices --json` lists them.".into()),
        );
    }
    let tool = format!("tts.{}", voice.engine);
    let plan_id = plan::plan_id(
        &json!({"command": "voices --preview", "engine": voice.engine, "voice": voice.id,
        "model": voice.model, "text": voiced, "usd_per_1k_chars": voice.usd_per_1k_chars}),
    );
    let stages = json!([plan::Stage {
        stage: "voice",
        tool,
        cost_usd: cost,
        network: voice.network,
        explain: if made {
            format!("{}; already made: no call and no charge", voice.explain())
        } else {
            voice.explain()
        },
    }]);
    let again = format!(
        "scriptvox voices --preview {} ... --approve {plan_id}",
        voice.id
    );
    if a.approve != Some(plan_id.as_str()) {
        if let Some(env) = plan::over_cap("voices", &plan_id, cost, a.max_spend, vec![]) {
            return env.with("stages", stages);
        }
    }
    if a.plan_only {
        let mut env = Envelope::ok("voices");
        env.plan_id = Some(plan_id);
        env.next = vec![if cost > 0.0 {
            again
        } else {
            "Run the same command without --plan.".to_string()
        }];
        env.metrics.insert("cost_usd".into(), json!(cost));
        env.metrics.insert("chars".into(), json!(chars));
        return env.with("text", json!(text)).with("stages", stages);
    }
    if let Some(why) = plan::needs_approval(&plan_id, cost, a.approve) {
        let mut env = Envelope::fail(
            "voices",
            Exit::ApprovalNeeded,
            format!(
                "approval needed: {} (${cost:.2})",
                why["reason"].as_str().unwrap_or("")
            ),
            Some(format!("Run the same command with --approve {plan_id}")),
        );
        env.status = "approval_needed";
        env.next = vec![again];
        env.plan_id = Some(plan_id);
        env.metrics.insert("cost_usd".into(), json!(cost));
        return env.with("stages", stages);
    }
    let ff = match crate::engine_ffmpeg::Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("voices", e),
    };
    // The worker's files (audio and word times) stay in a scratch folder: only the audio goes out.
    let scratch = std::path::Path::new(".scriptvox/preview");
    let _ = std::fs::remove_dir_all(scratch);
    // Without an approval of this plan, `--max-spend` alone buys nothing past the plan's cost.
    let ledger = plan::Ledger::new(
        &plan::run_id_for(&plan_id, "voices"),
        a.max_spend
            .filter(|_| a.approve == Some(plan_id.as_str()))
            .unwrap_or(cost),
    );
    let made = make::synthesize(
        &ff,
        &voice,
        &[],
        &segs,
        scratch,
        a.local_only,
        None,
        1.0,
        &ledger,
        false,
    );
    let audio = match made {
        Ok((audio, _)) => audio,
        Err(e) => {
            let mut env = match e.code {
                plan::RESERVE_REFUSED => Envelope::fail_code(
                    "voices",
                    Exit::ApprovalNeeded,
                    "SVX_SPEND_CAP",
                    e.message,
                    Some("Raise --max-spend.".into()),
                ),
                make::PROVIDER_REFUSED => needs(needs_input(
                    "tts.voice",
                    "dependency",
                    &format!("{}. Pick a voice that speaks '{lang}'.", e.message),
                    refused_alternatives(&lang, &voice.engine),
                )),
                make::PROVIDER_RETRYABLE => Envelope::fail(
                    "voices",
                    Exit::ProviderRetryable,
                    e.message,
                    Some("Run the same command again.".into()),
                ),
                _ => Envelope::fail(
                    "voices",
                    Exit::Error,
                    e.message,
                    Some(
                        "Run `scriptvox doctor` (or `scriptvox worker`) to check the worker."
                            .into(),
                    ),
                ),
            };
            env.plan_id = Some(plan_id);
            return env;
        }
    };
    // The local voices write WAV, and the cache gives every voice back under an .mp3 name: the bytes say which.
    let wav = std::fs::read(&audio).is_ok_and(|b| b.starts_with(b"RIFF"));
    let ext = if wav { "wav" } else { "mp3" };
    let same = |p: &std::path::Path| {
        crate::store::hash_file(p).ok() == crate::store::hash_file(&audio).ok()
    };
    let out = match &a.out {
        Some(p) if p.exists() && !same(p) => {
            return usage(
                format!("{} exists and is a different file", p.display()),
                "Give another --out, or remove that file.",
            )
        }
        Some(p) => p.clone(),
        // The same sound keeps its name; a different file at that name is never replaced.
        None => {
            let id: String = voice
                .id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            (1..)
                .map(|n| match n {
                    1 => format!("voice-{id}.{ext}"),
                    n => format!("voice-{id}-{n}.{ext}"),
                })
                .map(|name| std::path::PathBuf::from("scriptvox-out").join(name))
                .find(|p| !p.exists() || same(p))
                .unwrap()
        }
    };
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::copy(&audio, &out) {
        return Envelope::fail(
            "voices",
            Exit::Error,
            format!("cannot write {}: {e}", out.display()),
            Some("Give an --out in a folder you can write.".into()),
        );
    }
    let secs = ff.duration(&out).unwrap_or(0.0);
    let mut env = Envelope::ok("voices");
    env.plan_id = Some(plan_id);
    let shown = std::path::absolute(&out).unwrap_or(out);
    env.artifacts
        .insert("audio".into(), json!(shown.display().to_string()));
    env.metrics
        .insert("duration_s".into(), json!((secs * 1000.0).round() / 1000.0));
    env.metrics.insert("chars".into(), json!(chars));
    env.metrics.insert("cost_usd".into(), json!(cost));
    env.next = vec![format!(
        "scriptvox make --script script.txt --tts {} --voice {} --json",
        voice.engine, voice.id
    )];
    env.with("text", json!(text))
        .with("explain", json!(voice.explain()))
        .with(
            "voice",
            json!({"engine": voice.engine, "id": voice.id, "lang": lang, "terms": voice.terms, "service": voice.service()}),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_voice_id_is_named_and_a_paid_alternative_has_no_price_sentinel() {
        let e = choose("en", Some("edge"), Some("no-such-voice"), false).unwrap_err();
        let action = e["human_action"].as_str().unwrap();
        assert!(action.starts_with("Voice no-such-voice"), "{action}");
        let alts = refused_alternatives("en", "edge");
        let paid = alts.iter().find(|a| a["args"][1] == "elevenlabs").unwrap();
        assert!(paid["cost_usd"].is_null(), "{paid}");
        assert_eq!(paid["paid"], true);
        let free = all().into_iter().find(|v| v.cost == "free").unwrap();
        let free = alternative(&free, false);
        assert_eq!(free["cost_usd"], 0);
        assert_eq!(free["paid"], false);
    }

    #[test]
    fn a_network_voice_says_where_the_script_goes() {
        let v = all();
        let edge = v.iter().find(|v| v.engine == "edge").unwrap();
        assert!(edge.explain().contains("the script text goes to Microsoft"));
        let local = v.iter().find(|v| v.engine == "kokoro").unwrap();
        assert!(!local.explain().contains("goes to"));
    }

    #[test]
    fn a_local_voice_wins_over_edge_when_both_can_run() {
        let en: Vec<Voice> = all().into_iter().filter(|v| v.speaks("en")).collect();
        assert!(en[0].network, "Edge is first in the registry");
        assert_eq!(prefer_local(en.iter()).unwrap().engine, "kokoro");
        let net: Vec<&Voice> = en.iter().filter(|v| v.network).collect();
        assert_eq!(prefer_local(net.into_iter()).unwrap().id, en[0].id);
    }

    #[test]
    fn the_rust_voice_is_never_the_default_and_answers_when_asked() {
        // Not installed in tests, so `--tts kokoro-rust` says what to install; with no engine named it is never picked.
        assert_ne!(
            choose("en", None, None, false).unwrap().engine,
            crate::kokoro_rust::ENGINE
        );
        let row = all()
            .into_iter()
            .find(|v| v.engine == crate::kokoro_rust::ENGINE)
            .unwrap();
        assert!(!row.network && row.lang == "en" && row.commercial);
        assert!(!row.available());
    }

    #[test]
    fn a_named_engine_gets_its_real_cause() {
        let cause = |e: &str, lang: &str| {
            choose(lang, Some(e), None, false).unwrap_err()["human_action"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert!(
            cause("foo", "en").contains("Valid engines: edge"),
            "{}",
            cause("foo", "en")
        );
        assert!(cause("kokoro", "ar").contains("no voice for lang 'ar'"));
        assert!(cause("kokoro-rust", "en").contains("setup voice-rust"));
        assert!(cause("selfhost", "en").contains("services/gen"));
    }

    #[test]
    fn a_language_name_becomes_its_code() {
        assert_eq!(lang_code("Arabic"), "ar");
        assert_eq!(lang_code("العربية"), "ar");
        assert_eq!(lang_code("الإنجليزية"), "en");
        assert_eq!(lang_code("ar"), "ar");
        let e = choose("zz", None, None, false).unwrap_err();
        assert!(
            e["human_action"]
                .as_str()
                .unwrap()
                .contains("Languages that exist: "),
            "{e}"
        );
    }

    #[test]
    fn edge_is_default_and_explains_who_gets_the_text_not_its_terms() {
        let v = choose("ar", None, None, false).unwrap();
        assert_eq!(v.engine, "edge");
        // Owner, 13 Nov 2026: no line about commercial use; the terms stay as data on the row.
        let line = v.explain();
        assert!(
            line.contains("the script text goes to") && !line.contains("commercial"),
            "{line}"
        );
        assert!(v.terms.contains("non-commercial") && !v.commercial);
    }

    #[test]
    fn local_only_never_picks_a_network_voice() {
        let e = choose("en", Some("edge"), None, true).unwrap_err();
        assert!(e["human_action"]
            .as_str()
            .unwrap()
            .contains("Edge is a network option"));
        assert_eq!(e["alternatives"][0]["args"][1], "edge");
    }

    #[test]
    fn arabic_refusal_offers_no_english_only_engine() {
        let alts = refused_alternatives("ar", "edge");
        assert!(!alts.is_empty());
        for a in &alts {
            let engine = a["args"][1].as_str().unwrap();
            assert!(
                all().iter().any(|v| v.engine == engine && v.speaks("ar")),
                "{engine}"
            );
            assert_ne!(engine, "edge");
        }
    }

    #[test]
    fn every_row_has_terms() {
        assert!(all().iter().all(|v| !v.terms.is_empty()));
    }

    #[test]
    fn a_paid_voice_says_when_its_price_was_read() {
        let mut paid = all().into_iter().find(|v| v.cost == "paid").unwrap();
        assert!(all()
            .iter()
            .filter(|v| v.usd_per_1k_chars.is_some())
            .all(|v| v.price_checked.is_some() && v.price_source.is_some()));
        paid.price_checked = Some("2020-01-01".into());
        assert!(paid.explain().contains("days old: check https://"));
        paid.price_checked = Some("2999-01-01".into());
        assert!(!paid.explain().contains("days old"));
    }
}
