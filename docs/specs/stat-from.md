# Stat card: count from a start number (10 Nov 2026)

Before: a `stat` card always counted up from 0 to its number. A change from one value to another ("churn fell from 9 percent to 4") could not be shown; the 10 Nov comparison pass lists this as weaker than a tool whose numbers can run between two values.

- Card field `from`, a plain number, for `stat` only. The count runs from `from` to the number in `text` in the same 12 steps and 0.8 s, with the same ease. A `from` above the number counts down.
- The steps take the unit, the decimals and the digits (Arabic-Indic too) of `text`, as before.
- The brief check refuses `from` on another card kind, a `from` that is not a finite number, and a `from` on a stat whose `text` holds no number.
- No `from` means 0, so an old brief gives the same frames.

Run on 10 Nov 2026 at home, free, `tts: none`, 16:9: `"4%"` from 9 and `"$42M"` from 30. Frames by eye at 0.2, 0.5 and 2.0 s of each scene: 8%, 5%, 4%, and $35M, $41M, $42M. Unit test `a_stat_counts_from_its_start_number_up_or_down`: first and last step, every step inside the range, the count down, and the three refusals.

Not done: `compare` and `donut` still count from 0. Not run: an Arabic stat with `from`.
