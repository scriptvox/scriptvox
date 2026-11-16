# `scriptvox batch`: one video per row

> 5 Nov 2026: `--commercial` is deleted (`docs/specs/commercial-note.md`). Text below about it is history.

Source: HyperFrames cherry #23 (variables and batch renders, for personalised or localised videos at scale).

- `scriptvox batch --brief b.json --rows rows.json`: the brief text holds `{{key}}` placeholders; `rows.json` is a list
  of up to 50 objects. Every row must fill every placeholder (checked for all rows before the first video; exit 2
  names the missing one). Values are escaped for JSON strings.
- Each row runs as its own `scriptvox make` (a child process, same money rules: a paid row stops at its own plan),
  from a hidden filled brief next to the original so relative paths still work; the hidden file is removed after.
  Outputs `scriptvox-out/<brief>-<n>.mp4`. `--commercial` and `--local-only` pass through.
- Result: `rows` with each row's values, exit, status, run id, video, error and non-pass findings. Exit 0 when all
  rows are ok, else the first failing row's exit code.
- Also an MCP tool (`scriptvox_batch`, 19 tools now).

Measured 3 Nov 2026: 2 rows (names and districts in the script and the hook), `--local-only`, 6 s, both hooks and
captions filled correctly (checked on a frame). Test `batch::tests::placeholders_fill_escaped_and_missing_ones_are_named`.
