# Consent record for a cloned voice (G036, 7 Nov 2026)

`generate --audio` on a voice row cloned any sample with only a note on the row. Public docs of HeyGen describe a consent step; no code read.

- Your own voice (owner, 7 Nov 2026): `scriptvox voices add --sample f --own` records `own: true` and the hash of the statement "I state that this sample is my own voice.", with `proof_hash` null. No proof file. `--own` and `--consent` do not go together; one of them is required. The exit 10 message names both.
- `scriptvox voices add --sample f --consent proof [--name x]` stores `id`, `sample_hash`, `proof_hash`, `name` and `added_unix` in `consent.json` in the config folder. Hashes only: no voice and no form is copied.
- `generate --audio` on a `tts` row with a sample that has no record stops at exit 10, `needs_input` with kind `consent`, before anything is sent.
- The tool does not judge the proof. It makes the step explicit, and records that the user took it. The record is by content, so a changed sample needs a new record.
- `make` has no brief field for a sample, so `generate` is the only path. There are no presenter rows yet. A lip sync row moves a mouth to a voice: it is not covered here.

## Measured, 7 Nov 2026, real commands on `chatterbox-mtl-self`

| Check | Result |
|---|---|
| A sample, no record | exit 10, nothing sent |
| `voices add`, then the same call | exit 0, a real clone at $0 |
| A sample changed by one byte | exit 10 again |
| Missing proof file | exit 2 |

Listening to the clone was not done. The test sample is an Edge voice, not a person.
