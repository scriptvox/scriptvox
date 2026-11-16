---
name: scriptvox-providers
description: Choose where a scriptvox clip comes from - your own GPU service, free stock, or a paid model - by need, cost, licence and offline rules; read a price and its age; handle keys safely; and act on the exit codes of paid calls (3, 4, 5, 6, 9, 10). Use whenever a brief scene has `generate`, before `scriptvox generate`, when the user asks which AI video model to use or what a video will cost, when a key is missing, or when a paid call stops with an exit code. Load it together with scriptvox-prompts.
---

# scriptvox-providers

Money is spent only through a plan the user approves. This skill decides which row a clip uses, says what it costs,
and keeps keys and charges safe.

## 1. Start from what scriptvox lists

1. `scriptvox providers --json`. Use only rows in `generators`. `service` says who runs the row and who pays: tell the
   user when it is a paid external API on their own key. Read for each: `key_set`, `modes` (t2v, i2v),
   `durations_s`, `aspects`, `usd_per_s`, `price` (exact or base), `price_checked`, `price_age_days`,
   `price_stale`, and `output_terms.commercial`.
2. Price the job: `scriptvox generate --model <row> --prompt "..." --plan --json`, or `scriptvox plan --brief b.json
   --json` for a whole video. The plan is the only price you quote.
3. Never name a model that has no row, and never quote a price from memory.

## 2. Order of choice

Decide in this order: offline need, then cost, then look. Licence terms are data, and the user decides.

| Need | Use |
|---|---|
| a real place, a real person, a flag or an emblem | your own media or `stock` (free), never a generated clip |
| an illustrative shot, text to video, 5 s | `wan22-ti2v-5b-self`: free, on your own GPU service, Apache-2.0 |
| image to video (a first frame) | `wan22-ti2v-5b-self` with `--image` (free); make the frame with `flux2-klein-4b-self` (free) |
| a length other than 5 s | a paid row whose `durations_s` covers it |
| a shot the free row failed twice | the same prompt on the cheapest paid row that fits |
| no network at all (`--local-only`) | own media and the gradient template; a still after `scriptvox setup image-local`, and every other generator row is refused. See `scriptvox-local` |

When you give no `model`, the planner already picks the cheapest row that has a key and fits the aspect and the mode.
Licence terms play no part. With the `selfhost` key set, that is the free row. Override it with `--model` only for a reason you
can say to the user in one line.

- The planner does not look at `--seconds` when it picks: today `generate --prompt "..." --seconds 10 --plan` with no
  `--model` plans the free 5 s row and gives no warning. For another length, name a row whose `durations_s` has it,
  and read the seconds in the plan line.
- `edit --background`, `--alpha` (a matte row), `edit --enhance` (an upscale row) and `export --cover-prompt` (an
  image row, `--cover-model free`, `cheapest` or `best`) pick their row the same way: a free row with a key first.
  `--plan` shows the row and the cost. See `scriptvox-edit` and `scriptvox-review`.

## 3. The built-in rows on 11 Nov 2026

| Row | Cost | Modes | Measured |
|---|---|---|---|
| `wan22-ti2v-5b-self` | $0 | t2v, i2v, 5 s | 704x1280 in about 4 min (measured on one H100); clip checks passed; camera moves faint; i2v held the subject |
| `fastwan22-self` | $0 | t2v, i2v, 5 s | about 35 s a clip, against 226 s on `wan22-ti2v-5b-self`; loses exact colours of a `character` by frame 15 |
| `flux2-klein-4b-self` | $0 | image | 768x1344 first frame in under 10 s |
| `seedvr2-3b-self` | $0 | upscale (`--video`, brief `"upscale": true`, or `edit --enhance`) | 704x1280 Wan clip to 1080x1962 in about 2 min for 5 s; wood, rope and skyline detail checked by eye against plain scaling; no flicker at batch joins |
| `rife426-self` | $0 | interpolate (`--video`, or brief `"smooth": true`) | 24 fps clip to 48 fps, same length, about 10 s for 5 s; a 5 s clip then fills an 8 s scene with motion instead of a frozen last frame (slideshow gate 32% frozen without, 0% with) |
| `birefnet-self` | $0 | matte (`edit --background`, `--alpha`, a title with `behind`) | a mask of one person on a 5 s 1080p clip, 14 s for the whole command (11 Nov) |
| `demucs-self` | $0 | stems (`base_sound` `background`) | an English video with music dubbed to Arabic in 29 s; 0 of 20 old words heard in the kept background (3 Nov) |
| `acestep-xl-turbo-self` | $0 | music, 15 to 120 s (`music_prompt`) | a 15 s bed in 5.7 s (2 Nov) |
| `mmaudio-v2a-self`, `mmaudio-t2a-self` | $0 | sound for a clip, or from text, 1 to 10 s (`ambience`) | nobody has listened to a result; Personal use only (non-commercial weights) |
| `latentsync-16-self` | $0 | lip sync (`dub --lipsync`, `edit --lipsync`) | a 30 s dub with lip sync took 9 min 50 s; Personal use only (weights with use limits) |
| `chatterbox-mtl-self`, `whisper-large-v3-self` | $0 | speech, transcript | open voice for `make --tts selfhost`; Arabic sentence in about 45 s |
| `whisper-large-v3-turbo-local` | $0, on this machine, after `scriptvox setup speech-large` (about 1.6 GB, MIT) | transcript, used over the small row when 3 GB of memory is free | 29 s of Arabic in 23 s on 2 cores, 48 of 49 words where the small row had 45; peak memory 2.05 GB |
| `whisper-small-local` | $0, on this machine, after `scriptvox setup speech-local` (about 490 MB to download, about 880 MB on disk, MIT) | transcript | 16 s of speech in about 8 s on 2 cores; English 71 of 73 words, Arabic with a few wrong words. Used only when no self-hosted speech row has a key |
| `kling-2.6-pro` | $0.07/s exact, $0.35 for 5 s | t2v, i2v, 5 or 10 s | 1080x1920 in about 5 min; one ledger charge after a download retry |
| `kling-3.0-pro` | $0.112/s exact, sound off | t2v, 3 to 15 s | no figure recorded here |
| `kling-4.0-fal` | $0.235/s exact, at 1080p | t2v, 3 to 30 s | not run live yet |
| `seedance-2-fal` | $0.3034/s base | t2v, 4 to 15 s | not run live yet |
| `luma-ray-3.2-fal` | $0.2/s exact | t2v, 5 or 10 s | not run live yet |
| `gemini-omni-flash-fal` | $0.0819/s exact | t2v, 3 to 15 s | not run live yet |
| `minimax-h3-max-turbo` | $0.046/s base, about $0.23 for 5 s | t2v, i2v, 5 s | not run live yet |
| `grok-imagine` | $0.0704/s base, about $0.43 for 6 s | t2v, i2v, 6 s | not run live yet |
| `flux-pro-ultra-fal` | $0.06 an image, base | image | not run live yet. A free image row with a key goes first |
| `scribe-fal` | $0.0005/s of audio ($0.03 a minute) | transcript; the one row that tells speakers apart | not run live yet. `edit`, `clips`, `analyze` and `dub` use it only when no free speech row is set up, and only with `--approve` of its own plan |
| `sync-lipsync-2-fal` | $0.05/s of the source | lip sync | not run live yet. `dub --lipsync` takes a free row only |

A name ending in `-self` runs on your own GPU service (the `selfhost` key). A name ending in `-local` runs on this
machine. Every other row is a paid external API on the user's own fal key: plan, then `--approve`. Rows of your own
from `generators.toml` show in the same list. Read `durations_s` for the exact lengths a row takes.

A `base` price is an estimate: the page does not say whether size or length changes it. Say "estimate" next to it.

## 4. When to pay

- Pay only when section 2 sends you to a paid row, or when the user names a paid model.
- Tell the user in one line: the row, the seconds, the cost, and why the free path is not enough.
- Show the plan cost and the `plan_id`. The user gives the approval. Never add `--approve` on your own choice.
- Keep `--max-spend` at the plan cost (the default). A bigger cap needs the user's word.
- At most two paid tries per shot. Then make the shot simpler, or use stock.

## 5. Licence and commercial use

- Every row is allowed, whatever its licence, and the user decides. There is no flag and no question.
- The tool prints no line about commercial use (owner, 13 Nov 2026). Give the terms when the person asks.
- Each row in `providers` and `voices` has `business_use`: Business use yes, Personal use only, or Not confirmed.
- Terms stay as data: `output_terms` in `providers`, and `parts` in `metadata.json` with `commercial_ok`.
- Edge terms are for personal use, and MMAudio (`ambience`, `mmaudio-*`) has non-commercial weights. The fal rows leave `commercial` empty (null): not checked, so do not call them safe.
- The spend cap and paid approval do not change.
- A row is marked commercial only when someone reads the model licence and puts the link in its `terms.note`.

## 6. Keys

- Never ask for a key in chat. Never echo one, put one in a command line, or write one to a file.
- The user runs `scriptvox keys set <provider>` in their own terminal. The input is hidden and the file is
  user-only. Providers: elevenlabs, pexels, pixabay, fal, vision, selfhost, hf (a Hugging Face token, for a gated model).
- Check with `scriptvox keys list --json`. It shows where each key comes from (environment or file), never the value.
- An environment variable wins over the file. If a new key "does not work", look for an old variable.
- If the user pastes a key in chat: do not use it. Tell them to revoke it at the provider and set a new one.
- A `selfhost` row on a private certificate also needs that CA at `<config>/ca/selfhost.pem`. Without it the call stops with a fix
  line before anything is sent.

## 7. Price age

- A price older than 30 days shows `price_stale: true`, and the plan line says how old it is.
- With a stale price, link the `price_source` page and ask the user to check it before they approve.
- Before a paid batch over $2, ask the user to check the price page, or update the row first.
- Update a price only from the provider's own page, with the new `price_checked` date. Never from memory or a blog.

## 8. Exit codes on generated clips

| Exit | Meaning | Do |
|---|---|---|
| 2 | usage: unknown or unfit row, or replay of a generated clip | fix the command; nothing was charged |
| 3 | clip check failed (length, size, black, frozen) | the clip was paid for and is kept; read `findings`, change the prompt or the row (a new plan) |
| 4 | approval needed | show the cost and `plan_id`; rerun with `--approve <plan_id>` only after the user says yes |
| 5 | retryable: network, status check, download | rerun the same command; it resumes the same job; after three tries, report |
| 6 | unconfirmed charge | stop; never resubmit; ask the user to check the provider dashboard; the job record stays in the run folder |
| 9 | still running | run the same command later, or `scriptvox wait <run_id>`; it never submits twice; no sleep loops |
| 10 | needs input: a missing or refused key, or `--local-only` with a generated scene | show `needs_input`; for a key, the user runs `scriptvox keys set` |

A changed prompt, row or length is a new `plan_id`. The old approval does not carry over, and the old job is not
reused. That is on purpose.

A voice row with `--audio` clones the sample. It stops at exit 10 until the user records consent: `scriptvox voices add --sample f --own` for their own voice, or `--consent proof` for another person's voice. Never run this step for them: it is their statement, and they hold the proof.

## 9. Add a new row

1. Start from `scriptvox providers template fal` (or `selfhost`, `openai-speech`): it prints a block. Or let
   `scriptvox providers add` write it. For a Hugging Face id, a local file or a fal endpoint, `scriptvox models add`
   makes the row.
   The block goes in `generators.toml` in the config folder (next to `keys.toml`). A block with a known
   name changes only the fields it sets (`name` and `base` are enough), and a new name adds a row. A bad block costs
   only itself: the built-in rows stay. `scriptvox providers --json` shows any errors in `generator_errors`.
   A new provider's key needs one `[[key]]` block in the same file, with `name` and `env`. The variable name ends in
   KEY, TOKEN or SECRET. The user then runs `scriptvox keys set <name>`.
2. Endpoints come from the provider's own API page. List only the `durations` the page states.
3. `price_source` is the https price page, `price_checked` is today, and `price = "base"` unless the page says the
   price is flat.
4. `hosts` lists only download hosts seen in a real result. Leave `terms.commercial` out (null) until the licence is read; `false` only for a limit you read.
5. If the model makes audio by default, turn it off in `extra`: scriptvox adds its own voice.
6. Prove it: `scriptvox providers test <row> --json` runs the smallest call. A paid row plans first and needs the user's yes for `--approve`. Report the `terms` it prints.
7. Test against the fake queue first. Then one paid smoke call the owner approves, and check that the ledger shows
   exactly one charge.
