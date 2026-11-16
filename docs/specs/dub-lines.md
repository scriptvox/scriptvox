# Dub lines on the source's times (G018, 6 Nov 2026)

`fit.rs` fits only the whole narration, so one long sentence moves every later line away from its picture. Dubbing tools (studied by behaviour and public docs: pyVideoTrans, VoiceStudio, ElevenLabs, Synthesia) keep each line on its source time. No code was read for this.

## Brief

`lines: [{"start": s, "end": s, "text": "..."}]` in place of `script`, `script_file` and `scenes`. It needs `base_video`, and it cannot go with `narration`. `duration` defaults to the length of the video. Lines must be in time order, without overlap, inside the video.

## Fit, per line

1. Read each line once and measure its speech (the last word's time, not the file length: a voice file ends in silence).
2. Up to 3 percent over stays as it is. Faster up to `LINE_RATE` 1.10 if that fits the line's own time.
3. Else borrow the gap, up to 0.05 s before the next line.
4. Else go faster, up to `LINE_MAX` 1.25.
5. Past that: exit 3, `SVX_LINES_TOO_LONG`, nothing rendered, and `line_budgets` holds `budget_chars` for every line over. A second read is not paid for until all lines fit.

A voice with a rate (Edge, ElevenLabs) reads the line again at that speed, and `atempo` takes what is left. The open voice takes only `atempo`. `plan` gives the same check from an estimate, as warning `line_fit`. `metrics.lines` reports `ran_s`, `borrowed_s`, `rate_pct` and `tempo`. Both limits are constants in `fit.rs`, set by judgement, not by a listening test.

## Measured, 6 Nov 2026

The eval's 30 s English promo with its 6 sentence times, Arabic lines, Edge `ar-AE-FatimaNeural`, `base_sound: background`:

| Check | Result |
|---|---|
| All gates | pass; `words` 37 of 39 |
| Line starts in the voice track (`silencedetect`) | each 0.19 to 0.21 s after its `start`, which is the voice's own lead-in |
| Line 3, 3.24 s | rate +15 percent, borrowed 0.208 s, ended 0.05 s before line 4 |
| A line of 157 characters in a 3.24 s line | exit 3, `speed_needed` 5.5, `budget_chars` 35; `plan` warned first with 37 |

Sheet read by eye: the Arabic captions join and read right to left, with the words of the line at that time. Listening was not done by a person.

## The plan's estimate (fault of 6 Nov, fixed 7 Nov)

The plan warned 1.36x on line 3 of the eval promo, and the real read needed +18 percent. The whole-script count rounded 31 letters to 7 words, and a whole-script rate holds pauses that a single line does not. `fit::line_seconds` keeps the fraction, and Arabic on a voice with a rate reads 7.3 letters a second, the slow end of 12 real lines (7.3 to 9.1).

| Check, Edge | Result |
|---|---|
| The eval's Arabic lines, `plan` | no warning, real run needed 1.18x at most |
| Line 3 at 62 characters | `plan` 2.00x, budget 38; `make` exit 3 with 1.89x, budget 41 |
| 6 English lines of 9 to 18 words | no warning, real run needed no rate; 2.7 to 3.9 words a second, so 3.3 stays |

The estimate stays a little high on purpose: a warning is cheap, and `make` measures. Only one video and voice were run. Paid voices and the open voice were not.

## Not done

- The row's last step, a small slowdown of the picture. It moves every later time, so it waits for a real dub that needs it.
- A paid voice and the open voice: Edge only. The open voice path is `atempo` and did not run on a real line.
- Lip sync with lines, and `tts: none` on real text beyond the test.
