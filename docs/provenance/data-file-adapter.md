# Data-file service row (`connect --spec`)

Source of the idea: `docs/design/declarative-adapter.md` in storytold/craft-ai-integrations, Apache-2.0 (LICENSE-APACHE in that repository), read on 10 Nov 2026 through raw.githubusercontent.com. The design note was read. The code in `crates/ai-declarative` was fetched in part and was not used.

Taken as ideas: one queue flow described by a small JSON file, unknown fields refused, holes in string leaves only, a closed status table where an unlisted word is an error, poll address built from the file and not from the reply, no redirects, a reserved header list.

Ours: the file shape, `crates/scriptvox/src/data_row.rs`, its admission through the existing reservation and submit marker, and the row in `generators.toml`. No identifier or text comes from that project.
