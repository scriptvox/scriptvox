# `scriptvox models scan` (section 6 row 6, G077)

Files a setup target would download may already be on the machine (a Hugging Face cache, a folder of models). `scriptvox models scan [folder]` walks a folder (default: the Hugging Face cache) to 8 levels, skips symlinks, and for each file with the exact size of a raw model pin computes the sha256. A match goes into `models-found.json` in the data folder: the pin name and the path. ScriptVox only reads the file; it never copies, moves or deletes it.

`provision::installed` counts a found file with the pin's size as installed, so every setup and `--dry-run` skips it. When a model is linked into a plugin folder, the pin's own file name is used, because a cache blob has another. A size match with another hash is not recorded.

Test: `models_scan_records_a_matching_file_and_skips_the_rest`. Real run: a copy of the real faster-whisper-small config in a folder: `models scan` found it by hash, and `setup speech-local --dry-run` went from 4 files to 3. Not run: a real multi-GB model through a full `setup`, the default cache on Windows.

Same command and MCP tool `scriptvox_models` (args `{"subcommand":"scan","args":["<folder>"]}`).
