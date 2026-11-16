//! Schemas (RVX-025): generated files are fresh, the released result schema
//! loses no field, and every recipe is a valid brief.
use scriptvox::brief::{schema, Brief, SCHEMAS};
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(p: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{}", p.display())))
        .unwrap()
}

#[test]
fn generated_schemas_are_not_stale() {
    for name in SCHEMAS {
        assert_eq!(
            read(root().join(format!("schemas/{name}.json"))),
            schema(name).unwrap(),
            "schemas/{name}.json is stale: run `just schemas`"
        );
    }
}

/// Every property path of a schema, for example `properties.error`.
fn props(v: &Value, prefix: &str, out: &mut Vec<String>) {
    if let Some(p) = v.get("properties").and_then(Value::as_object) {
        for (k, sub) in p {
            let path = format!("{prefix}{k}");
            out.push(path.clone());
            props(sub, &format!("{path}."), out);
        }
    }
    if let Some(defs) = v.get("$defs").and_then(Value::as_object) {
        for (k, sub) in defs {
            props(sub, &format!("$defs.{k}."), out);
        }
    }
}

#[test]
fn released_result_fields_are_never_removed() {
    let (mut old, mut new) = (vec![], vec![]);
    props(
        &read(root().join("schemas/released/result.json")),
        "",
        &mut old,
    );
    props(&schema("result").unwrap(), "", &mut new);
    let gone: Vec<&String> = old.iter().filter(|p| !new.contains(p)).collect();
    assert!(
        gone.is_empty(),
        "removed from scriptvox.result/v1: {gone:?} (a removal needs result/v2)"
    );
}

#[test]
fn every_recipe_is_a_valid_brief() {
    let mut n = 0;
    for e in std::fs::read_dir(root().join("recipes")).unwrap() {
        let p = e.unwrap().path();
        Brief::load(&p).unwrap_or_else(|err| panic!("{err}"));
        n += 1;
    }
    assert_eq!(n, 7);
}

#[test]
fn a_brief_needs_exactly_one_script_source() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("b.json");
    std::fs::write(&p, r#"{"script": "x", "scenes": [{"script": "y"}]}"#).unwrap();
    assert!(Brief::load(&p).unwrap_err().contains("exactly one"));
}

#[test]
fn unknown_brief_fields_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("b.json");
    std::fs::write(&p, r#"{"script": "x", "voise": "typo"}"#).unwrap();
    assert!(Brief::load(&p).unwrap_err().contains("voise"));
}
