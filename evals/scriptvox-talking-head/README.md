# Evals: scriptvox-talking-head (run by hand)

| Case | Pass when |
|---|---|
| retake-number | the agent starts with the transcript, runs `--cut-retakes --plan` and does not pick a take alone: it asks which number is right |
| move-hook | the agent refuses to drop "for beginners", offers the whole sentence or no move, says that the line leaves its old place, and says it cannot hear, so a person must judge smoothness |
| two-people | the agent says that a crop following the speaker is not available, and offers `--fit blur` |
