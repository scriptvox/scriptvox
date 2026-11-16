# Scene `overlay`: a full-frame layer with alpha (G048, 8 Nov 2026)

Motion tools (HyperFrames, Remotion, Manim, EffectCraft) can render a transparent video. Their public docs describe the formats; no code read. This lets such a render sit on one scene of a scriptvox video.

- Scene field `overlay`: a file inside the project, relative to the brief. WebM (VP8 or VP9 with alpha), MOV (ProRes 4444 or Animation), or a PNG, which is held for the scene.
- It fits the frame without stretching (centred), starts with the scene and stops at the scene's end or its own end. It goes over the picture and any `inset`, under captions, cards and the logo. Its own sound is not used.
- WebM keeps its alpha in a side stream that only libvpx decodes, so scriptvox picks `libvpx-vp9` or `libvpx` for those inputs.
- A file with no alpha channel is refused with exit 2 before any voice is made, as it would hide the scene: the check reads the pixel format and the WebM `alpha_mode` tag. Use `inset` for an opaque picture.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| `plan` with an opaque MP4 as overlay | exit 2, "has no alpha channel" |
| `make`, 2 scenes, Kokoro voice, a VP9 alpha WebM with a moving red square on scene 1, an RGBA PNG with a cyan bar on scene 2 | 21 s, 7.4 s video, 7 gates pass |
| Sheet of 6 frames read by eye | the square moves left to right over the gradient on scene 1, the bar holds on scene 2, the gradient shows through, captions on top |

My first test files were fully transparent (FFmpeg `drawbox` on RGBA does not write alpha), so the first sheet showed no layer; the files were at fault, not the code. A file whose alpha is 0 everywhere passes the check and shows nothing; that is not detected.

## Not done

A real Manim, Remotion or HyperFrames render. A MOV ProRes 4444 file (only WebM and PNG were run). An overlay over a whole video, not one scene.
