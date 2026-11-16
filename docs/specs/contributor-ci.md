# Contributor CI

A contributor runs `just ci` on a fresh clone with no owner tools.

- Cargo: `cargo := env("SCRIPTVOX_CARGO", "cargo")`. The justfile loads a local `.env` (`set dotenv-load`, ignored by git), so a shared machine sets `SCRIPTVOX_CARGO="light cargo"` there or in the shell.
- FFmpeg for tests: `SCRIPTVOX_FFMPEG`, else the newest `<data>/tools/ffmpeg/*/bin/ffmpeg` that `scriptvox setup` installs (`<data>` is `SCRIPTVOX_DATA`, else `$XDG_DATA_HOME/scriptvox`, else `~/.local/share/scriptvox`). When neither exists, the tests that need FFmpeg are skipped.
- Hooks: `hooks/commit-msg` and `hooks/pre-commit` are public. `hooks/pre-push` refuses every push and is local only: `scripts/public-tree.sh` leaves it out.

Checked: `just --evaluate cargo` gives `cargo` with no `.env`, `light cargo` with the owner's `.env`, the value of `SCRIPTVOX_CARGO` when set; `SCRIPTVOX_FFMPEG` resolves to the setup path; public-tree.sh output has no `hooks/pre-push`.

## One result with and without keys (13 Nov 2026)

Fault: four tests failed on a machine with keys or a local voice, and passed in CI. Causes found: a `FAL_KEY` in the shell gave `plan` an image row; a `tts` row in the user's `generators.toml` and the files of `setup voice-rust` in the user's data folder changed what `voices::choose` returns.

Rule: no test reads a key of the shell, the user's config folder or the user's data folder.

- A test starts the binary through `common::bin()`, `common::std_bin()`, or `common::clean(&mut command)` for a shell that calls it (`crates/scriptvox/tests/common/mod.rs`). They remove every key variable (the names of `keys::PROVIDERS` and the `[[key]]` names of the user's `generators.toml`) and `SCRIPTVOX_GENERATORS`, and set `SCRIPTVOX_CONFIG` and `SCRIPTVOX_DATA` to `test-config` and `test-data` in the cargo test folder (`target/tmp`). A test sets its own values after that.
- A unit test (in `src/`) runs in the test process, so the seam is in the code: under `cfg(test)`, `provision::scriptvox_dir` gives an empty folder for each process, and `provision::user_env` gives no key and no `SCRIPTVOX_GENERATORS`.
- Test `no_test_starts_the_binary_with_the_environment_of_the_shell` (`tests/key_names.rs`) fails when a test file starts the binary in another way.
- `just test` removes the two folders before it runs. A bare `cargo test` keeps them between runs.

Checked: the four tests pass with made-up values in `FAL_KEY` and `ELEVENLABS_API_KEY` and the user's folders, and with none.
