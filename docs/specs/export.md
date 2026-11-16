# `scriptvox export <run_id>`: one folder to publish from

Source: OpenMontage cherry #28 (export bundle and publish metadata).

- Writes `scriptvox-out/<run>-export/` (or `--dir`): `video.mp4`, `captions.srt`, `cover.jpg` (the hook frame at
  0.6 s, else 0.5 s), `credits.txt` when stock was used, and `metadata.json`.
- `metadata.json`: `title` (the hook, else the first sentence, at most 70 characters cut on a word), `description`
  (the script without pause tags), `lang`, `aspect`, `duration_s`, `cost_usd`, `parts` (the voice with its terms
  from `voices.toml`, every generator row found in the run's job records with its terms, the stock items) and
  `commercial_ok`: true only when every voice and generator part is cleared for commercial use; unknown terms count
  as not cleared. When false, the result has a `warning`.

Measured 3 Nov 2026: the ambience test run (Edge voice, Wan, MMAudio) exported in under a second, 4 files;
`commercial_ok` false with the Edge voice and MMAudio named. Test `export::tests::titles_cut_on_a_word`.

`--gif` (4 Nov 2026, Manim cherry #15: render to GIF, GIF quality): also writes `preview.gif`, the first 6 s at
12 fps, 360 px wide, one palette from the clip (`palettegen stats_mode=diff`, 192 colours) and ordered (bayer)
dithering, which stays still from frame to frame (error diffusion crawls on a moving background). Measured on the
formula run: 0.5 s, 2.9 MB (the animated gradient changes every pixel; 10 fps and 128 colours saved only 10
percent). Checked by eye on a crop: the formula and panel are clean, a fine dither pattern in the panel.
Test: `export::tests::gif_args_make_one_palette`.

Cover candidates (4 Nov 2026, Manim cherry #18, stills and thumbnails): a run with scene cards also writes
`cover-1.jpg` to `cover-3.jpg`, one per card at its `landed` time from `cards.json` (the same frames as
`snapshot --cards`), listed in `artifacts.covers` and in `metadata.json` `covers` (file, time, kind, scene).
`cover.jpg` stays the hook frame. Measured on the plot run r_b4b31e: 3 covers, each with its graph fully drawn,
checked by eye. A run without cards writes none.

## Export a file (5 Nov 2026)

`export <video.mp4>`, for results of `edit` and `join`: video, cover, metadata with `--title` (default the file name) and `--tags`. No run means no script, no chapters and no known terms, so `commercial_ok` is false.

## More formats and a transparent overlay (5 Nov 2026, rerun)

Gap: Remotion RM-18 (AV1 #3917, HLS #2930, alpha, 41 issues) and HyperFrames #10. ScriptVox only gave H.264 MP4 and a GIF.

- `export --format webm,av1,hevc,prores,hls`: `video.webm` (VP9, Opus), `video.av1.mp4`, `video.hevc.mp4` (tag hvc1, plays on Apple), `video.mov` (ProRes HQ, PCM), `hls/index.m3u8` (a keyframe every 6 s, so a second H.264 encode, the sound copied; one rendition; a part over 6.5 s is exit 1). Each file is read back: its codec and its length within 0.5 s, else exit 1. A `%` in the folder or the video's name stays a `%`.
- `export --overlay`: captions and cards alone on a transparent frame, for an editor to lay over their own cut: `overlay.mov` (ProRes 4444, quality 9) and `overlay.webm` (VP9 alpha). Needs a run id: a video file holds its captions only as pixels. Not in the overlay: logo, behind titles, the picture.
- `artifacts` gives `video_webm` and the rest, and `overlay_mov`, `overlay_webm`; `metadata.json` lists `formats` and `overlays` with bytes.
- Not taken: several HLS renditions (a ladder), add when someone streams with it.

Measured, 15.9 s Arabic 4:3 run r_5926c0, 1440x1080: H.264 2.8 MB, VP9 1.0 MB, AV1 0.9 MB, HEVC 0.9 MB, ProRes 257 MB, HLS 2.9 MB, overlay webm 0.9 MB; overlay ProRes 212 MB at the fixed rate, about 25 MB for 6 s at quality 9. All 7 files in 3 min 10 s on 2 cores. Overlay over a coloured frame checked by eye: text clean, box half transparent, Arabic right, alpha plane right.

Also: `make brief.json` and `plan script.txt` take the file (it was read as a topic); a `.json` that does not exist says so. `plan --draft` gives `draft_leaves_out`.

5 Nov 2026: when `commercial_ok` is false the envelope has a small `terms_note` in place of `warning` (`commercial-note.md`). 13 Nov 2026: `terms_note` is removed (owner decision); `metrics.commercial_ok` and `metadata.json` hold the data.

Audit fix, 9 Nov 2026: `engine_ffmpeg::same_file` compares canonical paths, so a symlink or a `..` counts. `export` skips the copy when the input already is the target `video.mp4` and refuses a `--format` that would land on the input. `edit`, `join` and `dub` refuse with exit 2 an `--out` that reaches an input. `export` refuses with exit 2 a file with no readable video stream.
