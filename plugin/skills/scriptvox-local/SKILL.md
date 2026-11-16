---
name: scriptvox-local
description: Run scriptvox free on the user's own machine - read the card and the disk, tell the download size and the measured time, set up speech, voice and pictures, and say what each machine can do today. Use when the user asks for no key, no cost, no service, "on my GPU", "on my machine", offline, `--local-only`, what their hardware can run, how big a download is, or why a local picture is slow.
---

# scriptvox-local

A local setup costs no money, but it costs disk and time. Tell the user both before you start. Say "unknown" when nobody measured a figure. Do not guess a speed.

## 1. Read, tell, wait for a yes

1. Run `scriptvox doctor --json`. Read `gpus`. An empty list means that no NVIDIA card was seen. It does not mean that the machine has no card: AMD and Apple are not read yet.
2. Run `scriptvox setup image-local --dry-run --json` (or `speech-local`, `speech-large`, `voice-local`, `voice-rust`). This fetches nothing. Read `downloads`, `metrics.download_bytes`, `metrics.disk_free_bytes`, a `disk` finding, and `licence` (for `voice-local` and `voice-rust` the licence text is in `notice`). For pictures, also read `expected`.
3. Tell the user the size, the time figure with where it was measured, and the licence.
4. Wait for a yes. A large download is the user's choice, as with money. Through MCP, `scriptvox_setup` refuses without `--dry-run` or `--yes`.
5. Run the setup. After a network fault, run the same command again: it skips what is done.

## 2. What each setup gives

| Setup | Download | Gives | Limit |
|---|---|---|---|
| `setup speech-local` | about 490 MB, about 880 MB on disk, MIT | transcript, captions from speech, `--cut-fillers`, `check --hear`, on the CPU | Arabic is weaker than English on this model: read the captions |
| `setup speech-large` | about 1.6 GB, MIT | the same work as `speech-local`, with better Arabic | needs 3 GB of free memory; about 3 times slower |
| `setup voice-local` | about 180 MB with packages, has GPL-3.0 parts in a separate process | an English voice with no network | English only |
| `setup voice-rust` | about 332 MB, Apache-2.0 and BSD-2-Clause, no GPL part | the same English voices in pure Rust on the CPU, with `"tts": "kokoro-rust"` | English only. Offer it when the user cannot take GPL parts |
| `setup image-local` | about 5 GB, about 4.8 GB on disk, sd.cpp MIT, FLUX.2 klein 4B and Qwen3 4B Apache-2.0, no token | stills through the row `flux2-klein-4b-local`, and `generate --image` with a reference. `character` ran as a plan only, and `first_frame` did not run | see section 3 |

There is no local clip row yet. A free generated clip needs a self-hosted service (`selfhost` key) today.

## 3. Pictures: what is true per machine, 6 Nov 2026

| Machine | Today |
|---|---|
| CPU | **Works and is slow.** The 1052 build, 2 cores, 4 GB: 446 s at 512x512, 743 s with a reference picture (Linux now pins 1043, not timed). A full-size picture on the CPU did not run |
| Linux, any card, Vulkan | The pinned build needs glibc 2.38 (Ubuntu 24.04, Debian 13 or newer). On an older system `setup` stops before the model files come down. It has not run on a real Vulkan device, so its speed is not measured |
| Linux, NVIDIA | **No CUDA pack exists in this install yet**, because the pack has no host. It gets the Vulkan build above |
| macOS, Windows | A build is pinned. It was not run on these systems |

One more figure exists, and it is not what `setup` installs: an own CUDA build on one H100 took about 44 s at 768x1344, with 3.9 GB on the card. Quote it only with those words. Do not offer it as the time on a home card.

The tier in `expected` compares the largest NVIDIA card with that 3.9 GB. `card` means that the memory may be enough. It gives no time.

## 4. A small machine

Set `SCRIPTVOX_SD_ARGS` for a smaller picture and less memory. Later flags win:

```
SCRIPTVOX_SD_ARGS="-W 512 -H 512 -t 2 --vae-tiling" scriptvox generate --model flux2-klein-4b-local --prompt "a white falcon on a wall at dawn" --json
```

Set `-t` to the number of physical cores. Warn the user that the size changes. One picture runs at a time on a machine, and a second job waits for the first.

When the model runs out of memory, scriptvox stops with its own message and this same next step. With short memory the job can also only get slow, with no error. If a picture takes much longer than the figure above, make the picture smaller.

## 5. No network at all

`--local-only` sends no part of the video, the sound or the text to a service, on every command that makes, edits or checks a video. A step that needs a speech row uses only the local one (`scriptvox setup speech-local`), and stops at exit 10 when it is not installed. `judge`, `--track` and every other step that needs the vision service stop at exit 10. `setup`, `model add`, `connect` and `doctor` still use the network: they download or test a service, and send no media. What still works:

- your own media and the gradient template;
- a still, after `setup image-local`;
- a voice, after `setup voice-local` or `setup voice-rust`, or a video with `--tts none`.

What stops with `needs_input`: a generated clip. Offer `media` or `stock` for that scene, or a run without `--local-only`. In `make`, the listening check (gate `words`) is skipped under `--local-only`.

## 6. Never

- Never ask for a key or a token in chat. No local setup here needs a Hugging Face token. A gated model gives exit 10 with its page: the user clicks Agree there and runs `scriptvox keys set hf`. The user runs `scriptvox keys set <provider>` for a key.
- Never say "works on your GPU" from the tier alone. No GPU has run through `setup image-local`.
- Never choose a paid row because a local picture is slow. Tell the user, and let the user choose.
