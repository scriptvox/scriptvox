# Formula morph: one equation turns into the next on the spoken word

Gap: manim's signature feature (TransformMatchingTex, cherry-manim #2), and narration sync (#3), which manim has not
(no TTS; manim-voiceover lives outside the repo). No other source project has either.

- Brief: a `formula` card takes `then` (a second formula, same TeX subset) and `on` (a word of the scene's
  narration). `plan` refuses `then` on another card kind, a formula's `on` without `then`, and a `then` that does not
  parse. (Since 10 Nov 2026 other kinds take `on` with another meaning: see `card-on-word.md`.)
- Both formulas are set at one size (the wider one fits 84 percent of the frame); the panel covers both.
- Morph time: the first spoken word that starts with `on` (folded like the words gate: Arabic letter forms, "four"
  equals "4"), else halfway (at least 1.6 s in); never later than 0.9 s before the scene ends, so the result is
  seen; never before 1.3 s (the write-on). `snapshot --cards` takes the frame 0.25 s before the end.
- Matching (`formula::glyphs`, `formula::pair`, ours): each text piece is split into glyphs at the shaped width of
  each prefix; the longest common subsequence of the two glyph strings pairs the shared symbols, in order.
  Paired glyphs glide (`\move`, 0.6 s) and change size if a script level changes; the first formula's other glyphs
  fade out in 0.3 s; the second's fade in from 0.3 s. Fraction bars and root signs fade out and in.
- Measured 4 Nov 2026, run r_5f6192: `A = \pi r^2` then `A = \pi (2r)^2 = 4\pi r^2` on "four" (5.57 s), scene
  7.1 s. 6 gates pass. By eye at 3.0 s (first formula), 5.85 s (glyphs on the move) and 6.9 s (second formula
  clean): the old r² becomes the (2r)², "= 4πr²" fades in.
- Tests: `formula::tests::two_formulas_share_their_glyphs_in_order`, `cards::tests::then_and_on_belong_to_a_formula`.
- Not done: more than two steps; matching by meaning across a reorder (LCS keeps order).

Arabic check, 4 Nov 2026 (run r_f01405 then again): the word "أربعة" came 0.87 s before the cut, and the first rule
(0.9 s left or no change) silently kept the first formula. Now the change starts at 6.40 s (0.03 s early) and the
second formula shows; the Arabic label with "≥" and a Latin "r" reads right (≥ mirrors in a right-to-left line).
