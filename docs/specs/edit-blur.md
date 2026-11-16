# `edit --blur "x,y,w,h@12-15"` (G068, 8 Nov 2026)

Government footage often shows faces, number plates or screens that must not be readable. Drift, ComeCut and ffmpeg-skill describe a box blur; no code read.

- `--blur "x,y,w,h@from-to"`, repeat for more boxes: pixels of the source picture and seconds of the source. Each box 8 pixels or more each way, inside the picture; the time inside the video, start before end.
- The box is cut out, blurred hard (`boxblur`, radius half the short side, colour planes a quarter, 3 passes) and laid back only for its time. It runs first, on the source's pixels and times, so it works with `--reframe`, `--captions`, `--hook` and the rest. Not with a cut or `--from/--to` (the times would move): blur first, then cut.
- The result has `blur_check`, one frame from the middle of each box's time, and `blur_note`: a blur is not proof of redaction; blurred text or a face can sometimes be read or matched. For a release under a redaction rule, use the agency's own redaction tool.
- A box is fixed: it does not follow a moving face.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| A 17.5 s 1280x720 test pattern, box 0,0,300,80 at 2 to 8 s (the time code) and 500,250,280,220 at 10 to 14 s | 6.7 s, `av_drift` pass, 17.5 s |
| `blur_check` read by eye | at 5 s the time code is a smear; at 12 s the time code is sharp again and the centre box is a smear: each box only in its time |
| A box outside the picture, a time past the end, a box under 8 pixels, 3 numbers, with `--cut-silence` | each exit 2 with the reason |
| First try | FFmpeg refused: the blur radius did not fit the half-size colour planes; fixed with its own colour radius |

## Not done

A box that follows a subject. Pixelation or a solid box as other styles. A check that the blurred text cannot be read.
