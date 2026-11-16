# `make --draft` and `resume --final` (5 Nov 2026)

Gap: Manim `-ql`, Remotion `--scale` and HyperFrames previews give a fast look before the full render. ScriptVox rendered in full each time.

- `make --draft`: half the size (1080x1920 to 540x960; 4:5 is 540x676), 15 fps. Same words, voice, captions, cards and clips. No upscale, no smoothing, no listening (gate `words`). Generated clips are made as usual: their size follows the aspect, not the draft, so `--final` gets them free from the cache. They still cost what the plan says.
- Result: metric `draft: true`, `next` names `resume <run> --final`. `export` refuses a draft by run or by file, also after an `edit` of it (the recipe stays in the MP4). A generated bed is kept in the content store, so the final has the same music. A paid voice is priced again at `--final` though its cache makes no call: the known limit of the plan.
- `resume <run> --final` makes the child run in full size, with upscale and smoothing as the brief asks.
- Not exact: the plain background moves by frame, so at 15 fps it moves at half speed. Layout does not change.
- Also: `make` now listens to `master.wav`, not the video. The transcript cache is keyed on the file, so a change to the picture alone does not send the sound again.

Measured (84 s video, 8 scenes, title cards, Edge voice cached): full make 71 s (render 25 s, listening 25 s); draft 8.8 s; `--final` after the draft 43 s; the same again with the sound heard already 29 s. Wide brief (clip, still with push, bars) and Arabic 4:5 with pop captions and a list: sheets of draft and final side by side have the same layout, checked by eye.
