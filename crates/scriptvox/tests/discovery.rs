//! Self-discovery commands (RVX-022): JSON shapes locked by snapshots.
mod common;
use serde_json::Value;

fn run(args: &[&str]) -> Value {
    let o = common::bin()
        .env_remove("PEXELS_API_KEY")
        .env_remove("PIXABAY_API_KEY")
        .env_remove("ELEVENLABS_API_KEY")
        .env_remove("FAL_KEY")
        .env_remove("REPLICATE_API_TOKEN")
        .env("SCRIPTVOX_CONFIG", "/nonexistent-scriptvox-config")
        .env_remove("CLAUDECODE")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(o.status.success());
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn providers_show_output_terms() {
    let v = run(&["providers"]);
    for p in v["providers"].as_array().unwrap() {
        assert!(p["output_terms"]["redistribution"].is_boolean(), "{p}");
    }
    insta::assert_json_snapshot!("providers", v["providers"]);
}

#[test]
fn providers_list_generators_with_their_price_and_source() {
    let v = run(&["providers"]);
    let g = v["generators"].as_array().unwrap();
    let kling = g.iter().find(|x| x["name"] == "kling-2.6-pro").unwrap();
    assert_eq!(kling["usd_per_s"], 0.07);
    assert_eq!(kling["key"], "FAL_KEY");
    assert_eq!(kling["key_set"], false);
    assert!(kling["price_source"]
        .as_str()
        .unwrap()
        .starts_with("https://"));
    assert_eq!(v["generator_errors"], serde_json::json!([]));
}

#[test]
fn a_bad_generator_row_is_reported_and_never_used() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("g.toml");
    let good = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../registry/generators.toml"
    ))
    .unwrap();
    std::fs::write(&f, good.replacen("usd_per_s = 0.07", "usd_per_s = 0", 1)).unwrap();
    let o = common::bin()
        .env("SCRIPTVOX_GENERATORS", &f)
        .env("SCRIPTVOX_CONFIG", "/nonexistent-scriptvox-config")
        .args(["providers", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let errs = v["generator_errors"].as_array().unwrap();
    assert!(
        errs.iter()
            .any(|e| e.as_str().unwrap().contains("kling-2.6-pro: usd_per_s")),
        "{v}"
    );
    assert!(v["generators"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| g["name"] != "kling-2.6-pro"));
}

#[test]
fn voices_filter_by_language() {
    let v = run(&["voices", "--lang", "ar"]);
    let list = v["voices"].as_array().unwrap();
    assert!(!list.is_empty());
    assert!(list
        .iter()
        .all(|x| x["lang"].as_str().unwrap().split(',').any(|l| l == "ar")));
    insta::assert_json_snapshot!("voices_ar", v["voices"]);
}

#[test]
fn tools_and_templates_are_listed() {
    insta::assert_json_snapshot!("tools", run(&["tools"])["tools"]);
    insta::assert_json_snapshot!("templates", run(&["templates"])["templates"]);
}

#[test]
fn a_row_with_no_key_says_so_first_and_the_plan_stays_valid() {
    let d = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_CONFIG", d.path())
            .env("SCRIPTVOX_DATA", d.path())
            .env_remove("FAL_KEY")
            .env_remove("SCRIPTVOX_GENERATORS")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        (
            o.status.code(),
            serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        )
    };
    let (_, v) = run(&["providers"]);
    let row = v["generators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "kling-2.6-pro")
        .unwrap()
        .clone();
    assert_eq!(row["runnable"], false, "{row}");
    assert!(
        row["why_not"]
            .as_str()
            .unwrap()
            .contains("scriptvox keys set fal"),
        "{row}"
    );
    let (code, v) = run(&[
        "generate",
        "--model",
        "kling-2.6-pro",
        "--prompt",
        "x",
        "--seconds",
        "5",
        "--plan",
    ]);
    assert_eq!(code, Some(0), "{v}");
    assert!(v["plan_id"].is_string(), "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "secret", "{v}");
    assert!(
        v["next"][0]
            .as_str()
            .unwrap()
            .contains("scriptvox keys set fal"),
        "{v}"
    );
    // Approval does not change: still exit 4, with the key step first.
    let (code, v) = run(&[
        "generate",
        "--model",
        "kling-2.6-pro",
        "--prompt",
        "x",
        "--seconds",
        "5",
    ]);
    assert_eq!(code, Some(4), "{v}");
    assert!(
        v["next"][0]
            .as_str()
            .unwrap()
            .contains("scriptvox keys set fal"),
        "{v}"
    );
}
