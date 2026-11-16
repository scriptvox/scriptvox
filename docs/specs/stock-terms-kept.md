# Stock files: every stated licence is kept (owner, 7 Nov 2026)

`http_rows::file_terms` reads one file's licence (Commons, Internet Archive). It drops nothing that has a licence:

| Licence | `commercial` | Note added |
|---|---|---|
| CC0, public domain | true | none |
| CC BY, CC BY-SA | true | credit the author; SA: the finished video takes the same licence |
| CC BY-NC (-SA) | false | not for commercial use |
| any ND form | false | personal use only, do not share an edited copy |
| a licence the code cannot read (GFDL, FAL, "No restrictions", ...) | null | licence not read: check it before you share |
| no licence text | the file is dropped | |

- Clips are ordered by fit only: the frame's shape, then the size. The sort key on non-commercial is gone.
- The note goes into the credits line of the shot. `export` gave `terms_note` when a stock part is false, until 13 Nov 2026 (removed, owner decision); null parts make `commercial_ok` null (a run is not cleared while one file's licence is unread). A stock item with no terms of its own (an old receipt, or a row with general terms) does not count.
- Not changed: the Archive search asks for CC or public domain items (`providers.toml`).

Test: `export::tests::an_unread_stock_licence_is_not_cleared`, `http_rows::tests::file_terms_read_each_licence_form`, `tests/stock.rs` `keyless_row_commons_needs_no_key_and_keeps_terms_per_file` (ND first by rank, with its note; NC and GFDL kept; the 15 min file still skipped).
