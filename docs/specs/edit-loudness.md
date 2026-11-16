# `edit` output at -14 LUFS, and the clip factory hand eval

Found in the clip factory hand eval (3 Nov 2026): a clip cut from a quiet talk kept the talk's level, -20.6 LUFS, and
`check` warned. `make` always masters to -14 LUFS; `edit` did not.

- After the encode, `edit` measures the output (`audio::measure`). Outside -14 plus or minus 1 LU, one linear
  `loudnorm` pass on the sound only (picture copied), `-shortest` so the new AAC start delay does not leave the sound
  a frame longer than the picture. `metrics.loudness_lufs` reports the target or the measured level.
- A video with no sound is left as it is.

Measured: the 36.95 s clip (captions, 9:16) -20.6 to -14.9 LUFS on `check`, A/V drift 22.3 ms (one frame is 33.3);
a plain 30 s cut -14.9 LUFS, drift 7.0 ms.

Eval: a 4 min 53 s talk made for the test (Edge narration over a still, no burned captions). `analyze --transcript`
59 s, 40 sentences (12.5 KB of JSON); the skill's rule (start 0.15 s before the first sentence, end 0.2 s after the
last) cut sentences 3 to 7 in 6 s; `check --hear` 115 of 120 words; captions read and end with the last word (sheet).
The slideshow gate failed because the test talk is one still picture, as expected.
