# A video with no words (14 Nov 2026)

Before: an empty script was exit 2, "the script is empty". A reel with a picture and a song, and no words, could not be made without false words and captions. A user test on 14 Nov 2026 failed on this. The same test found three more faults in the same code, and this spec covers them too.

## 1. No words

Two ways. Both are brief fields; `make` has no flag for them.

```json
{"aspect": "9:16", "music": "pulse", "scenes": [
  {"seconds": 4, "media": "cat1.jpg"},
  {"seconds": 5, "stock": "cat kitchen"},
  {"seconds": 3, "media": "cat2.mp4"}
]}
```

```json
{"aspect": "9:16", "duration": 12, "music": "pulse", "media": "clips/"}
```

- Scene field `seconds` (0.2 to 600): the scene has no `script`. Its picture shows for that time. No voice, no caption. `music` plays on. All other scene fields work as before (`media`, `stock`, `open_image`, `generate`, `card`, `camera`, `transition`, `sound`).
- A brief can mix scenes with a `script` and scenes with `seconds`. A scene with `seconds` is exact: no breath is added before or after it, and a caption page leaves after its words and does not stay over the scene with no words.
- `duration` with no `script`, `script_file`, `scenes` and `lines`: a video of that length with no words. The picture is `media`, `stock`, or the built-in background.
- With no words at all, no voice is made and `tts` is not used: no voice row is picked, no key is asked for, and `--local-only` passes.
- Length. With no words at all the length is exact: the sum of the `seconds`, or the `duration`. The fit to `duration` does not run, so nothing is stretched. When the last scene has `seconds`, the video ends there, with no tail.
- With words and `duration`, the fit works on the whole narration as before, and a tempo change (8 percent at most, 16 for a voice with no rate) also changes each `seconds`. For exact times, drop `duration`.
- Plan id: `seconds` is in the plan id only when a scene has it. A brief with no `seconds` keeps its id.

Errors, all exit 2, at `plan` and at `make`:

| Message | Fix line |
|---|---|
| `the script is empty` | Says: write the script, or give `duration` and no script, or give each scene with no words its `seconds`. |
| `scenes[i] has seconds and a script: a scene with words takes the time of its words` | Drop one. For a hold after the words use a pause tag. |
| `scenes[i].seconds is a number from 0.2 to 600, not x` | Gives an example. |
| `duration is 12 s, and the seconds of the scenes add up to 9 s` | Drop `duration`, or change the scenes. Only for a brief with no words. |

Gates on a video with no words:
- `silence`: passes with `music`. With no music the video is silent, and the gate gives its usual warning.
- `slideshow`: as before. A still gets the slow zoom, so it is not frozen.
- `captions`, `text_bounds`, `contrast`: there is no page, so they are not in the findings. `captions.ass` has no `Dialogue` line.

Limits:
- No `make` flag. A brief only.
- The run folder still has `captions.srt` and `captions.vtt`, with no cue. No caption file is written beside the video.
- `narration` (a recording) and `lines` (a dub) need their words: no change there.
- With a voice that is made, a scene with no words starts its `seconds` before the first word of the next scene, so it starts a little after the last word before it (0.3 s in the run below).

## 2. A pause at the end of a script

Before: with `tts: none`, `"Dance. [pause 11]"` gave a 1.4 s video. The length came from the last word, so a pause tag after it was lost with no message. (A voice that is made kept it.)

Now: a pause at the end holds the picture and the music for its time. The caption page leaves after its words when the pause is longer than 1 s, as at a long pause in the middle.

A pause tag is 0.1 to 10 s. A tag above 10 s is cut to 10 s, and now `plan` and `make` say so: finding `pauses`, status warn, with the fix "give a scene `seconds` and no script". Limit: a run of tags that add up to more than 10 s is also cut to 10 s, and that case has no message.

## 3. One answer from `plan` and `make`

Cause of the fault: `plan` counted the pause at the end and `make` did not (part 2). So the plan said ok, and the run stopped with exit 3 `SVX_NEEDS_LONGER_SCRIPT`.

Now:
- The length of `tts: none` comes from one function (`make::reading_len`) for the plan and the run.
- The reading pace of `tts: none` comes from one function too (`silent_lang`: Arabic when the script has Arabic letters).
- The fit to `duration` is one function (`fit::ladder`). `make` runs it on the measured voice. For `tts: none`, `plan` runs it on the same exact numbers, before any work. So `plan` and `make` stop at the same place with the same code, exit and message, and no run folder is made.
- A voice that is made (local or network): the plan cannot know its length before it is read. The plan stops only at a clear miss (as before), and the voice stage now says: "the fit to `duration` is checked at make, after the voice is read". Limit: a plan does not read a free local voice to measure it. A plan makes no file (`voice-plan-cache.md`).
- The render estimate ("estimates for a 12 s video") and the length asked from a music row now count the pauses and the `seconds`. Before, a brief with no `duration` was estimated from its words alone ("a 1 s video").

## Checked

`tests/no_words.rs`, no network: three scenes with `seconds` (length, the two cuts, no caption, gates), words mixed with `seconds`, `duration` alone, the four errors, a pause at the end, the same answer from `plan` and `make`, and the plan id input of a scene with no `seconds`.

Real runs on 14 Nov 2026, worktree build, `--local-only`, 720x1280:
- One scene, `{"seconds": 12}`, built-in background, music `pulse`: 12.000 s picture and sound by `ffprobe`. `make` and `check`: black, first_frame, flashes, slideshow (0 percent frozen), silence (0.0 s), loudness (-13.8 LUFS), av_drift pass. No captions gate. Sheet of 12 frames read by eye: the background moves, no text.
- Three scenes with `seconds` 4, 5 and 3 (a photo, a clip, a photo with `push_in`), `transition: cut`: 12.000 s, the same 7 gates pass (slideshow 26 percent frozen), sheet of 12 frames: 4, 5 and 3 frames of each picture, no text.
- `"Dance. [pause 11]"` with `tts: none`, `duration` 12, music `pulse`: plan ok with the warning `pauses`, make 12.0 s (before: plan ok, make exit 3). Without the fix of the caption page the word stayed on screen for 12 s (seen on the sheet); now the page leaves after its word.
- Local voice `kokoro-rust`: a scene with words, a scene with `seconds` 3, a scene with words and `[pause 2]`. 8.1 s; scene starts 0.35, 1.62 and 4.62 s; the second page ends at 6.07 s and the picture holds to the end; 10 gates pass.
- The loop of a user: `make --detach`, `wait`, then `resume <run> --brief` with the first scene at 2 s in place of 4: the child is 10.0 s with scene starts 0.0, 2.0 and 7.0. A parent with `duration` 6 alone, then `resume --brief` with the three scenes: 10.0 s (the scenes with `seconds` set the length; the parent's `duration` is not kept).
- The three scenes with `cut_on: "beat"` and `pulse`: 12.0 s, both cuts moved to a beat (4.0 to 4.086 s, 9.0 to 8.875 s, 100.2 BPM).
- The briefs of the failed test, `plan` only: the brief with three clips and `[pause 3]` tags is estimated "for a 12 s video" (was "a 1 s video").
