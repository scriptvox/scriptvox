//! The OpenAPI 3.1 document of the planned `scriptvox serve` (G166, `docs/plan/MILESTONE-API.md`). The paths are
//! written here by hand; the bodies are the same schemars schemas as `scriptvox schema brief|result|event`, so the
//! document cannot drift from the types. `scriptvox schema openapi` prints it, `just schemas` writes
//! `schemas/openapi.json`, and a test checks that every `$ref` resolves.

use serde_json::{json, Map, Value};

/// Method, path, scope (None: no token), operation id, summary, request body schema.
type Route = (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static str,
    Option<&'static str>,
);

const ROUTES: &[Route] = &[
    ("get", "/v1/health", None, "health", "The server answers.", None),
    ("get", "/v1/openapi.json", None, "openapi", "This document.", None),
    ("get", "/v1/providers", Some("read"), "providers", "Rows, keys set, prices and terms.", None),
    ("get", "/v1/tools", Some("read"), "tools", "Tools and what each one does.", None),
    ("get", "/v1/voices", Some("read"), "voices", "Voices by engine and language.", None),
    ("get", "/v1/templates", Some("read"), "templates", "Built-in templates.", None),
    ("get", "/v1/licenses", Some("read"), "licenses", "Licences of scriptvox and its parts.", None),
    ("get", "/v1/doctor", Some("read"), "doctor", "What this install has and lacks.", None),
    ("post", "/v1/plans", Some("read"), "plan", "Plan a brief: stages, cost and plan_id. Nothing runs.", Some("Brief")),
    ("post", "/v1/runs", Some("write"), "run", "Start a run from a brief (202 and run_id). Paid plans need an approve first. Send an Idempotency-Key header.", Some("Brief")),
    ("get", "/v1/runs", Some("read"), "runs", "Runs of this server.", None),
    ("get", "/v1/runs/{id}", Some("read"), "run_status", "One run: status, result when done.", None),
    ("get", "/v1/runs/{id}/events", Some("read"), "run_events", "The run's events after `after` (one JSON event per line).", None),
    ("post", "/v1/runs/{id}/cancel", Some("write"), "run_cancel", "Stop a run.", None),
    ("get", "/v1/runs/{id}/files/{name}", Some("read"), "run_file", "Download a file the run made.", None),
    ("post", "/v1/uploads", Some("write"), "upload", "Put a media file into the server project; returns a media:// id.", None),
    ("post", "/v1/plans/{plan_id}/approve", Some("spend"), "approve", "Approve this exact plan with a max_spend. A token without the spend scope gets 403.", None),
    ("post", "/v1/tools/{name}", Some("write"), "tool", "Any other tool (edit, join, export, generate and more), by its MCP arguments. Its scope follows the tool.", None),
    ("post", "/v1/tokens", Some("admin"), "token_add", "Make a token; it is shown once.", None),
    ("delete", "/v1/tokens/{id}", Some("admin"), "token_revoke", "Revoke a token.", None),
];

/// Exit codes as HTTP statuses (`docs/plan/MILESTONE-API.md`). Every answer body is a `Result` envelope.
const STATUS: &[(&str, &str)] = &[
    ("200", "Done (exit 0)."),
    ("202", "Started or still running (exit 9); poll the run."),
    ("400", "Usage error (exit 2)."),
    ("401", "No token, or a token that is not valid."),
    (
        "402",
        "Approval needed (exit 4): `plan_id` and the cost are in the body.",
    ),
    ("403", "The token lacks the scope."),
    ("409", "Cancelled (exit 8)."),
    (
        "422",
        "A gate failed (exit 3), or input is needed (exit 10, `needs_input`).",
    ),
    ("429", "Too many runs at once; see Retry-After."),
    ("500", "Error (exit 1, 6 or 70)."),
    (
        "503",
        "A provider can be retried (exit 5); see Retry-After.",
    ),
];

/// Schemas with their own `$ref`s pointed at `#/components/schemas/...`.
fn components() -> Map<String, Value> {
    let mut s = schemars::generate::SchemaSettings::draft2020_12();
    s.definitions_path = "/components/schemas".into();
    let mut g = s.into_generator();
    let _ = g.subschema_for::<crate::brief::Brief>();
    let _ = g.subschema_for::<crate::envelope::Envelope>();
    let mut out = g.take_definitions(true);
    // The paths call the envelope `Result`, as `scriptvox schema result` does.
    if let Some(v) = out.remove("Envelope") {
        out.insert("Result".into(), v);
    }
    let mut event: Value =
        serde_json::from_str(include_str!("../../../schemas/event.json")).unwrap_or_default();
    if let Some(o) = event.as_object_mut() {
        o.remove("$schema");
        o.remove("$id");
    }
    out.insert("Event".into(), event);
    out
}

fn op(&(_, path, scope, id, summary, body): &Route) -> Value {
    let params: Vec<Value> = path
        .split('/')
        .filter_map(|p| p.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
        .map(|p| json!({"name": p, "in": "path", "required": true, "schema": {"type": "string"}}))
        .collect();
    let result = json!({"application/json": {"schema": {"$ref": "#/components/schemas/Result"}}});
    let responses: Map<String, Value> = STATUS
        .iter()
        .map(|(code, what)| {
            (
                (*code).into(),
                json!({"description": what, "content": result}),
            )
        })
        .collect();
    let mut o = json!({
        "operationId": id,
        "summary": summary,
        "security": scope.map_or(json!([]), |s| json!([{"bearer": [s]}])),
        "responses": responses,
    });
    if !params.is_empty() {
        o["parameters"] = json!(params);
    }
    if let Some(b) = body {
        o["requestBody"] = json!({"required": true,
            "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{b}")}}}});
    }
    if id == "run_events" {
        o["parameters"].as_array_mut().into_iter().for_each(|p| {
            p.push(json!({"name": "after", "in": "query", "required": false,
                "description": "Events after this line number.", "schema": {"type": "integer", "minimum": 0}}))
        });
        o["responses"]["200"]["content"] = json!({"application/x-ndjson":
            {"schema": {"type": "string"}, "x-item-schema": {"$ref": "#/components/schemas/Event"}}});
    }
    o
}

/// The whole document.
pub fn spec() -> Value {
    let mut paths = Map::new();
    for r in ROUTES {
        let entry = paths.entry(r.1).or_insert_with(|| json!({}));
        entry[r.0] = op(r);
    }
    json!({
        "openapi": "3.1.0",
        "jsonSchemaDialect": "https://json-schema.org/draft/2020-12/schema",
        "info": {
            "title": "scriptvox serve",
            "version": "1",
            "summary": "A plan and its approval before any paid call. Planned: `scriptvox serve` is not built yet.",
            "license": {"name": "Apache-2.0", "identifier": "Apache-2.0"}
        },
        "servers": [{"url": "http://127.0.0.1:7878"}],
        "paths": paths,
        "components": {
            "schemas": components(),
            "securitySchemes": {"bearer": {"type": "http", "scheme": "bearer",
                "description": "A token from `scriptvox serve token add`. Scopes: read, write, spend, admin."}}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
        match v {
            Value::Object(o) => {
                if let Some(r) = o.get("$ref").and_then(Value::as_str) {
                    out.push(r);
                }
                o.values().for_each(|x| refs(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| refs(x, out)),
            _ => {}
        }
    }

    #[test]
    fn every_ref_resolves_and_every_route_is_there() {
        let doc = spec();
        let mut all = vec![];
        refs(&doc, &mut all);
        assert!(all.len() > 20, "{}", all.len());
        for r in all {
            let ptr = r
                .strip_prefix('#')
                .unwrap_or_else(|| panic!("not local: {r}"));
            assert!(doc.pointer(ptr).is_some(), "{r} does not resolve");
        }
        for (m, p, ..) in ROUTES {
            assert!(doc["paths"][p][m].is_object(), "{m} {p}");
        }
        assert_eq!(doc["paths"]["/v1/health"]["get"]["security"], json!([]));
        assert_eq!(
            doc["paths"]["/v1/plans/{plan_id}/approve"]["post"]["security"][0]["bearer"][0],
            "spend"
        );
    }
}
