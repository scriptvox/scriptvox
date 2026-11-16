# `edit --captions-file` (G031, 7 Nov 2026)

A user can have an SRT, or a transcript from another tool. Public docs of Shotstack and OpusClip describe import; no code read.

- `--captions-file x.srt`, or `x.json` with `words` (text, start, end), the transcript file of `analyze --transcript`. It implies `--captions karaoke`, and `--captions pop` still picks a style.
- No speech to text runs, so no sound leaves the machine.
- Each cue's time is shared among its words by letters. A cue's own line breaks do not stay: the caption pages come from the same rules as every other caption.
- SRT handling: a byte order mark, CRLF, both `,` and `.` in times, tags such as `<i>` and `{\an8}`, and a position after the end time.
- Times are the source's, so it refuses a cut, `--speed`, `--text` and `--terms`. With `--from/--to` it keeps the words of that part, moved to start at 0 (G141, 8 Nov). A file with no captions gives exit 2.

## Measured, 7 Nov 2026, Edge voice on a plain picture, 2 cores

| Check | Result |
|---|---|
| English SRT with a mark, CRLF, `<i>`, 2 cues | 16 words, 2 pages, on the times of the file, drift 10.0 ms |
| Arabic SRT with vowel marks | 8 words, right to left, marks kept |
| Word JSON | 3 words |
| 5 frames read by eye | each caption with its own words and no tags. `.scriptvox/analyze` stayed empty, so nothing was sent to the speech row |

Not run: an SRT from another tool, a cue past the end of the video (it does not show, with no warning), a file with word-level times in VTT. Karaoke highlights by letter share here, so with a coarse SRT the highlight is an estimate.

Audit fix, 9 Nov 2026: with `--out` or `--captions-file`, `edit` and `make --out` do not replace a different `.srt` or `.vtt` already beside the video. The new captions take the next free number (`x-2.srt`), with a `sidecar` warning. `artifacts.sidecar_srt` and `sidecar_vtt` say where the caption files beside the video are, in every result since 13 Nov 2026 (before, only when the number changed; an agent then took the run folder's `captions_srt` for the only new captions). `make` writes them after the render. A file ScriptVox wrote there itself and nobody changed since (a note in `.scriptvox/sidecars/` holds its hash) is replaced, so a rerun never leaves old words beside a new video. `credits.txt` beside `--out` follows the same rule, and `artifacts.credits` says where it landed.
