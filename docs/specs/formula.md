# Formula card: math with no LaTeX install

Gap: Manim's top complaint is the LaTeX install (#505, 34 comments; #1904; #2446; cherry-manim.md item 1).
HyperFrames, Remotion and OpenMontage reach math only through a browser library or LaTeX. ScriptVox had no math.

- Brief card `{"kind": "formula", "text": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}", "label": "..."}`.
- `formula.rs` (ours): a parser for a small TeX subset (`^`, `_`, `{}`, `\frac`, `\sqrt`, `\text`, Greek names,
  operators, `\sin` and other functions, `\,` spaces) and a box layout (TeX-like spacing around relations and
  binary operators, a sign after a relation has no space, scripts at 60 percent, fractions on the math axis,
  a drawn root sign with its overbar). Constants from the font's MATH table.
- Each glyph run and each line becomes its own ASS event (`\an1` at the baseline, or a drawing), so libass draws
  and nothing else lays out. Letters turn math italic, Greek lower case too. Widths come from the shaper
  (`captions::Measurer`) on Noto Sans Math (OFL, 1 MB, embedded, written to the run's fonts folder).
- Font mapping: in Noto Sans Math the ASS font size is win ascent plus win descent (5655 units for a 1000-unit
  em), so a formula em of E pixels is font size E / 0.1768, and the `\an1` box ends 2.55 em under the baseline.
- Motion: a dark panel fades in; the formula writes itself through a clip sweeping left to right (0.25 to 1.25 s);
  the label fades in at 1.3 s. A whoosh at 0.3 s with `sfx`. Too wide: the em shrinks to 84 percent of the frame.
- Errors at `plan`: unknown command, unmatched brace, `^` with no argument, two `^` on one symbol, more than 4
  levels, more than 120 characters; the message lists what is supported.
- Measured 4 Nov 2026: four English scenes (E = mc^2, A = \pi r^2 with a label, v = \frac{d}{t}, the quadratic
  formula), 14.5 s, all 6 gates pass. By eye at the end of each sweep: baselines line up, superscripts sit high,
  fraction rules on the axis, the root sign and overbar cover b^2 - 4ac, the quadratic formula fits the width.
  Mid-sweep frames show the formula half written. The label first touched the panel; moved down.
- Not done: matrices, `\left`/`\right` sized brackets, multi-line alignment, morphing one formula into another
  (Manim's TransformMatchingTex).
- Tests: `formula::tests` (italics and spacing, fraction geometry, signs and roots, error messages, ASS baseline).
