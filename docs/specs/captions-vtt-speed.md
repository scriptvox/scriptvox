# VTT captions, and captions with `--speed` (G081, 8 Nov 2026)

- Every caption file now comes as WebVTT too: `captions.vtt` beside `captions.srt` in a `make` run and in `edit`, next to the video (`<name>.vtt`), in `export`, and in `artifacts` as `captions_vtt`. Same cues as the SRT, `WEBVTT` header, dots in the times. The web player `<track>` and most platforms read it.
- `edit --speed` (and `--duration`) now go with `--captions`. The captions are heard from the sound after every cut and the speed change, so their times are the result's; the old refusal (from when the hearing came first) is gone.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| `edit --speed 1.5 --captions karaoke` on the 17.5 s talk | 11.67 s, `av_drift` pass; SRT cues at 0.00, 0.42, 3.62, 5.46, 6.92, 9.26 s; a fresh `analyze --transcript` of the result gives the same sentence starts; sheet read by eye: the words on screen match |
| `edit --duration 12 --captions karaoke` on a clean base | 12.0 s, `av_drift` 12 ms pass; `d12c.vtt` beside it |
| The same on my old base with 28 ms of its own drift | `av_drift` failed, as in item 269: the source |
| `webvtt-py` (PyPI, through uvx) reads `d12c.vtt` | cues and times read back |
| A `make` run and its `export` | `tl2.vtt` beside the video, `captions_vtt` in both results |

## Not done

VTT styling (position, colour). Soft captions in the MP4 (G072).
