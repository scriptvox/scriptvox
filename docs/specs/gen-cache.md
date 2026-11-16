# Generated clips come back from the cache (5 Nov 2026)

Gap: OpenMontage #6 and #7, MoneyPrinterTurbo #5, and fresh-agent test 4: `resume` that changed only the look made new pictures, and would have paid again for a paid clip (with `plan` asking first). Each clip or still now has a key from its row and input in the content store. Same key: `plan` says "already made, no call and no charge" at cost 0, and `make` restores it (`reused_scenes`). `voice_reused` says the same for the voice. Change a word of the prompt for a new take. The receipt lists `generators`, so `export` still counts the terms of a reused clip.

Measured: r_52c3a5 with 2 stills, then `resume` with another look and captions: same picture hash, 0 calls, 6.7 s of render.

Other test 4 fixes: `edit --captions` warns when the source has burned text (free vision row), writes an SRT next to the output, takes `karaoke` as a name; `edit --reframe` reports `crop_keeps_width_pct`; the crop refusal's fix works with no run; `check`'s silence fix no longer tells a file to re-render; `export` parts list music (a music file counts as not cleared); `make --help` names the real output folder; `batch` gives `next`.
