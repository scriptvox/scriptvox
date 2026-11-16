# `scriptvox dub` (G019, 6 Nov 2026)

Dubbing tools (studied by behaviour and public docs: pyVideoTrans, ComeCut, HeyGen, Adobe) give one step from a video to its dub. No code was read for this. The agent writes the words: there is no text model inside.

- `scriptvox dub <video> --plan`: the source's sentences from `analyze::transcribe`, as `lines` in the envelope and in `.scriptvox/dub/<stem>-lines.json`. Each has `start`, `end`, `text` and `room_s`. It costs nothing with a free speech row. With only a paid one it asks for `--approve` first (`paid-speech-rows.md`).
- `scriptvox dub <video> --lines ar.json --lang ar [--plan] [--tts] [--voice] [--base-sound] [--lipsync] [--out]`: writes one brief with the `lines` field of `dub-lines.md` and runs `make` on it, so there is one plan, one cost and one `--approve`. Fit, budgets and gates are those of `dub-lines.md`.
- `--base-sound` defaults to `background` when a free stem row has a key, else `none`.
- Frame: the nearest shape within 5 percent, because video models give shapes such as 1280x704. Further off: exit 2, with `edit --reframe` as the fix. `make` renders that frame, then `finish` scales and crops back to the source size. Up to 5 percent of the edges can go. Gate `source_size` fails with exit 3 when the sizes differ.
- One line again: the voice cache holds each line by text, so a changed line is the only one read again. The picture is rendered again.
- `--lipsync`: only a free row. A paid one would need a second plan, so `dub` says no row instead. The row returns the voice alone, so the full mix goes back on its picture. It runs after the captions are burned in, so it works on the captioned picture.

## Measured, 6 Nov 2026

The eval's 30 s promo, 1280x704, Arabic lines, Edge `ar-AE-FatimaNeural`:

| Check | Result |
|---|---|
| `--plan` sheet | 2.8 s, 6 lines, cached transcript from an earlier run of this source |
| `--lines --plan` | exit 0, $0 |
| Full run with `background` | 45 s, all 9 gates pass, output 1280x704 |
| A changed line 44 characters long | 2.4 s, exit 3, `budget_chars` 42: the budget works, and the 5 other lines came from the cache |
| A changed line that fits | 44.5 s, all gates pass, line 5 runs 3.2 s |
| Full run with `--lipsync` on `latentsync-16-self`, default `background` | 9 min 50 s, $0, all 9 gates pass, 1280x704, 30.00 s, sheet read by eye: captions and face are right |
| `--lines` file with an empty list, or with no list | exit 2, "has no lines" |

Measured faults, not fixed: `plan` warned that line 3 runs at 1.36x, and the real rate was +18 percent with 0.189 s borrowed. The warning is too high on this line.

## Not done

- A person did not listen. A person did not watch the mouths: a still sheet cannot show that they match the words.
- The picture slowdown of G018.
- `--detach`: `dub` runs in the foreground.
