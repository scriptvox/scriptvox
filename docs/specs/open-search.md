# Keyless open image and sound search (section 6 row 22)

`scriptvox media search "<words>" --kind image|sound [--limit n] [--get n]`, through Openverse with no key.

- Each result carries its licence, credit and page, and its own terms through `file_terms`, so one result can be CC0 and the next `by-nc`. A result with no licence is dropped. Nothing is refused on a licence.
- `--get n` saves one result in `scriptvox-out/open-media/`, with `<file>.terms.json` beside it, so the terms travel with the file.
- MCP: `scriptvox_media` with `search`.

## `make` (G144 and G060, 9 Nov 2026)

- Scene `open_image: "<words>"`: a free picture for the scene. Brief `music_search: "<words>"`: a free music bed, instead of `music` and `music_prompt`. No key.
- The first result that downloads and is the right kind of file, at most 5 tried. A picture with a short side under 480 px is skipped. Two scenes with the same words get two files.
- Each file keeps its own licence: a credits.txt line with author, page and licence; the receipt's `stock` list (so `export` counts it and `commercial_ok` goes false on a non-commercial file); the one small terms note. Nothing is refused on a licence.
- A file is kept in the content store under its words and position, so a rerun or a remake of this brief on this machine uses it with no search. On another machine the remake searches again and may get another file: the recipe does not record a result URL, because a recipe can be untrusted and these files live on any host. `--cache-only` with no saved file stops; `--local-only` is refused before any call.

A download gets one try on a 30 s clock, because the next result is on another host. Test: `tests/stock.rs` with a loopback Openverse. Real run, 2 scenes and `music_search`: exit 0, the sheet read by eye shows both Flickr pictures under their captions, credits name each author and licence, a Wikimedia sound as the bed, and a rerun with the search host dead used the same files. Freesound and Jamendo hosts failed here on an expired certificate; they are skipped, the check was not relaxed, and a `music_search` where only those hosts answer ends with exit 5.

Ceiling: one source only, and the top ranked file is taken, not judged for fit.

Tests: unit tests in `open_search.rs`. Real run: one image and one sound search on the real API, one small file saved.

Real run result: image and sound searches gave results with licences that differ per file. `--get` saved one CC BY picture and its terms. A sound `--get` failed on the host's expired TLS certificate; the check was not relaxed.
