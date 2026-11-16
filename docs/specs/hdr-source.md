# HDR source (G015, 6 Nov 2026)

A clip tagged HLG or PQ went through `edit` with no word. The output had 8-bit HDR values, an HDR transfer tag and a BT.709 matrix tag, so its colours came out washed out: average saturation 39.5, against 116.7 for the same picture in SDR.

- `Ffmpeg::hdr_kind` reads the transfer tag of the first video stream. A file with no tag counts as SDR, so an untagged HDR clip is not found.
- `Ffmpeg::sdr_source` makes one BT.709 copy in `.scriptvox/hdr/<hash>/<stem>.mp4`. Every step then works on plain SDR. The stem stays, so an output name does not change. Filter: `zscale` to linear, `tonemap=mobius`, `zscale` to BT.709. Sound is re-encoded as AAC 256 kb/s for this copy, because `-c copy` of phone PCM into MP4 can fail.
- `edit`, `clips` and `join` swap the input first, and give a `hdr` finding at `warn`. `make` swaps every video that plays, at the one place before the graph, and gives an `explain` line. `analyze` and `media add` only report: `metrics.hdr` and a finding, and `hdr` per item.
- `clips --plan` makes the copy too, because the step reads the frames. It adds time, not cost.

## Curve

| Curve | Average luma | Average saturation |
|---|---|---|
| Same picture, SDR | 126 | 114 |
| `mobius` (chosen) | 112 | 87 |
| `hable` | 86 | 64 |
| `linear` | 59 | 39 |

These are from the synthetic clip below. The target is not exact, because that clip holds SDR levels inside an HLG container. A real phone clip has highlights this test does not have.

## Measured, 6 Nov 2026

**No real phone clip was on hand. All runs used synthetic clips:** `testsrc2` through `zscale` to HLG or PQ, 10-bit HEVC, made with the pinned FFmpeg, under `light`.

| Run | Result |
|---|---|
| `edit --reframe 1:1`, HLG | Finding `hdr`. Tags BT.709 on all 3. Saturation 39.5 to 88. Side by side read by eye: vivid against washed out |
| `edit --from --to`, PQ | Finding `hdr`, saturation 87 |
| `join` of HLG and SDR | Finding `hdr` once, BT.709, saturation 88 |
| `analyze`, `media add`, HLG | `metrics.hdr` and `hdr` per item |
| `make --media`, HLG, Edge voice | `explain` line, BT.709, sheet read by eye: vivid, captions right |

Two faults came from the real runs and are fixed: a relative copy path did not open in `make`, which runs FFmpeg in the run's folder; and a clip with a rotation read as SDR, because side data changes the ffprobe line. A rotated clip has its own test.

Not run: a real phone clip, Dolby Vision, a real `clips` run (it needs a vision service), `edit` with `--lipsync`. A brief `base_video` passes through the same place in `make`, not run.
