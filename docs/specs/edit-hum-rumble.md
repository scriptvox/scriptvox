# `edit --clean-voice`: mains hum and low rumble (G196, 13 Nov 2026)

Why: `--clean-voice` lowered broad noise only (`edit-clean-voice.md`, "Not done: a real room, wind or hum"). A
recording near a power supply has a steady tone at 50 or 60 Hz and its harmonics, and a room or a hand on the
microphone adds sound under the voice. Idea from the public feature list of auto-editor; no code read.

No new option. Every `--clean-voice` level now does three things, in this order:

1. **Finds hum.** The first 60 s of the sound, mono at 11025 Hz, in windows of 1 s. For 50 Hz and for 60 Hz it
   measures the first 8 harmonics. A harmonic is found when its level is 12 dB or more above the median of the
   levels 4 to 8 Hz on each side of it, and above -75 dB. Hum is found when the base tone is found, or 2 harmonics
   are. 300 Hz belongs to both, so it does not count for the choice; when both pass, the one with more height
   above its neighbours wins. Once the mains frequency is known, each of its harmonics that is 6 dB or more above
   its neighbours is removed too, because a voice around a harmonic hides part of its height.
2. **Finds rumble.** The level of the band from 18 to 46 Hz, under both hum tones and under every voice. Rumble is
   found from -50 dB.
3. **Removes only what it found**, before the noise filter: a 4 Hz wide notch on each found harmonic
   (`bandreject`), and for rumble a 60 Hz high-pass of 24 dB an octave. Nothing found: the sound path is as before.

Result, in `clean_voice` beside the old keys:

- `hum`: `found`; when found also `hz` (50 or 60), `removed_hz`, `db_before`, `db_after` (one level per removed
  harmonic, measured the same way on the source and on the result).
- `rumble`: `found`, `db_before`, `db_after`.

No new error: a video with no sound is refused as before (exit 2, "Drop --clean-voice").

## Measured, 13 Nov 2026, 2 cores

Real run: the 11.33 s local-voice talk (`kokoro-rust`, a woman's voice, pitch near 200 Hz), with made hum and
rumble mixed in, so the hum is synthetic. `edit <file> --clean-voice light`. Levels by my own measure (the
amplitude of that sine over seconds 1 to 10), in dBFS, before and after:

| File | Tone | Before | After |
|---|---|---|---|
| Talk + 50 Hz hum (50, 100, 150, 250 Hz) + brown noise under 40 Hz | 50 Hz | -33.6 | -85.8 |
| | 100 Hz | -39.4 | -99.1 |
| | 150 Hz | -44.5 | -77.9 |
| | 250 Hz (not found: no higher than the voice beside it) | -49.0 | -48.9 |
| | 23 Hz | -53.5 | -90.2 |
| | 37 Hz | -59.5 | -77.2 |
| | 440 Hz (voice) | -52.5 | -52.6 |
| Talk + 60 Hz hum (60, 120, 180 Hz) | 60 Hz | -33.5 | -76.5 |
| | 120 Hz | -39.6 | -85.8 |
| | 180 Hz (not found: under the voice's own pitch) | -47.9 | -47.5 |
| The clean talk | 50, 100, 150, 200 Hz | -78.1, -82.6, -68.6, -41.1 | -78.1, -85.9, -70.1, -41.2 |

What the result said:

- 50 Hz file: `hum` found, `hz` 50, `removed_hz` 50, 100, 150, `db_before` -33.7, -39.6, -44.1, `db_after`
  -73.1, -77.8, -64.7; `rumble` found, -30.9 to -53.7 dB; noise floor -30.0 to -48.4 dB.
- 60 Hz file: `hz` 60, `removed_hz` 60, 120; `rumble` not found (-58.9 dB).
- Clean talk: `hum` not found, `rumble` not found (-58.8 dB): no notch and no high-pass.
- All three: picture copied (340 frames, the same MD5 of the picture stream), -14.1 LUFS, `av_drift` pass.

Fault found on the way: with one limit of 12 dB, the 150 Hz harmonic (9 dB above the voice around it) stayed in
the result at -44.9 dBFS. So a harmonic of a mains frequency that is already found now needs 6 dB.

Tests: `tests/edit_hum.rs` (50 and 60 Hz found, each tone falls more than 20 dB against the voice; rumble found
and lowered; a clean sound gets nothing) and 4 unit tests in `src/hum.rs`.

Not run: a real recording with real hum; a man's voice (pitch near 100 Hz); hum that moves in frequency.

## Limits

- A harmonic that is not higher than the voice around it stays (250 Hz and 180 Hz above). The voice also hides it
  from the ear in part; no person listened.
- `db_after` is read on the result, after the -14 LUFS pass, so it holds that pass's gain too.

- One mains frequency per file, steady, within about 0.5 Hz of 50 or 60 Hz times the harmonic number.
- A long steady bass note of music on one of these frequencies reads as hum. `removed_hz` says what was cut.
- A notch also cuts 4 Hz of the voice at that place. Not heard by a person.
- Wind and handling noise above 46 Hz is not rumble here; the noise filter of the level works on it.
- Only the first 60 s are measured.
- Every `--clean-voice` call can now get the 60 Hz high-pass: the low end of a music bed under the talk, or of
  broad noise, counts as rumble from -50 dB. The table in `edit-clean-voice.md` (pink noise, 7 Nov) is older than
  this change and was not measured again.
