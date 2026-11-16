# Notes: scriptvox-local

Written 6 Nov 2026 before the skill. Full read: Hugging Face `huggingface-local-models` (`SKILL.md`, `references/hardware.md`, Apache-2.0), Ollama `docs/faq.mdx` (MIT), LM Studio `lms load` page (closed, public docs only), HyperFrames `skills/media-use/references/local.md` (Apache-2.0). Ours: `docs/specs/image-local.md`, `speech-local.md`, `setup-dry-run.md`, `research/g006-sdcpp-20261106.md`, `generators.rs`, `genjob.rs`, the `--local-only` branch of `make.rs`, and live `setup --dry-run` output. No text carried over.

## What the sources teach (our words)

- Check the machine before you load a model, and tell the person the result. A size estimate with no load is worth a separate command.
- Tell the person where the model landed after it starts: card, system memory, or a split. A silent fall to the CPU looks like a broken tool.
- The size of a model file is the first filter. Pick the largest quality step that still fits the card, and leave room.
- Thread count follows physical cores, not logical ones.
- A gated repo needs a login and a click on the model page, and that is the person's step.
- A local tool lists what it does on this machine and names the point where the network starts.

## Weak points of the sources

- The Hugging Face skill is about text models and llama.cpp only. Its hardware commands assume that you build from source.
- It gives no time figure and no failure path for a model that does not fit.
- Ollama and LM Studio estimate memory, but not the wait. A stranger asks how long it takes.
- The HyperFrames page lists commands and says nothing about the size of a first download.

## What our skill does better

- It gives the size and the measured time, with where each was measured, before the download. It says "unknown" where nothing was measured.
- It states per machine what ran, including the case with no build (NVIDIA on Linux).
- The person says yes to the size, as with money.
- It ties `--local-only` to what still works, and names the clip as refused.
