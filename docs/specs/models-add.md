# Add any model by id, file or endpoint (section 6 row 24, E4)

`scriptvox models add <hf id, local file or fal endpoint>` makes one row in the user's `generators.toml`. Before this there was no route: exit 2 and a list of names.

- **Hugging Face id.** `/api/models/<id>?blobs=true` gives the commit, file sizes, sha256 of LFS files, and the licence from the card. The URL is pinned to the commit. A file the host gives no sha256 for is recorded as "observed, not verified". More than one model file: `--file`, with the list and sizes.
- **Local file.** Never copied or moved. The path, size and sha256 are recorded, always "observed, not verified". A file that changes size later stops the row with the reason.
- **fal endpoint.** The kind comes from the endpoint's name, or `--kind`. Price and terms are unknown, so `generate` needs `--max-spend` (row 17). No call is made at add time.
- **Licence.** No tick, flag or question. It stays on the row (`source.licence`, `terms.note`). Only a name that settles it sets `commercial` (`apache-2.0`, `mit`, and `-nc` names); everything else stays not confirmed, and the existing small note applies.
- **Code-running formats.** The first bytes decide, not the file name. Pickle (a zip or a protocol byte) gets a warning finding and is not refused. A pickle is never parsed by scriptvox.
- **Engine.** The family comes from the safetensors header, read with a Range request, or `--family`. sd.cpp runs SD 1.x, 2.x and SDXL from one file. Other families are refused and the error names what is missing. `sd-cli` is fetched alone (38 MB), without the 5 GB of `setup image-local`.
- **MCP.** `scriptvox_models` with `add` asks for `--dry-run` first, then `yes`, as `setup` does.
- **Hint.** The unknown-model error in `generate` and in a brief now says that adding is possible.

Not taken: `storytold/craft-ai-integrations` `ai-comfy-env` was not read. api.github.com did not answer at the start of this run, and the code here is smaller than a port would be.

Ceiling: FLUX, SD3 and Wan need separate VAE and encoder files, which a user row cannot name yet. No fal endpoint check at add time: `providers test` proves a row.

Tests: `tests/models_add.rs` (loopback host, stub engine) and the unit tests in `model_add.rs`. Real run, scratch config: dry runs on real Hugging Face rows: SD 1.5 (revision, 4.3 GB, published sha256, sd1 from the header), a repo with several files, a pickle with no family, an unknown id. Not run: a real model download, a picture from an added row.
