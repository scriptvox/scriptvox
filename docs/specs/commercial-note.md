# Commercial use: one small note, no flag (G005)

**Changed on 13 Nov 2026, owner decision** ("I don't want any label in rendering like no commercial for end users", then "Yes remove all"). The tool gives no note now. `terms_note` is gone from `plan`, `make`, `export`, `generate --plan` and `voices preview`; the `terms:` line of the terminal is gone; the `explain` line of a voice and of a row step no longer holds the terms text; a voice in `needs_input` alternatives shows its cost only. The data stays: `commercial_ok` and `parts` in `metadata.json`, `credits.txt`, the receipt, the recipe, and the `terms`, `output_terms` and `business_use` fields of `voices`, `providers` and `tools`. The text below is the history.

Owner decision, 5 Nov 2026. All rows are allowed, the user decides, and the tool gives one small note.

- `--commercial` is deleted, with every refusal based on licence terms. Row and voice choice ignore terms: the plan id no longer has a `commercial` part.
- `plan` gives `terms_note` when the voice or a row in its stages has `commercial = false`, a known limit. A row whose terms were not checked leaves `commercial` out (null) and gives no note (audit M1, 7 Nov). `export` gives `terms_note` when `commercial_ok` is false; `commercial_ok` is true when every part is cleared, false on a known limit, and null when a part was not checked. The note stops nothing.
- `metadata.json` keeps `parts` with their terms and `commercial_ok`. `providers` keeps `output_terms`.
- The spend cap and paid approval do not change.
- `setup voice-local` shows its GPL-3.0 notice once, at the install, and asks nothing (audit M3). `--accept-gpl` is accepted and ignored for one version.
- A saved `request.json` with the old `commercial` field still loads: the field is ignored. The replay fixture keeps that field as the check.
- Tests: `approval::terms_never_stop_a_run_and_the_plan_gives_one_note`, `make::tests::row_choice_ignores_licence_terms`.

## Plain words (L3, section 6 row 4, 9 Nov 2026)

The rule above does not change: every row is allowed, no flag, no question, one small note. The note fires on the same parts as before (a known limit only, never on a part not checked) and appears in the same places.

- The text names the part and a free way to it: `Personal use only: the edge voice. For business use, free: `--tts kokoro`. The choice is yours.` The free way is a free voice with business use for the same language, a free row of the same kind, or the user's own clips for stock. No free way: only the part is named.
- `plan`, `make` and `export` use one function, `make::use_note`. The old text pointed at `providers`, which does not list voices; there is no pointer now.
- One derived word per row: `business_use` in `providers` and `voices` is Business use yes (`commercial` true), Personal use only (false), or Not confirmed (null).
- The MCP text about the yes for a setup does not name the licence any more. The setup plan still shows it.
- Test: `make::tests::the_use_note_names_the_part_and_a_free_way`.

## `generate --plan` (section 6 row 9, L5)

`generate --plan` now says the same note as `make` for a row with personal-use terms (`use_note`, the shared function), and asks for `--approve` only when the plan costs money. Test `a_free_plan_gives_the_note_and_no_approve_hint`. Real run: `mmaudio-t2a-self` gave the note and "Run the same command without --plan."
