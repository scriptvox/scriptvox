# Short help per command (5 Nov 2026)

> 5 Nov 2026: `--commercial` is deleted (`docs/specs/commercial-note.md`). Text below about it is history.

Fresh-agent test of 4 Nov: every subcommand help listed all 9 global flags, so `check --help` was mostly about money.
Now `--approve`, `--max-spend`, `--commercial`, `--idempotency-key`, `--no-embed`, `--plan` and `--local-only` appear
only in the help of `make`, `generate`, `remake`, `resume`, `batch` and the top level, as a text block. Every command
still accepts them. `check --help` went from 30 lines to 14. The skill text and the MCP tools use the full command list and do not change.
