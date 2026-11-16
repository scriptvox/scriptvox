# Warn when a base video has its own text

Found by the hand evals of 4 Nov 2026: a promo with burned-in English captions was dubbed into Arabic; the old
captions stayed under the new ones and nothing said so.

- `judge::burned_text(ff, video, span, dir)`: 3 frames (20, 50, 80 percent of the base clip, at most the video
  length), 640 px, one request to the free vision model (Qwen3.8-27B on the owner's GPU service, the `judge` default), JSON reply
  `added_text` and `what`. Signs and screens filmed in the scene do not count.
- `make` with `base_video`, after the gates and the words check: finding `base_text`, `warn` with what was seen,
  or `pass`. Advice only: never fails a run; no finding when offline (`--local-only`, `--cache-only`) or when the
  model cannot be reached. The frames stay in the run folder (`base_text1.jpg` to `3`).
- Measured 4 Nov 2026: the eval promo (English captions in the picture) gave `warn` "subtitles at the bottom"; a
  clean FastWan clip gave `pass`. The step took 0.9 s (milestone to end of `make`).
- A first try sampled past the end of a 5 s base clip under a 15 s voice; the grab failed and no finding came. Now
  the frames come from the clip's own length.
