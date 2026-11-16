# `edit --broll`: pictures over a talk (G027, part, 7 Nov 2026)

A talk needs pictures over it, with the talk's sound under them. Public docs of Descript, OpusClip and OpenChatCut describe this; no code read.

- `--broll road.mp4@12.0:4`, repeat for more: the file, seconds of the result, how long. In order, no overlap, each 0.5 s or more, inside the video. A clip or a still (png, jpg, webp).
- It fills the frame by a crop, and nothing stretches. A clip shorter than its time holds its last frame. Its own sound is not used.
- Filter order: the talk's filters, then the files, then a logo and captions. So captions stay on top.
- The times are seconds of the result, so it refuses a cut or `--speed`: cut first. It works with `--zoom`, whose push is under the file.
- A paid clip comes from `scriptvox generate` first, which has its own plan and approval. Then give the file here.

## Measured, 7 Nov 2026, the eval's test pattern talk, first 20 s, 2 cores

| Check | Result |
|---|---|
| A 2 s clip at 6 s for 4 s, a tall still at 12 s for 3 s, `--captions` | exit 0, 502 frames as the source, `av_drift` 0.0 ms, 20.08 s |
| 6 frames read by eye at 5, 7, 9.5, 10.5, 13, 16 s | the talk at 5, 10.5 and 16 s; the clip at 7 s and held at 9.5 s; the still at 13 s, cropped to fill; captions on top with the talk's words each time |

## Not done

The row's brief field `cutaways`: one plan for a base video with generated clips. Today an agent runs `generate`, then `edit --broll`, and that has two plans. A logo with `--broll`, a crossfade, Arabic, and a real generated clip were not run.
