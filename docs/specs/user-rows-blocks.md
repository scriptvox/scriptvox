# The user's generators.toml, block by block (E2, section 6 row 2, 9 Nov 2026)

Before: one mistake anywhere in the file made `load` fail, so `all()` had no rows at all. The printed deploy fix ("add a row named X with your host as `base`") caused exactly that, because a two-field block was not a whole row.

- The file is cut at each line that opens `[[...]]` and each block is parsed alone. A `[[generator]]` block with a mistake gives one line and costs only itself. The `[[key]]` blocks are read the same way, so a typo in a row does not lose a key.
- The built-in rows always stay. A block with a known name changes only the fields it sets, so `name` and `base` are enough.
- When the merged row fails `validate`, the built-in row stays listed.
- `--model X`, `providers test X` and a scene's `model` naming a row with a problem give exit 2 with that problem and the file. scriptvox never quietly runs another row than the one the user wrote.
- `generator_errors`, a `generators` check in `doctor`, and a `generators` warning in `plan` and `generate --plan` list the problems. A clean file adds nothing.
- `SCRIPTVOX_GENERATORS` (tests, mirrors) stays a replacement of the built-in rows, also block by block.

Not done: all missing fields of a block at once (section 6 row 17). Serde names the first one.

Test: `generators::tests::one_bad_block_costs_only_itself_and_a_known_name_merges`.
