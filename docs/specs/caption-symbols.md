# Symbols in the script: drawn at full size, read as words

Gap: a script with "≤", "→", "≈" or "√" failed at `plan` (SVX_FONT_COVERAGE). Libass fell back to Noto Sans Math,
whose tall win metrics (5655 units on a 1000 em) drew the symbol at about a quarter of the caption size. The open
voice read "≤" as "SF", "2×" as "2km" and "≈ 4" as "04".

- Font: `assets/fonts/RawivoxSymbols-Regular.ttf` (260 KB, OFL-1.1), 1354 symbols of U+2000 to U+2BFF that Noto Sans
  lacks, cut from Noto Sans Math with Noto Sans's win and typo metrics, renamed. Made by
  `scripts/make_symbols_font.py`. Embedded and written to each run's fonts folder; fontconfig accepts it after
  Noto Sans. The plan lint counts it, so these symbols pass `plan`.
- Voice: `say::spoken` turns symbols into words for the voice only (en and ar): ≤ at most / حتى, ≥ at least, ≠,
  ≈ about / حوالي, × times / في, ÷, ±, √, ∞; arrows become a short pause (a comma). Captions keep the symbol.
- Pages: a bare symbol never ends a caption page (it leads into what follows).
- Open voice trim: the letter count that cuts invented speech ended early when a unit was read out ("km/h" as
  "kilometres per hour") and lost the last words. The cut now moves to the script's last word when it comes within
  8 words. `opentts::VERSION` open-v3.

Measured 4 Nov 2026 (one English script with ≤, →, 2×, ≈, voice `selfhost`):
- Before: symbols a quarter size (A/B still render); words gate 18 of 23.
- Symbol table only: "about 4 seconds" cut by the trim, words gate fail 17 of 23.
- All three fixes: symbols full size, the whole script spoken ("at most 40", "two times", "about four seconds"),
  words gate warn 20 of 23 (the gate counts "two" against "2" and "km/h" as a miss), 6 other gates pass.
  Arabic line with ≤ and → checked by eye (≤ mirrors in a right-to-left line, as Unicode asks).
- Number words fixed next (check-hear.md): the same run hears 22 of 23, pass.
