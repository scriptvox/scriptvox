# Brief `captions_second` (G072, rest, 8 Nov 2026)

A small second line under the main captions, for example a translation, as bilingual subtitle tools show it. Our own
design on the same ASS writer.

- Brief `captions_second`: a list, one entry per sentence of the script, in order. A sentence ends at `.`, `!`, `?`,
  `؟` or `۔`; words after the last mark are one more sentence. An initial or a short form (`W.`, `U.S.`, `Dr.`) is not
  an end (`docs/specs/caption-short-forms.md`). A count that differs is exit 2 before any voice, with
  both counts. An empty entry is exit 2. Not with `caption_at` `top` (exit 2): the line sits under the main captions.
- Each entry shows from its sentence's first word until the main page with its last word leaves. A short pause holds
  that page (`HOLD_S`), so the line does not blink there; after a long pause it goes with the page and does not sit
  alone. It never runs past the next sentence's first word, when one page holds both.
- Size: three fifths of the main size, scaled by the font's height like the main captions, so Arabic and Latin look
  the same size. Font: the brand font when set, else Noto Sans Arabic when the entries hold Arabic letters, else
  Noto Sans. Light grey on the same 65 percent black box, wrapped at 86 percent of the width.
- The main captions move up by the second line's height (its longest entry), so the two never touch.
- Part of the plan id only when set: old plan ids do not change.
- The SRT and VTT keep only the main captions; for a second language as a player track use `export --soft-subs`.

## Measured, 8 Nov 2026

| Check | Result |
|---|---|
| 9:16, English Kokoro voice, 3 sentences, Arabic second line | 20 s, 7 gates pass; frames at 1, 3, 5.5 s read by eye: each Arabic line joined, right to left, under its English page, a gap between the boxes |
| First run | the Arabic line looked smaller than the English one (same px size, taller font): now scaled, 50 to 71 px |
| 16:9, Arabic Edge voice, `pop`, English second line of 101 letters | 7 gates pass; the line fits on one line (Noto Sans 50 px on 1080), under each pop page |
| 1 entry for a 2-sentence script | exit 2, "captions_second has 1 entries, the script has 2 sentences" |

## `edit` and `dub --captions-second FILE` (8 Nov 2026)

- One text file, one line per sentence, in order. Blank lines at the end go; an empty line inside is exit 2.
- `edit`: needs `--captions` (or `--captions-file`), else exit 2. Same layout code as `make` (`make::second_layout`,
  `second_raise`, `second_cues`). Edit has no brand font: Noto Sans, or Noto Sans Arabic for Arabic lines.
  - With `--text` the count is checked against its sentences before the speech step. Else against the sentences of
    the caption words (speech to text or the file), after that step and before the encode: exit 2, both counts.
    Whisper's punctuation decides those sentences, so give `--text` when the count must be sure.
  - Envelope: `captions.second_lines`.
- `dub`: the lines go into the brief as `captions_second`, so `make` checks them (count against the sentences of
  the joined lines, empty entry) before any voice. Needs `--lines`.

| Check | Result |
|---|---|
| `edit` 1080x1920, SRT of 2 English sentences, 2 Arabic lines | exit 0, sheet of 4 frames read by eye: each Arabic line joined, right to left, under its English page, a gap between the boxes; Cap 84 px, Cap2 71 px (Noto Sans Arabic) |
| `edit` 1 line for 2 sentences; an empty line inside | exit 2, no video written |
| `dub` 2 lines for 1 sentence; 1 line with `--plan` | exit 2 "has 2 entries, the script has 1 sentences"; exit 0 |

## End of the line, fixed 9 Nov 2026

`edit` on a 10 s 1280x720 clip, SRT with a 3 s pause between 2 sentences, 2 French lines: the ASS gives Cap and Cap2 the
same times, 0.5 to 2.5 s and 5.5 to 7.5 s. Frames at 1.5, 3.5 and 6.5 s read by eye: both lines, nothing, both lines.

## Not done

A second line timed by its own SRT. A full `dub` render with the second line read by eye: only its checks and
`--plan` ran (the same layout code as `make` draws it).
