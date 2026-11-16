# Code card: a snippet typed in, coloured

Gap: Manim's `Code` class (cherry-manim.md item 17, #2355 broken, PR #2465 unmerged) and Remotion users' code-hike
demos. Explainer and developer videos show code; scriptvox had only text cards.

- Brief card `{"kind": "code", "text": "def area(r):\n    return 3.14 * r ** 2", "label": "area.py"}`.
- `code_card.rs` (ours): a one-pass tokenizer per line: comments (`//`, `#` after a space and not `#[`, `#!`,
  `#{`, `-- ` in SQL), strings in `"`, `'` or backtick with escapes, numbers, a union keyword list (Python,
  JavaScript, Rust, shell; SQL words only in upper case so `d.update(x)` stays a call), calls before `(`.
  Colours of our own, comments in italics.
- ASS safety: `{` and `}` written `\{` and `\}` (libass prints them, checked on a test render), a backslash before
  `n`, `N` or `h` gets a word joiner after it so `"\n"` prints as written, and every space is a hard space `\h`
  (libass drops leading spaces; the first render lost the indentation).
- Layout: Noto Sans Mono (OFL, embedded), advance 600/1618 of the font size; the caption size, smaller when the
  longest line (at least 16 columns) would pass 84 percent of the width. Dark panel, a lighter strip with the
  label. Each line opens through a clip, left to right, 0.025 s a character (0.12 to 0.6 s); lines start 0.3 s in,
  all in by 60 percent of the card time, at most 0.6 s apart. A tick per line with `sfx`.
- Limits checked at `plan`: 1 to 12 lines, 44 characters a line.
- Measured 4 Nov 2026: three scenes (Python with an f-string and `\n`, JavaScript with braces and a template
  string, SQL with a `--` comment), 6 gates pass. By eye: colours as intended, braces and `\n` literal,
  indentation kept after the fix, a half-typed line at 1.2 s.
- Not done: line numbers, scrolling. Diff: done on 13 Nov 2026, see the last section.
- Tests: `code_card::tests` (tokens for Python, Rust, SQL; escapes; hard spaces; limits).

Arabic check (4 Nov 2026): an Arabic file name in the mono font came out broken (Noto Sans Mono has no Arabic).
An Arabic label now uses the video's card font, on the right of the strip. The same check found a regression of
the formula commit: with Noto Sans Math and Mono in the fonts folder, fontconfig chose one of them for Latin
letters inside Arabic text (captions too), drawn at about a third of the size. `filterpath::fontconfig` now
writes an alias that puts Noto Sans right after Noto Sans Arabic. Proven with the same ASS line rendered with and
without the alias, then on the Arabic formula and code scenes: `r` and `py` at full size.

## The spoken line lights up (Manim gap 3, narration sync)

- `cards::code_focus`: a line's keys are its words of 3 letters or more that no other line has. When the voice
  says a key, or a word that starts with a key of 4 letters or more ("returns" for `return`), after that line
  typed in, a soft bar lights the line until the voice names another line or the card ends. No brief field: the
  narration drives it, the same way list items appear on their spoken word. One-line snippets get no bar.
- Tests: `cards::tests::the_spoken_line_lights_up`.
- Measured 4 Nov 2026 (run r_5eef2a, 6-line Python, open voice, 7.2 s): the voice says "area" at 1.55 s, before
  that line typed in at 1.8 s; the first build skipped such early words and lit only the last two lines. Now the
  light waits for the line: bars at 1.80 (`def area`), 4.28 (`raise ... "negative"`), 4.97 (`return`). 6 gates
  pass, words 28 of 28. Checked by eye at 3.0, 4.6 and 5.6 s.

## Diff and terminal cards (G187, 13 Nov 2026)

Rival: claude-code-video-toolkit 0.28 shows a terminal and a code change. A coding agent mostly reports commands and changes, so these two are the cards it needs most.

- `{"kind": "diff", "text": " def area(r):\n-    return 3.14 * r * r\n+    return math.pi * r ** 2", "label": "area.py"}`: the code card, and a line that starts with `+` has a green bar under it, a line that starts with `-` a red bar. The bar comes with its line. The marker stays in the text. A kind of its own, not a guess from the text: a YAML list also starts its lines with `-`.
- `{"kind": "terminal", "text": "$ cargo test\ntest result: ok", "label": "bash"}`: a line that starts with `$ ` is a command: green prompt, white text, typed in like code. Any other line is output: grey, no code colours, shown whole at its time.
- Both use the code card's layout, limits (12 lines, 44 characters), line timing, tick sound and the light on the spoken line. The text lint reads only the label.
- Measured 13 Nov 2026 (run r_f812a1, 16:9, no voice, 10 s): both cards drawn as said, checked by eye on `snapshot --cards`; gates pass.
- Seen in that run, not fixed: a panel of 6 lines with a label reaches down to the captions at 16:9, and the text gate gave no warning for it. The same holds for a `code` card of that height.
- Not done: scrolling for more than 12 lines, line numbers, a cursor that blinks, output that comes line by line on the spoken word.
- Test: `cards::tests::a_diff_marks_changed_lines_and_a_terminal_types_only_commands`.
