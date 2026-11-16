# Data cards: table, number line, diagram, steps, progress and gauge (P1.6, G105, 13 Nov 2026)

Before: a scene card could show a number, a list, a chart or a formula. It could not show a table, a value on a scale, boxes with arrows, or the step of a process. The ideas are ours from one-line notes that explainer tools have such blocks; no other tool's code or docs were read.

Each kind is a `kind` of the scene field `card`, in its own file (`card_table.rs` and so on). `card_kit.rs` holds what they share. The rules for all of them:

- **Fields.** A kind takes only the fields it draws. `plan` and `make` refuse any other field with the list of the fields it takes (exit 2, with the scene number). A field of these cards on another kind is refused too ("only a table card takes `rows`").
- **Time.** `on` and `hold` work as on every card (`card-on-word.md`). These cards add no timing rule.
- **Room.** A card stays between the top limit and the captions, as the other cards do. The size of the text is the caption size or smaller. A width is measured with the font, not counted in letters.
- **Arabic.** Text is right to left with joined letters. What mirrors is said for each kind.
- **Latin words in an Arabic video.** A text with no Arabic letter (`Python`, `Q1`, `50x`) is set in the Latin font at the Latin size. Before, its letters came from the fallback font and its digits from the Arabic font, at two sizes (seen by eye in a run with such cells, nodes, steps and names). A text that mixes the two scripts (`خطة Plan`) is not changed: its Latin letters are larger than its Arabic ones, as in the captions and the older cards. It stays inside its cell or box in the run; no test measures it, because the test ruler has one font.
- **A label** of a data card has at most 40 characters.
- **Old briefs.** The new fields are not written when absent, so a card of an old kind has the same JSON and the same plan id (`card_table::tests::an_old_card_is_written_as_before`).
- **End to end.** `tests/cards_data.rs` makes one video with all six kinds in English (9:16) and one in Arabic (16:9) with `tts: none`: the gates `text_bounds` and `contrast` pass, the table's band, the traveller's 12 steps, the diagram's arrows and the lit step are in `captions.ass` inside their card's time, and `plan` refuses seven bad cards with exit 2, the scene number and a fix.
- **Gates.** `text_bounds` reads only text, not drawings. So each kind has a unit test that puts every drawing and every text box inside the room on 9:16, 16:9, 4:3 and 21:9, in English and Arabic, with the largest content the check allows.

## `table`

```json
{"kind": "table", "label": "Plans",
 "rows": [["Plan", "Price", "Seats"], ["Free", "$0", "1"], ["Team", "$12", "10"]],
 "mark": [{"part": "Team", "on": "team"}]}
```

- `rows`: 2 to 8 rows of 2 to 5 cells. The first row is the header. Every row has as many cells as the header. A cell is one line of at most 24 characters; an empty cell is `""`, but not in the header.
- The longest cells of the columns have at most 40 characters together. More has no readable size on a tall frame.
- `label`: an optional heading above the table, one line.
- `mark`: 1 to 4 entries `{"part", "on"}`. `part` is the whole text of exactly one cell (case and punctuation do not count). `on` is a word of the scene's script. From that word the cell is lit with a dim band in the accent colour, to the next mark's word or to the card's end. So one light moves through the table.
  - A cell of the first column lights its whole row.
  - A cell of the header lights its whole column.
  - Any other cell lights only itself. The corner cell (first row, first column) is refused.
  - No `style`.
- Drawing: a dark panel, a lighter header band, header text in the accent colour, thin lines between the rows. The first column starts at the reading side; the other columns are centred. Rows come in one after another, 0.12 s apart.
- Size: each column is as wide as its widest cell plus padding. The text shrinks when the columns are wider than 88 percent of the frame or the rows are taller than the room. A narrow table takes at least 60 percent of the width. The heading is at most 1.2 times the cell text.
- Arabic: the first column stands at the right and the columns run to the left. A row's height is counted with the font's real letter size, so an Arabic table is not half empty (the first build gave 8 Arabic rows at 16:9 a text of 36, now 64).
- Refusals: no `rows`, too many or too few rows or cells, rows of unequal length, an empty header cell, a cell that is too long, columns that are too wide together, a `part` that is not exactly one cell, the corner cell, a `style`, an `on` word the script does not have, and the voice's words without that word at `make` (the rule of every mark, `card-mark.md`).

Tests: `card_table::tests` (the check with each refusal; the room and the columns on 4 frames in 2 languages with the 8 by 5 table, no two cells of a row touch, the text is 45 percent of the caption size or more; the bands of a row, a column and a cell with their times, and the Arabic mirror). Goldens `table-en` and `table-ar`.

Run on 13 Nov 2026, `tts: none` (word times spread by characters), two scenes each (3 by 4 table with three marks, 8 by 5 table with one mark), 14.2 s English and 14.9 s Arabic, at 16:9 and at 9:16. All 8 gates pass on the four videos; `text_bounds` 60 text boxes inside the frame, `contrast` lowest 10.2 to 1. By eye on `snapshot --cards` and on frames at 4.5 s and 12 s: every cell in its column, the lit row, column and cell on the right cells, Arabic columns from the right with joined letters. Found by eye and fixed: the text was capped at 80 percent of the caption size and the 5-column table used only three quarters of a tall frame (now the caption size at most); the Arabic heading took a quarter of the room at 16:9.

Limits of the table mark, not refused: a mark whose word comes in the last half second of the card shows only in the card's fade (a text card refuses this case, `card-mark.md`); two marks whose words are less than 0.1 s apart show only the second.

Not done: a cell over two lines, a number column aligned at the right, a row that comes in on a spoken word, more than 8 rows by scrolling. Not run: a real voice; a brand font.

## `number_line`

```json
{"kind": "number_line", "label": "Customer score", "values": [0, 100],
 "points": [80, 35], "items": ["Now", "Before"], "from": 20}
```

- `values`: `[from, to]`, the two ends of the line, the first below the second, each at most 1e9 in size.
- `points`: 1 to 4 values to mark, each on the line.
- `items`: optional names of the points, one for each, at most 16 characters.
- `from`: optional. The first marker starts at this value, and 0.9 s after the card comes in it travels to `points[0]` in 1.2 s. Its number counts with it in 12 steps. `from` must be on the line and must not be the first point itself.
- `label`: an optional heading, at most 40 characters.
- Drawing: a white line with a dark edge, ticks at round steps (1, 2 or 5 times a power of ten, at most 6 steps), a number under each tick. Each point is an accent dot on the line, a stem, and a label over it: the name, and under it the value in the accent colour. The label is one text with one box.
- Labels that would touch stand on two or more levels. The traveller's label keeps the whole way from its start to its end free. Stems are drawn first, so a stem never lies over a label.
- A label near an end of the line is pushed inside the frame; its dot stays on its value.
- Numbers: as many decimal places as the points, the ticks and `from` need, at most 2. Arabic-Indic digits when a name uses them (the rule of the charts).
- When the numbers under the ticks would touch, every second one is shown, then only the two ends; then everything shrinks.
- Arabic: names are right to left with joined letters. The line itself runs from the left and the numbers read left to right, as on the `plot` card (maths reads left to right in Arabic too).
- Refusals: `values` that are not two numbers in order, no `points` or more than 4, a point off the line, `items` of another length or too long, a `from` off the line or equal to the first point, a label over 40 characters, and every field the kind does not draw (`text`, `mark` and so on).

Tests: `card_number_line::tests` (round ticks; the check with each refusal, and `from` still works on a stat and is refused on a title; dots at their values, two close labels on two levels and no two labels touch; the travel: 12 steps from 0.9 s to 2.1 s, the dot and the number move together, rest before and after; the room on 4 frames in 2 languages with 4 named points, a traveller and values up to a million). Goldens `number-line-en` and `number-line-ar`.

Run on 13 Nov 2026, `tts: none`, three scenes each (a traveller from 20 to 80; three named points with 50 and 52 close together; a range from -50 to 150 with three names), 16.6 s English and 16.0 s Arabic, at 16:9 and 9:16. All 8 gates pass on the four videos; `text_bounds` 45 text boxes inside the frame, `contrast` lowest 10.2 to 1. By eye on `snapshot --cards`, on a frame at 1.6 s (the dot at 55 on its way, the number reads 55) and on full size crops: dots on their values, "Mean 52" one level over "Median 50", "-25" and "-50" with the minus at the left in the Arabic video, Arabic names joined. Found by eye and fixed: a name and its value were two texts, and the box of the value hid the lower half of an Arabic name (now one text, with a box padding of its own size); a stem crossed the label under it (stems are drawn first, label boxes are nearly solid).

Not done: a marker that travels on a spoken word (the travel has a fixed time); a marked range between two values; a logarithmic line; more than one traveller. Not run: a real voice.

## `diagram`

```json
{"kind": "diagram", "label": "From brief to video",
 "nodes": ["Brief", "Media", "Render", "Video", "Report"],
 "edges": [["Brief", "Render"], ["Media", "Render"], ["Render", "Video"], ["Render", "Report"]]}
```

- `nodes`: 2 to 8 box labels, each 1 to 18 characters on one line, each there once.
- `edges`: 1 to 12 arrows. Each is `["from", "to"]` with two labels of `nodes`, written as `nodes` writes them.
- `label`: an optional heading, at most 40 characters.
- **Layout, with no field for it.** The arrows put the boxes in steps: a box stands one step after the latest box that points at it. A box with nothing before it stands right before the first box it points at. A wide frame runs the steps along the reading direction; a tall or square frame runs them from top to bottom. Boxes of one step stand beside each other, centred. All boxes have one size and one text size; a label takes one or two lines.
- **Order inside a step.** Every order of every step is tried (at most 6 orders a step, 4 steps). The first one with no two arrows crossed and the least slant is used. The first step keeps the order of `nodes` when it can.
- Arrows: white with a dark edge, from the side of a box that faces the next step to the facing side of its target. An arrow comes in with the box it points at; steps come in one after another, 0.45 s apart or less.
- Arabic: on a wide frame the first step is at the right and the arrows point left. On a tall frame the steps still run down, and the boxes of a step run from the right.
- **A graph with no clean drawing is refused**, each time with what to do:
  - arrows in a circle (no first box);
  - an arrow that would jump over a step (it would cross a box): from a box to one that is two or more steps on;
  - more than 4 steps in a row, or more than 3 boxes in one step;
  - arrows that cross in every order of the boxes;
  - a box with no arrow; an arrow to itself; the same arrow twice; an arrow that names a label `nodes` does not have; a label twice; a label that is empty or over 18 characters;
  - fewer than 2 or more than 8 nodes; no `edges` or more than 12; an edge that is not two labels.
- So "at most 8 nodes" does not mean that every graph of 8 nodes is drawn: a chain of 5 or more is refused, and the message names the `steps` card for a plain sequence.

Tests: `card_diagram::tests` (the steps of a chain, of 2 into 1 into 2, the turned order that takes a crossing away, a late source; each refusal with its fix line, also through `Card::problem`; the room on 4 frames in 2 languages for 3-3-2, 2-2-2-2 and 1-3-1 boxes with 18-character labels: every text inside its box, no box on another, text 40 percent of the caption size or more; sideways on a wide frame, down on a tall one, the arrow's tip on the side of its target, Arabic from the right). Goldens `diagram-en` and `diagram-ar`.

Run on 13 Nov 2026, `tts: none`, three scenes each (a chain of 3 with a heading; 2 into 1 into 2; 8 boxes in 3 steps), 16.6 s English and 13.8 s Arabic, at 16:9 and 9:16. All 8 gates pass on the four videos; `text_bounds` 24 and 23 text boxes inside the frame, `contrast` lowest 9.5 to 1. By eye on `snapshot --cards` and one full frame: boxes in their steps, no arrow over a box, no two arrows crossed, two-line labels inside their boxes, the Arabic wide video runs from the right with arrowheads to the left. Found by eye and fixed: the box of an Arabic heading touched the first box (the heading's room now counts its box padding, for the table and the number line too).

Not done: a label on an arrow; an arrow back to an earlier step; a box or an arrow lit on a spoken word; shapes other than a box; the direction chosen by the brief. Not run: a real voice; 4:3 and 21:9 by eye (unit test only).

## `steps`

```json
{"kind": "steps", "items": ["Write the brief", "Check the plan", "Make the video"]}
```

- `items`: 2 to 5 steps of at most 40 characters, in their order. No other field.
- Every step is on screen from the card's start: a numbered disc at the reading side and the text beside it, one or two lines.
- **The step the voice is at is lit**: its disc takes the accent colour and its text is white and bold. The other steps are grey. The light moves with no fade, so nothing flickers.
- When a step is lit: its keys are its words of 4 letters or more that no other step has. The step is lit from the first moment, after the step before it, that the voice says a key. A spoken word that starts or ends with the key counts ("checks" for "check", "والخطة" for "الخطة"). The step stays lit until the next step is lit; the last step stays lit to the card's end. A step the voice never names follows the step before it after an even share of the card's time.
- This is not the list card's rule. That rule takes any word of 3 letters, so "the" lit the second step while the voice was still at the first (seen by eye in the first run).
- Numbers: Latin digits in the Latin font, which stands in the middle of its line. Arabic-Indic digits when a step uses them.
- Arabic: the discs stand at the right and the text runs to the left of them.
- Refusals: fewer than 2 or more than 5 items, an empty item, an item over 40 characters, and every other field (`label`, `text`, `ease`, `stagger`, `mark` and so on).

Tests: `card_steps::tests` (the check; the lit step from the voice's words with "the" said early, the three states of a step with their times, one accent disc at a time, no fade inside the card; the room on 4 frames in 2 languages with 5 steps of 36 to 39 characters, no two steps share a height, text 45 percent of the caption size or more, Arabic from the right). Goldens `steps-en` and `steps-ar`.

## `gauge` and `progress`

```json
{"kind": "gauge", "values": [72], "label": "Battery"}
{"kind": "progress", "values": [3, 5], "text": "3 of 5", "label": "Steps done"}
```

- `values`: `[value]` out of 100, or `[value, top]`. The value is from 0 to the top; the top is above 0.
- `text`: optional, 1 to 12 characters, shown in place of the percent. It does not count.
- `label`: optional, at most 40 characters, on one line under the ring or the bar.
- `gauge`: a dark dial with a grey ring. An arc in the accent colour grows clockwise from the top to the value's share in 0.9 s, fast first and slow as it lands (12 steps). The percent in the middle counts with it.
- `progress`: the percent in the accent colour, a grey bar under it at 80 percent of the width, and the label under the bar. The fill grows in the same 0.9 s.
- A value of 0 draws no arc and no fill.
- Arabic: the bar fills from the right. The ring runs clockwise, as the `donut` card does. A number in Latin digits is set in the Latin font at the Latin size (in the first run the digits came from the Arabic font and the "%" from the Latin one, at two sizes). Arabic-Indic digits are used when the label uses them, and a `text` is shown as written.
- Refusals: no `values`, more than two, a value below 0 or above the top, a top of 0 or less, a `text` that is empty or over 12 characters, a label over 40 characters, and every other field.
- `pie` is not a kind: the `donut` card already draws parts of a whole.

Tests: `card_gauge::tests` (the check and the share; the number's 14 texts from 0 to the value, the arc at rest over the right half and past the bottom for 72 percent, no arc at 0, a `text` that does not count; the bar from the left and in Arabic from the right, 60 percent of its width, growing from 0.2 s to 1.1 s; the room on 4 frames in 2 languages, with and without `text`, at 100 percent with a 40-character label). Goldens `gauge-en`, `gauge-ar`, `progress-en` and `progress-ar`.

Run on 13 Nov 2026 for these three kinds together, `tts: none`, four scenes each (4 steps; a gauge at 72; a progress bar at 3 of 5; a gauge with the text "12 of 16"), 17.0 s English and 18.2 s Arabic, at 16:9 and 9:16. All 8 gates pass on the four videos; `text_bounds` 61 text boxes inside the frame. Lit times against the captions of the English tall video: step 1 at 0.54 s ("write"), step 2 at 2.19 s ("check"), step 3 at 4.83 s ("make"), step 4 at 6.48 s ("look"). By eye on `snapshot --cards`, on frames at 1, 3.2, 5.5 and 8.6 s: the lit step is the spoken one, the ring at 30 percent on its way with "30%" in the middle, the bar at 60 percent from the left and in Arabic from the right, Arabic discs at the right.

Not done: a step with a tick when it is done; a gauge over half a circle; a second value on one bar; a bar that fills on a spoken word; colours by value (red, green). Not run: a real voice.

## Checks after the first full pass (13 Nov 2026)

- Arabic video with Latin words in every kind (`tts: none`, 9.9 s, 9:16 and 16:9): all 8 gates pass, `text_bounds` 48 text boxes inside the frame; by eye every text is inside its cell or box. This run found the two sizes named under "Latin words in an Arabic video".
- Steps, contrast: the first runs read 3.2 to 1 (grey number on a grey disc; the limit is 3). With a darker disc and a nearly solid box the reruns read 5.0 (English 9:16) and 4.9 (Arabic 16:9).
- Squarer frames: `card_kit::tests::a_usual_card_of_each_kind_stays_in_the_room_of_a_square_frame` puts a usual card of each kind inside the room at 1:1, 4:5 and 3:4 in both languages. The largest cards (8 by 5 table, 8 boxes, 5 long steps) are tested only at 9:16, 16:9, 4:3 and 21:9; on a square frame their text can be smaller than the floors named above. Not seen by eye at 1:1.
- Plan id of an old brief (stat with `from`, list, plot with `grid`, transitions `wipe` and `dissolve`): `p_acc787097a4ee617` from this branch and from the main checkout's binary.
- Cost: the unit tests of the six kinds measure real text on 4 frames in 2 languages and take about 60 s of the library's test run.
