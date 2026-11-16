# Evals: scriptvox-brief (run by hand: they cost model calls)

| Case | Pass when |
|---|---|
| en-topic | brief.json has `aspect` 16:9 and `duration` 45; spoken words 126 to 154 (budget 140 within 10 percent); the first sentence is not a greeting and does not start with "In this video"; `scriptvox plan` ran before `make` |
| ar-topic | `lang` ar, an `ar-AE-*` voice, spoken words 43 to 53 (budget 48); no Latin words in the script; the hook has 7 words or fewer |
| stat-card | the scene that says the number has `card` kind `stat` with `text` "90%" in Western digits and a label under 40 characters; no other scene repeats the number as a card |
| edit-no-trigger | the skill does not load (editing an existing clip is out of scope) |

Count spoken words with the `[pause N]` tags removed.
| match-folder | `media` plus `media_match`, reads `media_match` in the plan result before `make`, names any `unused` file, and writes `scenes` only to change a wrong pick |
| long-first-look | `make --draft`, reads the sheet, then `resume --final`; `export` only on the full video |
