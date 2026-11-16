# The fitter knows the open voice

Found by the hand evals of 4 Nov 2026: with `"tts": "selfhost"` (Chatterbox), `duration` 8 and 12 ended after TTS in
`SVX_NEEDS_LONGER_SCRIPT`, and a fitted run reported `voice_rate_pct` -14 although the open voice has no rate.

- `fit::words_per_s(lang, engine)`: the open voice reads about 4.5 words a second in English (24 words in 5.35 s,
  4 Nov) and 2.9 in Arabic (about 175 a minute, 3 Nov); Edge and ElevenLabs keep 3.3 and 1.8. The precheck, the
  word hints and the clip lengths of generated scenes use it.
- `fit::takes_rate(engine)`: false for `selfhost`. The fitter then skips the TTS re-read (it gave the same length and
  cost one more TTS call): within 16 percent it applies `atempo`, else it stops with the word hint.
  `voice_rate_pct` stays 0, so a reviewer reads the real change in `voice_tempo`.
- `decide`, `miss`, `precheck` take words a second instead of the language.
- Measured on the eval brief (24 words, open English voice): `duration` 12 now stops before TTS ("0.44x, add about
  30 words"), it stopped after TTS before at 0.43x; `duration` 6.1 fits with `atempo` 0.848, no re-read,
  `voice_rate_pct` 0, all 6 gates pass.
- Test: `fit::tests::the_open_voice_reads_faster_and_takes_no_rate`. The `scriptvox-brief` word table has open-voice rows.
