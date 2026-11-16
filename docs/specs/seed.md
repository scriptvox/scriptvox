# `--seed` (G075, 8 Nov 2026)

ComfyUI and most image tools let a user keep a seed, so a good picture can be made again or changed a little (public docs; no code read).

- `generate --seed N` and brief `seed`: every generated still, clip and first frame of the video gets the same seed. `cutaways` take the brief's seed too.
- Row field `takes_seed`: the row takes `seed` and gives the same result for the same seed and input. Only then is `seed` sent, so the plan id changes with it. `scriptvox providers` lists the field.
- A row that does not take a seed gets nothing extra, and the plan line says "this row takes no seed, so each run gives a new picture". A row that takes one says "seed N".
- Today only `flux2-klein-4b-local` takes a seed (sd-cli `-s`; its own default 42 when none is given). The fal rows were not checked against their pages, and the `services/gen` video and image engines do not read a seed, so those rows stay `false`. An overlay row may set `takes_seed = true` once its page or service is checked.

## Measured, 8 Nov 2026, 2 CPU cores, 384x672 (`SCRIPTVOX_SD_ARGS`)

| Check | Result |
|---|---|
| `generate --seed 7` twice (own idempotency keys, so two real runs), then `--seed 8`, same prompt | 473 s, 473 s, 485 s; the two seed 7 files have the same sha256, seed 8 differs; read by eye: the same kite twice, another kite and dune with seed 8 |
| Plan ids | seed 7 and seed 8 give different plan ids |
| Plan line, `fastwan22-self --seed 7` | "this row takes no seed, so each run gives a new picture" |
| Unit test | `seed` only in a request for a row that takes it; the plan notes |

## Not done

Seeds on the fal rows and on `services/gen` (a service change and a roll). A seed per scene (all scenes share one).
