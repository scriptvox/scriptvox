# Brief `look`: one colour grade for the whole picture

Gap: ComfyUI ships grading blueprints (colour curves, film looks), Remotion and HyperFrames users grade with CSS
filters. ScriptVox had none.

- `look`: `natural` (default, no filter), `warm`, `cool`, `film`, `punchy`, `mono`. `media::look` gives the FFmpeg
  filter. It goes on the background track before behind titles, cards, the logo and captions, so text keeps its own
  colours (yellow karaoke stays yellow in `mono`).
- No film grain: per-frame noise would hide a frozen picture from the slideshow gate.
- Fault fixed with it: an unknown `transition` panicked after TTS (exit 101, no envelope). `make` now checks
  `transition` and `look` names first: exit 2 with the list of names.

Measured 3 Nov 2026: one still and one Wan clip in each of the 6 looks, all gates pass, the difference is clear on a
side-by-side sheet; a gradient-only video in `mono` works (empty graph case). Test `media::tests::looks_are_known_or_refused`.

`.cube` (5 Nov 2026): `look` also takes a 3D LUT file, relative to the brief or the brand file (HyperFrames #27, ComfyUI colour blueprints). FFmpeg `lut3d`, same place in the graph, so text keeps its colours. A file with no `LUT_3D_SIZE` line stops at `plan`, before any voice.
