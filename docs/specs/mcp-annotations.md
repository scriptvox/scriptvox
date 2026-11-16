# Tool annotations (G033, 7 Nov 2026)

A client can show or gate a call only if it knows what the call does. `ANNOTATIONS` in `mcp.rs` is the one table, one row per tool, and a test fails on a tool with no row.

- `annotations`: `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`, the standard hints.
- `_meta`: `scriptvox/paid` and `scriptvox/writes`, the words for what a call writes.
- These are hints. Money still stops at `--approve` and the spend cap, in the command.
- A command that can write over a file named with `--out` counts as destructive. `edit` was run twice with the same `--out` while testing G029, and it replaces the file.
- `check`, `judge` and `analyze` count as reaching a network: their sound or frames can go to your services.

Measured, 7 Nov 2026, real stdio session: `tools/list` has the hints on all 24 command tools, and `scriptvox_guide` has none. No real client read them: only a pipe. The table was written by reading the commands, not by a test on each one.
