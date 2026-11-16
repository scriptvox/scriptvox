# Plan lists the row steps and refuses what cannot run

> 5 Nov 2026: `--commercial` is deleted (`docs/specs/commercial-note.md`). Text below about it is history.

Found by the hand evals of 4 Nov 2026 (`docs/skills/evals-20261104.md`): `plan --commercial` on a brief with
`ambience` said exit 0 and $0, then `make` failed after the voice and the clips with "every sound-from-video row is
non-commercial". The plan also never named the stem row of `base_sound: background`.

- One picker `make::pick_row(field, what, commercial, fits)`: the first row that fits, allows client use under
  `--commercial`, and has a key. Exit 2 when every fitting row is non-commercial, exit 3 (needs input) when none
  has a key. `make` uses it for upscale, smooth, ambience and the stems; `plan` uses it for the same four.
- The plan gets a stage per step that is on: `upscale`, `smooth`, `ambience`, `base_sound`, with the row
  (`kind.name`) and its licence note. Cost $0 (all four rows are free selfhost rows).
- Stems are never refused for `--commercial` (Demucs is MIT and only cleans the user's own sound).
- Test: `make::tests::a_client_video_cannot_plan_non_commercial_ambience`. Checked on the eval briefs: ambience
  under `--commercial` now stops at `plan` with exit 2; the dub plan lists `stems.demucs-self`.
