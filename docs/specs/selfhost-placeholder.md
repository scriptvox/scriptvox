# Selfhost rows with a placeholder host

The built-in `selfhost` rows point at `gen.example.invalid`: no server answers there, and a key alone cannot help.

- `Generator::undeployed()`: gateway `selfhost`, `base` host ends in `.invalid`, and `SCRIPTVOX_FAL_BASE` (loopback test servers) is not set.
- Plan: every row's explain line ends with its `service` (who runs it, whose key), selfhost rows too. An undeployed row adds "not deployed yet: <deploy step>".
- Run (`genjob::run_priced`, used by `generate`, `make`, `edit`): an undeployed row stops with exit 10 (`needs_input`) before the key check and before any network. The fix: deploy `services/gen` on your own GPU, add a row of the same name with your host as `base` in `<config>/generators.toml`, then `scriptvox keys set selfhost`.
- `make` with a brief field (for example `ambience`) and no row with a key: the fix also says that the free rows run on your own GPU service, with the same deploy step.
- Until G010 ships local video, the deploy step of a video row also says that free video needs a server, and that of an image row names `scriptvox setup image-local`, which needs none.

Test: `tests/generate.rs` `a_placeholder_selfhost_row_asks_for_the_deploy_step`. Real run (no key, built-in rows): plan explain names the service and the deploy step; `generate` exits 10 with the deploy step; an `ambience` brief plan gives the deploy step in its fix.
