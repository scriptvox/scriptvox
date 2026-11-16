# `edit --keep`: cut a talk by its transcript

Source: HyperFrames cherry #24 (cut by transcript; their `transcript-cut` script). The edit skill said it did not
exist.

- `--keep "a-b,c-d"`: parts in seconds, in time order (a later part cannot come first), not overlapping, each at least 0.3 s, all inside the video (exit 2
  otherwise, with the reason). Same select and aselect path as `--cut-silence`, so one encode; captions come from a
  sound-only pass with the same parts, so they follow the cut. Not with `--from/--to` or `--cut-silence`.
- Found with it: the loudness pass of `edit` left the AAC at 96 kHz (`loudnorm` works at 192 kHz inside); it now
  resamples to 48 kHz like `make`.

Measured 3 Nov 2026 on a 52 s talk of 12 distinct sentences (Edge voice over a still): sentences 1, 5 and 12 kept,
12.9 s, words 40 of 40, -14.7 LUFS, drift 19.3 ms, captions follow the kept parts (sheet). Test
`edit::tests::keep_parts_parse_in_order`.

Test-data lesson: a talk made of one sentence said 40 times is not a valid speech test. Whisper rejects repeated
text (compression check) and falls back to random decoding, so the same audio gave 115 of 120 words once and 23 of
120 later. Use distinct sentences.
