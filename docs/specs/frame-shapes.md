# More frame shapes (5 Nov 2026)

Gap: MoneyPrinterTurbo #17, ComfyUI A17 (8 presets). `aspect`, `edit --reframe`, `join --aspect`, `clips --aspect`: 9:16, 16:9, 1:1, 4:5, and now 3:4 (1080x1440), 4:3 (1440x1080), 2:3 (1080x1620), 21:9 (2520x1080). Generator rows offer 9:16, 16:9 and 1:1, so tall shapes ask for 9:16 and wide shapes for 16:9, and the graph crops the middle (as 4:5 already did). `analyze --like` knows them too.

Fault found by eye: on 4:3 a bars card with long names beside the bars left almost no bar. Names now go over the bars when the side layout would leave a bar under 30 percent of the width (also 16:9 with very long names).

Checked by eye, drafts with a title, bars and list card: all 4 new shapes; every gate passes.

Also: `generate --image` on an image row that takes references sends the picture as a reference on the text endpoint (it was refused).

## Cards on 4:3 and 21:9, every kind in both languages (5 Nov 2026, rerun)

The first check held only title, bars and list. All 13 kinds in English and Arabic found:

- Plot on 4:3: the function ran off the left edge. Title and function now share a row over a full-width plot; the function shrinks to its room.
- Arabic plot: the numbers and the function are Latin but took the raised Arabic size, covered the curve and cut the title. Formulas and plot numbers now stay at the Latin size in every language (goldens `formula-ar`, `plot-ar` blessed by eye).
- Arabic formula and stat on 4:3: the label lay over the captions. The group moves up, then shrinks, and ends at 72 percent of the height (66 in Arabic, whose captions are taller).
- Zero-crossing values that touch: a value is left out, its dot stays.
- Arabic names in compare, bars and donut were sized at 0.6 em a letter: much too small, and bars lost the side layout. Compare measures names with a space; a single word and the bars and donut estimates use 0.45 em for Arabic.
- `snapshot --cards` with more than 4 cards makes a grid: 13 in one row could not be read.

Checked by eye: r_5ff53b (4:3 en), r_6d980c (21:9 en), r_8b5e02 (21:9 ar), r_fa6010 (4:3 ar).

Long labels (5 Nov 2026, independent review): a label shrinks to 55 percent at most to fit its room, then the number, formula or rows fit the rest. `plan` refuses a label over 60 characters (plot 40, compare 30). Test `every_labelled_card_stays_on_a_4_3_frame_with_real_wrapping`.
