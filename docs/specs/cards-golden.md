# Card goldens: a pixel check for every scene card kind

Gap: Manim #9 (visual regression gate) and HyperFrames #19 (PSNR golden harness). Card faults so far were found by
eye in real runs (4 Nov 2026: Latin letters inside Arabic text drawn at a third of the size by a font fallback).

- Test `tests/cards_golden.rs`, part of `just ci` (it needs `SCRIPTVOX_FFMPEG`, which the justfile sets): ten cards,
  title, stat, list (en, ar), line (ar), formula (en, ar with a Latin letter in the label), code, plot (en, ar). Each
  is drawn by the real `cards::events` and `cards::insert` into an ASS file, rendered by libass with our fonts and
  fontconfig (`template::write_fonts`) over a flat 540x960 frame, at 3.5 s (a title at 1.5 s), and compared with
  `fixtures/golden/cards/<name>.png` by PSNR. Below 45 dB fails and keeps the new frame in the temp folder.
- Bless after a deliberate change: `SCRIPTVOX_BLESS=1 cargo test --test cards_golden`, look at every PNG, commit, add a
  line to `fixtures/golden/BLESS.md`.
- Runtime 0.7 s for all ten. Unchanged cards give inf (renders are exact); the first layout change measured 21 to
  24 dB, so 45 leaves room.

Found on the first bless (4 Nov 2026): the Arabic plot panel ran off the right edge at 540 px (the y label margin
grows with the Arabic font size, the plot kept 64 percent of the width). The plot now keeps its panel inside 4 to
96 percent of the width. The title golden first showed nothing (a title leaves at 2.5 s), so titles are taken at
1.5 s. All ten PNGs checked by eye.
