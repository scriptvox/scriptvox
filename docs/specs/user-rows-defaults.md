# A user's own row needs no proof (section 6 row 17, L2)

A new row in the user's `generators.toml` was refused until it held licence text and a price page. Registry rows stay strict; a new user row needs only `name`, `gateway`, `kind` and `t2v` or `i2v`, reported together if missing. The rest gets defaults: `price = "unknown"`, terms "not checked" (`commercial` empty), hosts from `base`.

An unknown price is never free: `paid()` is true, the plan and `providers` say "price not known", `generate` needs `--max-spend` and the plan quotes that cap (so `--approve` works as for any paid row), and no automatic plan picks the row or takes it as a scene's `model`. The spend cap and approval do not change.

New: `scriptvox providers template <gateway>` prints a block, `scriptvox providers add <name> --gateway g --kind k [--t2v e] [--i2v e] [--base u]` checks a row and appends it; a wrong row writes nothing and names every problem at once. Both go through the `scriptvox_providers` tool.

Tests: `a_new_users_row_gets_defaults_and_its_missing_fields_named_together`, `built_in_rows_never_have_an_unknown_price`. Real run on an empty config: template, a missing-field refusal, an add, then `generate --plan` refused without `--max-spend` and planned $2.00 with it. Not run: a real provider call.
