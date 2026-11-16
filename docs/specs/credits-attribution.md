# Rows that need attribution reach credits.txt (section 6 row 7, L6)

Export copied the run's credits file, which only stock clips wrote. A generator row with `attribution_required = true` in its terms was named in `metadata.json` but never in credits.

`export::attribution_lines` gives one line per such part, in the row's own words, added to `credits.txt` in the export folder, and the file becomes an artifact even when the run had none.

Test: `a_row_asking_attribution_gets_a_credit_line`. Real run: a real run with a hand-edited receipt naming `mmaudio-v2a-self` gave "Made with mmaudio-v2a-self: MMAudio (hkchengrex): ...". Not run: a real GPU row through `make`.
