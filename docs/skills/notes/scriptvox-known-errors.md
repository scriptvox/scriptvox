# Notes: scriptvox-known-errors

Written 10 Nov 2026 before the skill. G086 names OpenChatCut (CL-G, AGPL) as the idea only: a page of known failures for an agent. Its text was not read for this skill, and no text comes from it. The sources are our own: `crates/scriptvox/src/exit.rs`, the fix lines in `gates.rs`, `TRIAGE.md`.

## What the sources teach (our words)

- An agent that fails will retry. The one dangerous retry is the one that can spend money, so that case needs its own line and no exception.
- The envelope already holds a fix line for the exact failure. The skill must send the agent there first, and only then give general advice.
- A warning that nobody says out loud is the same as no warning.

## Weak points

- `TRIAGE.md` is for people who sort issues, not for an agent in a run.
- `exit.rs` has one short meaning per code, and no next step.

## What our skill does better

- One row per exit code, each with the next step. Exit 6 is stated as a stop.
- One row per gate, with the usual cause. A test in `tests/docs.rs` fails when an exit code or a gate has no row, so the page cannot go stale.

## 13 Nov 2026, on the new tree

- Since 10 Nov the tree has more warn-only findings (`lang`, `picture_shape`, `emphasis`, `behind`) and the media input rule (item 385: the first bytes decide, exit 2). The skill now has a short table for the warnings and one rule for the refused input.
- Each new line was checked against the code first: the finding names in `make.rs`, the messages in `media_gate.rs`, the fields `not_checked`, `coverage` and `limitations`.
