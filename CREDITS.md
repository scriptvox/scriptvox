# Credits and prior art

ScriptVox stands on the work of others. This file names the projects we learned from and the components we use.

## Prior art (ideas only, no code)

We studied these projects and their issue trackers to learn what users need. No code, prompt or text from them is in ScriptVox.

- **OpenMontage** (AGPL-3.0): agent-driven video production, and the budget and slideshow-risk ideas.
- **ComfyUI** (GPL-3.0): workflows saved inside the output file.
- **Remotion** (source-available): word-level caption highlight and render retries.

## Code and designs reused with attribution

- **MoneyPrinterTurbo** (MIT, harry0703): pause-tag rules, the encoder fallback idea, script-order stock search, Range resume. See `docs/provenance/`.
- **HyperFrames** (Apache-2.0, HeyGen): agent-mode environment variables, the doctor checklist, the plugin layout.
- **Manim** (MIT): render-test ideas.
- **FilmCraft** (MIT OR Apache-2.0, used here as Apache-2.0, the ArtCraft Team and the FilmCraft contributors): the Rust Kokoro voice and its English text front end, at commit 72831a4. See `docs/provenance/kokoro-rust.md`.
- **craft-ai-integrations** (MIT OR Apache-2.0, used here as Apache-2.0, the ArtCraft Team and the craft-ai-integrations contributors): small helpers of the ComfyUI bridge, at commit 29297b2. See `docs/provenance/comfyui-bridge.md`.

## Components

ScriptVox's own code is Apache-2.0. Models and components are used as separate parts, like an API call. Each keeps its own licence unchanged, and ScriptVox claims no ownership of them. A licence is recorded, not a reason to stop. Do not paste AGPL, GPL or source-available code into our tree. Run such a part as a separate component. Terms data, `commercial_ok` and the spend cap stay.

- FFmpeg (GPL-3.0-or-later build by BtbN), downloaded at first use and not mirrored without sources.
- Noto Sans, Noto Sans Arabic, Noto Sans Math and Noto Sans Mono fonts (SIL OFL 1.1), embedded. Rawivox Symbols is cut from Noto Sans Math (OFL 1.1).
- edge-tts (LGPL-3.0, unmodified, in the Python worker), and Microsoft Edge online voices (unofficial, for personal or non-commercial use).
- The Arabic speech model of `setup speech-local --arabic` (version r12-a70, not held in this repository): OpenAI Whisper small (`openai/whisper-small`, Apache-2.0 on its model card, MIT code), fine-tuned on the Meta Omnilingual ASR Corpus (CC BY 4.0), Google FLEURS (CC BY 4.0), LibriSpeech (CC BY 4.0) and Mozilla Common Voice 18 Arabic (CC0).
- uv (MIT or Apache-2.0), harfrust, read-fonts, rusqlite with SQLite, blake3, ureq, clap, serde and the other crates listed by `cargo deny`.
