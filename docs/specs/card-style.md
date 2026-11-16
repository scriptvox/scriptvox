# Cards for a light page: `card_style` (14 Nov 2026)

## The fault

A video in the style of a printed manual has a light page (`background` `#f6f4ee,#ebe7db`), dark ink and one accent (`#b3421f`). The cards did not follow it. Every card drew with the colours of a dark page:

- text in white on boxes of 60 percent black, which are grey boxes on a light page;
- the table with a navy header and dark rows, the diagram with navy boxes, the terminal with its own dark panel;
- the steps card grey and pale; the digit of the step that is said, dark grey on the accent disc, had 2.4 to 1 and the gate `contrast` gave a warning;
- the end card on a page that was made dark.

No brief field changed these colours.

## The field

One new optional brief field:

```json
"card_style": "paper"
```

| Value | Result |
|---|---|
| absent, or `"dark"` | The cards of every earlier version. The ASS file and the video are the same, byte for byte, and the plan id is the same. |
| `"paper"` | Cards, hook and end card for a light page. |

`paper` enters the plan id. `dark` and no field do not. `resume --patch '{"card_style": "paper"}'` sets it on a run that was made dark, and `make` does the same checks there.

There is no `auto` value. The default must stay the dark look, so a brief would have to write `auto` to get it, and then `paper` is as short and says what it gives. A card can also stand on a picture or a clip and not on the `background`, so the colour of the background does not always decide.

## What `paper` draws

| Part | Colour |
|---|---|
| Text, axes, rules, arrows, dots | Ink `#16181a` |
| The box behind text (title, stat, list, quote, labels, hook, end card) | Almost white, 60 percent solid. On the page it is nearly not seen. On a picture it keeps the ink readable. |
| Panels: table rows, code and terminal, compare, gauge dial | `#ece8df` |
| Lit panels: table header, diagram box, the second compare panel, the bar over code | `#dcd6ca` |
| Backdrops of a plot, a line chart and a formula, and tracks of bars | A wash of ink (10 percent) |
| Rules of a table, the edge of a diagram box, the rail of a timeline | Thin ink |
| Header text of a table, numbers of stat, compare, bars, number line, gauge and progress, bars, dots, the prompt of a terminal, marks | The brief's `accent` |
| A highlight (mark `highlight`, a marked row or column of a table, the code line that is said) | A wash of the accent (15 percent; 18 for the code line), so that ink and accent letters stay readable on it. The thinnest pair is accent header text in a marked table column: 3.3 to 1 by the sum, with `#b3421f`. |
| Text that is not the point now (steps not said, output of a terminal, comments in code, a file name) | Grey ink `#55585b` (6.5 to 1 on the page, 4.9 on a lit panel) |
| The digit on the accent disc of a step | Almost white on a dark accent, ink on a light one |
| Code | Ink, with dark purple, blue, green and brown for keyword, call, number and text |
| Series of a donut and a plot after the accent | Dark blue, magenta, green, brown, grey |
| A diff | A light green and a light red line |
| The end card | The page under it is blurred and is not made dark |

It covers every card kind (title, stat, lower_third, list, bars, line, formula, code, diff, terminal, plot, quote, compare, donut, timeline, table, number_line, diagram, steps, progress, gauge), the marks (underline, ring, highlight), each card of a `cards` list, a title `behind` the subject, the hook, the end card and the card of `sources_card`.

The layout, the sizes, the times and the motion are the same as in the dark style. So is the Arabic layout: the mirror is made before the colours are changed.

With no `accent` in the brief, a paper card takes `#b3421f`, because the default yellow cannot be read on a light page. The captions keep their own colours.

## What it does not change

- The captions (style `Cap`) and the second caption line. See "Captions on a light page".
- The corner label of `label`.
- Pictures, clips, overlays and insets.

## Errors

All are exit 2 (usage), from `plan` and from `make`, before any work.

| Case | Message | Fix |
|---|---|---|
| A value that is not `dark` or `paper` | `card_style is dark or paper, not X` | Use one of the two, or drop the field. |
| `paper` with no `background`, a named background, or dark colours | `card_style paper draws dark text for a light page, and background X is not light: ...` | Set `background` to light colours, for example `"#f6f4ee,#ebe7db"`, or drop `card_style`. Each colour needs 7 to 1 against the ink. |
| `paper` with an `accent` under 3 to 1 on the light box | `accent X is too light on a paper card (contrast N, needs 3); pick a darker shade, for example #B3421F` | A darker accent. |

The older rule stays: an `accent` under 3 to 1 on black is refused, because the captions put it on a black box. So with `paper` the accent must be between the two, for example `#b3421f` (3.7 on black, 5.5 on the light box).

## How it is made

`card_style.rs`. The cards are drawn as before, with the dark-page colours. Then each colour of each Card and Hook line of the ASS document is changed to its part on a light page, from one table (`part`), and the Card and Hook styles get ink letters and a light box. `make` calls it in three places: the cards and hook, the titles behind the subject, and the end card. The end card's filter graph loses its darkening step.

A brief with no `card_style` never reaches this code.

A colour that a card file draws and the table does not know stays as it is. The test `no_dark_page_colour_is_left_on_paper` fails then, with the colour and the line.

## Limits

- The gate `contrast` measures at most 8 text boxes of a video. The test below runs each card kind alone, so a kind with more than 8 text boxes (a table) is measured in part.
- A title `behind` the subject and the card of `sources_card` take the same code path and have no picture and no test of their own.
- A brand file can hold `card_style`.
- A paper card over a dark picture or clip: text with a box stays readable; ink shapes with no box (arrows, a rail, an axis) can be hard to see. The gate `contrast` reads letters only.
- The colours of `paper` are fixed. A brief cannot set the ink or a panel colour.
- A light page that is not near white (for example a mid grey) passes the 7 to 1 rule only when it is light enough; the panels were tuned on `#f6f4ee`.

## Captions on a light page (not done, a proposal)

Checked: `caption_box` and `caption_outline` do not give an ink caption. `caption_box` sets how solid the black box is (0 to 1). `caption_box: 0` gives white letters with a black outline. No field sets the colour of the caption letters or of the box.

Measured in `tests/card_style.rs`: on `#f6f4ee` the default box (65 percent black) is a grey box, and the said word in `#b3421f` has 3.0 to 1 on it, so the gate `contrast` gives a warning. With `caption_box: 0.9` it passes. The brief check of `accent` uses pure black, not the box as it is seen on the page.

Proposal, as its own item: one more value, for example `caption_style: "paper"`: ink letters on the light box, the said word in the accent, the second speaker in dark blue, and the accent check against the box that is used. It touches karaoke, reveal, pop, emphasis and the second line, so it is not a small fix.

## Tests

- `tests/cards_golden.rs`
  - `every_card_kind_matches_its_paper_golden`: 42 pictures in `fixtures/golden/cards-paper/`, English and Arabic, on `#f6f4ee` with accent `#b3421f`. New cases, in the dark set too: lower third, hook, diff, terminal, and marks on a title, a quote, a list and a table.
  - `no_dark_page_colour_is_left_on_paper`: every colour of every case is one that the paper style gives.
  - `every_paper_card_passes_contrast_and_text_bounds_on_a_light_page`: each case alone as a video, through the gates. It also proves that the measure sees the fault: the dark steps card with this accent gives a warning.
- `tests/card_style.rs`
  - `an_old_brief_keeps_its_plan_id_and_only_paper_changes_it`: the id that the build before this field gave, and the three refusals with a fix line.
  - `a_paper_video_has_ink_cards_the_same_captions_and_passes_the_gates`: through `make`, English and Arabic, with a hook, two cards in one scene, marks, a table and an end card: the caption lines are the same as in the dark video, the gates `contrast` and `text_bounds` pass, the end card's page is light.
- Unit tests in `card_style.rs`.
