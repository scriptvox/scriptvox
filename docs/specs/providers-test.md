# `scriptvox providers test <row>` (G034, 7 Nov 2026)

A new row or key should be proved with the smallest call, so a user is first with a new model at a small cost. An idea from public docs of Nomi; no code read.

- It runs `generate` on the row: its shortest length, its first aspect, a short prompt, or a small test file from FFmpeg in `.scriptvox/provider-test/` (2 s tone, 1 s 256x256 clip).
- A paid row gives exit 4 with a `plan_id`, as `generate` does. `--approve` and the spend cap work as usual, and the $0 default does not change.
- The result has `test`: `row`, `kind`, `seconds`, `terms`.
- The `scriptvox-providers` skill now points at `generators.toml` in the config folder, not a repo path a stranger does not have.

## Measured, 7 Nov 2026, all rows through a real command

| Row | Result |
|---|---|
| 12 free rows on `-self` (speech, picture, voice, stems, matte, interpolate, 2 sound, music, upscale, 2 clip) | exit 0, $0, 2 s to 216 s, the slowest `wan22-ti2v-5b-self`. The picture read by eye is right |
| `latentsync-16-self` | the row answered "Face not detected": the test clip has no face. So this proves key and row only, and the error says so |
| `kling-2.6-pro` $0.35, `grok-imagine` $0.43, `flux-pro-ultra-fal` $0.06, `scribe-fal` $0.01 | stops with a plan, no call was made |

A first run found a fault, fixed: a video-to-sound row needs a clip, and got none. No paid call ran, so the path with `--approve` is `generate`'s own and was not run here. Only the picture was read by eye; clips and sounds came back with exit 0, and no person watched or listened.
