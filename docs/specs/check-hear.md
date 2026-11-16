# `check --hear`: the final audio against the script

Source: OpenMontage cherry #10 (self-review with a transcript diff; its final review samples 4 frames and checks
punctuation words). ScriptVox already had full-file gates; this adds the words.

- `scriptvox check <run|mp4> --hear [--script f] [--lang l]`: 16 kHz audio to the free Whisper row (same path as
  `analyze --transcript`), then gate `words` (`gates::words_heard`): longest common subsequence of normalised words,
  share of script words heard. Pass from 90 percent, warn from 75, fail below (exit 3).
- Normalising (`gates::norm_word`): lower case, punctuation off, Arabic diacritics and tatweel off, alef forms as
  bare alef, final ta marbuta as ha, alef maqsura as ya, Arabic-Indic digits as ASCII. Pause tags are stripped.
- The script comes from the run's `request.json` (`script`, or every scene's `script` in order) unless `--script`.
  For a file (13 Nov 2026): from the recipe in the MP4, with its `lang` and `say`. Before, `check out.mp4 --hear`
  stopped with "--hear needs a script" though the file held it. Only a file with no recipe (`--no-embed`, or not
  made by scriptvox) stops now, and the fix names `--script <file>`. Test `tests/gates.rs`
  `check_hear_on_a_file_takes_the_script_from_its_recipe`. Real run: see `docs/daily/2026-11-13.md` item 400.

Measured 3 Nov 2026: English 2-scene run 25 of 25; Arabic run 17 of 18 (94 percent); the English video under pink
noise 4 times the voice level 0 of 25 (fail). Test `gates::tests::words_heard_counts_script_words_in_order`.

`make` listens too (3 Nov, evening): when a free speech to text row has a key and the run is not `--local-only`,
the finished video goes through the same `words` gate; a fail is exit 3 like any gate, and the video stays. The
transcript cache (`.scriptvox/analyze/`) is now keyed on the file content, not the path: a video made again at the
same output path was given the old transcript. Measured: an Arabic make with listening, 16 s in all, words 17 of 18.

Whisper on music-only audio (3 Nov, evening): on the voice-free Demucs stem it wrote "So, let's take a look at the
city." (8 words, close to its own prompt). The engine now reports `no_speech` and `logprob` per segment and drops a
segment when no-speech is over 0.4 and log probability under -0.6 (measured: that hallucination 0.505 and -0.80, real
speech 0.026 and -0.13), or when 4 or more of its words are 80 percent prompt words with a no-speech score over 0.3.
After (image `rawivox-gen-audio:20261103g`): the stem gives 0 words, the English track still 20 of 20.

Correction, 3 Nov 21:00: the score rule above (no-speech over 0.4 and log probability under -0.6) was removed again
in image `rawivox-gen-audio:20261103h`. Real speech can score the same as that hallucination, so the rule could drop
real words. Only the strict prompt-echo rule stays, and the scores stay in the output. Whisper may still write a
short invented sentence on music-only audio; no product path transcribes music-only audio today.

## Number words (4 Nov 2026)

The recogniser writes "two" where the script has "2". `norm_word` maps a single number word (zero to twenty, the
tens, hundred; Arabic one to ten) to its digits on both sides. Measured on run r_ef4e31 (script with "2×" and
"≈ 4"): 20 of 23 (warn) became 22 of 23 (pass); the one miss left is "km/h" read as words. Compound numbers
("forty two" against "42") stay a miss.

## Repeated sentences (5 Nov 2026)

Speech to text heard a sentence said twice in a row only once: 15 of 30 words, so a refrain failed the video. A script with repeated sentences also counts with each sentence once; at 90 percent or more the gate passes and its detail says to listen to the repeat.
