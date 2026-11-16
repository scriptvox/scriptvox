# Evals (run by hand before a release: they cost model calls)

40 prompts under `evals/<skill>/<case>/prompt.md`, each skill's `README.md` says what a good run does. There are no graders and no `case.yaml` yet, so `claude plugin eval` cannot run them: a person or a fresh agent runs each one by hand and reads the result against the README.
