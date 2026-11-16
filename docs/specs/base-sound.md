# Brief `base_sound`: a dub that keeps the original background

Gap: dubbing tools (HeyGen, ElevenLabs dubbing; OpenMontage's localization-dub plan) keep the music and ambience
of the source under the new voice. ScriptVox dropped the base video's sound.

- `base_sound`: `none` (default), `keep` (the whole original track) or `background` (the original without its
  voice). Needs `base_video`; replaces `music` and `music_prompt` (exit 2 if both are set, or for an unknown name).
- `background`: the track (44.1 kHz stereo WAV) goes to the new `stems` row kind, `demucs-self` on the audio pod:
  Demucs v4 htdemucs (MIT), returns the sum of drums, bass and other. Weights relayed from Meta's own URL
  (`955717e8-8726e21a.th`); the engine checks the SHA-256 start in the name before loading (torch 2.6 needs
  `weights_only=False` for this pickled checkpoint).
- The base sound takes the bed's slot (`audio::Music::Base`): no extra level cut and no loop (it is already mixed
  at its own level), ducked under the new voice, faded out at the end.
- `push_weights.sh`: `REPO=-` skips the Hugging Face step; a failing `EXTRA` now stops the job (a 403 once pushed an
  empty artifact).

Measured 3 Nov 2026: an 8.3 s English video (voice plus the `pulse` bed) dubbed to Arabic with `background` in 29 s,
all gates pass, `words` 15 of 15. Demucs took 5 s. Whisper on the original track heard 20 English words, on the
kept background none of them (it echoed its own prompt instead). The voice band shows a faint rest about 30 dB down.
Test: `tests/edit.rs` `a_base_video_plays_under_a_new_voice` (keep works offline; the 3 refusals).
