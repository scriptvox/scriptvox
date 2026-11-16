# Clips at 24 or 25 fps move evenly at 30 fps

Gap: generated clips (Wan, Kling) are 24 fps and many stock clips 25 fps. ScriptVox renders 30 fps. The `fps` filter
repeats every 4th (or 5th) frame, a visible judder about 6 times a second. RIFE (L3-D) would interpolate on the GPU;
first the cheap fix.

- `media::graph` uses `framerate=fps=30` (blends the two nearest frames) instead of `fps=30` for video clips, then
  `fps=30` again only to give `xfade` the same time base as the stills (without it a 15 fps clip failed the
  encoder: test `every_template_passes_the_gates`).

Measured 3 Nov 2026 on two Wan clips (own-media brief, 2 scenes), frame-to-frame change in scene 2:
- variation (std/mean) 0.49 with `fps`, 0.19 with `framerate`; whole make 6.4 s before, 7.0 s after.
- On a 5 s clip alone: repeat 0.59, blend 0.26, `minterpolate` (motion compensated) 0.25 but 6 times slower
  (22.8 s against 3.8 s). The blend shows no ghosting on the dhow at normal speed (checked by eye).

RIFE stays open for fast motion, where a blend can show a double edge.
