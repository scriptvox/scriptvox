# Gates `first_frame` and `flashes` (G188, 13 Nov 2026)

Rival: video-director (MIT, README only read) says a script measures frame 0 and flashes with no model. ScriptVox had no such measure: the black gate counts only stretches of 0.5 s or more, so a black first frame passed.

Both gates run in the one FFmpeg pass that `check` and `make` already do. No model, no network, no new download.

- **`first_frame`**: warn when the video starts on a black frame (blackdetect from one frame up, start at 0). A feed or a player shows the first frame before the video plays. Pass detail: `not black`.
- **`flashes`**: warn when 3 or more hard changes of the picture fall inside 1 s (`scdet`, score 30 or more). The detail gives the count and the time. Reason: more than 3 flashes a second can harm a viewer with photosensitive epilepsy. Very fast cuts warn too, on purpose.
- Both are warnings: exit 0. The black gate is unchanged: only stretches of 0.5 s or more count, 2 s in one stretch fails.

Measured 13 Nov 2026 with `scriptvox check`:

| File | `first_frame` | `flashes` |
|---|---|---|
| a strobe, bright and dark 5 times a second for 1 s | pass | warn, 10 changes in 1 s from 1.1 s |
| a test picture after 2 black frames | warn, 0.07 s | pass |
| a ScriptVox render with two card scenes | pass | pass |
| an edited talking clip of the 11 Nov evals | pass | pass |
| a generated 9.4 s clip (832x480) | pass | pass |

Limits, also in `check`'s `not_checked` text: only black counts for the first frame (a white, blurred or empty first frame passes); a slow pulse, or a flash in a small part of the frame, passes.

Not done of G188: text contrast against the picture behind the text (captions sit on their own box, so the risk is in titles and hooks; a measure needs the picture under each text box), and a loop seam check. Not measured: recorded footage with real camera flashes.

Test: `tests/gates.rs`, `a_black_first_frame_and_a_strobe_are_warnings`.
