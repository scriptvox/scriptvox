# Keyless stock rows (G038)

A first video with real pictures and no key. Rows `commons` (Wikimedia Commons) and `archive` (Internet Archive) in `registry/providers.toml`, `key = ""`, tried after the rows with a key.

- **Terms per file.** `license` names a path to each result's licence. `http_rows::file_terms` reads CC0, public domain and CC BY forms into `Terms`, kept in the recipe's `StockPick.terms`, the cache and the credits file. The note says "as its uploader states": nothing here checks that claim.
- **What is dropped.** No-derivatives (`make` cuts and captions every clip) and a licence the code cannot read (GFDL, "Attribution", an empty field). NC is kept, last in the order, with `commercial = false` and a note, per the owner rule that the user decides.
- **Order.** Rows with no orientation filter sort: any use first, then the frame's shape, then the size.
- **Size.** A search answer has no bytes for a file, so `max_duration = 180` s drops long results. The Internet Archive holds many whole films, so it often finds nothing for a word.
- **New row fields, all optional:** `license`, `file_ext`, `max_duration`, `detail_url` and `file_duration` (Archive files are in a second request, one per result, `rows=8`), `file_base`, `credit_base`, `rank` (Commons does not answer in rank order).
- `search` uses `intitle:` on Commons: plain text matched "Iran country" for "camel".
- **Download faults, in `http_rows::download`, for every caller:** a 429 or a connect or DNS fault waits (`Retry-After`, then 2 to 30 s, 6 tries, about 2 min at most) before it returns a retryable exit. The body limit is now 600 s: 60 s cut a 1080p clip. A `.part` and a project on two disks no longer fail on rename.
- **Measured, 7 Nov, one office address.** Commons search and terms work on the live API. `upload.wikimedia.org` first served curl 200, then answered 429 to scriptvox and curl alike, `Retry-After` 1 to 10, the same with a descriptive user agent. It is a limit on the address, and my test calls used part of it. scriptvox only waits as asked, and no way round the limit was tried. No Commons clip reached a video on this address. A home address was not tried.
- **Real run through `archive` alone:** 3 shots, 110 s, all gates pass, credits per file. The palm clip fits. "camel" found a 1944 cigarette commercial: title search matches the word, not the subject. A word like "city" found nothing inside 180 s. So this row is a last resort, and `judge` or a look at the sheet stays needed.
- Tests: `tests/stock.rs` `keyless_row_*` on a local server (no ND, no unread licence, no long file, HTML stripped from the credit, Archive file name encoded), `http_rows` unit tests.
- Not done: Commons or Archive pictures, audio rows, more than the first 20 and 8 results, a check that a clip shows the word.
- **A refused download moves to the next row.** `download` waits first. If the host still answers an error, `make` writes a `fallback` event, drops that row for this run, and searches the next one. A broken transfer stays as before: a rerun resumes it. A recipe's recorded clip does not fall back.
