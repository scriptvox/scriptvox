# `edit --captions --text` (G029, 7 Nov 2026)

Speech to text can get a spelling wrong. A speaker who has the script should not need to fix it. Public docs of Tscaps and MioSub describe aligning known words; no code read.

- `--text script.txt` with `--captions`: `captions::align` puts the script's words on the heard times, the same aligner `make` uses. Spelling and punctuation are exactly the script's.
- Gate `words` compares the script with what was heard. A failed gate (under 75 percent) stops at exit 3 before the encode: this text is not what the video says.
- After a cut, give only the words that stay.
- Fault found in this work, fixed: `edit --captions` copied a `captions.srt` that it never wrote, so no `.srt` came out. It writes one now.

## Measured, 7 Nov 2026, Edge voice on a plain picture, 2 cores

| Check | Result |
|---|---|
| English, "litre" in the text and "liter" in the speech | caption "One litre", 52 of 53 words, drift 10.0 ms, sheet read by eye |
| Arabic with short vowel marks in the text | captions keep the marks, read right to left, 24 of 26 words, drift 16.0 ms, sheet read by eye |
| A text that was not said | exit 3, "heard 3 of 18", no video written |

Not run: a real speaker, a long talk, a script that differs from the speech in a whole sentence.
