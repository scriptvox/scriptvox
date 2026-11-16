# SeedVR2 upscale: what we run and under which licence

- Model: SeedVR2 3B by ByteDance-Seed, Apache-2.0 (https://huggingface.co/ByteDance-Seed/SeedVR2-3B). fp16
  safetensors from https://huggingface.co/numz/SeedVR2_comfyUI (Apache-2.0, base model SeedVR2-3B).
- Code: numz/ComfyUI-SeedVR2_VideoUpscaler v2.5.23, Apache-2.0 (https://github.com/numz/ComfyUI-SeedVR2_VideoUpscaler).
  Downloaded unchanged into the scriptvox-gen image at build time (tarball sha256
  1838948a224a3defaf82adbc7e995f7afd3839b7b94cc4f24a0dfd2283b9d4c4) and run as a separate process through its own
  `inference_cli.py`. Not copied into this repo; its LICENSE file ships inside the image at /opt/seedvr2/LICENSE.
- Ours: the engine wrapper in `services/gen/engines_video.py`, the row, `generate --video`, and the brief
  `upscale` step in `make`.

# BiRefNet cut-out

- Model and its code: ZhengPeng7/BiRefNet on Hugging Face, MIT (https://huggingface.co/ZhengPeng7/BiRefNet). The code
  files `birefnet.py` and `BiRefNet_config.py` travel with the weights artifact and are loaded unchanged; not in this
  repo. Ours: the engine wrapper, the `matte` row, `behind` in cards, the overlay graph and the coverage gate.

# RIFE frame interpolation

- Model: RIFE 4.26 by hzwer, MIT. Weights and the network file `IFNet_HDv3.py` from the author's own Hugging Face
  upload `hzwer/RIFE`, file `RIFEv4.26_0921.zip` (sha256 1fa9b9cda3d9b8c3e301359e2595960902f97bf926c08598b0e9957a3f3f760e).
- Code: hzwer/Practical-RIFE at commit bbfd2ea90910789a860ea3e2b32a240cd577b75e, MIT
  (https://github.com/hzwer/Practical-RIFE; tarball sha256 b4c979a966e0070ae513e3757bf4d41baaa53f1ab301d0c9d20c596329e32726),
  for `model/warplayer.py`. Both downloaded unchanged into the scriptvox-gen image at build time under /opt/rife (its
  LICENSE at /opt/rife/LICENSE); not copied into this repo.
- Ours: the engine wrapper `RifeInterpolate` (batching, padding, frame order, encode), the `interpolate` row, and
  the brief `smooth` step plus the frame-rate-aware slow-down in `make`.

# LatentSync lip sync

- Model: LatentSync 1.6 by ByteDance (https://huggingface.co/ByteDance/LatentSync-1.6), weights OpenRAIL++ with use limits. Face models: InsightFace `buffalo_l`, non-commercial research only. VAE: stabilityai/sd-vae-ft-mse, MIT. Whisper tiny inside the repo's weights.
- Code: bytedance/LatentSync, Apache-2.0, commit a229c3948406bc2cf6eaf4873e662e70c6a04746, downloaded unchanged into its own image at build time (tarball sha256 49e285678f27b7ac84136f2e47887284c2572771a3ca29d64df003caeff8d956). Not copied into this repo.
- Ours: the engine wrapper `services/gen/engines_lipsync.py`, the row, `generate --video --audio`.

