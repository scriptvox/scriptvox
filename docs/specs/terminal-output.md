# A person at a terminal gets words (section 6 row 16, E1)

A person in a terminal got raw JSON with no video path, no plan id and no next step. Now the non-JSON path of `emit` prints `Envelope::human_lines`: the error and its fix (stderr), else a result line (`result: <file> · 30.0 s · $0.00 · checks passed`), the plan id, what is needed, failed and warning checks, `next` steps, and the terms note last. `--json`, an agent and a pipe still get exactly the JSON.

Bare `scriptvox` in a terminal prints 3 starters. A topic alone says to write `script.txt`. `next` after `setup` names the command for that target. An error with no fix and no needed input still ends with a next step (`a_terminal_error_always_says_what_to_do_next`).

Real runs on a real pty in a clean environment: bare `scriptvox`, an unknown engine, a topic alone, and a `make` to the end. Not run: Windows console, colours.
