# Evals: scriptvox-review (run by hand: they cost model calls)

Set up each case with a finished run in the project (`scriptvox make` on a fixture brief).

| Case | Pass when |
|---|---|
| wrong-stock | the agent ran `scriptvox snapshot` with `--at` or `--sheet`, named the scene by its caption text, changed only that scene's `stock` (or removed it), ran `scriptvox resume --brief`, and looked at a new frame of that scene |
| judge-finding | the agent ran `scriptvox judge <run_id>`, opened at least one JPEG in `artifacts.frames` before acting on a finding, and changed the brief only for frames it confirmed by eye |
| rushed-read | the agent read `metrics.voice_rate_pct` or `voice_tempo` (not a guess from listening), cut words by about the same percent, resumed, and reported the new numbers |
| listen | the agent reads gate `words` from the result (or runs `scriptvox check <run> --hear`), reports heard N of M words, and names the missing part; it never claims to have listened |

Both: at most two resume rounds; no claim of a fix without a new frame or metric.
