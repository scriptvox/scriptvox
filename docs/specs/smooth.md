# Smooth motion: RIFE on scriptvox-gen (L3-D, second part)

Gap: Wan clips are 24 fps and 5 s long. A scene longer than its clip either plays the clip slower (at most 1.43
times, `short-clip.md`) or holds the last frame, which reads as a frozen picture. ComfyUI users put a RIFE or FILM
node after the video model for this; OpenMontage, MoneyPrinterTurbo and HyperFrames have no interpolation step.

- Model: RIFE 4.26 (hzwer, MIT). Network file and weights from the author's `hzwer/RIFE` upload, `warplayer.py`
  from Practical-RIFE at a pinned commit; both downloaded unchanged into the scriptvox-gen image at build time, pinned
  by sha256 (`docs/provenance/gen-models.md`). 23 MB, so no weights relay.
- Engine `rife426/interpolate` in `services/gen/engines_video.py`: input `video_url` (data:video/mp4), `multi` (2 or
  4, default 2). Frames padded to a multiple of 64, 8 pairs a batch in fp16. Output: the source frames with the new
  ones between, at `fps * multi`, the same length.
- Row `rife426-self` (kind `interpolate`, free, commercial use allowed, `multi = 2`).
  `scriptvox generate --model rife426-self --video clip.mp4`.
- Brief `"smooth": true`: each generated scene clip goes through the first interpolate row with a key, after the
  upscale when both are on (the upscale then works on half the frames), as shot `scene<n>_smooth`, so a rerun resumes.
- `media::stretch` reads the clip's frame rate (`Ffmpeg::frame_rate`): a clip of 48 fps or more may slow down to 2
  times (`SLOW_MIN_SMOOTH = 0.5`), still 24 real frames a second; other clips keep 1.43 times.
- Tests: the row request (unit), `stretch` with 24 and 48 fps (unit); a real run, checked by eye.

Measured 4 Nov 2026 on the GPU cluster (image `rawivox-gen:20261104b`, sha256 `7f95328c...`):
- A 5 s FastWan clip (falcon in flight, 704x1280, 121 frames at 24 fps) to 242 frames at 48 fps, 5.04 s both, in
  10 s end to end, $0. By eye: the new frames between real ones are clean on the beating wings, no double edges.
- `make` on one 8 s English scene with one 5 s FastWan clip: without `smooth` the slideshow gate warns "32% of the
  video does not move" (the clip at 63 percent of the scene is below 70 percent, so its last frame holds 3 s); with
  `smooth` "0% frozen", the clip plays 1.59 times slower and the bird still moves at 5.5 s and 7.5 s (snapshot
  checked by eye). All other gates pass on both.
- First image `20261104a` failed: a `.half()` net met the float32 warp grid. Fixed with float32 weights and fp16
  autocast.
