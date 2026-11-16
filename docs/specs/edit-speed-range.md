# `edit --speed "12-15@0.5"`: speed over a time range (G195, 13 Nov 2026)

Why: `--speed 1.25` changes the whole video. A slow or fast part (a slow look at one move, a fast walk through a
long step) needed three cuts and a `join`. Idea from a public feature claim in the 10 Nov comparison; no code read.

- `scriptvox edit <video> --speed "FROM-TO@SPEED"`, with commas for more: `--speed "12-15@0.5,20-24@2"`.
  A plain number (`--speed 1.25`) is the whole video, as before.
- `FROM` and `TO` are seconds of the video after any cut or `--from/--to` of the same call, and before the speed
  change. `SPEED` is 0.5 to 2. A range is at least 0.5 s long. Ranges are in time order and do not overlap. At
  most 8 ranges.
- Ease in and out: at each end of a range the speed moves between 1 and `SPEED` in 4 steps of equal length, over
  0.4 s of the source (a quarter of the range when the range is shorter than 1.6 s). The steps are equal ratios, so
  0.5 goes 0.87, 0.76, 0.66, 0.57, then 0.5.
- The picture and the sound use one time map. The voice keeps its pitch (`atempo`). Each step of the sound is cut
  and set to its exact number of samples, so picture and sound end together.
- Captions:
  - Burned captions are part of the picture: they slow down with it and stay on their words.
  - `--captions` in the same call is heard on the new sound, so its times are the result's.
  - Caption files of the run: an `.srt` or a `.vtt` beside the input with the input's name (as `make` and `edit`
    write them) is written again beside the result with every time moved through the same map. Only when the call
    has no cut, no `--from/--to` and no `--captions` (a cut makes the old times wrong). `speed.captions_retimed`
    lists them.
- All rules of `--speed` hold: not with `--duration`, `--zoom`, `--broll`, `--captions-file`, `--bleep`,
  `--background`. `--sfx` and `--music` come after the speed, so their times are the result's.
- Result: `speed` with `ease_s`, `steps`, `captions_retimed` and one entry per range: `from`, `to`, `speed`,
  `result_from`, `result_to`. `metrics.seconds_after` is the new length.

## Errors (all exit 2, nothing is written)

| Message | Fix line |
|---|---|
| not `FROM-TO@SPEED` | Give seconds and a speed: --speed "12-15@0.5", or a plain number for the whole video. |
| the speed is not 0.5 to 2 | the same |
| a range is shorter than 0.5 s, out of order, or overlaps the one before | the same, and the message names the range |
| a range ends after the video (the message has the length) | Give times from 0 to the length. |
| more than 8 ranges | Run two calls. |

## Measured, 13 Nov 2026, 2 cores

Real run: the same 11.33 s local-voice talk as in `edit-sfx.md` (30 fps, 340 frames, burned karaoke captions,
`talk.srt` and `talk.vtt` beside it).

| Check | `--speed "4.2-7@0.5"` | `--speed "1.6-4.1@2,7-11.33@0.75" --sfx pop@2.0` |
|---|---|---|
| Length by the map | 13.678 s | 11.558 s |
| Picture | 410 frames, 13.667 s | 347 frames, 11.567 s |
| Sound stream | 13.694 s | 11.584 s |
| `av_drift` | pass, 27.3 ms | pass, 17.3 ms |
| Range in the result | 4.2 to 9.345 s | 1.6 to 3.023 s, 5.923 to 11.555 s |
| Caption files written again | `.srt`, `.vtt` | `.srt`, `.vtt` |

For the first run:

- Sound before the range (1.0 to 2.0 s) against the source: best match at a shift of 0.00 ms.
- Sound after the range (8.0 to 9.0 s of the source) against the map (2.345 s later): best match at 0.02 ms.
- `slow.srt`: cue 3 is 4.157 to 9.344 s (source 4.157 to 6.999), cue 4 starts at 9.344 s. Cues 1 and 2 as before.
- Frames at 4.0, 5.5, 7.0, 8.5, 9.2 and 9.6 s, read by eye: the burned caption of sentence 3 stays for the whole
  slow part and its highlight moves on slowly; sentence 4 is on at 9.6 s.
- Seams on the talk: the largest sample to sample step at the 10 step edges is 0.12 of full scale; the same
  measure at 200 random places in the range has a median of 0.013 and a 95th percentile of 0.186. So no click
  stands out. 7 of the 10 edges are in a pause of the voice (0.000), so this says little about a dip.
- Dips, on a steady 440 Hz tone (6 s, the test clip): the level in each 10 ms of the result against the median.

| `--speed` | Lowest 10 ms, first version | Lowest 10 ms, now |
|---|---|---|
| `2-4@0.5` | -67.5 dB (silence), and 6 more dips of 4 to 8 dB | -0.2 dB |
| `2-2.5@0.5` (the shortest range, steps of 31 ms) | -54.4 dB, and 7 more dips of 4 to 10 dB | -0.3 dB |
| `2-2.5@2` | -0.2 dB | -0.2 dB |
| `2-4@2` | -0.4 dB | -0.3 dB |
| `2-4@0.75` | -3.5 dB | -0.3 dB |
| `5-6@0.5` (the range ends with the file) | not run | -0.2 dB |

Fault found on the way (the review asked for this measure): `atempo` gives the last sound of a short piece late
or not at all, so at a slow speed each step ended in padding. Each step with a new speed now reads 0.25 s of the
source past its own end, and the cut to its exact length cuts real sound. The numbers of the talk run did not
change (410 frames, 13.694 s of sound, 27.3 ms, the same `.srt`).

Tests: `tests/edit_speed.rs` (a 3 kHz burst before, inside and after the range lands where the map says, within
10 ms outside the range; 440 Hz stays 440 Hz; the frame count; the `.srt`; a plain number still works; a steady
tone has no 10 ms more than 1 dB under its median at `2-4@0.5`, `2-2.5@0.5` and `2-2.5@2`; 5 refusals), 4 unit
tests in `src/edit_speed.rs`, and in `src/mcp.rs` a number and a range from a client both become `--speed`.

Not run: `--captions` in the same call (no speech to text row on this machine without the network), a range with
a cut in the same call, a variable-rate phone clip, a video longer than 14 s.

## Limits

- The ease is a staircase of 4 constant speeds, not a curve. At 0.5 the speed changes by 13 percent a step.
- Each step is its own `atempo` pass, so the sound has a seam at each of the 10 step edges of a range: the
  level holds (measured above), the phase of a steady tone can jump. Not heard by a person.
- `--speed` is text now, in the CLI and in the MCP input schema (it was a number). A number still works in both.
- Closed 13 Nov 2026: a plain `--speed 1.25` (and `--duration`) moves the caption files of the run too. It is one step over the whole video with no ease, through the same map and the same rule (no cut, no `--from/--to`, no `--captions`). The result has `speed` with `speed` and `captions_retimed`, and no `ranges`. Test `a_plain_speed_moves_the_caption_files_with_the_video`: cues at 1, 2, 5 and 5.5 s land at 0.8, 1.6, 4.0 and 4.4 s in the `.srt` and the `.vtt`, and the sound burst of 5 s is heard at 4.0 s.
