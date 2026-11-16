# `edit --keep-words`: cut by deleting words (G023, 7 Nov 2026)

With `--keep`, an agent must do time arithmetic. Text-based editors (Descript, by its public docs and behaviour; no code read) let the user delete words. An agent can do the same in JSON.

1. `scriptvox analyze talk.mp4 --transcript --json`. `transcript.file` holds the word list.
2. Copy that file, and delete words from its `words` list. A bare list of words also works.
3. `scriptvox edit talk.mp4 --keep-words copy.json --json`.

- The tool gets the whole transcript again, from the `analyze` cache, and compares. Only a word that was deleted makes a cut, so a long pause between kept words stays.
- A word stays when its middle is inside a word of the file, so words that `--terms` joined still count.
- A part has `--pad` of air, never into a removed word. With nothing removed before or after a part, it runs to the start or the end of the video. So a file with nothing removed keeps the whole video.
- `keep_words` in the envelope: `heard`, `kept`, `removed` (the words) and `parts`.
- Exit 2: an empty file, or a word that is not in this video's transcript. It does not mix with `--keep`, `--from/--to`, `--cut-silence` or `--cut-fillers`. The same path as `--keep` makes the cut, so captions, `--reframe` and `--logo` work with it.

## Measured, 7 Nov 2026, the eval's talk, 396 words, 120 s, 2 cores

| Check | Result |
|---|---|
| 3 sentences and one mid-sentence word removed, with `--captions` | 37 s, 45 words listed as removed, 3 parts, all gates pass, sheet read by eye |
| The result heard again | 346 words against 351 kept, in order. Differences: "their water" read "the rainwater" at the mid-sentence cut; 2 places far from a cut lost words |
| One word kept (`tap.`) | 0.40 s, exit 0 |
| An empty file; a word at 900 s | exit 2, each with its reason |

The "rainwater" reading may mean that the cut clipped a sound: a whisper word time is not exact. No person listened. To cut less tightly, raise `--pad`. A single word inside a sentence is the hardest case, and `--keep-words` does not fix a bad join. Arabic was not run.
