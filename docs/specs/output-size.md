# Output size: brief `size` (13 Nov 2026)

Gap G194 (P6.1). Rivals give 720 to 2160 as a choice. ScriptVox made every video with a short side of 1080.

## The field

`"size": 720 | 1080 | 1440 | 2160`: the short side of the frame in px, for every `aspect`. No field means 1080.

| aspect | 720 | 1080 | 1440 | 2160 |
|---|---|---|---|---|
| 9:16 | 720x1280 | 1080x1920 | 1440x2560 | 2160x3840 |
| 16:9 | 1280x720 | 1920x1080 | 2560x1440 | 3840x2160 |
| 1:1 | 720x720 | 1080x1080 | 1440x1440 | 2160x2160 |
| 4:5 | 720x900 | 1080x1350 | 1440x1800 | 2160x2700 |
| 3:4 | 720x960 | 1080x1440 | 1440x1920 | 2160x2880 |
| 4:3 | 960x720 | 1440x1080 | 1920x1440 | 2880x2160 |
| 2:3 | 720x1080 | 1080x1620 | 1440x2160 | 2160x3240 |
| 21:9 | 1680x720 | 2520x1080 | 3360x1440 | 5040x2160 |

- Captions, cards, the hook, the logo, the outro and every measured text follow the frame: 84 px captions on 1080 are 56 px on 720 and 168 px on 2160. The layout is the same at each size.
- A clip or a picture smaller than the frame is scaled up to it. It gets no new detail. `plan` says so on its render line when the size is over 1080. `upscale: true` is a different step: a model row on generated clips.
- Generated clips are asked from their row as before: the row decides their size. Checked: `plan` of a `generate` scene gives the same stage line and cost with and without `size: 2160`.
- `--draft` makes half of the asked size. `resume --final` makes the asked size. `resume` with a brief that has no `size` keeps the size of the parent run.
- A brand file can hold `size`. The brief wins.
- The plan id holds `size` only when it is set and is not 1080. A brief with no `size` keeps its old id (checked against the build before this change).

## Errors

| Case | Exit | Fix line |
|---|---|---|
| `size` is another number | 2 | Use "size": 720, 1080, 1440 or 2160 (the short side in px), or drop the field for 1080. |
| `plan`: the estimate of memory is over 7 GB | 0, finding `size` with status `warn` | Use a smaller `size`, or fewer scenes in one video (make two videos and `scriptvox join` them). |

## What `plan` states

The render stage line, for example: `3840x2160 at 30 fps, chunked, resumable; about 38 s to render, a file of at most 47 MB (twice that on disk while it renders), about 6.5 GB of memory (estimates for a 15 s video on a 12-thread CPU)`.

The estimate (`size.rs`), with MP as megapixels of the frame:

- Render time: 0.3 s for each MP and each second of video at 30 fps.
- File: at most 0.1 bit for each pixel of each frame. A plain background needs much less.
- Memory (address space): 2.3 GB + MP x (0.14 + 0.074 x scenes) GB.

## Measured (13 Nov 2026, libx264 `veryfast`, 12-thread CPU, FFmpeg n8.1.3 under `ulimit -v 8000000`)

Peak memory was read from `/proc/<pid>/status` of the FFmpeg child each 0.1 s: VmPeak (address space, the value the limit counts) and VmHWM (real memory).

The same brief at each size, 5 scenes on the plain background, cards, outro, 17.5 s, no voice:

| Frame | Render | VmPeak | VmHWM | File |
|---|---|---|---|---|
| 1280x720 | 5.7 s | 2.71 GB | 0.75 GB | 0.97 MB |
| 1920x1080 | 11.3 s | 3.48 GB | 1.47 GB | 2.64 MB |
| 2560x1440 | 18.9 s | 4.05 GB | 1.95 GB | 5.65 MB |
| 3840x2160 | 36.1 s | 6.50 GB | 4.22 GB | 3.08 MB |
| 5040x2160 (21:9) | 50.2 s | 7.99 GB, at the limit | 5.50 GB | 6.41 MB |

Real runs with `--local-only`, 2 scenes (a 1280x720 clip with a title card, a bars card on the plain background), logo, outro, no `duration`, sizes from `ffprobe`:

| Run | Frame | Voice | Length | Render | VmPeak | VmHWM | File | Gates |
|---|---|---|---|---|---|---|---|---|
| en1080 (no `size`) | 1920x1080 | kokoro-rust, local | 6.27 s | 3.9 s | 2.83 GB | 0.96 GB | 2.35 MB | 9 pass, `contrast` warns |
| en720 | 1280x720 | kokoro-rust | 6.27 s | 1.8 s | 2.35 GB | 0.52 GB | 1.36 MB | 9 pass, `contrast` warns |
| en1440 | 2560x1440 | kokoro-rust | 6.27 s | 6.6 s | 3.34 GB | 1.44 GB | 3.68 MB | 9 pass, `contrast` warns |
| en2160 | 3840x2160 | kokoro-rust | 6.27 s | 12.7 s | 5.12 GB | 3.03 GB | 4.86 MB | 9 pass, `contrast` warns |
| en2160tall | 2160x3840 (9:16) | kokoro-rust | 6.27 s | 13.8 s | 5.38 GB | 3.30 GB | 4.33 MB | 9 pass, `contrast` warns |
| ar1440 | 2560x1440 | none (Arabic) | 7.50 s | 8.1 s | 3.34 GB | 1.44 GB | 4.20 MB | 8 pass |
| ar2160 | 3840x2160 | none (Arabic) | 7.50 s | 16.7 s | 5.09 GB | 3.01 GB | 5.72 MB | 8 pass |

- The `contrast` warning is not from `size`: the same brief with no `size` warns with the same number (1.2 to 1 for the caption at 3.5 s, on the yellow part of the plain background).
- `--local-only` stops the speech row in `make`, so gate `words` did not run there. `check <run> --hear --local-only` on en2160: heard 12 of 12 script words.
- No Arabic voice was run: the free Arabic voices are network services.
- A first set of the English runs (not the table) ran without `--local-only`, and `make` listened through the speech to text row that the install picks (a `selfhost` row with a key, else the local one). The run does not record which. The table is the second set.

By eye: frames of the 720, 2160, Arabic 2160 and tall 2160 runs at the title, the bars and the outro have the same layout; an Arabic caption at 3840x2160 is sharp at full size. A 1080 and a 2160 render of one brief (clip, photo with `push_in`, bars, hook, logo, outro), with the 2160 frames scaled to 1080: mean SSIM 0.959, lowest 0.908. The difference is the plain background, see Limits.

Every pixel number was compared between a 1080 and a 2160 run of one brief (4 scenes: clip with `pan_left` and a lower third, photo with `push_in` and a quote, a list, a clip with a stat; hook, logo, outro, `slide`): in the filter graph and in `captions.ass`, each number that changed is twice the 1080 number. One number did not change and was wrong: the blur of the outro (24 px). It now follows the short side. The side margins of the text styles (60, 40, 80) do not change; the text is centred and wrapped by measurement, so they have no effect.

## Memory: what was found and what was done

- The chunk length does not change the memory. One FFmpeg process makes all chunks, so `SCRIPTVOX_CHUNK_S=2` gave the same peak and the same failure as the default. The length of the video does not change it also (1080, 5 scenes: 1.43 GB at 6 s a scene and 1.40 GB at 12 s).
- The number of scenes does. FFmpeg holds some frames of every scene's source at once: 1080 with 2, 5 and 10 plain scenes is 0.93, 1.43 and 2.23 GB of real memory.
- Before this change, 3840x2160 with 5 plain scenes failed under the limit: "Cannot allocate memory" at VmPeak 7.99 GB.
- Done, 1: on a frame over 1080 the plain background source gives YUV 4:4:4, not RGBA (25 MB a frame at 3840x2160 against 33, and no converter for each scene). Smaller frames keep the old source, so their pictures do not change.
- Done, 2: the render child runs with `MALLOC_ARENA_MAX=2` when the variable is not set. It lowers address space only. Together: 5 scenes at 3840x2160 use 6.50 GB and pass; 2 scenes went from 5.49 to 4.67 GB.
- Tried and not used: fewer threads (`-filter_complex_threads 4 -threads 8`) made the peak higher at 10 scenes (7.6 against 6.7 GB), because the slower graph holds more frames. `-thread_queue_size 1` changed nothing.

## Limits and open work

More scenes at 3840x2160 through `make`, under the 8 GB limit (plain scenes, some with cards, outro):

| Scenes | Estimate | Result |
|---|---|---|
| 5 | 6.5 GB | pass, VmPeak 6.50 GB |
| 7 | 7.8 GB | pass, VmPeak 7.67 GB, render 54 s for 28.7 s |
| 8 | 8.4 GB | fail at the limit |
| 10 | 9.6 GB | fail at the limit |

- So the limit is 7 scenes at 3840x2160, and `plan` warns from 6 (over 7 GB). 5040x2160 with 5 scenes ran at the limit: it passed once, and it is not safe.
- A render that fails for memory names the cause and gives the fix of the plan warning (smaller `size`, or fewer scenes and `join`), not "run the same command again". FFmpeg fails in three ways there: "Cannot allocate memory", an x264 "malloc of size", or exit 0 with too few frames ("only 3 of 4 chunks were written"). All three get this fix. A rerun on kept chunks can also stop at the readback of the frame count ("765 frames, planned 969"); that message has no fix line yet.
- Done 13 Nov, `render-split.md`: over a load of 45 megapixels times scenes the picture is made in groups of scenes, one FFmpeg process each. 3840x2160 with 8, 12 and 16 scenes passes with at most 4.3 GB, and 1920x1080 with 30 scenes with 3.2 GB. The limit of 7 scenes above is the limit of one process (`SCRIPTVOX_SPLIT_SCENES=0`). `plan` then states the groups and the extra disk, and does not warn.
- The plain background is not the same picture at each size: the FFmpeg `gradients` source places its colours by pixel. Text and layout are the same.
- The estimate of time is for one CPU class. The memory estimate is fitted on plain scenes; scenes with clips were lower in the runs here. A brief whose length the plan does not know (a base video, a narration file) gets the numbers for 10 s.
- `edit`, `join`, `clips` and `dub` have no `size`: they keep the size of their input or their own rule.
