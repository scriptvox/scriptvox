# `scriptvox compare a.mp4 b.mp4` (G080, part, 7 Nov 2026)

An agent that changes a video cannot say what changed without reading many frames. Idea from the public docs of Craft Anything and earlier rows of G080; own code, no code read.

Each frame of `b` is scaled to the size and rate of `a`, and FFmpeg's `ssim` filter gives one value per frame. Difference is 1 minus that value. Frames above `--threshold` make stretches, and stretches closer than 0.5 s join.

The result has `same`, `differs` (start, end, peak, at most 50), `stretches`, `metrics` (frames, both lengths, mean and peak) and `not_checked`. A different length or size gives a warning. It exits 0 when videos differ: a difference is an answer here, not a fault.

## Measured, 7 Nov 2026, default threshold from this table

Peak difference on real files. "Talk" is 30 s of the eval talk, which has a person and a fine picture. "Made" is a ScriptVox gradient video.

| Pair | Peak | At default 0.02 |
|---|---|---|
| Made, `replay` of it | 0.000 | same |
| Made, `remake` of it | 0.000 | same |
| Made, re-encode at CRF 23 | 0.002 | same |
| Made, re-encode at CRF 33 | 0.005 | same |
| Talk, re-encode at CRF 33 | 0.017 | same |
| Talk, full-width bar from 10 s to 14 s | 0.029 | 10.0 s to 14.04 s |
| Talk, small tag from 10 s to 14 s | 0.007 | same: **missed**. At 0.005 it shows 10.36 s to 14.04 s, with one false 0.04 s stretch at 7.32 s |
| Made, `edit --speed 1.1` | 0.027 | listed, with a `length` warning |

Frames at 11 s of the talk pair read by eye: the bar shows on one and not on the other.

Limits:
- Picture only. Sound is not compared.
- A small change on a fine picture stays under the default, so give a lower value for that.
- A smooth picture hides change: a whole caption change on the gradient video peaked at only 0.027.
- A cut or a speed change moves all later frames. The tool warns, and it does not line up the 2 videos.
- A number says that frames differ. It does not say which looks better.

Not built from G080: reading check, review strips.
