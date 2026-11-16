# Long sources (G030, 7 Nov 2026)

The 20 minute limit was a 38 MB cap on one speech to text request, in `analyze::transcribe`. Tools for long talks (OpusClip, by public docs; no code read) work in pieces, and so does this.

- A WAV over 38 MB goes in windows of 15 minutes that overlap by 10 s, each its own job. In an overlap, each window keeps the words before the middle, so no word comes twice. The joined reply is a file in `.scriptvox/analyze`, so `timed_words`, `sentences`, `analyze`, `clips`, `dub` and `--like` all see one transcript.
- A rerun uses the jobs that finished.
- `clips` asks the model about 20 minutes of sentences at a time, overlapping by `--max` seconds. All answers go to the existing check, which sorts them by score and removes overlaps. Scores from two requests are not on one scale; there is no rerank call.
- An audio of 20 minutes or less takes the old path with the same keys, so old caches still work.

## Measured, 7 Nov 2026, the eval's talk looped to 25 min, `large-v3` on `-self`

| Check | Result |
|---|---|
| `analyze --transcript` | 88 s, 2 windows, 5,191 words, 430 sentences |
| Words around the edge, 888 to 902 s | each word once, in time order, and the sentence across the edge is whole |
| Time of the same word in each of the 13 loops | within 0.2 s of the others, so no drift from the merge |
| `clips --count 3 --plan` | 3 picks, score 9; one is past the first 5 minutes |
| `clips --count 1`, real render | exit 0, sheet read by eye |

Words per loop ran 391 to 425 against 397: this is speech to text repeating words on the row itself, known from G004 and G158, not the merge.

## Not run, and left out

A real long talk: this one repeats, so picks repeat too. A video over 40 minutes. `edit --captions` and `--cut-fillers` on a long file: `edit::hear` sends one file to the row with no cap, and a very long one may time out.

## `edit --captions` and `--cut-fillers` (7 Nov 2026)

`edit::hear` now uses the same windows and the same merge. Before, a long file sent one request over the limit.

Real run, a 25 min file of one 16.6 s clip in a loop, 320x180, `whisper-large-v3-self`, 2 cores for FFmpeg: `edit --captions` 287 s, 550 pages, 4,549 words, exit 0, av_drift pass. At the window edge near 900 s, each line comes once. 90 loops hold about 4,410 words, so about 3 percent are extra. "palms" comes 93 times against 90, and "Most" and "Farmers" 90 times each. The extras are not at the window edge I read, and the cause is not known. A loop is a hard case for Whisper, and a real long talk was not run.
