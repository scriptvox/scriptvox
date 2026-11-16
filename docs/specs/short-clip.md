# Short clip: slow down, then hold

Source: radar HF#6974 (HyperFrames moved a short clip from loop to hold). ScriptVox already held the last frame; this
adds the step before the hold.

- `media::graph` gets each video item's length (`ffprobe` through `Ffmpeg::duration`, in `make::clip_lens`).
- A clip shorter than its shot but at least 70 percent of it (`SLOW_MIN`) plays slower: `setpts=(shot/clip)*PTS`
  before the scale. At most about 1.43x slower, which still reads as motion.
- A clip under 70 percent holds its last frame as before (`tpad stop_mode=clone`). `tpad` stays on every clip as the
  guard for rounding.
- Applies to brief scenes, own media, stock and `base_video` alike. A length that cannot be probed means no slow-down.

Measured 3 Nov 2026: one scene of 3.5 s on a 3.2 s test clip (91 percent). Old build: the last 0.3 s is the clip's
last frame (clock 3.167 at the end). New build: the clip still moves at the end (clock 3.000, factor 1.094). All
gates pass on both. Test: `media::tests::short_clip_slows_or_holds`.
