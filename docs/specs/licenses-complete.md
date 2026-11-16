# `licenses` lists every part (section 6 row 21, L9)

`licenses` missed the Python packages and most downloaded files, and no GPL text shipped with the Kokoro plugin.

- `LICENSES/GPL-3.0-or-later.txt` (the FSF text) ships in the binary and in the dist. `setup voice-local` writes it as `LICENSE` beside the plugin, and `licenses --full` prints it.
- `python_packages`: one row per package of the worker and the plugins, from `licenses/python-packages.tsv`. `scripts/python-licences.py` generates it from the uv.lock files and the licence PyPI states. A test fails when a locked package has no row. Rows where PyPI states nothing say `not published`: espeakng-loader and kokoro-onnx on this run.
- `downloads`: one row for every pinned file for this machine, from `toolchain.lock`, so weights, tokenizers and voice packs each carry their own licence.

Real run, scratch config: `scriptvox licenses --json` gives 57 Python rows and the pinned files; `--full` holds the GPL text. Not run: a real `setup voice-local` writing the file.
