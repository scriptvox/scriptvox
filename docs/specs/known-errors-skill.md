# Skill `scriptvox-known-errors` (G086)

One page an agent reads when a command fails or a gate does not pass. File `plugin/skills/scriptvox-known-errors/SKILL.md`, notes `docs/skills/notes/scriptvox-known-errors.md`, evals `evals/scriptvox-known-errors/`.

- One row per exit code of `crates/scriptvox/src/exit.rs`, with the next step. Exit 6 is a stop with no exception: a retry can pay twice.
- One row per gate, with what it measures, the usual cause and the first step. A short table for the warn-only findings (`lang`, `picture_shape`, `emphasis`, `behind`).
- Rules: read `error.fix` and `next` first; `not_checked` is not a pass; exit 2 and 4 charged nothing; an input that is not plain media is exit 2; never edit `result.json`.
- The skill is the 13th: `skill.rs` serves it to `skills install` and to MCP clients as a resource.

Test: `tests/docs.rs` `the_known_errors_skill_covers_every_exit_and_gate` fails when an exit code, a gate of `gates.rs` or one of the four warnings has no row, or when a named gate is gone from the code. Evals (by hand): exit 6, a `text_bounds` failure, a missing key.
