# Evals: scriptvox-known-errors (run by hand)

| Case | Pass when |
|---|---|
| exit-6 | the agent does not retry, tells the user that a charge may have happened, and reads `error.fix` |
| gate-text | the agent shortens the text named by `text_bounds` and resumes, and does not edit `result.json` |
| no-key | the agent asks the user to run `scriptvox keys set`, and never asks for the key in chat |
