# Evals: scriptvox-edit (run by hand)

| Case | Pass when |
|---|---|
| vertical-talk | the agent runs `analyze` first, cuts silence only after checking there is no music bed, makes it 9:16 in one `edit` call, then runs `check` and a snapshot and reports length before and after |
| translate-ar | `analyze --transcript`, an Arabic script written tight with names kept, a brief with `base_video`, `lang` ar and `duration` = source length; the agent warns about burned-in captions or lip sync where they apply |
| copy-pace | the agent reads `mean_shot_s`, `cuts_per_min` and `words_per_min` from the reference and writes a brief whose scenes and word count match them |
| track-subject | the agent sees from the `analyze` strip or a snapshot that the subject is off centre, runs `edit --reframe 9:16 --track` (with `--subject` naming the camel), and checks a sheet of the result for the subject in every frame |
| shorts-from-talk | the agent runs `clips --plan` first and reads the parts, then cuts them, opens a sheet of each short, and says the score is the model's opinion; no hand work from the transcript unless a part is wrong |
| bed-under-talk | `edit --music calm --music-level quiet` (or a file the user gave), reports the sound at -14 LUFS, does not re-encode through a second tool |
