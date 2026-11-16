# Paid speech rows feed `edit`, `clips`, `analyze` and `dub` (G141, 8 Nov 2026)

Before this, these commands used only free speech to text rows (`selfhost`, or the local Whisper plugin). A user with a fal key and no local Whisper got no captions, though `scribe-fal` works through `generate`.

- One route for all four: `opentts::stt_row()`. A free row first, always. Only when none is set up: the cheapest paid `stt` row with a key. `make` keeps to the free rows, because its own plan does not price this: `opentts::rows()` never returns a known-price paid row (an unknown price is a local speech service on loopback).
- A paid row hears a WAV only when `--approve` names its plan. The plan id covers the row, the WAV's hash and the price per second. The price is from the WAV's length (16 kHz mono, from its size) times the row's price, rounded up to the cent. Without approval: exit 4, `approval_needed`, `plan_id`, and a message with the seconds, the price, "an external paid API on your own key", and the free `setup speech-local`.
- The approval is checked once on the whole WAV, before long audio goes in windows. Each call runs through the ledger with its price as the cap, so a rerun reads the same job and pays nothing.
- `clips` heard the talk, then heard each clip again for its captions. Now each clip takes its words from the talk's transcript (`--captions-file` with `--from/--to`), so a paid row is paid once. `edit --captions-file` now works with `--from/--to`: the words of that part, moved to start at 0. It also no longer asks for a speech row, as it needs none.
- Row field `lang_field`: the request name of the language, when it is not `language` (`scribe-fal`: `language_code`). ElevenLabs lists spaces and sound events as words with a `type`; only `type: word` counts.

## Measured, 8 Nov 2026

| Check | Result |
|---|---|
| Fake fal queue, 120 s WAV, only a fal key | `analyze --transcript`: exit 4, $0.06, no submit. With `--approve`: exit 0, 1 submit, 1 word (the spacing entry is not counted). Again: no second submit |
| Same, `edit --captions karaoke` on a 6 s clip | exit 4 with a `plan_id`, no submit |
| Real `analyze --transcript` on a 17.5 s talk, local Whisper | 3.6 s, 57 words, text right: the free route did not change |
| Real `edit --captions-file <that transcript> --from 8 --to 14` | 3.8 s, 6.0 s, `av_drift` pass; sheet of 6 frames read by eye: starts at "Then the drains carry it to the sea", the spoken word moves on |

## Not done

No real paid call (owner approval needed). Whether fal takes a data URI as `audio_url` for this row is still not known. `clips` was not run for real (it needs a vision service). `make`'s `words` gate stays on free rows. An approved `edit` run on the fake queue was not tested past the approval.
