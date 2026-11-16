# Filler-word cut (`edit --cut-fillers`)

Gap: talking videos keep "um", "uh" and Arabic hesitations; HyperFrames-style editors cut them. ScriptVox had only
`--cut-silence` and `--keep`.

- Hearing: the free selfhost Whisper row with a filler prompt (English or, with `--lang ar`, Arabic). Without a prompt
  that has fillers, Whisper leaves them out. The STT row now passes `prompt` to the engine (`initial_prompt`).
- Fillers: hesitation sounds after letters that run together count once (`um`, `uh`, `erm`, `hmm`, `اممم`, `ااا`).
  "أم" and "ام" also mean "or", so they count only when drawn out. "يعني" counts only with a pause of 0.2 s or
  more in the sound on both sides.
- Placement: Whisper puts a filler in the pause before its sound (4 Nov: 5 of 5). Each filler snaps to the sound
  island (between two silences of 0.1 s at `--noise-db`) of 0.8 s or less that overlaps most with its word time
  plus 0.4 s. The cut takes the island and its pauses, less `--pad` on each side. With no such island the filler
  stays and is counted in `fillers.skipped`.
- Output: the same encode as `--keep`; `fillers` = heard, cut, skipped, words (the cut sound times).
- Not with `--keep` or `--cut-silence`. Works with `--from/--to`, `--captions`, `--reframe`, `--logo`.
- Measured 4 Nov: English 5 of 5 cut, 14.0 s to 10.15 s, 25 of 25 words remain, a second run finds none;
  Arabic 2 of 2 cut, all words remain. Test: `edit::tests::fillers_snap_to_their_sound_and_go_with_their_pause`.
- Limit: tested on synthetic voices only. A real recorded talk is still to test.
