# Say free, cheapest, best, or a language name (section 6 row 18, E6)

`generate --model` is now optional and takes `free`, `cheapest` or `best` (default `cheapest`, kind `video`, or `--kind`). It picks among rows that can run now, prints the row and why (`row_choice`), and goes through the same plan and approval. `best` means the highest price among rows that can run: scriptvox has no quality score, and it says so. A wrong name gets the 3 nearest names (of the same kind with `--kind`).

`providers --kind video --ready --free` filters the generator rows. `--lang Arabic` or `--lang العربية` becomes `ar` at plan time in `make` and `voices`; the small table covers English, Arabic, French, Spanish, Hindi and Urdu. No voice for a language now names the languages that exist.

Tests: `a_word_picks_a_row_and_says_why`, `a_language_name_becomes_its_code`. Real runs: `generate --model` free, cheapest and best planned on the real rows at no call; a misspelt name got 3 nearest; `--lang العربية` gave the Arabic voice; `klingon` named `ar, en`. Not run: `generate` with a real provider call.

`providers --kind` refuses a value that no row kind starts with, with a usage error that lists the kinds (audit contract-9).
