# Keyless stock: User-Agent, IPv4 retry, reasons, `providers test` (audit H4)

- User-Agent on every stock search and download: `scriptvox/<version> (+https://github.com/scriptvox/scriptvox)`. Wikimedia asks clients to say where to reach them.
- A search with a connect fault (no HTTP status) is tried once more with IPv4 only; from then on the process connects over IPv4. A download's connect fault also turns IPv4 on before its next try. Cause seen in the audit: an AAAA record and no IPv6 route ("Network is unreachable").
- When every row fails for a term, the error lists each row's reason, joined by "; " (with a refused download first).
- `scriptvox providers test <stock row>` (pexels, pixabay, commons, archive): one search for "ocean", no download, $0. It gives the result count and the first file's page and terms. `--plan` says what it would call.
- Credits decode the common HTML entities from Commons authors (`&amp;` and others).

Measured 7 Nov 2026, no key, this machine: `providers test commons` 16 results (first: public domain), `providers test archive` 6. `make --brief recipes/stock-short.json` exit 0 in 45 s on 2 cores: 3 Commons shots (2 public domain, 1 CC BY-SA), no fallback and no 429 in the events, contact sheet checked by eye. The 429 of the audit run did not come back; its cause is not proven.

Tests: `tests/stock.rs` `every_row_reason_is_in_the_final_error`, `http_rows::tests::strip_tags_keeps_text_and_decodes_entities`.
