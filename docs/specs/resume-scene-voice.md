# `resume` with one scene changed keeps the voice of the other scenes (13 Nov 2026)

Seen in agent trial T8: `resume <run> --patch` changed the words of scene 2 of 3. The tool voiced the whole narration again. The fit to `duration` then chose another pace, so the voice and the start time of scenes 1 and 3 changed too.

## Cause

- A brief with `scenes` and one speaker was one speech part with one cache key (the whole script). One new word was a cache miss for all of it.
- The fit to `duration` measured the whole narration and changed the pace of all of it: a re-read at another rate, then one `atempo` on the joined file.

## The rule now

1. **One speech part per scene.** A brief with `scenes` is read scene by scene (`make::scene_segs`), with a pause tag still cutting inside a scene. Not when no voice is made (`tts: none`, a `narration` recording) and not with `media_match`: its scenes come from the picks, which a plan and `make --until voice` do not have yet, so those runs keep the one-part reading. Each part has its own key in the voice cache (`make::voice_key`: text, voice, rate), as scenes with their own `voice` had before. There is 0.25 s of silence between two scenes. Before, that silence was only where the speaker changed.
2. **The tempo change is made on each part.** When the fit ends with a tempo change, each part gets the same `atempo` before the join (`synthesize`, argument `tempo`). The joined file is not changed after. So a part gives the same samples in every run that reads it at the same rate and tempo. This step reads from the cache only and can not buy a voice. A script of one part keeps the old path (`voice_fit.wav`).
3. **A run writes down its pace.** After the fit, the run folder gets `pace.json`: the rate in percent, the exact tempo, and the `duration` it was fitted to. `metrics.voice_tempo` is rounded and is not used for this.
4. **`resume` keeps that pace when a scene is kept.** If the parent has `pace.json`, at least one scene has the same words and voice as before, at least one has not, and the language, engine, voice, `voice_rate` and `duration` did not change, the child's request gets `pace` and no `duration`. `make` then reads every part at the parent's rate and tempo and does no fit. The scenes that did not change come from the cache. Only the other scenes are voiced.
5. **The plan says it.** The `voice` stage of the child's plan reads, for example: `voiced again: scene 2; voice kept: scene 1, 3; the pace of the parent run is kept (rate +10 percent, tempo 1.048) ...`. The answer of `resume --plan` and of the run has `voiced_again` (a list of scene numbers). The list comes from the voice cache, so it is what the run will do.

## The choice: later scenes shift, the total length is not kept

Two ways keep the samples of the unchanged scenes: (a) the changed scene gets its natural length at the same pace and the later scenes shift, or (b) the changed scene alone is fitted so the total stays at `duration`.

(a) is built. Reasons:

- With (b) scene N has another pace than the scenes before and after it. A listener hears that. With (a) all scenes have one pace.
- (b) fails when the new words are much longer or shorter than the old ones. (a) never fails.
- (a) is a part of (b): (b) can be added later as one more fit of scene N.

The cost of (a): the child is not `duration` long. The result has a `fit` warning with both lengths and the command for a new fit of all scenes: `scriptvox resume <child> --duration <seconds>`. `--duration` on `resume` always fits all scenes again (run on the child of the table below: 20.0 s again, see the end of the table).

A child of a child keeps the pace too, also when only the look changes (a hook, the music).

## Measured, 13 Nov 2026

Brief: 3 scenes, `duration` 20, `tts` `kokoro-rust`, voice `af_heart`, 16:9, its own empty data folder. Patch: other words in scene 2 (21 words to 15).

| | Before the fix | After the fix |
|---|---|---|
| Parent pace | rate +15 percent, tempo 1.073 | rate +10 percent, tempo 1.048 |
| Child pace | rate 0, tempo 1.0 (new fit) | rate +10 percent, tempo 1.048 (kept) |
| Scene starts, parent | 0, 6.06, 13.48 s | 0, 6.18, 13.88 s |
| Scene starts, child | 0, 6.92, 12.77 s | 0, 6.18, 11.24 s |
| Scene 1 audio, child against parent | another read | 142,610 samples, all equal |
| Scene 3 audio, child against parent | another read | 146,841 samples, all equal |
| Child length | 20.0 s | 17.97 s, with the `fit` warning |
| Plan of the resume | no word on scenes | `voiced again: scene 2; voice kept: scene 1, 3` |
| `resume <child> --duration 20` | | 20.0 s, new fit of all scenes (rate 0, tempo 0.954), no `pace` in its request |
| `resume <child> --patch` with a new `hook` only | | pace kept, `voice.wav` equal to the child's, captions beside the video |

The audio was compared two ways: the part files (`seg_00/trimmed.wav`, `seg_04/trimmed.wav`) with `cmp`, and the first and last sample ranges of the joined `voice.wav`. Silence at a scene join with this voice: 0.3 s without the 0.25 s breath, against 0.7 to 0.8 s at a sentence end inside one read; the breath was added for this.

Test: `tests/resume.rs` `a_change_in_one_scene_voices_that_scene_alone_and_keeps_the_pace` (stub voice: the plan names scene 2, parts 1 and 3 have the same bytes and no new call, the tempo and the first two scene starts are the parent's).

## What else changes

- A brief with `scenes` is read in parts, so its voice is not the same audio as before this change: the pause between two scenes is now the engine's own start silence plus 0.25 s. Old cache entries of a whole script are not used by a brief with scenes; the first run after the update voices it again (a paid voice pays again one time).
- A paid voice with `scenes`: the plan prices only the scenes that are not in the cache (`docs/specs/voice-plan-cache.md`), also in a plain second `make`.
- `voice_fit.wav` is written only for a script of one part. In other runs the fitted voice is `voice.wav`.
- The local voice `kokoro-rust` spreads its word times over each part, so each scene has its own word times. They were not yet right at the join: see the next section. The scene starts in the table above were measured before that fix.

## Fault fixed the same day: two caption pages on screen together

Seen in a normal video with `kokoro-rust` and `scenes`: the last caption page of a scene and the first page of the next scene were on screen at one time. The renderer draws the second page above the first, where it can lie on a card (gate `contrast` warned 1.2 to 1).

Cause: `kokoro-rust` (and the Python `kokoro`) estimate the word times. They spread the words over the whole clip, with its silence before (about 0.3 s) and after (about 0.6 s) the words. The join cuts the end silence of each part and did not move the word times. So the last words of a part were after the end of its audio, in the next scene. The cut and the estimate are both older than the one-part-per-scene rule: a script with a pause tag and no `scenes` had the same fault. With that rule, each brief with `scenes` got it.

The rule now, three layers:

1. **The word times are right.** `kokoro-rust` spreads its words from the first to the last sample that is not silence (-50 dB, the level of the cut). Its cache key has a new field, so an old entry with the wrong times does not come back. The audio is the same.
2. **No word ends after the audio of its part** (`make::part_words`). When an engine's last word of a part ends after the cut audio, the words of that part are scaled to its length. This is for the Python `kokoro` and any other engine that estimates. Measured word times (Edge) end before the cut and do not change.
3. **One caption page at a time** (`captions::paginate`): a page ends when the next page starts, at the latest. The gate `captions` fails (exit code of a failed gate) when two pages of the main track are on screen together. The second language line (`captions_second`) is another track and is not checked against the main one.

Measured, 13 Nov 2026 (3 scenes, `duration` 12, `kokoro-rust`, `af_heart`, a `stat` card in scene 2):

| | Before | After |
|---|---|---|
| Clip of scene 1, audio after the cut | 3.825 s, 3.229 s | the same |
| Last word of scene 1 ends | 3.825 s | 3.193 s |
| First word of scene 2 starts | 3.479 s | 3.865 s (0.25 s breath and 0.386 s of start silence) |
| Caption pages | 0 to 3.825, 3.479 to 6.947, 6.947 to 8.379, 8.004 to 10.719, 10.719 to 12.504 | 0.358 to 3.865, 3.865 to 8.390, 8.390 to 12.000 |
| Gate `contrast` | warn, 1.2 to 1 | pass, 10.2 to 1 |
| `voice.wav` and the three part files | | same MD5 as before |

Arabic with the Edge voice (measured word times), the same brief shape: 4 pages, none at one time with the next, `captions` and `contrast` pass.

Tests: `captions::tests::a_page_ends_before_the_next_one_starts_when_word_times_overlap`, `gates::tests::two_caption_pages_at_one_time_fail`, `make::tests::the_words_of_a_part_end_with_its_audio`, `kokoro_rust::tests::words_stay_inside_the_voiced_part`.

Side effects: with `kokoro-rust` the first caption and scene 1 now start with the first sound (about 0.3 s), not at 0, as with a voice that gives measured times. The first run after the update voices a `kokoro-rust` script again (a local voice, no cost).

Open: the Python `kokoro` still spreads its words from 0, over the start silence; only its end is corrected. A script of one part is not cut, so its words (Python `kokoro`) still run to the end of the clip.

## Open

- Only the voice track is the same. `master.wav` and the video sound go through one loudness pass over the whole track, so their samples are not equal between parent and child.
- (b), the fit of scene N alone to keep `duration`, is not built.
- A parent made before this change has no `pace.json`: its child fits all scenes again.
- A parent with one scene, a dub (`lines`), a recording (`narration`), `tts: none`, `media_match` and a base video keep the old path: their child fits all scenes again.
- A paid voice with a kept pace has no test against the fake server. The plan prices the scenes that are not in the cache, by the code path of `voice-plan-cache.md`.
- The `pace` field is in the request and in the recipe of the child. `make` refuses a pace outside the engine's rate limits or a tempo outside 0.5 to 2 (a recipe is text from a file).
- A scene that moves to another place in the list keeps its voice. A scene cut in two does not.
- Not run: a paid voice or the Edge voice with this change (no network voice in this work). The Edge join silence is not measured.
- The picture of the unchanged scenes is rendered again in the child.
