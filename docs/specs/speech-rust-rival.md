# Rust Whisper as a rival to the local speech route (section 6 row 23, G112)

Owner, 8 Nov 2026: measure the Rust Whisper from FilmCraft (`crates/speech` at 72831a4) against the current local route, Arabic first. The better one stays first. Not its download feature.

Measured 9 Nov 2026, 2 cores, `light`, one Arabic script of 70 words, Edge voice, 43.6 s. Score: the `words` gate on the script.

| Route | Heard | Time for the clip | Peak memory |
|---|---|---|---|
| faster-whisper large-v3-turbo, int8 (local first row when 3 GB are free) | 68 of 70 | 20.9 s | 2.0 GB |
| faster-whisper small, int8 (local fallback) | 59 of 70 | 12.0 s | 0.85 GB |
| Rust Whisper small, float32 on candle, greedy decode, weights 967 MB (openai/whisper-small at 973afd2) | 60 of 70 | 25.6 s | 2.0 GB |

Result: the Rust route ties the small row on words (one word more), twice as slow, 2.4 times the memory, and clearly behind the first row. English was not measured because Arabic is the deciding language and it did not win. The current route stays first. No code is added: the source would add 60 KB and a second 967 MB download for no gain. G112 (a Python-free route through whisper.cpp) stays open.

The Rust route was built in a scratch copy with the words and output types replaced, not in the repo. To repeat, take `crates/speech/src/{whisper.rs,whisper/*.rs,mel.rs}` at that commit.
