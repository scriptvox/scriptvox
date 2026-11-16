# Why a row cannot run (E3, section 6 row 3, 9 Nov 2026)

Before: `generate --plan` offered a plan for a row with no key, and the missing key showed only after the approval. A self-hosted row at a placeholder host asked for a key that could not help.

`Generator::why_not()` is the one answer, in this order: retired, not installed (local), not deployed (placeholder host), no key. It gives a kind, the reason and the one next step. `ready()` is now "no `why_not`", so a row at a placeholder host is not picked by the planner even with a key set: a paid row with a key is planned instead, and it still needs approval.

- `providers --json`: each row has `runnable` and `why_not` (a sentence, or null).
- `generate --plan` stays valid (exit 0, `plan_id`, price). It adds a `needs_input` item (`secret` for a key, `dependency` otherwise), and `next[0]` is that step.
- `generate` without `--plan` on a paid row: exit 4 as before, with the same item and `next[0]`. Approval and the spend cap do not change.
- `plan --brief` on a row at a placeholder host: exit 10, kind `dependency`, the deploy step and no key question. A missing key there is exit 10 as before.
- Nothing here calls a provider or costs money.

Not done: the free alternative row in the item (section 6 row 18).

Tests: `discovery::a_row_with_no_key_says_so_first_and_the_plan_stays_valid`, `generators::tests::why_not_names_a_placeholder_host_before_any_key`. Measured: see `docs/daily/2026-11-09.md`.

Exit 10 has one shape (audit contract-1, 9 Nov 2026). `Envelope::fail` sets status `needs_input` and one item from the fix, or from the message when there is no fix, with `kind` `input`. A caller that knows the field and kind replaces that item.
