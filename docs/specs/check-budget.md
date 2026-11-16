# `check --budget`: a time limit for a long check (G147, 10 Nov 2026)

Why: `check` reads the whole file, and a long recording can take longer than an agent's tool time limit. The agent then gets nothing.

- `scriptvox check <file or run> --budget <seconds>`, 1 to 3600. FFmpeg is stopped after that time, and the gates report on the part that was read.
- The result has `partial: true` and a `budget` finding, `warn`: "read the first 91 s of 240 s in 2 s: the rest is not checked".
- `black`, `slideshow` and `silence` speak for the part that was read. `av_drift` comes from the file's header, so it is whole. `loudness` is `warn`, "not measured": the loudness of a programme needs the whole file. The test for a damaged file (a decode that stops early) is not made, because the read was stopped on purpose.
- A part that passes is not a pass for the file: the `budget` finding says so, and the exit code is still 0 when no gate failed in the part.
- A read that ends inside the budget gives the normal whole result, with no `partial` and no `budget` finding.
- `--budget` with `--hear` is a usage error: the listening gate needs the whole sound.

Run on 10 Nov 2026 at home on a 240 s file of 1920x1080 (468 MB): no budget 4.2 s, the whole result; `--budget 2` 2.3 s, 91 s read, `partial`, loudness not measured; `--budget 0.2` and `--budget 2 --hear` exit 2. Tests: `the_last_time_of_a_log_is_how_far_a_read_got` (unit) and `check_budget_stops_the_read_and_says_the_result_is_partial`.

Not done: no way to read a later part of the file (a start time); the partial result does not say which gates would need the rest. Not run: Windows, a file of more than an hour.
