# Rust Kokoro voice as a second English engine

Owner, 8 Nov 2026: use the Rust voice code from FilmCraft (Apache-2.0 option). Section 6 row 19 of the 8 Nov discovery.

## What

`--tts kokoro-rust`: Kokoro-82M on candle, CPU only, in the `scriptvox` process, with a text normalizer and a CMUdict plus letter-to-sound front end. No Python and no GPL part. Same two voices as the plugin, `af_heart` and `am_michael`. English only, so Arabic routes do not change. `scriptvox setup voice-rust` downloads 332 MB through the pinned setup route, sha256 per file in `toolchain.lock`; the weights are checked again before the pickle is read. Source and changes: `docs/provenance/kokoro-rust.md`.

The Python plugin stays. The new engine is picked only with `--tts kokoro-rust`, never by default, until a measurement says equal or better.

Word times: the model gives no word events, so the reading is spread over the real audio length (`timing: estimated`, as the plugin).

## Measured, 9 Nov 2026

One English script, 135 words with numbers, a date, a price and a name, `af_heart`, under `light` (2 cores), the same local Whisper row for both.

| | `kokoro-rust` | plugin `kokoro` |
|---|---|---|
| Gate `words` | 136 of 142 (95.8 percent) | 137 of 142 (96.5 percent) |
| Audio length | 51.2 s | 46.1 s |
| Voice time | 50 s (1.0 x real time) | 38 s (1.2 x real time) |
| Peak memory, draft run | 712 MB | 830 MB (includes Python) |

Result: one word worse on one script, slower by about 30 percent per second of audio, less memory. The default does not change. One script is a small sample. Checked by eye with a 9-frame sheet: captions are correct and readable; timing is estimated as for the plugin.

Tests: `voices::tests::the_rust_voice_is_never_the_default_and_answers_when_asked`, `kokoro_rust::tests`, and the crate's text test need no model. A real run needs the weights.
