# services/gen: your own GPU service for the free `-self` rows

A small queue (`server.py`) that speaks the fal queue protocol, so `scriptvox` calls it like a hosted row. One job at a
time on one GPU. Three images, each with its own engines:

| Image | Dockerfile | `ENGINES` | Rows it serves |
|---|---|---|---|
| video | `Dockerfile.video` | `video` | `wan22-ti2v-5b-self`, `fastwan22-self`, `flux2-klein-4b-self`, `acestep-xl-turbo-self`, `seedvr2-3b-self`, `birefnet-self`, `rife426-self` |
| audio | `Dockerfile.audio` | `audio` | `chatterbox-mtl-self` (voices `chatterbox-ar`, `chatterbox-en`), `whisper-large-v3-self`, `mmaudio-v2a-self`, `mmaudio-t2a-self`, `demucs-self` |
| lip sync | `Dockerfile.lipsync` | `lipsync` | `latentsync-16-self` |

## Card

Every image was run only on one NVIDIA H100 (94 GB). The peak card memory of each engine was not recorded, so a
smaller card is not known to work. Host memory given to each service: about 48 GB (video), 24 GB (audio and lip sync).
`/dev/shm` must be large: the deploy mounted a memory-backed volume; `--shm-size 16g` below is a starting value, not a measured one.

## Weights

The images hold no weights and run with `HF_HUB_OFFLINE=1`. Each engine reads `MODELS_DIR/<folder>`. Download each
folder once on a machine with internet. The deploy took the repo's main branch on the day in the table, with no
pinned revision: pin a commit (`revision=` in `snapshot_download`) when you download, and keep the hash.

Default files from a Hugging Face repo: `*.json`, `*.safetensors`, `*.model`, `tokenizer/*`.

| Folder | Source | Files | Licence (read on) |
|---|---|---|---|
| `wan22-ti2v-5b` | HF `Wan-AI/Wan2.2-TI2V-5B-Diffusers` | default | Apache-2.0 (2 Nov 2026) |
| `fastwan22-ti2v-5b` | HF `FastVideo/FastWan2.2-TI2V-5B-FullAttn-Diffusers` | default | Apache-2.0 (3 Nov 2026) |
| `flux2-klein-4b` | HF `black-forest-labs/FLUX.2-klein-4B` | default | Apache-2.0 (2 Nov 2026) |
| `acestep-xl-turbo` | HF `ACE-Step/acestep-v15-xl-turbo-diffusers` | default | MIT (2 Nov 2026) |
| `seedvr2-3b` | HF `numz/SeedVR2_comfyUI` (base model `ByteDance-Seed/SeedVR2-3B`) | `seedvr2_ema_3b_fp16.safetensors` and the VAE | Apache-2.0 (3 Nov 2026) |
| `birefnet` | HF `ZhengPeng7/BiRefNet` | all files (the model code comes with the weights) | MIT (3 Nov 2026) |
| (in the image) | `hzwer/Practical-RIFE` commit `bbfd2ea`, HF `hzwer/RIFE` `RIFEv4.26_0921.zip` | pinned by sha256 in `Dockerfile.video` | MIT (4 Nov 2026) |
| `chatterbox` | HF `ResembleAI/chatterbox` | all files | MIT (2 Nov 2026); output has an inaudible watermark |
| `faster-whisper-large-v3` | HF `Systran/faster-whisper-large-v3` | all files | MIT |
| `mmaudio` | HF `hkchengrex/MMAudio` plus its release files | `weights/mmaudio_large_44k_v2.pth`, `ext_weights/` (VAE, Synchformer), `hf/` (a Hugging Face cache with the CLIP DFN5B and BigVGAN v2 repos) | code MIT, **weights CC BY-NC 4.0** (3 Nov 2026) |
| `demucs` | `facebookresearch/demucs` release | `htdemucs.yaml`, `955717e8-8726e21a.th` | MIT (3 Nov 2026) |
| `latentsync-16` | HF `ByteDance/LatentSync-1.6` | all checkpoints | code Apache-2.0, weights OpenRAIL++ with use limits, InsightFace `buffalo_l` non-commercial research only (5 Nov 2026) |

Each licence stays with its owner. `scriptvox providers` shows the terms of each row, and you decide.

## Run

Build each image on a machine with the room for it (the images are large):

```sh
docker build -f services/gen/Dockerfile.video -t scriptvox-gen-video services/gen
```

Put a random key in a file (the same value goes into `scriptvox keys set selfhost`), then run one image per GPU.
`PUBLIC_BASE` is the address `scriptvox` reaches, with the path prefix; `PREFIX` is that path.

```sh
docker run -d --gpus '"device=0"' --shm-size 16g -p 8080:8080 \
  -v /srv/scriptvox/key:/key/value:ro -v /srv/scriptvox/models:/models -v /srv/scriptvox/out:/out \
  -e KEY_FILE=/key/value -e MODELS_DIR=/models -e OUT_DIR=/out \
  -e PREFIX=/scriptvox-gen -e PUBLIC_BASE=https://gen.your.host/scriptvox-gen scriptvox-gen-video
docker run -d --gpus '"device=1"' --shm-size 16g -p 8081:8080 \
  -v /srv/scriptvox/key:/key/value:ro -v /srv/scriptvox/models:/models -v /srv/scriptvox/out-audio:/out \
  -e KEY_FILE=/key/value -e MODELS_DIR=/models -e OUT_DIR=/out \
  -e PREFIX=/scriptvox-audio -e PUBLIC_BASE=https://gen.your.host/scriptvox-audio scriptvox-gen-audio
docker run -d --gpus '"device=2"' --shm-size 16g -p 8082:8080 \
  -v /srv/scriptvox/key:/key/value:ro -v /srv/scriptvox/models:/models -v /srv/scriptvox/out-lipsync:/out \
  -e KEY_FILE=/key/value -e MODELS_DIR=/models -e OUT_DIR=/out \
  -e PREFIX=/scriptvox-lipsync -e PUBLIC_BASE=https://gen.your.host/scriptvox-lipsync scriptvox-gen-lipsync
```

Put HTTPS in front (a reverse proxy). `GET <PUBLIC_BASE>/healthz` lists the engines that loaded.

## Point scriptvox at it

The built-in rows use the placeholder host `gen.example.invalid`, and a run stops there with the deploy step. Copy the
row from `registry/generators.toml` into `<config>/generators.toml` (next to `keys.toml`) with the same name, and set
`base` and `hosts` to your host; a row with a known name replaces the built-in one. Then `scriptvox keys set selfhost`. If your proxy uses a private CA, put it at
`<config>/ca/selfhost.pem`. `scriptvox providers test <row>` makes the smallest call.

Not done: a `setup video-local` that runs the open video models on the user's own machine without this service.
