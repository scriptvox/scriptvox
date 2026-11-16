# Retired rows (G082, 8 Nov 2026)

Providers take models down. A row that just fails at run time wastes a plan and an approval.

- Row fields `retired` (when and why, in plain words) and `instead` (a running row to use). The row stays in `scriptvox providers`, with `retired` and `instead` shown, and never runs: `ready` is false, so no default picker takes it.
- Naming it is exit 2 at once, before any plan or cost: `generate --model old` and a brief scene `model` both say "<row> is retired: <why>" with the fix "Use row <instead> instead". The job runner refuses it too, so no path reaches the provider.
- `instead` must name a row that runs, else the row is listed in `generator_errors`.
- No built-in row is retired today; the fields are for the registry and for overlays.

## Measured, 8 Nov 2026

An overlay row `kling-old-fal` (a copy of `kling-2.6-pro`, made-up retirement date): `providers` lists it with both fields; `generate --model kling-old-fal --plan` and `plan --brief` with that scene model give exit 2 with the reason and `--model kling-2.6-pro`; `instead = "no-such-row"` gives a `generator_errors` line. Unit test: not ready, reason and fix.

## Not done

A dated check that finds retired models on the providers' pages (discovery runs read the pages by hand).
