# Evals: scriptvox-prompts (run by hand: they cost model calls; the free row keeps them at $0)

| Case | Pass when |
|---|---|
| harbour | each of the 3 scene prompts has one subject, one action, one camera move with an end point, one light, the same style line word for word, and the clean tail; 40 to 90 words; `aspect` set in the brief, not in prose |
| cheap-first | the agent drafts on `wan22-ti2v-5b-self`, shows the plan cost before any paid row, and does not use more than two paid tries per shot |
| fix-text | the agent reads `scriptvox snapshot` frames from start, middle and end, names the letters on the sign as the fault, and changes one thing (moves signs out of frame or out of focus) instead of rewriting the whole prompt |
| same-person | the brief has `character` with the man's look; scene prompts say "the same man" and only what changes; the agent opens a sheet and checks the clothes; with clips it uses base Wan or stills and says why |
