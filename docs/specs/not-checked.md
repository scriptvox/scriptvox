# Result field `not_checked` (G037, 7 Nov 2026)

An agent can read "all gates pass" and report a pass that no gate gave. BaoCut's public text names this problem; no code read.

`make`, `edit` and `check` add `not_checked`, a list of strings: the gate, and why it did not run.

- `judge` and `person` are always there: frames are not scored by these commands, and no person watched or listened. Gates measure, they do not see or hear.
- `words` when the speech row did not listen: a draft, `--local-only`, no row, or a row that failed. A `words` finding that says "not checked" counts as not run.
- `base_text` when a base video got no answer from a vision service. In `edit` the gate has no pass finding, so it is listed whenever captions or a zoom are on.
- `edit` lists `black`, `loudness` and `words`, because it runs only `av_drift`.
- The skills tell an agent to report this list with any pass.

`make` writes it in the result, so `wait`, `resume`, `dub` and `check <run>` see it where they read that file.

## Measured, 7 Nov 2026, real commands

| Run | `not_checked` |
|---|---|
| `make` with Edge and a speech row | judge, person |
| `make --draft` | words, judge, person |
| `edit --cut-silence` | black, loudness, words, judge, person |
| `check f.mp4` | words, judge, person |
| `check f.mp4 --hear --script s.txt` | judge, person |
| `check <run>` | judge, person: it has the run's own words gate |

`--local-only` was not run. `clips` is not covered: its findings come from `edit`, and its top result does not hold the list of each clip.

## Fields `coverage` and `limitations` (G160, 7 Nov 2026)

A passed gate looks at part of a video, and a report can claim more than that. Idea from the public docs of Craft Anything (`docs/plan/research/discovery-craft-suite-20261107.md`); own design, no code read.

`make`, `edit` and `check` add 2 lists of strings, one line for each gate that ran, from one table in `gates.rs`. Each number in the table is the threshold in the FFmpeg command of that gate.

- `coverage`: what the gate looked at.
- `limitations`: what a pass from it cannot show.

**No `confidence` field.** The only real number is the share of words heard, and `words` already gives it in `detail`. A value for other gates would be invented.

Real runs are in `docs/daily/2026-11-07.md`.
