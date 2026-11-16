# OpenAPI 3.1 document (G166, 8 Nov 2026)

Second part of `docs/plan/MILESTONE-API.md`. The plan says this part can start before the owner's Go; the server itself is not built, and the document says so in `info.summary`.

- `crates/scriptvox/src/openapi.rs`: the 20 routes of the plan's API table, written by hand (method, path, scope, operation id, summary, body). Path parameters come from the `{...}` parts. Each route's security is `bearer` with its scope; `/v1/health` and `/v1/openapi.json` have none.
- Bodies are the schemars types, not a second model: `Brief`, `Result` (the envelope) and `Event` (`schemas/event.json`), with every `$ref` at `#/components/schemas/...` (schemars `definitions_path`). Draft 2020-12 is the 3.1 dialect, named in `jsonSchemaDialect`.
- Every status of the plan's exit to HTTP table is listed on each route, each with a `Result` body. `/v1/runs/{id}/events` answers `application/x-ndjson`, with `x-item-schema` for one `Event` (`itemSchema` is OpenAPI 3.2, so not used).
- `scriptvox schema openapi` prints it; `just schemas` writes `schemas/openapi.json`, and the stale-schema test covers it with the other three.

## Measured, 8 Nov 2026

| Check | Result |
|---|---|
| Unit test | every `$ref` resolves in the document, every route is in `paths`, health has no security, approve needs `spend` |
| `schemas/openapi.json` | 108 KB, 19 paths, 15 component schemas |
| `uvx openapi-spec-validator schemas/openapi.json` (from PyPI) | OK |

## Not done

No server answers these paths (G168 onward). The contract test that every route is served comes with G173. Request bodies for `uploads`, `approve`, `tools` and `tokens` have no schema yet; they come with their rows.
