# Card motion: `ease` and `stagger` (list and bars)

Gap: every card kind had one fixed motion, so cards looked like templates (Manim rate functions and lagged starts,
HyperFrames easing). Our own design, in ASS tags, no new renderer.

- Brief: `card.ease` = `out`, `spring` or `linear`; `card.stagger` = seconds between items (0.05 to 1.5). Only
  `list` and `bars`; on other kinds the brief check refuses them.
- `out`: ASS `\t` with acceleration 0.4 (fast, then slow). `spring`: to 108 percent in the first 70 percent of the
  time (acceleration 0.5), then back to 100. `linear`: plain `\t`. Bars scale in width from 0; list items from
  70 percent, over 350 ms, from their reading-side corner.
- `stagger` on a list: the first item still waits for its spoken word, the rest follow at the gap, never later
  than 0.3 s before the card ends. On bars it replaces the 0.15 s gap.
- Without the fields the output is byte-identical to before (the 10 old goldens unchanged).
- Goldens: `bars-en` (ease out), `list-en-spring` (landed, same picture as `list-en`), `list-en-spring-mid`
  (0.6 s, in motion). Unit test `cards::tests::ease_and_stagger_move_list_items_and_bars`.
- Measured 4 Nov on a real make: the spring list grows from 1.83 s, is past its size at 2.03 s, still by 2.17 s;
  all 7 gates pass.
