# Captions after a merged word event; decimals in the words gate

Found by the hand eval `scriptvox-review rushed-read` (4 Nov 2026, one Sonnet agent, skills only).

## Merged word event (captions)

Edge sent "In 2005", "By 2015" and "By 2025" as one word event each. `captions::align` gave the whole event to "In"
and every later script word took the event before its own: "one" stayed alone on screen from 3.2 to 7.0 s, the
captions ran up to 4 s late, and the last page ("It needs more roads and parks.") got zero length (run
r_6d3155.1.1). Now `align` first splits an event that holds several words into one event per word, its time
shared by letter count. Same brief again (run r_45350e): five pages, each on its spoken time, last page 11.2 to
12.7 s; checked by eye at 3.0 and 6.5 s (the spoken word is highlighted). Test
`captions::tests::align_splits_an_event_that_holds_two_words`.

## Decimals (words gate)

The recogniser writes "1.4" where the script says "one point four", so each decimal cost three misses: a clean read
failed the gate at 22 of 31 and the run ended `ok: false`. `gates::heard_counts` now reads a decimal on either side
as its parts ("1", "point", "4"). Run r_45350e: `check --hear` 31 of 31, pass. Test
`gates::tests::a_decimal_is_heard_as_its_spoken_parts`.

## Skill

`scriptvox-review` said "+48 means cut 48 percent". The agent cut 54 to 37 words and still read at +20. The skill now
says to take the `fit` finding's word count, or keep words divided by (1 + rate/100).

## Gate `captions`

`make` adds `gates::caption_pages`: warn when a page has under 0.2 s, or one or two words stay over 2.5 s (the
signs of the fault above), else pass with the page count. Run of the same brief after the fix: pass, "5 pages,
each readable". The broken run's pages (one word 3.16 to 7.0 s, a last page with 0 s) warn in
`gates::tests::caption_pages_out_of_step_warn`. The review skill says to open a snapshot at the time it names.

## Metrics of a finished run

Rerun of the eval (run r_9f0fec, pass): `check <run_id>` and `status <run_id>` returned empty `metrics`, so the agent
read `voice_rate_pct` from a saved file. Both now add the run's metrics from `result.json` (`check r_9f0fec`:
+48, 1.0, 13.0 s); a plain MP4 still gets gates only. Review skill updated.

## Resume keeps the earlier video

Every resume wrote the parent's `out` path, so the video before the fix was overwritten. A child now writes
`<name>.<n>.<ext>` next to it, following the run id (`rushed.mp4`, `rushed.1.mp4`, `rushed.1.1.mp4`). Measured:
`resume r_45350e --wait` wrote `fixed.1.mp4` and `fixed.1.srt`; `fixed.mp4` stayed. A run with no `out` already had
its own name.
