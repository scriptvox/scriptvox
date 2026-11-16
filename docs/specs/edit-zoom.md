# `edit --zoom` and `--zoom-on-emphasis` (G026, 7 Nov 2026)

A still talk gets slow pushes in. Ideas from public docs of Captions, Drift and OpenChatCut; no code read.

- `--zoom "12-15@1.2"` or `"12-15@1.2,0.3,0.4"`, repeat for more: seconds of the result, zoom 1.05 to 2, centre as shares of the frame. In order, no overlap, each 0.8 s or more, inside the video. Exit 2 otherwise.
- `--zoom-on-emphasis` with `--emphasis` and `--captions`: 1.12x from 0.3 s before each word to 0.5 s after, at least 0.9 s from its start. Words within 0.3 s share one push. It takes the caption's times, so it follows a cut.
- Each push eases in and out over up to 0.4 s, by smoothstep. One `zoompan` on a 2x frame, as `media::camera` does. The centre stays on the frame when it is near an edge.
- Filter order: after cuts and crop, before captions and logo. So a new caption or logo stays still and is never hidden.
- `--zoom` times are seconds of the result, so it refuses a cut or `--speed`: cut first. A source with burned text gets warning `base_text`, because a push moves it and can cut it off.
- `zoom` in the envelope lists each push.

## Measured, 7 Nov 2026

| Check | Result |
|---|---|
| 16.83 s clip, 2 pushes, one off centre | exit 0, 505 frames as before, `av_drift` 23.3 ms, same as the source |
| Sheet read by eye | Off-centre push on the burned captions: they are larger and cut at the side, so the warning is right. |
| Test pattern talk, `--zoom-on-emphasis` on "cities, third" | 1 push, 1.9 s. 4 frames read by eye: picture grows and returns, captions the same size and place, emphasis words larger |
| With `--cut-silence` | exit 2 with the reason |

Not run: smoothness in motion (a still sheet cannot show it, and no person watched), Arabic, `--zoom` with `--reframe` or `--logo`.
