# `snapshot --cards`: one frame per card, after it has landed

Gap (4 Nov 2026 hand eval): the 9-frame sheet missed the formula landing, and the agent guessed `--at` times
(7 s showed the formula half written). HyperFrames and Remotion agents also work from a timeline they can read.

- `make` writes `cards.json` in the run folder: for each scene card its scene number, kind, start, end and a
  `landed` time = 75 percent through the card, at least 1.5 s in, at most 0.25 s before it ends
  (`cards::landed`). Lists and code finish by 70 percent; a formula writes for 1.25 s.
- `scriptvox snapshot <run or video> --cards`: frames at those times; each frame in the result names its `scene`
  and `card`. A video file finds its run through the runs' results. No cards: exit 2 with `--at` or `--sheet` as
  the fix. Conflicts with `--at` and `--sheet`.
- Skills: `scriptvox-motion` section 5, `scriptvox-review`, and step 6 of `scriptvox`.
- Measured 4 Nov 2026, run r_1e4af3 (stat, formula, list; hook on): landed at 2.47, 5.30, 8.38 s; by eye the stat,
  the whole formula and all three list items show. The stat card starts at 1.6 s, after the hook.
- Tests: `cards::tests::landed_is_inside_the_card`.
- A formula with `then` is caught 0.25 s before its end, after the change (`cards::landed_card`).
