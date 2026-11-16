# `edit`: small operations (13 Nov 2026)

Gap G133 (P6.4). Six small changes to a video, local and free, in `edit_small.rs`. Own design; the gap rows name where the ideas were seen.

## Rules for all of them

- They work alone or with each other, in one call. The order is fixed: deinterlace, bars, tilt, hold, loop.
- Every other `edit` option needs its own call (exit 2: "use two calls"). Run these first, then the other options on the result.
- The result is `--out`, or `scriptvox-out/<name>-edit.mp4`. The picture is encoded again; the sound is AAC 192k.
- The answer has the usual `edit` fields (`artifacts.video`, `metrics.seconds_before`, `seconds_after`, `size`, gate `av_drift`) and one field for each operation that ran.
- A picture change with a loop is two encodes: the change, then the loop of its result.

## `--remove-bars`

Crops black bars (letterbox or pillarbox). FFmpeg `cropdetect` (limit 24, round 2) reads 2 frames a second of the first 120 s; the last box is used.

- Answer: `bars` with `crop` (`w:h:x:y`), `size_in`, `size_out`, `removed_px` for each side.
- No bars (under 8 px on each axis): exit 2, "the video has no black bars". Fix: drop the flag. Nothing is written.
- The found picture is under a third of the frame on an axis: exit 2, "too dark to find its bars". Fix: crop by hand with `--reframe`, or give a brighter part.
- Limit: bars that are not black (a blurred or coloured fill) are not found. Bars that are burned in for only a part of the video are cut by the last box found.

## `--hold AT@SECONDS`

Holds the frame at AT (seconds of the source) for SECONDS more, with silence under it. Repeat the flag for more. The video gets longer by the sum.

- AT is moved to a frame boundary, so picture and sound are cut at the same place. AT 0 holds the first frame; AT at the end holds the last frame.
- Limits: at most 16 holds, each at most 60 s, two holds at least 0.1 s apart.
- Errors (exit 2), each with the fix line "Give --hold AT@SECONDS, for example --hold 4.0@2 ...": not `AT@SECONDS`; the time is not inside the video; the length is 0 or over 60; two holds at the same place; more than 16.
- Answer: `hold`, a list of `at` and `seconds`.

## `--loop TIMES` and `--loop-to SECONDS`

`--loop 3` plays the video 3 times. `--loop-to 30` repeats it up to 30 s and cuts the last play there.

- Limits: 2 to 100 plays, and a result of at most 600 s. One of the two flags, not both.
- Errors (exit 2), fix line "Give --loop 3 (three plays), or --loop-to 30 ...": TIMES under 2 or over 100; SECONDS not longer than the video; a result over the limit.
- Answer: `loop` with `plays` and `seconds`. Through MCP the field is `loop`.
- Limit: when the sound of the file is some ms longer than its picture (usual for AAC), each seam holds the last frame for that time. For a loop with no visible seam, make the video with brief `loop: true`.

## `--deinterlace`

Makes an interlaced video progressive with `bwdif` (one frame out for each frame in: the frame rate and the length stay).

- Before the work, two measurements: the field order of the stream (`ffprobe`), and FFmpeg `idet` on the first 300 frames.
- The stream says progressive and under 90 percent of the measured frames look interlaced (or none could be measured): exit 2, "the video is not interlaced". Fix: drop the flag. Nothing is written. Reason for the 90 percent: `idet` alone called 80 of 120 frames of a progressive test picture interlaced.
- Answer: `deinterlace` with `filter`, `stream_says_interlaced`, and the `idet` counts `before` and `after`.
- Limit: telecined film (3:2 pulldown) needs another filter and is not handled.

## `--tilt DEGREES`

Turns the picture, for a horizon that is not level. -15 to 15, not 0; a minus turns to the left. The picture is zoomed by `cos(a) + sin(a) x (long side / short side)`, the smallest zoom that leaves no empty corner. The frame size stays.

- Error (exit 2): a value outside the range. Fix: "Give for example --tilt -2.5 ...".
- Answer: `tilt` with `degrees` and `zoom`.
- Limit: the angle is given, not found. 4 degrees on 16:9 costs 12 percent of the picture.

## Speed steps

Already there, not built again: `edit --speed "0-2@2,2-4@0.5"` (`edit-speed-range.md`, G195) gives each range its own speed with a short ease. Checked on 13 Nov: a 4.0 s clip with these two ranges came out at 4.73 s.

## Measured real runs (13 Nov 2026, capped FFmpeg, clips made with `testsrc2` and a 440 Hz tone, 4.0 s)

| Call | Result, by `ffprobe` and FFmpeg filters | Time |
|---|---|---|
| `--remove-bars` on 1280x720 with 90 px bars | 1280x540, 120 frames, `crop` 1280:540:0:90 | 1.5 s |
| `--remove-bars` on a clip with no bars | exit 2, no file | |
| `--hold 1.0@2 --hold 3@0.5` | 6.50 s, 195 frames; `freezedetect` 0.97 to 3.00 s and 4.97 to 5.50 s; `silencedetect` 1.00 to 3.00 s and 5.00 to 5.50 s | 1.5 s |
| `--loop 3` | 360 frames of 120, 12.10 s, av_drift 30 ms | 2.7 s |
| `--loop-to 10` | 10.00 s, 298 frames (2 seams) | 2.3 s |
| `--deinterlace` on 720x576 `tt` | `progressive`, 100 frames of 100; `idet` 100 interlaced to 5 | 1.3 s |
| `--deinterlace` on a progressive clip | exit 2, no file | |
| `--tilt -4` | 1280x720, zoom 1.122, no empty corner on the frame | 1.5 s |
| `--remove-bars --tilt 3 --hold 2@1 --loop 2` | 1280x540, 300 frames, 10.05 s | 2.0 s |

Frames of the bars, tilt and mixed results were read by eye on one sheet.

## Not done from the gap row

360 view, grid and green screen. They are larger than a flag and stay open in G133.

## Inputs are plain media only

A review asked whether the encode passes give FFmpeg an input that is not checked. They do not: `edit` checks the input by its first bytes before any step (`media-inputs.md`), and `Ffmpeg::run` checks every `-i` of every encode again, the loop pass included. The measuring passes (`cropdetect`, `idet`, the field order) build their input with the same gate. Test `a_playlist_as_the_input_is_refused_by_each_operation_before_ffmpeg_starts`: a concat list and an HLS list named `.mp4` are refused by each of the 6 flags (exit 2), and a recording FFmpeg and ffprobe show that neither was started on the file.
