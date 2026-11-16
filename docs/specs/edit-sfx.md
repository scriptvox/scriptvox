# `edit --sfx "whoosh@12.0"` (G191, 13 Nov 2026)

Why: a sound effect at a set time was possible only in `make` (brief `sfx`, on scene cuts and cards). A finished
video could not get one. Idea from the public feature list of claude-code-video-toolkit; no code read.

- `scriptvox edit <video> --sfx NAME@SECONDS`, repeat for more (at most 32).
  - `NAME` is a built-in effect, `whoosh`, `pop` or `tick` (the same three as in `make`, made by FFmpeg, no
    third-party audio), or a sound file inside this project.
  - `SECONDS` is a time in the result, after every cut and speed change of the same call.
  - A `whoosh` peaks at its time: it starts 0.4 s before (at 0 when the time is under 0.4 s). `pop`, `tick` and an
    own file start at their time.
- `--sfx-level quiet|normal|loud`: the peak of each effect is 18, 12 (default) or 6 dB under the peak of the
  video's own sound. A video with no sound: the same number of dB under full scale.
- The effects are mixed after the cuts, the speed and the music, and before the -14 LUFS pass. They are not ducked.
  The length does not change. An effect that runs past the end is cut at the end.
- `--sfx` alone copies the picture (`picture_copied: true`) when the codec fits an MP4.
- Result: `sfx` with `level`, `under_peak_db`, `base_peak_db` and one entry per cue: `name`, `at`, `starts_at`,
  `gain_db`.

## Errors (all exit 2, nothing is written)

| Message | Fix line |
|---|---|
| `--sfx whoosh`: no time | Give NAME@SECONDS, for example --sfx whoosh@12.0. |
| no sound effect `x` | Use whoosh, pop or tick, or a sound file inside this project. |
| the file is outside this project folder | Copy the sound file into the project. |
| the time is after the end of the video | Give a time from 0 to the length (the message has it). |
| more than 32 | Run two calls. |
| `--sfx-level` is not quiet, normal or loud | Use --sfx-level quiet, normal or loud. |
| the file is not plain media, or has no sound | Give a real sound file. |

## Measured, 13 Nov 2026, 2 cores

Real run: an 11.33 s talk made with the local voice (`kokoro-rust`, `--local-only`), 1920x1080, 340 frames, peak
-1.5 dBFS. `edit talk.mp4 --sfx whoosh@4.0 --sfx pop@1.0 --sfx ding.wav@7.5` (`ding.wav`: an own file, 0.3 s at
1,500 Hz). "Added sound" is the result minus the source, sample by sample, left channel.

| Check | quiet | normal | loud |
|---|---|---|---|
| Target peak of each effect (dBFS) | -19.5 | -13.5 | -7.5 |
| Pop, 1.00 to 1.15 s, peak of the added sound | -19.5 | -13.5 | -7.5 |
| Whoosh, 3.6 to 4.1 s, peak of the added sound | -18.8 | -14.6 | -8.6 |
| Own file, 7.5 to 7.8 s, peak of the added sound | -19.4 | -13.4 | -7.5 |
| Added sound with no cue, 5.0 to 6.5 s, RMS | -61.8 | -62.3 | -62.1 |
| Result loudness (LUFS) | -14.20 | -14.16 | -13.82 |

- Picture: 340 frames before and after, MD5 of the picture stream the same (`6bb1f24c...`), `picture_copied: true`.
- Length: picture 11.333 s before and after; `av_drift` pass.
- Peak of the whole file: -1.49 dBFS before and after.
- Fault found on the way: a mono effect came out 3 dB under its number, because the peak was measured before the
  mono to stereo step. The peak is now measured in the form the mix gets.

Tests: `tests/edit_sfx.rs` (an own effect at its time, 12 and 6 dB under the sound, picture packets the same; a
whoosh on a video with no sound; 5 refusals with no file left) and 3 unit tests in `src/edit_sfx.rs`.

## Limits

- One level for all effects of a call. For two levels, run two calls.
- The level is set from peaks, so a file with one loud click and a soft body sounds softer than its number.
- No person listened: the proof is in numbers only.
