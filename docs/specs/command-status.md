# Command status table (G162, 7 Nov 2026)

The prose in `GAPS.md` and `STATUS.md` says what ran, but nobody can read the state of one command from it. Idea from the public docs of Craft Anything (`docs/plan/research/discovery-craft-suite-20261107.md`); own text, no code read.

`docs/command-status.md`: one row per command, with state, date and how it was checked. A test in `tests/docs.rs` compares the rows with the clap tree, so a new command needs a row and a removed command loses its row. The test checks the form, and it cannot check that a row is true.

## Measured, 7 Nov 2026

One run of each command, 2 cores with `light`, a scratch project, a scratch config with no keys. 31 of the 35 commands ran. All ended as promised, with these notes:

- `make --wait` does not exist, so my first line failed at exit 2. `make --detach`, then `wait`, is the real flow.
- `resume` failed once, exit 5, on a connection fault to Edge TTS, then passed on the same command. A network fault also hit `gh` the same morning.
- `uninstall --purge` ran only with `SCRIPTVOX_DATA` on an empty scratch folder.
- `judge`, `dub`, `clips` and `mcp` did not run in the sweep. Their rows name their older runs.

Sheet of the run read by eye: all 4 frames show the moving gradient and the right caption.
