# `line` card: a trend that draws itself

Gap: Manim (graphs, coordinate systems, data-driven motion) and Remotion users chart trends; scriptvox had only bars.

- Scene card `kind: "line"`: `label` (title), `items` (3 to 8 point names, at most 12 characters), `values` (as many,
  any sign). A dark panel behind the plot; the line is one shape per segment, shown through a clip that sweeps across
  in 1.2 s, in the reading direction (right to left in Arabic). Each point (a white square), its value above it and
  its name below appear as the line reaches it. The plot stays above the captions (bottom at 55 percent).
- Values of `line` and `bars` charts follow the digits of the names: Arabic-Indic names give Arabic-Indic values
  (`cards::indic`).
- Sound: one whoosh as the line starts when `sfx` is on.

Found while building it: drawings need `\4a&HFF&`, or the card style's opaque box shows as a dark rectangle at the top
left.

Measured 3 Nov 2026: English and Arabic 9:16 (Abu Dhabi population 2005 to 2025), all gates pass, frames checked at
0.6, 1.2 and 3.5 s: the line draws in, the Arabic chart runs right to left with ٢٠٠٥ on the right and ٣٫٨ at the end.
Test `cards::tests::chart_values_follow_the_digits_of_the_names`.

Fix 4 Nov 2026 (hand eval): the first and last points sat on the panel edges, so "310" and "2021" hung outside the
panel, about 5 percent from the frame edge. Points now run from 20 to 80 percent of the width; the panel and the
draw sweep keep 12 to 88 percent. Checked by eye at 2.5 s on an English and an Arabic card: every value and name
is inside the panel.
