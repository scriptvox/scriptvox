# `--fit blur` (G028, 7 Nov 2026)

A wide video in 9:16 loses its sides in a crop, and bars look empty. A common way (ffmpeg-skill docs; no code read) is the whole picture over a blurred copy.

- `edit --reframe 9:16 --fit blur`, and `join --fit blur`. `media::fit_blur` is the one graph.
- Nothing is cut, so a ScriptVox MP4 with burned captions does not refuse, and the crop warnings do not run. It refuses `--track`, which has nothing to follow. `--fit` with no other edit gives "nothing to do".
- The blur runs at a quarter of the size, to cost less.

## Measured, 7 Nov 2026, 2 cores

| Check | Result |
|---|---|
| 1280x720 talk, 20 s, to 9:16, `--captions` | 18 s, 1080x1920, 502 frames as the source, `av_drift` 0.0 ms |
| `join`, a wide and a tall clip, 1:1 | 5.76 s, 1080x1080, -14.0 LUFS |
| 3 frames read by eye | whole picture in the middle, blur fills the rest, captions readable on the blur |

Not run: a real phone clip. Captions sit where `edit` puts them for 9:16, on the blur here, and a different source shape can put them on the picture.
