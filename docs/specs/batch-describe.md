# `batch --describe`: what a batch brief needs (G115, 10 Nov 2026)

Why: an agent that gets a batch brief has to read it to learn which columns its rows need, and a wrong field name showed only when row 1 failed.

- `scriptvox batch --brief b.json --describe` makes nothing. `--rows` is not needed with it, and both together are a usage error.
- The result has `keys` (the `{{key}}` names in the order they first appear), `needs_a_number_or_true_false` (keys that stand outside a JSON string at least once, such as `"fps": {{fps}}`), `sample_rows` (one row: text keys get `"<key>"`, bare keys get 1), `csv_header`, and `valid_when_filled`.
- `valid_when_filled`: the brief with the sample row is read as a brief. A wrong field name or a wrong value type gives false and a `brief` warning with the reader's message.
- A brief with no key gets a `keys` warning: every row would make the same video.
- A key name is letters, digits, `_` and `-`. `{{ not a key }}` is left alone, as `batch` itself leaves it.

Run on 10 Nov 2026 at home: a brief with 2 text keys (valid, no warning); a brief with a typo in a field name (false, the warning names `scirpt2`); a brief with no key (the `keys` warning); a brief with a bare `{{fps}}`, then a real `batch` of 2 rows from its sample shape with fps 24 and 30: both videos ok. Tests: `placeholders_are_found_in_order_with_their_place` (unit) and `batch_describe_names_the_keys_and_makes_nothing`.

Not done: no check that a rows file matches the keys before the first video (the run still names a missing value per row).
