# A named TTS engine gets its real cause (section 6 row 12, E9)

`--tts foo` said "No voice matches lang 'en'", as did an installed-but-not-set-up `kokoro`, so the user looked at the language. `voices::engine_cause` is checked first when an engine is named, in this order: the engine name is unknown (it lists the valid ones); it has no voice for this language; it is known and not ready, with its setup command (`voice-local`, `voice-rust`) or, for `selfhost`, the deploy line. A paid engine still reaches the key line. The alternatives stay.

Test: `a_named_engine_gets_its_real_cause`. Real run on an empty config: unknown name, English Kokoro and the Rust voice before setup, Kokoro in Arabic, selfhost: each gave the cause above, and elevenlabs still gave its key line.

A wrong voice id with a named engine that speaks the language gets the line about the voice, not the line about the engine (audit contract-3). A paid alternative has `cost_usd` null and `paid` true, and a free one has `cost_usd` 0 and `paid` false (audit contract-4).
