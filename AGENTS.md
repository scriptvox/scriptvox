# AGENTS.md

## For contributors (humans and agents)

- Build and test: `just ci` (format, clippy, licences, provenance, Rust and Python tests). It must be green before a commit. `just commit "msg"` runs it, then commits with a sign-off.
- The justfile runs plain `cargo`. On a shared machine set `SCRIPTVOX_CARGO` (for example `"light cargo"`) in your shell or in a local `.env`.
- Every commit is signed off (DCO): `git commit -s`.
- Owner rules (5 Nov 2026):
  - **Models.** Any model at any licence, local or through an API. The licence stays on its row.
  - **Tools.** Any tool, AGPL or closed source: learn the ideas and build a better own version under Apache-2.0. AGPL and GPL code is never copied, translated or closely paraphrased, and runs only as a separate component. Closed source: public docs and product behaviour only, no decompiling and no leaked code. No names, logos or brands copied.
  - **Free image and video.** They come from open models that ScriptVox downloads and runs on the user's hardware, with setup at most a Hugging Face login.
  - **Commercial use.** All rows are allowed, the user decides, and the tool shows no note about it (owner, 13 Nov 2026: "I don't want any label in rendering like no commercial for end users"). No flag and no question. `metadata.json` keeps terms as data. The spend cap and paid approval do not change.
  - **Discovery.** It repeats per `docs/plan/DISCOVERY.md`, and gaps are taken from `docs/plan/GAPS.md`.
- Ideas, not code: read `docs/specs/idea-reuse.md`. MIT and Apache code (MoneyPrinterTurbo, HyperFrames, Manim) may be reused with attribution in `docs/provenance/`.
- Every media file needs a source: a row in `PROVENANCE.tsv`.
- Each feature has a short spec in `docs/specs/`. Exit codes live in one table: `crates/scriptvox/src/exit.rs`.
- After changing CLI help or the brief: `just docs` regenerates `plugin/skills/scriptvox/SKILL.md`, and `just schemas` regenerates `schemas/`. CI fails when they are stale.

## For users of scriptvox

- `scriptvox --help`. Every command takes `--json` and prints one envelope.
- Loop: brief, `plan`, `make --detach`, `wait`, `snapshot --sheet 9`, `resume --brief`.
- Never ask for keys in chat: the user runs `scriptvox keys set <provider>`.
- MCP clients (Claude Desktop, Cursor, Codex): `scriptvox setup mcp --client cursor --dry-run`, then without `--dry-run`, gives most commands as tools (`docs/specs/mcp.md` says which are not offered). By hand: `{"mcpServers": {"scriptvox": {"command": "scriptvox", "args": ["mcp"]}}}`.
