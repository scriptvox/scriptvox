# Key names as data (G137)

A row in the config overlay can name a new provider key. The key table in `keys.rs` was fixed, so a row with another
key was refused, and `keys set` refused the name.

## How to declare a key

Add one block to `generators.toml` in the config folder (or to the `SCRIPTVOX_GENERATORS` file):

    [[key]]
    name = "acme"
    env = "ACME_API_KEY"

The user then runs `scriptvox keys set acme`. `keys list`, `doctor` and `providers` show the name and the variable. They
do not show the value.

## Rules

- `name`: 1 to 32 of a-z, 0-9, `_`, `-`. A built-in name is refused.
- `env`: upper case, A-Z, 0-9, `_`, 64 at most, first a letter. It ends in KEY, TOKEN or SECRET. It does not start with
  `LD_` or `DYLD_`. A variable of a built-in key is refused. So PATH and HOME can never become a key.
- A name or variable used twice is refused.
- A bad block is never used. `providers --json` gives one line in `generator_errors` that names the file and the fix.
- `keys set` stays the only way to store a value. A value in the environment works as for a built-in key.

## Redaction

`Redactor::from_env` reads `keys::all_values`, which goes through the built-in keys and the declared keys at run time.
The value in the variable and the value in `keys.toml` are both replaced. The voice worker has each declared variable
removed from its environment, as for a built-in key.

## Limit

A declared key gives no runner. A row still needs a gateway that has a runner (G138 adds them). The key goes to the
gateway of the row, so the user who writes the row also chooses where the key goes.
