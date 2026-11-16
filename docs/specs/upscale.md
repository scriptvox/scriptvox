# Upscale: SeedVR2 3B on scriptvox-gen (L3-D, first part)

Gap: Wan 2.2 5B makes 704x1280; the final video is 1080x1920, so every generated clip was scaled up by plain
bicubic scaling and looked soft. ComfyUI users fix this with a SeedVR2 node; OpenMontage and MoneyPrinterTurbo have no
upscale step. ScriptVox now has a free upscale row on its own GPU pod.

- Model: SeedVR2 3B (ByteDance-Seed, Apache-2.0), one-step video restoration. fp16 weights from
  `numz/SeedVR2_comfyUI` (Apache-2.0), relayed as `mirror/weights-seedvr2-3b:v1` (DiT sha256 `2fd0e03a...`,
  the same as the numz registry). Code: the inference CLI of `numz/ComfyUI-SeedVR2_VideoUpscaler` v2.5.23
  (Apache-2.0), pinned by sha256 of the release tarball in `Dockerfile.video`, run as a child process so its GPU
  memory goes back when it ends (attribution in `docs/provenance/`).
- Engine `seedvr2-3b/upscale` in `services/gen/engines_video.py`: input `video_url` (data:video/mp4), `resolution`
  (short side 720, 1080, 1440 or 2160; default 1080), `batch_size` (4n+1, default 41), `seed`. Frees the resident
  Wan or FLUX pipeline first. Temporal overlap 4 frames between batches, LAB colour correction to the input.
- Row `seedvr2-3b-self` (kind `upscale`, free, commercial use allowed). `scriptvox generate --model seedvr2-3b-self
  --video clip.mp4`: an mp4 inside the project, at most 28 MB (the pod takes 40 MB bodies; base64 adds a third).
- Brief `"upscale": true`: each generated scene clip goes through the first upscale row with a key after its clip
  check, as shot `scene<n>_up`, so a rerun resumes it. Off by default.
- Image build: `services/gen/deploy/build_image.sh video <tag>`, a kaniko Job on a build machine, the context in a ConfigMap.
- Output keeps the input shape: 704x1280 gives 1080x1962 (Wan 9:16 is 0.55, not 0.5625); `make` crops to cover.
- Measured 3 Nov on the GPU cluster (image `rawivox-gen:20261103a`, sha256 `4804a0b7...`): a 5 s Wan clip (121 frames) in 4 min
  8 s, $0. By eye on a 540 px crop beside bicubic scaling: wood grain, ropes, cabin windows and skyline sharp, no new
  artefacts. Frame-to-frame luma change at the batch joins (frames 41, 82) the same as the source (max 0.35, mean
  0.125 against 0.114). `make` on a 2-scene Arabic brief with `"upscale": true`: both clips upscaled, all 5 gates pass,
  sheet checked by eye (`night-output/upscale/make/`). Cost: one upscale takes about as long as one Wan clip.
- Batch: the row sends `batch_size = 121`, so a 5 s clip is one batch with no joins. Measured on the same clip:
  3 min 38 s against 4 min 9 s with 41, the same detail by eye, luma change mean 0.123. Longer clips run in batches of
  121 with a 4-frame overlap.
- Where the time goes (log of one 121-frame clip, image `20261103c`): loading under 2 s, VAE encode 20 s, DiT 14 s,
  VAE decode 153 s, post 11 s. With `--vae_encode_tiled --vae_decode_tiled` (1024 px tiles, 128 overlap): decode
  52 s, total 102 s. Whole against tiled: PSNR 37.4 dB (min 35.1), no seam on a crop across the tile edges. Image
  `20261103d` uses tiles: a second Wan clip (oasis) in 1 min 58 s end to end (was 3 min 38 s to 4 min 9 s).
- 16:9 (3 Nov): a 1280x704 Wan clip to 1962x1080; sand ripples sharper than bicubic, the camel's coat a little
  smoother than the source (the restoration look); 121 frames.
- Not done: the fp8 DiT; 1:1 clips.
- Tests: the row request (unit); a real upscale of a Wan clip, checked by eye on a crop beside the plain scaling.

## `edit --enhance`: a clip of your own through the upscale row (G122, 11 Nov 2026)

Before: only a clip that ScriptVox generated could go through the upscale row inside `make`; a small or soft clip of the user's own needed `generate --model <row> --video`, which gives back a picture with no sound.

- `scriptvox edit clip.mp4 --enhance`. The picture goes to the first `upscale` row with a key (a free row first, else a paid one) and comes back at the row's size: the short side to the row's `resolution` (1080 on `seedvr2-3b-self`), the long side by the clip's own shape. The source's sound is put beside it again, both streams copied, no encode. The length and the frame count stay.
- The call goes through `generate` (the same code as `generate --model <row> --video`): plan, approval, job on record, the download held to the row's hosts. `edit ... --enhance --plan` shows the cost and, under `enhance`, the size in, the expected size out and the time (`takes_s_about`: 60 s to start and 25 s for each second, from the two measurements below; a busy queue adds its wait). A paid row with no approval is exit 4 and nothing is sent.
- What is sent: the picture alone, with no sound. An H.264, HEVC, AV1 or MPEG-4 picture is copied as it is, so the row sees the source's own frames; another codec is encoded once. The copy is named by the source's bytes, so the plan, its approval and a rerun are the same job: the same clip twice is one call.
- Limits, all exit 2 before any call:
  - Longer than 10 s: the message says so, and the fix line gives the cut (`scriptvox edit clip.mp4 --from 0 --to 10 --out part.mp4`). The limit is modest on purpose: the row is a shared GPU and 10 s holds it for about 5 minutes.
  - A picture over 28 MB (the upload limit of `generate --video`).
  - A clip whose short side is already at the row's size.
  - Any other edit option in the same call ("use two calls"): cut before, so the row gets only what stays; captions and a logo after, so they are drawn at the new size.
- A result with another frame count (more than one frame apart) is exit 1 and nothing is written.
- Gates as in `edit`: a picture stream, `av_drift`, the length. The result has `enhance`: the row, the sizes, `row_seconds`.
- MCP: the `edit` row says it can cost money.

Tests failed first ("unexpected argument '--enhance'", 3 of 3), then passed. `tests/edit_enhance.rs`, with the fake queue of `tests/common/mod.rs` (it answers a 320x180 clip with the same frames at 640x360): the plan names 320x180, about 1920x1080 and a time, with 0 submits; the result is 640x360 (the size the row gave), the same length, the source's sound packets byte for byte, `av_drift` pass; the same clip again is still 1 submit; 12 s, a 1920x1080 clip, `--label`, `--background` and `--reframe` with it are exit 2 with 0 submits; a row at $0.05 a second of source is exit 4 with 0 submits, $0.15 in the plan, and 1 submit with the approval.

Real run on the owner's GPU service, FFmpeg held to 8 GB: a 3 s cut of a 1080p clip of a person at a desk, scaled down to 640x360 first (72 frames, with a tone as sound). `--plan`: $0, 640x360 to about 1920x1080. The run: 1920x1080 out, 72 frames, 3.00 s before and after, the sound stream the same 131 packets, `av_drift` pass, $0, 134 s for the row's call (2 min 14 s for the whole command). By eye on a crop of the face beside the plain bicubic scaling of the same frame: hair strands, skin lines, the ear and the glasses frame are sharp where the scaled one is soft; no new artefact seen in the 4 frames of a sheet. Peak FFmpeg memory under 150 MB (two stream copies and the gate).

Not done, not run: a clip over 3 s through this command; a real paid upscale row; a vertical clip; a variable-rate clip (the frame count check would stop a result that does not fit); a choice of the size (the row's `resolution` is fixed); `--enhance` with another option in one call.
