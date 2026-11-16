//! A voice row refuses a sample with no consent record (G036). Stops before any network: the row has no key here.
mod common;
use serde_json::Value;

fn run(dir: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let o = common::std_bin()
        .args(args)
        .arg("--json")
        .current_dir(dir)
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env("SCRIPTVOX_SELFHOST_KEY", "test-not-a-key")
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

#[test]
fn a_clone_needs_a_record_of_this_very_sample() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path();
    std::fs::write(d.join("v.wav"), b"sample").unwrap();
    std::fs::write(d.join("proof.pdf"), b"signed").unwrap();
    let call = [
        "generate",
        "--model",
        "chatterbox-mtl-self",
        "--prompt",
        "Hello",
        "--audio",
        "v.wav",
        "--plan",
    ];
    let (code, v) = run(d, &call);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "consent");
    let (code, v) = run(
        d,
        &[
            "voices",
            "add",
            "--sample",
            "v.wav",
            "--consent",
            "gone.pdf",
        ],
    );
    assert_eq!(code, 2, "{v}");
    let (code, v) = run(
        d,
        &[
            "voices",
            "add",
            "--sample",
            "v.wav",
            "--consent",
            "proof.pdf",
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert!(v["consent"]["id"].as_str().unwrap().starts_with("c_"));
    let (code, v) = run(d, &call);
    assert_ne!(v["needs_input"][0]["kind"], "consent", "{v}");
    assert_ne!(code, 10, "{v}");
    std::fs::write(d.join("v.wav"), b"sample changed").unwrap();
    assert_eq!(run(d, &call).0, 10);
}

/// Your own voice is one step: `--own` records your statement by hash, no proof file (owner, 7 Nov 2026). Another
/// person's voice still needs `--consent`; the two flags do not go together.
#[test]
fn your_own_voice_needs_only_your_statement() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path();
    std::fs::write(d.join("me.wav"), b"my sample").unwrap();
    let call = [
        "generate",
        "--model",
        "chatterbox-mtl-self",
        "--prompt",
        "Hello",
        "--audio",
        "me.wav",
        "--plan",
    ];
    let (code, v) = run(d, &call);
    assert_eq!(code, 10, "{v}");
    assert!(
        v["needs_input"][0]["human_action"]
            .as_str()
            .unwrap()
            .contains("--own"),
        "{v}"
    );
    let (code, v) = run(d, &["voices", "add", "--sample", "me.wav"]);
    assert_eq!(code, 2, "no --own and no --consent: {v}");
    let (code, _) = run(
        d,
        &[
            "voices",
            "add",
            "--sample",
            "me.wav",
            "--own",
            "--consent",
            "me.wav",
        ],
    );
    assert_eq!(code, 2);
    let (code, v) = run(d, &["voices", "add", "--sample", "me.wav", "--own"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["consent"]["own"], true);
    assert!(
        v["consent"]["proof_hash"].is_null() && v["consent"]["statement_hash"].is_string(),
        "{v}"
    );
    let (code, v) = run(d, &call);
    assert_ne!(code, 10, "{v}");
}
