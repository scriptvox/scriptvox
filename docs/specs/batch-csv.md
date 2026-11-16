# `batch --rows rows.csv --jobs 2` (G076, 8 Nov 2026)

- `--rows` takes a CSV file too (by its `.csv` name): the first line names the columns, quoted fields may hold commas, line breaks and `""`; a line with another number of fields is exit 2 with its line number.
- A row is skipped when its video exists from the same filled brief: after a row finishes ok, `<video>.brief-hash` holds the blake3 of its filled brief. A changed row, or a deleted video, makes it again. Skipped rows show `status: skipped` and their video.
- `--jobs 1` (default) or `2`: that many rows at once, each its own `make` process. More is refused: the load limit of a shared machine. The order of `rows` in the result stays the row order.
- The row limit is now 200 (was 50).

## Measured, 8 Nov 2026, 2 cores (`light`)

| Check | Result |
|---|---|
| 2 CSV rows (one with a quoted comma), Kokoro, `--jobs 2` | 59 s, both exit 0; a frame of each read by eye: "Welcome to Al Ain, the garden city." and "Wide dunes and a long coast." |
| The same command again | 0 s, both `skipped` |
| `--jobs 4` | exit 2, "--jobs is 1 or 2" |
| Unit test | quotes, `""`, CRLF, blank lines; a short line and an open quote refused |

## Not done

Skip by a key column the user names (the row number and the brief decide today). More jobs on a machine of one's own.
