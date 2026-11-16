# Caption lines keep "pi r squared" together

Fault (hand eval of 4 Nov 2026, `eval-fixes-0340.md`): a caption line ended on "pi" and the next began "r squared".

- A word leans back on the word before it when it is a lone Latin letter other than "a" and "i" (a variable such as
  "r" or "x", punctuation after it ignored), or "squared" or "cubed".
- When a line is full and the next word leans back, the words it leans on go down with it (at most two), so
  "area equals pi r squared" breaks as "area equals" / "pi r squared".
- At a page end, the company a sentence's last word takes from the page (the orphan rule) is the whole leaning
  group, and only when the line above keeps a word; otherwise the last word goes alone.
- Test: `captions::tests::a_line_does_not_break_before_a_variable` (one and two lines a page).
