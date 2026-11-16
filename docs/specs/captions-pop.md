# Pop captions (daily, 3 Nov 2026)

Gap: the short-form look of 1 to 3 big words at a time is the default in HyperFrames' caption overlay, Remotion's
caption examples and MoneyPrinterTurbo; scriptvox had only two-line karaoke captions. Our own design, same engine.

- Brief: `"captions": "pop"` (or `"karaoke"`, the default). Any other value is a usage error (exit 2). Part of the
  plan id.
- Pop: caption size 1.35x, one line no wider than 62 percent of the short side, a new page after 1.0 s at most or
  a 0.4 s pause; each page scales in from 85 to 100 percent in 0.12 s. The spoken word still lights up, the box and
  the position (bottom margin) stay, so cards and the hook do not move.
- The SRT and the word times are the same in both styles.
- Checked by eye on 3 Nov, English and Arabic 9:16 with all card kinds: pages of 1 to 3 words ("How a desert" /
  "city drinks."), readable, clear of the cards, Arabic right to left and joined.
- Test: `pop_pages_scale_in` (the tag on each page, none without pop).

## No flash pages (5 Nov 2026)

A very short word alone on a page (Arabic "من", 0.16 s) flashed by and made gate `captions` warn. A page under 0.25 s now joins the next page's first line when it fits with no pause between, else the last line of the page before. Real run: "من مناطق الدولة." is one page, gate passes.

A short word with no room on either side ("Most floods start" | "with" | "a blocked drain.", 0.12 s, the `captions` warning): the page before gives it its last word ("Most floods" | "start with" | "a blocked drain.").
