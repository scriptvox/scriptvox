//! `setup speech-local --arabic`: the Arabic model beside the present one (docs/specs/speech-local-arabic.md).

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::provision as p;
use serde_json::{json, Value};
use std::path::Path;

/// Licence data of the Arabic model, for `providers`, `doctor` and credits. Kept as data, never shown in the flow.
pub fn listing() -> Value {
    json!({
        "installed": p::whisper_arabic_ready(),
        "version": p::pin(p::WHISPER_ARABIC_PINS[0]).map(|x| x.version),
        "used_for": "speech to text when the language is ar",
        "base_model": {"name": "openai/whisper-small", "licence": "Apache-2.0 (model card), MIT (code)"},
        "training_data": [
            {"name": "Meta Omnilingual ASR Corpus", "licence": "CC-BY-4.0"},
            {"name": "Google FLEURS", "licence": "CC-BY-4.0"},
            {"name": "LibriSpeech", "licence": "CC-BY-4.0"},
            {"name": "Mozilla Common Voice 18", "licence": "CC0-1.0"},
        ],
        "attribution_required": true,
        "add": "scriptvox setup speech-local --arabic",
        "download": "https://huggingface.co/scriptvox/whisper-small-ar",
    })
}

/// Runs after the present model's setup (`present`, already ok). Checks the source of the Arabic files first.
pub fn setup(dry_run: bool, from: Option<&Path>, present: impl FnOnce() -> Envelope) -> Envelope {
    let pins: Vec<p::Pin> = p::WHISPER_ARABIC_PINS
        .iter()
        .filter_map(|n| p::pin(n))
        .collect();
    // Files already in the tool folders (an earlier setup) need no address: they are linked again.
    let no_source = pins
        .iter()
        .any(|x| x.url.is_empty() && p::installed(x).is_none());
    if from.is_none() && no_source {
        let mut env = Envelope::fail(
            "setup",
            Exit::NeedsInput,
            "the Arabic speech model has no download address yet",
            Some("Give a local folder with its 5 files: `scriptvox setup speech-local --arabic --from <folder>`.".into()),
        );
        env.needs_input = vec![json!({"field": "from", "kind": "path",
            "human_action": "a folder with config.json, model.bin, preprocessor_config.json, tokenizer.json, vocabulary.json"})];
        return env;
    }
    if dry_run {
        let mut env = present();
        // A download (no `--from`) adds the Arabic files to the list and to the bytes of the disk check.
        if from.is_none() {
            let ar = crate::doctor::setup_dry_run(&pins);
            if let (Value::Array(rows), Some(Value::Array(more))) = (
                env.extra.entry("downloads").or_insert(json!([])),
                ar.extra.get("downloads"),
            ) {
                rows.extend(more.iter().cloned());
            }
            let n = |e: &Envelope, k: &str| e.metrics.get(k).and_then(Value::as_u64).unwrap_or(0);
            let bytes = n(&env, "download_bytes") + n(&ar, "download_bytes");
            env.metrics.insert("download_bytes".into(), json!(bytes));
            let free = env.metrics.get("disk_free_bytes").and_then(Value::as_u64);
            env.findings.retain(|f| f["gate"] != "disk");
            if free.is_some_and(|f| f < bytes) {
                env.findings.push(json!({"gate": "disk", "status": "warn",
                    "detail": format!("{bytes} bytes to download, {} free", free.unwrap_or(0)),
                    "fix": "Free some disk space, or set SCRIPTVOX_DATA to a folder on a larger disk."}));
            }
        }
        return env.with(
            "arabic",
            json!({"source": from.map_or_else(|| json!("download"), |f| json!(f)),
                "files": pins.iter().map(|x| json!({"name": x.name, "bytes": x.size, "sha256": x.sha256})).collect::<Vec<_>>()}),
        );
    }
    // The local files are checked before the present model's download starts.
    if let Some(f) = from {
        if let Err(e) = p::install_from_folder(&p::WHISPER_ARABIC_PINS, f) {
            return crate::doctor::setup_failure(e);
        }
    }
    let env = present();
    if !env.ok {
        return env;
    }
    match p::install_whisper(&p::WHISPER_ARABIC_PINS, "model-ar", &|m| {
        eprintln!("scriptvox: {m}")
    }) {
        Ok(()) => env
            .with("speech_arabic", json!("installed"))
            .with("arabic", listing()),
        Err(e) => crate::doctor::setup_failure(e),
    }
}
