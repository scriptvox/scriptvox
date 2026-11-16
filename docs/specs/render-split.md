# Render in groups of scenes (13 Nov 2026)

Open item of `output-size.md`. The picture of a video was made by one FFmpeg process with one filter graph for all scenes. FFmpeg holds some frames of every scene's source at once, so memory grew with the number of scenes, not with the length. Under the 8 GB limit, 3840x2160 passed with 7 scenes and failed with 8, and 1920x1080 failed with 30.

Now the shot track is made in groups of scenes, each group in its own FFmpeg process. The result is the same video, byte for byte in the picture stream and the sound stream.

There is no new command and no new brief field. Plan ids do not change.

## When it is used

- The load is megapixels of the frame times shots. Over 45 the picture is made in groups. That is 6 scenes at 3840x2160, 13 at 2560x1440, 22 at 1920x1080, 49 at 1280x720.
- Under that load `make` builds and runs the same FFmpeg command as before: the graph text for one process comes from the same code (`media::graph`). Checked: the 6 scene briefs below, made with the build of the main checkout (commit 9859c3e) and with this build with no variable, have the same picture stream MD5.
- A group holds 25 megapixels of shots, at most 6 shots: 3 scenes at 3840x2160, 6 at 1440 and below.
- `SCRIPTVOX_SPLIT_SCENES` decides when it is set: `0` is never, another number is that many scenes a group at any load. It is for tests, for measurement, and for a machine with less memory.
- A video with a base video, or with the plain background only, has one shot. It stays in one process.

## One more attempt in groups (14 Nov 2026)

The load rule counts megapixels times shots. A small frame with many scenes is under it and can still fail in one process: each FFmpeg input has its own threads, and their address space does not follow the frame size. Seen with `--draft` (960x540): 34 scenes with 18 overlays (load 18) and 18 scenes with 18 overlays (load 9) failed under the 8 GB limit; 9 scenes passed. The fix line was "Use a smaller `size`, or fewer scenes", or "Run the same command again", and neither helped.

- When the render in one process fails for memory, `make` makes the picture once more, in groups: 6 shots a group at 1440 and below, 3 at 3840x2160 (the group size of a load over the limit). It prints `the render in one process ran out of memory; one more attempt, with the picture in groups of N scenes`.
- "Fails for memory" is one of the texts that FFmpeg gave for this fault: `Cannot allocate memory`, `malloc of size`, too few chunks with no error (`chunks were written`), and the PNG decoder that gets no memory (`Decode error rate`, or its number -1145393733 when that line is cut).
- There is one more attempt only. It is not made when `SCRIPTVOX_SPLIT_SCENES` is set (the user decided), when the picture was in groups already, when the video has one shot, or when the shots are too few for 2 groups.
- An error that is not one of those texts (a missing file, a stopped render, the fault of the tests) gets no second attempt.
- If the picture was in one process and no second attempt was possible, the fix line names the variable: `Set the environment variable SCRIPTVOX_SPLIT_SCENES=4 ... and run the same command again.`

Measured 14 Nov 2026, the manual brief (33 scenes, 18 overlays, 79.5 s, `tts: none`), `--draft`, no variable, FFmpeg n8.1.3 under the 8 GB limit: 3 of 3 runs failed in the first attempt, made 6 groups in the second, and passed with 1193 frames in 23 s. Before the change the same command failed 3 of 3.

Open: the load rule itself does not count the inputs. A rule that also splits over a number of shots would make the first attempt pass. It changes when existing videos are made in groups, so it is not done here.

## Where the split is made

The video graph has two parts. The first is the shot track: one chain for each shot, joined by `xfade`, with the output `[bg]`. The second is every other layer: the look, titles behind the subject, the waveform, insets, overlays, the end card, the logo, and the captions and cards file. Only the first part grows with the scenes.

1. Groups. Group `g` holds its own shots and the first shot of the next group. Its graph is the graph of one process for those shots only. It writes frames `start..end` of the full track to `group_NNNN.mkv`.
2. Final pass. One FFmpeg input reads the group files in order (concat list). The graph starts with `[0:v]settb=1/FPS,setpts=N,setparams=...,format=...[bg]`, and then the text of the layers follows, the same text as in one process. Only the numbers of the FFmpeg inputs move, because the shot inputs are now one input. This pass goes through the chunked render as before.

No transition is made in the final pass. So the final pass has one picture input whatever the number of scenes.

## Frames at a group border

- FFmpeg's `xfade` rounds its `offset` to whole frames, halves up. `media::shot_frame` does the same sum with the 3 decimals that the graph prints. So the code knows the frame where each shot starts in the full track.
- A transition into shot `k` mixes frames `at..=at+d`, with `d` the length of the transition in frames. The cut between two groups is at frame `at + d + 2`. From there the track is shot `k` alone.
- The earlier group reads shot `k` as its last shot and stops at the cut, so it has the frames that the border transition needs. The later group starts shot `k` at its own first frame and drops the frames before the cut. So motion in the shot (a push in, a pan, the moving background) is at the same place in both.
- Each shot is built from its place in the full list: its length, its transition lengths, the zoom steps and the file follow the full list. A shot at a border is the same chain in both groups.
- Inside a group the `xfade` offsets are whole frames from the group's first shot, printed in microseconds. FFmpeg rounds them back to the same frames.
- A shot shorter than its transition plus 2 frames cannot be a border (the next transition would start before the cut). The border moves to the next shot. If no border is left, the render stays in one process.
- A group that is not the last one must have its exact frame count, or the render stops with an error.

## Time of captions and layers

The joined group files are the full track from frame 0. Frame `N` gets time `N / FPS` again in the final pass (`setpts=N`), because the times in the files are in milliseconds. Every layer uses the same absolute times as in one process: nothing in the layers text changes but the input numbers.

## Pixel format

FFmpeg chooses the pixel format, the colour matrix and the range of the track from all shots and from the filters after it. Examples measured: plain backgrounds alone give `gbrap`; a clip, a JPEG and an end card give `yuv444p` with the full range. A group alone could get another choice, and then the numbers change (first attempt: the end card had a PSNR of 29 dB against one process).

- Before the groups, one FFmpeg run of one frame reads the choice. The shots are built on a very small frame, `pad` brings them to the real size, and the real layers follow. FFmpeg writes its choice with `-print_graphs_file`, and the code reads the output of the `xfade`.
- Each shot chain of a group ends in `format=pix_fmts=...:color_spaces=...:color_ranges=...` with that choice. The final pass sets the same marks after the group files.
- The group files are FFV1 (lossless) in Matroska. `yuv444p`, `yuva444p`, `gray` and two 10 bit formats are stored as they are, `gbrp` and `gbrap` as packed RGB (the same samples in another order), and the full range is kept with `-color_range pc`.
- That run has one thread for each part (`-threads 1 -filter_threads 1 -filter_complex_threads 1`) and `MALLOC_ARENA_MAX=2`. Without them it needed 4.8 GB of address space for 30 inputs, more than any group.
- A track format that FFV1 cannot hold with no change, or an FFmpeg that cannot write its graph (before 7.1): the render stays in one process, and `make` prints a progress line with the reason (`picture in one process, not in groups of scenes: ...`).

## Resume

- Group files are in `.scriptvox/chunks/split-<key>/` in the project, next to the chunk folders. This is not the run folder: a rerun has a new run folder, and the chunks already live here for the same reason.
- The key holds what decides the pixels of the track: the version, every group's inputs and graph, the storage format, the frame rate, the length, and the bytes of every input file.
- A group is written as `group_NNNN.part.mkv` and renamed when it is whole. A rerun of the same command keeps each finished group and makes the others. The result has `metrics.scene_groups` and `metrics.scene_groups_kept`.
- The final pass has its own chunks and resumes as before.
- After the final file is in place, the group folder is removed with the chunks.

## A kept group file is not trusted

A file under the right name can be put there by another program. Before a kept file is used, and before a new part file gets its name:

- it must be a plain file, not a link, and its real path must be in the group folder; the group folder must be a real folder of the chunks folder;
- its first bytes must be Matroska (`media_gate::check`);
- its codec must be FFV1 and its frame count must be the planned count (the last group: the planned count or one frame less, because the track ends with its last shot).

A kept file that fails is removed and made again. This is not an error. The list file is removed before it is written, so a link under its name is not followed.

The join never lets FFmpeg guess a format: the list is an `ffconcat` list with `option format_whitelist matroska,webm` for each file, the input has `-protocol_whitelist file` and the decoder is named (`-c:v ffv1`). A playlist under a `.mkv` name is refused by FFmpeg itself (checked by hand).

## Errors

| Case | Exit | Fix line |
|---|---|---|
| A group fails or is stopped | 1 | Run the same command again: finished groups of scenes are kept and the render resumes. |
| A group gets no progress line for the watchdog time | 7 | The same line. |
| A group runs out of memory | 1 | Set SCRIPTVOX_SPLIT_SCENES=1 for groups of one scene and run the same command again, or use a smaller `size`. |
| The render in one process runs out of memory, and no second attempt in groups was possible | 1 | Set the environment variable SCRIPTVOX_SPLIT_SCENES=4 (the picture is then made in groups of 4 scenes, each in its own process) and run the same command again. If it fails again: a smaller `size`, or fewer scenes in one video. |
| The final pass runs out of memory after groups | 1 | Use a smaller `size`, or fewer scenes in one video (make two videos and `scriptvox join` them). |

## What `plan` says

The render line gets one more part when the split is used, for example: `the picture is made in 6 groups of scenes, one process each, so memory does not grow with the scenes: about 1145 MB more on disk while it renders (a noisy camera clip can need 7 times that)`.

- Memory: the estimate of `output-size.md` with the scenes of one group plus one, not all scenes. So the `size` warning (over 7 GB) does not come for a video in groups.
- Time: 3 s more for each group.
- Disk: 0.25 MB for each megapixel and frame.

## Measured (13 Nov 2026, FFmpeg n8.1.3 under `ulimit -v 8000000`, 12-thread CPU, debug build)

VmPeak (address space, what the limit counts) and VmHWM (real memory) were read from `/proc/<pid>/status` of each FFmpeg child each 0.1 s. Plain scenes, some with a title card, end card, no voice, `fade`. "Groups" is the highest group process, "final" is the final pass. Times are FFmpeg time only.

| Frame | Scenes | Length | One process | Groups | Final pass | Video |
|---|---|---|---|---|---|---|
| 3840x2160 | 5 | 11.5 s | 6.50 GB (4.16 real), 17.3 s | 2 of 3: 2.91 GB (1.49), 8.3 s | 4.31 GB (2.63), 16.5 s | the same MD5 |
| 3840x2160 | 7 | 15.5 s | 7.66 GB (4.96), 21.8 s | 3: 2.88 GB (1.47), 13.1 s | 4.29 GB (2.66), 22.1 s | the same MD5 |
| 3840x2160 | 8 | 17.5 s | fails at 8.00 GB, 417 of 525 frames | 3: 2.88 GB (1.47), 13.5 s | 4.29 GB (2.66), 24.3 s | pass, 525 frames |
| 3840x2160 | 12 | 19.5 s | not run | 4: 2.88 GB (1.47), 13.6 s | 4.31 GB (2.67), 22.2 s | pass, 585 frames |
| 3840x2160 | 16 | 19.9 s | not run | 6: 2.93 GB (1.52), 14.9 s | 4.28 GB (2.62), 22.2 s | pass, 597 frames |
| 1920x1080 | 30 | 21.0 s | fails at 8.00 GB, 1 of 3 chunks | 5 of 6: 3.21 GB (1.18), 9.0 s | 1.87 GB (0.82), 10.0 s | pass, 630 frames |

- The 5 scene row was forced with `SCRIPTVOX_SPLIT_SCENES=3` (its load is under 45). The 7 scene row was compared with `SCRIPTVOX_SPLIT_SCENES=0`.
- The highest process is the final pass at 3840x2160: 4.3 GB at 5 and at 16 scenes. Memory does not grow with the scenes. Every FFmpeg child of the run was read, also the run that reads the track format and the gates: at 16 scenes the highest of those is 3.78 GB of address space (0.14 real), at 1920x1080 with 30 scenes 1.34 GB.
- The 16 scene run was made three times: 49, 71 and 54 s of wall time (other work ran on the machine). The 30 scene video is 21.0 s long.
- Time: the groups add 60 percent to the FFmpeg time at 3840x2160 (the track is made once and the final pass costs the same as one process, because the encode and the layers decide it).
- 1920x1080 with 30 scenes in groups of 12 (the first rule) needed 5.32 GB for a group. So a group has at most 6 shots.
- Disk: the group files of the 16 scene run are 143 MB for 19.9 s (0.03 MB for each megapixel and frame, plain background). The 6 scene 1080 brief below: 265 MB for 18.2 s (0.23). One second of a very noisy 1080 picture: 114 MB (1.8).

Same picture, 1920x1080, 6 scenes (a clip with a title, a photo with `push_in`, a gauge card, a steps card with `dissolve`, a photo, the plain background), `slide`, captions, logo, end card, groups of 2:

| Brief | Frames | Picture stream | Sound stream | Gates |
|---|---|---|---|---|
| English | 546 and 546 | the same MD5 | the same MD5 | 8 pass in both |
| Arabic | 507 and 507 | the same MD5 | the same MD5 | 8 pass in both |

The MD5 is of the stored H.264 stream (`-c copy -f md5`). The same bytes mean the same pixels in every frame, so no PSNR is given. The gates run on the final file as before.

## Tests

- `tests/render_split.rs`, `a_one_process_render_that_fails_for_memory_is_made_again_in_groups`: through `make`, with the real FFmpeg behind a script that fails as FFmpeg does without memory when the last pass reads the 8 shots in one process: the second attempt makes 2 groups and passes; with `SCRIPTVOX_SPLIT_SCENES=0` there is no second attempt and the fix line names the variable. Unit test `a_memory_fault_in_one_process_asks_for_groups` in `split.rs`.
- `tests/transitions.rs`, `groups_of_scenes_give_the_frames_of_one_process_for_every_transition`: every transition name, 24 and 30 fps, a border after each shot and after each second shot, a still and the plain background in turn. The MD5 of every frame of the track is the MD5 of one process (160 cases).
- `tests/render_split.rs`, `groups_give_the_video_of_one_process_and_resume_and_trust_no_planted_file`: through `make`; no split with no variable; groups give the same picture and sound stream; a stop after 2 groups keeps 2; a stop in the final pass keeps all groups and the finished chunks; a short file under the last group's name, and a link, a playlist and a file with the wrong frame count under the first group's name, are each made again.
- Unit tests in `split.rs` (borders, a short shot, the load, the input numbers) and `size.rs` (the plan line).

## Limits and open work

- Insets and overlays of scenes are inputs of the final pass. Its memory still grows with their number.
- A title behind the subject (`behind`) cuts its windows from the one-process graph in its own FFmpeg run (`cut_out`). That run is not split.
- `storyboard`, `snapshot` and the other commands that build a shot graph are not split.
- The run that reads the track format opens every clip and picture of the video once, on a very small frame.
- Groups run one after the other. Two at a time would be faster and would need twice the memory.
- The disk estimate is for clean pictures. Camera noise was measured on one made-up picture only.
- Found 14 Nov 2026, not fixed: a video with plain backgrounds only (cards and overlays, no clip, no picture, no end card in the test) made in groups is marked full range (`yuvj420p`, `pc`), and the same video in one process is marked limited range (`yuv420p`, `tv`). 8 plain scenes at 1280x720 with `fade`: the picture stream MD5 differs, PSNR 50.9 dB, the same 702 frames. So "the same video, byte for byte" holds for the briefs in the table above and not for this one.
- The comparison with one process is not possible over the limit (8 scenes and more at 3840x2160): those rows prove the frame count and the gates, and the border test proves the pixels on a small frame.
