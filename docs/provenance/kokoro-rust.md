# Rust Kokoro voice: FilmCraft (MIT OR Apache-2.0, used as Apache-2.0)

Source: https://github.com/storytold/filmcraft, commit `72831a4dc2ad79f43235bf0c175c9a58afdd67f0` (read 9 Nov 2026 through the API). LICENSE-APACHE and LICENSE-MIT are in the repo, the workspace `Cargo.toml` says `license = "MIT OR Apache-2.0"`, and NOTICE says the same. We use the Apache-2.0 option.

Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. The ArtCraft name and logos are outside the licence and are not used by this crate.

Taken into `crates/scriptvox-kokoro/src`, changed:

- `crates/tts/src/kokoro.rs` and `kokoro/{dsp,layers,model,weights}.rs`: paths for the model files instead of a package folder, the pinned weights hash moved into the file. Their tests that need the model removed.
- `crates/tts-text/src/normalize.rs`, `phonemize.rs`: unchanged rules, product names and task numbers removed from comments, upstream test files not taken.
- `crates/tts/src/neural.rs` (the chunk loop) rewritten as `Engine::speak` in `lib.rs`, without the pitch shifter.

Not taken: `crates/audio-dsp` (only the pitch shifter used it), the catalogue and downloader (ScriptVox uses its own `toolchain.lock` pins), the formant voices, `crates/speech`. No git dependency, no build run from the source.

Model files (never bundled): Kokoro-82M by hexgrad, Apache-2.0 on its model card, at revision f3ff357; CMUdict, Carnegie Mellon University, BSD-2-Clause, at commit 7479086. Terms are on the `kokoro-rust` voice rows.

The GPL Kokoro Python plugin stays as it is. This engine runs under the new name `kokoro-rust`.
