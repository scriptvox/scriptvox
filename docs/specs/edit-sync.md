# `edit` keeps sound and picture the same length

Fault (4 Nov 2026 hand eval, scriptvox-edit vertical-talk): `edit --cut-silence --reframe 9:16` on a 24.5 s talk
with 4 cuts returned ok, but `check` failed `av_drift` (video 14.033 s, audio 14.080 s), every time.

- Cause: each kept part was selected with `between(t,s,e)`. The picture keeps whole frames and the sound keeps
  exact samples, so each cut could differ by up to a frame.
- Fix (`edit::select_expr`, used by `--cut-silence` and `--keep`): each part is snapped to the frame grid and kept
  as `gte(t,a-h)*lt(t,b-h)` with h half a frame, the same window for picture and sound.
- `edit` now runs the av_drift gate on its output, exits 3 with the file kept when it fails (G002), and returns it in `findings` (only that gate: a talk with a still
  camera is not a slideshow).
- Measured on the same input: 47 ms (fail) became 13.3 ms (pass); the rest is the AAC frame.
- Also from the eval: `make` says when the words gate could not listen (a `words` warn, "not checked"), instead
  of leaving the gate out; the edit skill warns that a 9:16 crop cuts burned-in text at both edges.
- Tests: `edit::tests::kept_parts_sit_on_the_frame_grid`.
