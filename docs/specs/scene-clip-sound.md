# A scene's clip keeps its own sound: `sound` (10 Nov 2026, G059)

Before: every `media` clip was silent in the result. RVX-013 dropped clip sound and deferred "ducked clip audio". A user's clip of a machine, a crowd or a person saying one line could not be heard.

- Scene field `sound`: `none` (default), `under` or `keep`. Another word is exit 2.
- `under`: the clip's sound sits 12 dB under the voice (the level of the scene `ambience`; the music bed's `normal` is 10 dB). It is not ducked.
- `keep`: the clip's sound sits at the voice's level. The voice still wins: the clip goes through the same duck as the music bed (the same compressor, fed by the voice), which lowers it while the voice speaks. Measured: 7 dB on the real run below (the bed's own note says about 9 dB).
- Both levels are set from the measured loudness of the cut sound, as the bed's is: the voice enters the mix at -22 LUFS, an `under` clip at -34 and a `keep` clip at -22. A very quiet clip is raised by at most 14 dB. The master pass then brings the whole mix to -14 LUFS.

## Timing

- The sound is the same part of the file as the picture: it starts at `media_from` and never goes past `media_to`.
- It starts at the scene's start on the final timeline and stops at the scene's end. A crossfade into the scene does not move it.
- The sound is never slowed. When the clip is shorter than its scene, the picture slows or holds its last frame; the sound plays at its own speed and then stops.
- 30 ms fades at the start and at a cut end, so a cut does not click.
- A scene has time only for its words. A scene with an empty script has no time in the video (this was so before), so `sound` on it is exit 2. The natural use of `keep` is a short script with a pause tag: `"script": "Listen. [pause 3]"` gives the clip 3 s alone.

## Refusals (exit 2, at `plan` and before any voice call in `make`; each names `scenes[i].sound`)

- The clip has no audio stream: the message names the file.
- The `media` is a photo, or the scene has no `media`.
- The scene has `stock`, `open_image` or `generate`: tonight `sound` is only for the user's own clip. The licence of stock sound is a separate question.
- The scene's script has no words.

## How

- Each kept sound is first cut in its own small FFmpeg pass (`-ss`, `-t`, one input) into `clip_sound_<scene>.wav` in the run folder, 48 kHz stereo. The mix then reads these files as plain inputs at their scene times. Nothing was added to the picture render graph (the rule from `audiogram.md`).
- The mix stage for clips is the last one and exists only when a scene asks for sound. For `keep`, the voice file is read once more as an input for the duck's side chain, so the older graph text does not change.
- Only numbers that scriptvox formats reach the filter text. File paths are `-i` arguments.
- Old briefs: no field, or `none`, gives the same plan id and the same `master.wav` bytes. `none` is removed on load, so it is also not in the recipe.
- Keys: the mix has no cache; `master.wav` is made again on every run. The scenes, with `sound`, are in the plan id, so a changed choice is a new plan and a new run. The render key leaves the sound out on purpose, so a changed `sound` does not change the picture; the test shows that the new video has the new mix.
- `replay` and `resume --patch` carry the field: it is in the recipe inside the MP4.

## Run on 10 Nov 2026

Worktree build, FFmpeg held to 8 GB of address space. Tests in `tests/clip_sound.rs`, stub voice (a 330 Hz tone), clips made with lavfi (`testsrc2` and `sine`), levels read with two 50 Hz band filters and `volumedetect`. The first 5 failed first with "unknown field `sound`" (the fifth on a missing artifact), then passed; the last 2 were written after the code, on a review, and passed at once:
- `keep` on scene 2 only: the 1000 Hz tone is 20 dB or more louder in scene 2 than in scene 1; the loudness, silence and av_drift gates pass. `replay` with a worker that fails every voice call gives the same level. `resume --patch` to `under` gives a lower level on the same picture.
- `under` against `keep`, with a voice that is silent after 1 s: 9 to 15 dB apart, and `keep` within 4 dB of the voice.
- Refused with `scenes[1]` in the message, by `plan` and by `make`: a clip with no audio stream (file named), a photo, `stock`, `open_image`, `generate`, no `media`, an unknown word, an empty script.
- No field and `none`: the same plan id and the same `master.wav` bytes. Also by hand: the same brief (2 scenes, `tts` `none`, `music` `calm`, a clip with `media_from`) on the code before this change and after gave the same plan id and the same SHA-256 of `master.wav`. With `sfx` the bytes differ from run to run on the old code too (the whoosh is noise).
- `media_from` 3.5 on a file with 1000 Hz then 2500 Hz from 3 s: only 2500 Hz is heard.
- `media_to` 0.6 in a scene of 1.5 s: the tone is heard for the first 0.6 s and is 20 dB or more lower after it, while the picture slows or holds.
- `music` `calm` and `sfx` with a `keep` scene and an `under` scene: exit 0, the gates pass, both tones are in their scenes.
- A render with `none`: `explain` shows no `sound` in the recipe.

Real run, free Edge voice, 16:9, 14.3 s, 3 scenes: narration on the plain background; a clip with a 1000 Hz tone, `under`, with narration; a clip with a 2500 Hz tone and pink noise, `media_from` 2, `keep`, script `Listen. [pause 3]`. All 8 gates pass (loudness -14.0 LUFS, longest silence 0.0 s, av_drift 0.3 ms, 34 of 35 words heard). Band levels in the final MP4 (mean, dBFS):
- 1000 Hz: scene 1 -39.2 (the voice's own share of that band), scene 2 -29.0, 0.4 s before scene 2 -59.4, 0.3 s after it -60.3, scene 3 -62.7. The whole sound of scene 1 is at -16.9, so the tone is about 12 dB under the voice.
- 2500 Hz: scene 3 in the pause -19.4, while the voice says "Listen" -26.7 (ducked 7 dB), scene 1 -53.5.
- Memory: the largest process of the whole run (voice, clip passes, mix, render, gates and the listening step) reached 934 MB. This is an upper bound for FFmpeg, not its own measure.

## Not done, not run

- Not for `stock`, `open_image`, `generate`, an `inset`, an `overlay`, `media` as a folder with no scene file, cutaways or `base_video` (that one has `base_sound`).
- No level field: `under` is 12 dB, `keep` is the voice's level.
- A clip whose sound ends before its scene ends stops there with no fade of its own.
- A sound shorter than about 0.4 s cannot be measured and goes in at its file level.
- A scene with no words at all cannot hold a clip (see Timing).
- Not run: a paid voice, 9:16, `--only-scene`, a kill and resume in the middle of a render with the field, `ambience` or a music file together with `sound`, a listening check by a person, Windows and macOS.
