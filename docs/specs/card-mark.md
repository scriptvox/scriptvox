# A mark on a phrase of a card: underline, ring, highlight (P1.4, G197, 13 Nov 2026)

Before: only a formula card could light a piece on a spoken word (`formula-mark.md`). A title, a stat, a quote or a list could not point at its own words. A presenter underlines or circles the words as the voice says them. The idea is ours from a one-line note that a tool can underline or circle words; no other tool's code or docs were read.

- **Brief.** The card field `mark` now also works on the kinds `title`, `stat`, `quote` and `list`: 1 to 4 entries `{"part", "on", "style"}`.
  - `part`: one or more whole words of the card's own text (`text`, `label` or one of `items`), in a row, as the card writes them. Case and punctuation do not count (`gates::norm_word`, Arabic letter forms included). It must be there exactly once.
  - `on`: a word of the scene's narration, with the matching of the card field `on` (`card-on-word.md`).
  - `style`: `underline` (default), `ring` or `highlight`.
- **What shows**, in the accent colour, from the word to the end of that text:
  - `underline`: a line under the words. It grows in 0.25 s from the reading side: from the left in English, from the right in Arabic.
  - `highlight`: a band behind the words, drawn under the card's own dark box, so the letters stay white on a dim accent band. It grows the same way.
  - `ring`: an oval line around the words. It fades in in 0.15 s.
- **Time.** The mark starts at the word, but not before the text has come in (0.3 s after that text's own start) and it goes with the text. If the word is said less than 0.5 s before that text goes (a title goes after 2.5 s), `make` stops and gives the word's time and the text's times. No silent fallback.
- **Where.** The place is measured, not guessed: `card_mark::events` reads the card's own text event (`\an`, `\pos`, `\fs`, the wrapped lines) and measures the words before the part and the part itself with `captions::Measurer`, the shaper that wrapped the text. An Arabic line is measured from its right edge.
- **A part over two lines.** When the frame wraps the part over a line break, `underline` and `highlight` draw one piece per line. A `ring` cannot: `make` stops and asks for fewer words or `underline`.
- **Refusals** by `plan` and `make`, exit 2, each with the scene number:
  - `mark` on a kind that is not `formula`, `title`, `stat`, `quote` or `list`; on a title with `behind`;
  - a `part` that is not in the card's text as whole words, or is there more than once;
  - a `part` with no letter or digit; an `on` word the scene's script does not have;
  - more than 4 entries, or an empty list;
  - `style` on a formula mark (a formula piece takes the accent colour only).
- **Old briefs.** `style` is not written when absent, and a formula mark is unchanged byte for byte. A card with no `mark` gives the same events as before.
- **`snapshot --cards`.** `landed` in `cards.json` is taken after the last mark has come in, as for a formula.

Limits:
- A line that mixes Arabic and Latin letters is refused at render: the place of a part in a line of two directions is not measured.
- The stat's number counts up in steps: a mark on the number starts when the count has landed (0.8 s).
- `lower_third`, `compare`, `timeline`, chart labels and code cards take no mark.
- One accent colour. A mark does not end before its text ends.
- The wrap is known only at render, so the `ring` over two lines is refused by `make`, not by `plan`.

Tests (no network): `card_mark::tests` (the place measured again with the shaper in English and in Arabic, the times, the ring, every refusal, the ring over a line break, a line in two scripts) and `tests/card_mark.rs` (a real `make` with `tts: none` in English and Arabic: the three drawings in `captions.ass`, their times inside the card's, `landed` after the mark; `plan` exit 2 with the scene number). `cards::tests::a_mark_that_cannot_be_placed_is_refused` changed on purpose: a stat now takes a mark, a chart does not, and a formula mark refuses `style`.

Run on 13 Nov 2026, `tts: none` (word times spread by characters), 16:9, 1920x1080, gates `text_bounds` and `contrast` pass on all three:

- English, 4 scenes, 17.8 s, background default and `night`. Title "The plan for next quarter", underline on "next quarter". Stat "42%" with a ring, and its label with a highlight on "year one". Quote with a highlight on "Simple" and an underline on "complex". List with a ring on "Lower prices" and an underline on "customers". By eye on full size frames at the `landed` times (2.46, 7.64, 11.91, 16.71 s) and on `snapshot --cards`: each mark stands on its own words; the ring closes around the number; the highlighted letters stay light on the band. `contrast`: 8 text boxes, lowest 10.2 to 1 (5.7 on an earlier build that read each box at another moment).
- Arabic, 3 scenes, 14.3 s. Title "خطة العمل للربع القادم" with an underline on the last two words, which stand at the left; stat "40%" with a ring and a highlight on "عدد الزوار"; list with a ring on "أسعار أقل" and an underline on "سعداء". By eye: every mark on its words, letters joined, right to left.
- Growth, one frame 0.13 s after the mark starts: the Arabic underline has grown from the right, the English one from the left.

Two faults found by the frames and fixed before this text: a band under the card's box could not be seen on a yellow picture (it is now over the text, two thirds clear), and an Arabic box drawn with `\an9` stood one width too far left (a drawing is placed from its own origin, so it is drawn to the left from `\an7`, as the bars of a chart).

Not run: a real voice (the word times here are estimates); 9:16 by eye; a mark with a card `on` and `hold` in a real video (unit rule only); a brand font.
