# ComfyUI bridge: craft-ai-comfyui (MIT OR Apache-2.0, used as Apache-2.0)

Source: https://github.com/storytold/craft-ai-integrations, commit `29297b2fbe11077201edc657bff3315e93841d22` (read 9 Nov 2026 through the API), `crates/ai-comfyui`. The workspace `Cargo.toml` says `MIT OR Apache-2.0`; `LICENSE-APACHE` and `LICENSE-MIT` are in the repo. Its copyright line: "Copyright (c) 2026 ArtCraft Team and the craft-ai-integrations contributors".

Taken into `crates/scriptvox/src/comfy.rs`, changed:

- `safe_name` and `safe_id`: the name and prompt id checks of `output.rs` and `provider.rs`.
- `encode`: the query encoding of `http.rs`, shorter.
- The idea of one explicit binding per request field, and of reading the output through `/view` with its folder type checked.

Written here: role detection, the sizes and frame counts, the runner, `connect --workflow`. Not taken: the crate's provider, library, and SAM code.

ComfyUI itself (GPL-3.0) is a separate program reached over HTTP. No ComfyUI code is copied or bundled.
