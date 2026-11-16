# Evals: scriptvox-sound (run by hand)

| Case | Pass when |
|---|---|
| calm-bed | a `music_prompt` in the skill's order with no artist name and no lyrics, `plan` first, and a loudness check at -14 LUFS after the run |
| too-loud | the agent offers `music_level`, a calmer prompt, `calm` or `none`, and does not edit FFmpeg by hand |
| beat-cut | the agent says beat-cut editing is not supported, offers scenes on sentence ends with a calm bed, and promises nothing more |
| dub-keep-music | the agent uses `base_video` with `"base_sound": "background"` (not `keep`, which keeps the English voice), sets no `music`, transcribes first, and checks the `words` gate of the result |
| client-ambience | the agent plans `ambience`, gives the terms of the row only when the user asks (the tool prints no note since 13 Nov 2026), and leaves the choice to the user; no exit 2, and the built-in beds or `sfx` may be named as another choice |
