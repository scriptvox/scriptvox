# Opt-in local pictures (G008, 6 Nov 2026)

Gap: a free picture needed a self-hosted service. A stranger had none.

- `scriptvox setup image-local [--dry-run]` fetches sd.cpp `master-1052-f5df5f8` (upstream release, MIT) and FLUX.2 klein 4B: the Q4_0 GGUF (2.46 GB), the Qwen3 4B Q4_0 text encoder (2.38 GB) and the VAE (168 MB). About 5 GB, Apache-2.0, not gated, so no token. All are in `toolchain.lock` with the sha256 that GitHub or Hugging Face publishes. The build comes first: if it does not start here, the model files stay on the server.
- The row `flux2-klein-4b-local` is in `generators::all()` only when installed. `providers` gives `image_local`. `plan` shows $0, no network, and a time with its limit: 44 s was measured on one H100 with an own CUDA build (G006), not on this build and not on a home card.
- One `sd-cli` process per picture, 4 steps, `--offload-to-cpu`, one at a time on a machine (a file lock in the data folder). 768x1344, 1344x768 or 1024x1024. `SCRIPTVOX_SD_ARGS` adds flags for a small machine: `-W 512 -H 896 --vae-tiling`.
- Out of memory (killed by signal 9, exit 137, or the words in stderr) stops with its own message and that next step.
- `--local-only` accepts a still after setup, and uses only this row. A clip stays refused. `character` and `first_frame` reach the row through the same list, with `-r` for a reference.
- A zip pin with strip 0 unpacks in the binary (crate `zip`), because GNU tar cannot read a zip.

## Open: Linux NVIDIA

There is no CUDA pin. G006 found that the own CUDA build is the path for NVIDIA on Linux, but its pack (about 915 MB before compression) has no public host, and that is the owner's decision. Until then Linux has the upstream Vulkan build, which needs glibc 2.43 and falls back to the CPU. `setup` says this in its `note`. A later pin goes in `toolchain.lock` or through `SCRIPTVOX_TOOLCHAIN`, with no code change. The CUDA build has not run through this setup path.

## Measured, 6 Nov 2026, WSL, CPU only, `light` (2 cores, 4 GB), real pins

No Vulkan loader exists on this machine, so `--list-devices` shows the CPU only and the workstation GPU stayed out.

| Check | Result |
|---|---|
| `setup image-local` | About 10 min on this network with 2 network faults: a rerun skips what is done. 4.8 GB on disk. It was the Vulkan build, unpacked in the binary |
| `generate`, 512x512, `-t 2 --mmap --vae-tiling`, prompt only | 446 s, exit 0, $0. A white falcon on a wall at dawn, read by eye: sharp and right |
| `generate --image`, the same settings, the falcon as reference | 743 s, exit 0, $0. A white falcon over a valley, read by eye. One picture does not prove how much the reference did |
| `plan --local-only --tts none` with a still, and with a `character` | ok, $0, `network: false`, the line names the row and the limit of its time. Plan only |
| 1024x1024 with a 700 MB limit | No kill: more than 600 s and stopped by my timeout. The out of memory error ran only with a stub. With `--mmap` a low limit is slow first, so this error may not show when memory is short |

`--mmap` is now a fixed flag, because it is how the picture fit in 4 GB. A run without it was not tried. The two pictures are in `night-output/g008-20261106/`.

Not run: any GPU through this path, the macOS and Windows pins, a full-size picture, a `first_frame` through this row, a real `character` picture, a real out of memory kill. Tests: `tests/image_local.rs`, with a stub `sd-cli`. CI does not run the model.
- Update 7 Nov 2026 (audit H3): Linux now pins `master-1043-3be2378`, the newest release with an Ubuntu 24.04 build (needs GLIBC_2.38 and GLIBCXX_3.4.32, read with objdump; Ubuntu 24.04, Debian 13 or newer). Releases 1044 on publish Linux only for Ubuntu 26.04 (glibc 2.43). macOS and Windows stay on 1052. The 9 commits from 1043 to 1052 are speed work and other models, none on FLUX.2. Checked: sha256 matches the GitHub digest, `sd-cli --version` starts through `setup image-local` on this machine. Not run on 1043: a FLUX.2 klein picture (5 GB of models), and a real Ubuntu 24.04 system. Plan when 1043 is too old: build sd-cli in a Ubuntu 22.04 container on a build machine and host the pack, an owner decision like the CUDA pack.
