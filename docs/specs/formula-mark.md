# Formula mark: a piece of the formula lights up on a spoken word (10 Nov 2026)

Before: a formula card wrote itself and stayed white. An explainer says "and this term is the energy": nothing on
screen showed which term. GAPS row G105 (formula parts on a word). The idea is ours from a one-line note that a part
of a formula can be given a colour; no other tool's code or docs were read.

- **Brief.** A `formula` card takes `mark`, 1 to 4 entries `{"part", "on"}`. `part` is a piece of the card's `text`
  as the formula writes it (`mc^2` in `E = mc^2`, `\frac{d}{t}`, `d`). `on` is a word of the scene's narration, with
  the matching of the card field `on` (`card-on-word.md`): the first spoken word that starts with it, folded by
  `gates::norm_word`, Arabic included.
- **What shows.** From the word on, the piece has the accent colour (the yellow of the spoken caption word). It
  turns in 0.15 s and stays so to the end of the card. A fraction's bar and a root's sign belong to their piece. A
  piece that is in two marks takes the earlier word. No box and no underline: only the colour of the piece's own
  glyphs and lines.
- **Time limits**, as for the change of a morph: not before the formula has written itself (1.3 s after the card
  starts) and at the latest 0.9 s before the card ends, so the colour is seen.
- **How the piece is found** (`formula::find`, ours). `part` goes through the same parser as the formula. Its items
  must stand side by side in one row of the parsed formula (the whole line, a `{...}` group, the top or the bottom
  of a fraction, the body of a root, an exponent or an index). Spaces and one outer pair of braces do not count.
  The layout puts the parts of every piece next to each other in source order, so the piece is an index range of
  the placed parts; the layout code is not changed.
- **A symbol keeps its `^` and `_`.** In `mc^2` the pieces are `m`, `c`, `2`, `c^2` and `mc^2`. `mc` is not a piece,
  because `c` carries the exponent; the refusal says so.
- **Refusals** by `plan` and `make`, exit 2, each with the scene number, and with the part where one part is at
  fault (the two about the count and the card kind name none):
  - a `part` that is not in the formula;
  - a `part` that is there more than once (the message gives the count and asks for a longer piece);
  - a `part` that is empty or holds only a space command;
  - an `on` word that the scene's script does not have, or an `on` with no letter or digit;
  - more than 4 entries, or an empty list;
  - `mark` on a card that is not a formula;
  - `mark` together with `then`: refused for now, because the pieces move when the formula changes.
  If the voice's word list lacks the word at render, `make` stops with the word and the part (`cards::window`). It
  also stops when a `hold` ends the card before the mark's word, or before the 1.3 s of the write-on: the message
  gives the part, the word's time and the card's times. No silent fallback.
- **Old briefs.** With no `mark` the events are byte for byte the same and the field is not written, so plan ids do
  not change. No card golden and no existing snapshot of a card changed; the schema and the skill text gained the
  field.
- **`snapshot --cards`.** `landed` in `cards.json` comes from the events' start times, so it is at least 0.5 s after
  the last mark: the frame shows all marks lit.

Tests (they failed first against a stub that found nothing): `formula::tests::a_piece_of_the_source_is_found_as_a_run_of_parts`,
`cards::tests::a_formula_part_takes_the_accent_from_its_word`, `cards::tests::a_mark_that_cannot_be_placed_is_refused`.
The `hold` refusal was added after a review, with its lines in the second test.

Run on 10 Nov 2026 at home, free Edge voice, 16:9, all 8 gates pass on both:

- English, run r_949569, 2 scenes, 14.1 s. Scene 1 `E = mc^2`, mark `mc^2` on "mass" (said at 5.33 s). By eye: at
  5.1 s the whole formula is white; at 5.8 s `mc²` is yellow and `E =` is white. Scene 2 `v = \frac{d}{t}`, marks
  `d` on "distance" (9.84 s) and `t` on "time" (12.14 s). By eye: 9.6 s all white; 10.3 s and 11.9 s `d` yellow,
  `t` white; 12.6 s both yellow, `v =` and the bar white. `snapshot --cards` (5.95 s and 12.64 s): both cards with
  all marks lit.
- Arabic, run r_940999, 1 scene, 10.5 s. `v = \frac{d}{t}`, label "السرعة", mark `d` on "المسافة" (4.72 s). By eye:
  4.5 s all white; 5.1 s `d` yellow; `--cards` at 7.93 s shows it. Words gate 13 of 14.
- `plan` on a brief with `a` in `a + a = 2a`: refused, "scene 2 ... the part "a" is in the formula 3 times"; with a
  word the script lacks: refused with the word and the part.

Not done: marks on a formula with `then`; a mark that ends (the accent goes back to white) or one piece lit at a
time; a box or an underline; a colour other than the accent; `mc` as a piece of `mc^2`; a card golden for a marked
formula (the golden test gives no spoken words). Not run: 9:16 by eye; a root's sign by eye (unit test only); a
paid voice; a mark with a hook on the first scene; a mark with `hold` in a real video (unit test only). Seen, not changed: an Arabic 16:9 formula is small next to its
label (the size rule of the formula card, as before).
