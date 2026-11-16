# Voice pace: `voice_rate` (10 Nov 2026)

Before: only the duration fit chose the speaking rate. A user could not ask for a slower or a faster voice.

- Brief field `voice_rate`, a number. 1.0 is the voice's own pace, 0.75 is the slowest and 1.3 the fastest. It becomes a whole percent (0.85 is -15) and goes to the engine as the rate the fit already sends. No `make` flag: the brief and `resume --patch` set it.
- Engines that take it: `edge` (its rate), `elevenlabs` (the voice setting `speed`, which its guide gives as 0.7 to 1.2, so the field is 0.75 to 1.2 there), `kokoro-rust` (its pace) and `none` (the reading pace of the words). `kokoro` (the plugin speaks at one speed) and `selfhost` take none: with them the field is exit 2 and the message names the four engines. `fit::takes_rate` is the one list.
- A value outside the range, or a rate with `narration` or `lines`, is exit 2, in a brief and in a patch.
- No field, or 1.0, is the old behaviour: the same plan id, the same voice key, the same request to the provider. Another value enters the plan id and the voice key (`rate` was already part of the key), so it is a new voice. For a paid voice that is a new plan and a new paid call, which waits for its own approval.
- With `duration`: the voice keeps the pace of `voice_rate`. The fit does not read again at another rate. It may only change the tempo, by at most 16 percent (the rule a voice without a rate already has). Past that the run stops after the voice (exit 3) and says how many words to add or cut, or to drop `voice_rate`. The plan prices one read, not two.
- `plan` says it: the `voice` stage ends with the pace in percent, and with `duration` also with the rule above. The `fit` warning counts the words at the new pace and warns from 16 percent.
- Result: `metrics.voice_rate_pct` is the rate the voice was read at (from `voice_rate`, or from the fit), `voice_tempo` the tempo change after it, and `explain` has `pace: voice_rate -15%`. The warning that a fitted voice "may sound dragged" does not show for a pace the user chose.
- `resume <run> --patch {"voice_rate": 0.9}` makes a child run at that pace. `resume --brief` keeps the parent's pace when the new brief has none. The recipe in the MP4 holds the field, so `replay` finds the voice in the cache.

Two faults found on the way, both fixed here because the field needs a true list of engines:
- The ElevenLabs worker did not send the rate. A fit with `duration` (and a dub line that ran long) paid for a second read that came back the same. It now sends `speed`, and the fit and the dub keep their re-read inside -30 to +20 percent for this engine. So an ElevenLabs run with `duration` and no `voice_rate` is not the same as before: its second read now changes the pace. The voice key of an ElevenLabs read at a rate has a new mark, so an old cache entry stored under a rate (which holds the voice at its own pace) is not used again; a read with no rate keeps its key.
- `takes_rate` was true for the `kokoro` plugin, which speaks at one speed. The fit now goes to the tempo step for it, as for `selfhost`.

Run on 10 Nov 2026, worktree build, free Edge voice, 16:9, one English script of 43 words and one Arabic script of 24 words:
- English at 0.85, 1.0 and 1.2: voice file 14.50 s, 12.31 s and 10.27 s (last word ends at 14.06, 11.95 and 9.96 s). 12.31 / 14.50 is 0.849 and 12.31 / 10.27 is 1.199. `voice_rate_pct` -15, 0 and 20. All 8 gates pass on each.
- Arabic (Hamdan) at 0.9: 16.85 s, `voice_rate_pct` -10, all 8 gates pass.
- English at 0.85 with `duration` 15.5: one read, `voice_rate_pct` -15, `voice_tempo` 0.935, 15.5 s, voice reused, all 8 gates pass. With `duration` 11: the plan warns (estimated 1.39x), and `make` stops with exit 3 at 1.32x and nothing rendered.
- Tests: `tests/voice_rate.rs` with the stub worker, which writes the rate it got (the worker, the key, the plan id, the metric, the range, the rule with `duration`, the patch with and without a duration, `replay`); `tests/elevenlabs.rs` `a_changed_voice_rate_is_a_new_plan_and_makes_no_call_before_its_approval` with the fake API (no call before approval, `speed` 0.9 in the body, 1.25 refused); `fit.rs` `voice_rate_is_a_percent_inside_the_range_of_the_engine`; `python/tests/test_tts_elevenlabs.py` (the setting and its limits).

Not done: no `make` flag; not a brand file field; `dub` lines and `voices --preview` take no pace; the `kokoro` plugin could pass a speed to its model but does not. Not run: ElevenLabs `speed` on the real API (a paid call; only the fake server saw the request); `kokoro-rust` with a rate (not installed on this machine; its pace argument is the one the fit already used); a listening check of 0.75 and 1.3.
