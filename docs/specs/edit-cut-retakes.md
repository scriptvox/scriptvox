# `edit --cut-retakes` (G024, 7 Nov 2026)

A speaker often says a sentence again to get it right. Editors with text cutting (public docs of Drift, BaoCut and Descript; no code read) cut the early takes.

- A sentence is a retake when one of the next 2 sentences, starting within 10 s of its end, starts with all its words (a false start), or shares 75 percent of the words of the longer of the two. All these limits are constants in `edit.rs`.
- Words pass through `gates::norm_word`, so Arabic spellings and vowel marks do not count, and "four" is "4".
- In a chain of takes, only the last stays. Cuts go through `word_parts` with `--pad`, as in `--keep-words`.
- `--plan` lists `retakes` (`start`, `end`, `text`, `kept`, `differs`) and `parts`, and renders nothing.
- Warning `retake_differs` names the words where two takes differ. The tool does not judge meaning: two meant sentences that look alike get cut, so an agent runs `--plan` first and asks.
- It does not mix with `--keep`, `--keep-words`, `--from/--to`, `--cut-silence` or `--cut-fillers`.

## Measured, 7 Nov 2026, clips made on Edge with heard retakes, 2 cores

| Check | Result |
|---|---|
| English, 3 retakes in 10 sentences | 3 cut, heard again: 7 sentences, each once and whole. 23 s to 16 s, gates pass |
| One retake has "four" then "three" | warning: `4, 3, underground`; the last take stayed |
| Arabic, 2 retakes in 6 sentences | 2 cut, 4 sentences each once and whole, gates pass |
| The eval's 120 s talk of 396 words with no retake, `--plan` | 0 found |

Sound of the joins: no person listened. These voices speak each take the same way, and a person's retake is less alike, so real rates of a miss or of a false find are not known. Whisper can also join a false start into one sentence; such a retake stays. `clips` does not use this.
