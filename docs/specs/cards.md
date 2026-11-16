# Scene cards: motion text in the brief (daily, 3 Nov 2026)

Gap: HyperFrames and Remotion ship title, lower-third and counter blocks, and Manim animates numbers, but each needs
a browser or Python scene code. ScriptVox draws the same kinds of motion text as ASS events in the captions file, so
one libass pass renders them with no new engine. Ideas: HF #17 (a small verified block set), Manim #11 (counters).
The design and the code are ScriptVox's own.

- Brief: `scenes[i].card = {kind, text?, label?, items?, values?}`. Kinds: `title`, `stat`, `lower_third`, `list`, `bars`.
  `Brief::load` refuses a card with missing text, a list outside 1 to 5 items or over 40 characters an item, a title
  over 60 characters. The text lint covers card text.
- Timing: from the scene's first word to the next scene's first word (the last scene to the end), the same word
  index rule as scene cuts.
- Look, in pixels of the frame (`PlayRes`), all in a `Card` style with a 40 percent black box:
  - `title`: 1.5x caption size, centre at 42 percent height, pop-in, 2.5 s at most.
  - `stat`: 2.6x caption size at 36 percent height. A number in the text counts up from 0 in 12 eased steps over
    0.8 s, with the same decimals and thousands commas, in the same digits (Western, or Arabic-Indic with ٬ and ٫;
    checked by eye: ٥٢, ٧٩, ٩٠٪); the label fades in under it.
  - `lower_third`: name 1.1x and role 0.8x, bottom at 62 percent height, slides in from the side over 0.3 s,
    4 s at most. Arabic: right side.
  - `list`: up to 5 items from 15 percent height. Each item appears when the voice says one of its first two longer
    words (a word ending with it counts, for Arabic prefixes like و); an item never said falls back to an even
    spread. Measured 3 Nov: English items at 9.77, 10.93 and 12.17 s, the same times as the spoken words. Arabic: right side.
  - `bars` (added later on 3 Nov): `label` as the chart title, then one row per item: the name over a bar drawn
    as an ASS shape on a dark track (40 percent black, full length, so the bar reads on any background). Bars grow
    from 0 in 0.6 s, each 0.15 s after the one above; the value appears at the bar end when it has grown. 2 to 6
    rows; when rows do not fit above the captions (16:9), the whole row shrinks, text and bar together. Arabic:
    bars grow from the right, value on the left. Checked by eye in English 9:16 and 16:9 and Arabic 9:16; the first
    16:9 render had names touching the bar above and yellow bars weak on the yellow background (both fixed).
- Wrapping: measured with the caption font; a wrapped label or title is balanced (the narrowest width with the same
  line count), so no line is one lone word.
- Measured 3 Nov (Chatterbox voice, 9:16 and 16:9, English and Arabic): every card readable and clear of the
  captions on contact sheets; a first version put the Arabic lower third against the taller Arabic caption box
  (moved from 66 to 62 percent). `scriptvox judge` read every card as legible (text_ok on 6 of 6 frames).
- Tests: number split and format, the count-up events, list order and Arabic side, card problems, style insert,
  balanced wrap, and the brief refusing a bad card.

## Wide frames (5 Nov 2026)

Fresh-agent test of 4 Nov: cards looked small on 16:9. A wide frame now draws title, stat, lower third, list and formula
1.5 times bigger and code, bars, line and plot 1.2 times. Rows end at 72 percent of the height, above a two-line caption.
Bars put the name beside the bar, so 4 rows stay readable (they shrank to 54 percent before). A long list shrinks its
text with its row. Tall and 1:1 frames do not change (goldens byte-identical). Checked by eye: all 9 kinds in English,
bars, list and line in Arabic with pop captions. The yellow karaoke word on the yellow background reads well because of the caption box.

## Quote, compare, donut, timeline (5 Nov 2026)

Gap: HyperFrames' block set and Remotion's templates hold these. Own code, ASS shapes only.

- `quote`: `text` up to 160 characters, 1.15x size, `label` under it. Arabic guillemets point outward (libass draws them mirrored, so the logical pair is swapped; seen by eye).
- `compare`: 2 panels, numbers count up over 0.7 s and fill the panel, second lit in the accent. First panel on the reading side.
- `donut`: ring in 6 steps per part, clockwise from the top in every language; legend under it on tall frames, beside it on wide ones; `whole_percents` adds up to 100.
- `timeline`: dots and a dark line (a dark edge keeps a yellow dot readable on yellow); each item on its spoken word.
- Checked by eye: all four, en and ar, 9:16 and 16:9 (r_914792, r_a75cd2, r_5d6cf3, r_c6a547). Six new goldens.

Donut on 1:1 and 4:5 (5 Nov 2026, seen by eye): the legend under the ring was squeezed to a few pixels. A frame under 1.5 times as tall as wide now puts it beside the ring. Long names get a smaller ring and more width. Checked on all four shapes, English and Arabic.


## A field that a kind does not draw (13 Nov 2026)

Fault: a `label` on a `title`, `list`, `donut` or `timeline` card was dropped without a word (the fix of the same day covered `text` on `bars` and `line` only). Cause: each old kind had its own check, and no check knew the full list of fields.

Rule: `Card::problem` refuses every field that the kind does not draw, for every kind, from one table (`card_kit::allowed`, read from what each renderer uses). `plan` and `make` stop with exit 2. The message names the field and the fields that the kind takes.

| Kind | Fields |
|---|---|
| `title` | `text`, `behind`, `mark` |
| `stat` | `text`, `label`, `from`, `mark` |
| `lower_third` | `text`, `label` |
| `list` | `items`, `ease`, `stagger`, `mark` |
| `bars` | `items`, `values`, `label`, `ease`, `stagger` |
| `line` | `items`, `values`, `label` |
| `formula` | `text`, `label`, `then`, `mark` |
| `code`, `diff`, `terminal` | `text`, `label` |
| `plot` | `text`, `values`, `label`, `y`, `grid`, `trace` |
| `quote` | `text`, `label`, `mark` |
| `compare` | `items`, `values`, `label` |
| `donut` | `items`, `values`, `text` |
| `timeline` | `items` |

The six data kinds are in `cards-data.md`; they use the same table. `on`, `at` and `hold` time every card. A field that holds nothing (`false`, a blank text) is not refused. The messages that say more stay: `text` on `bars` or `line` says to rename it to `label`; `from`, `behind`, `then`, `y`, `grid`, `trace`, `ease`, `stagger` and `mark` keep their own lines.

Checked: the 246 card examples in `recipes/`, `docs/`, the skills, `src/` and `tests/` were read against the table; only the examples that a test wants refused have a field outside it. Test `a_field_that_a_kind_does_not_draw_is_refused_for_every_kind`.
