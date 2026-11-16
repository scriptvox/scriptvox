# Notes: scriptvox-providers

Written 2 Nov 2026 before the skill. Full read of (OpenMontage, AGPL, ideas only): `.agents/skills/setup-api-key`,
`provider-model-refresh`, `provider-refresh-oct23`, `ai-video-gen` (both copies), `docs/PROVIDERS.md`,
the 3 Oct update plan and the 23 Oct refresh report in `docs/`, `skills/creative/image-provider-usage.md`,
the budget and tool sections of `docs/ARCHITECTURE.md`, `skills/meta/checkpoint-protocol.md`. HyperFrames (Apache-2.0):
`skills/media-use/SKILL.md`, `references/setup-providers.md`, `resolve.md`, `operations.md`, `audio/references/tts.md`,
`bgm.md`, `sfx.md`. MoneyPrinterTurbo (MIT): `config.example.toml`, `docs/chatgpt-provider.md`, `codex-provider.md`.
Ours: `registry/generators.toml`, `src/keys.rs`, `src/exit.rs`, `src/genjob.rs`, `src/generators.rs`, PHASE2, P2-002,
P2-003, P2-006, and the live output of `scriptvox providers --json` and the `keys`, `generate`, `plan` help. No text carried over.

## 1. What the sources teach (our words)

### Choosing a provider
- Sort options by what the job needs first: offline or not, budget, licence, then look. Taste comes last.
- Real footage often beats a generated clip for "a real place or person". Stock is free and has no model drift.
- Keep a ladder of paths per budget: all free, mostly free with one paid hero shot, all paid. Price each ladder.
- One aggregator key (fal-type gateway) opens many models. That is the cheapest setup to start paid work.
- The same model name on two hosts is not the same contract. Inputs, limits and prices differ by host.
- A key being present only proves setup. It does not prove the account may call that model or has credit.
- Unknown price is not zero. A tool that cannot price a call must say "unpriced", never score it as free.
- Ask before a paid call the agent chose by itself. A call the person asked for by name may run.

### Cascades and fallback
- HyperFrames tries sources in a fixed order per media type and drops to the next one when a tool is missing.
- Reuse comes before any fetch: the project cache, then a shared cache, only on an exact prompt match.
- A fuzzy reuse is the agent's call, never automatic. Brand assets from another project are never reused loosely.
- A pinned provider (`--provider`) turns the cascade off for that call.
- A paid fallback and a free fallback are not equal outputs. A local generic clip is not a stand-in for a presenter.
- OpenMontage later turned silent fallback off: an exact model, host or tool request fails rather than swaps.
- Sign-in missing is a stop and ask moment, not a licence to quietly use the local path.

### Keys setup
- Check for an existing key first (environment, then a file). Validate it with a cheap read call. Skip setup if good.
- Point the user to the provider page where keys are made, and say the key shows only once.
- Store per provider, one variable name each. Keep it out of the repo.
- MoneyPrinterTurbo keeps keys in a local config copied from an example file, with key lists for rotation.
- Subscription logins (OAuth, CLI sessions) live in their own private files and never switch to API billing on failure.

### Price reading and refresh
- Video is priced per second of output, images per image, speech per character. Multiply by the real length.
- Some pages give one number; resolution, audio or length may change it. Mark those as estimates.
- Record the page and the day each price was read. Promotions expire; do not keep them as the price.
- A refresh is research, then exact model ids from the host's own schema, then mocked tests, then a paid smoke.
- Do not invent a model id from a naming pattern. Wait for a public contract.
- Budget control: estimate, reserve, then reconcile the real charge. A threshold per action asks for approval.
- Long jobs: save the provider job id before polling. A timeout resumes that job, never submits a second one.
- A submit that timed out with no job id is ambiguous. Check the dashboard before any new submit.

### Offline mode
- One hard switch skips every network source. What is left is the cache and installed local tools.
- Local tools are opt-in installs with RAM or VRAM tiers. Missing weights fail; they never download silently.
- "Runs in a node graph" is not "offline": some graph nodes call paid hosted services.

## 2. Weak points of the sources
- OpenMontage `setup-api-key` asks the user to paste the key into chat. The key then sits in the transcript and logs.
- It writes the key to a plain `.env` with no file mode. No word on git ignore or rotation.
- It covers one vendor only, yet sits under a generic name. Every other provider has no setup flow.
- `ai-video-gen` names one premium model the default and cites a leaderboard rank. That is vendor bias, not a test.
- `ai-video-gen` tells the agent to trust a selector's "automatic fallback", while the refresh docs forbid silent fallback.
- `PROVIDERS.md` FAQ still says a missing provider falls through to the next one. It contradicts its own top section.
- `PROVIDERS.md` is about 1,500 lines, mostly price tables with no read date per row. Staleness is invisible.
- Several prices say "endpoint-dependent" or "~". An agent cannot plan a budget from that.
- Its local video section says every local model is free and offline, but omits model licences entirely.
- No source has a commercial-use flag per model. Licence checks are one line for one local model.
- Refresh docs report thousands of mocked tests and zero paid calls. No measured time, cost or clip check per model.
- HyperFrames favours one vendor first in almost every cascade. Free paths come second.
- HyperFrames cost rule depends on who "chose" the call. That is hard to judge from a transcript.
- MoneyPrinterTurbo puts sponsor and referral links next to key fields. Key text sits in a plain config file.
- MoneyPrinterTurbo clamps durations silently to the model range. The user pays for a length they did not ask for.
- None of the sources give exit codes. The agent reads prose errors and guesses what to do next.

## 3. What our skill must do better (rules for ScriptVox)

### Start from facts, not memory
- Run `scriptvox providers --json` first. Use only rows it lists. Read `key_set`, `modes`, `durations_s`, `aspects`,
  `usd_per_s`, `price`, `price_checked`, `output_terms.commercial`. Never name a model that has no row.
- Then `scriptvox generate --model <row> --prompt "..." --plan --json` (or `scriptvox plan --brief b.json --json`).
  The plan is the only price the agent quotes.

### Free row first
- Default to `wan22-ti2v-5b-self`: $0, Apache-2.0, `commercial = true`. Measured: 5 s at 704x1280 in 3 min 46 s on
  one H100, all clip checks passed. No approval, no ledger charge.
- Limits of that row: text to video only (no `--image`), 5 s only, needs the `selfhost` key and your service's CA file
  `<config>/ca/selfhost.pem`. It is a network call to your own GPU service, so `--local-only` refuses it.
- Stock (Pexels, Pixabay with a key; Commons, Internet Archive with none) is also free. Prefer it for real places and people. The keyless rows have terms per file in the credits file.
- The planner already takes the cheapest row with a key that fits aspect and mode. Do not override
  it with `--model` unless there is a reason the user can read.

### When to pay
- Pay only when the free row cannot do the shot: image to video, a length other than 5 s, a look Wan failed twice,
  or the user names a paid model.
- Say the row, seconds, price and why, in one line. Kling 2.6 Pro via fal: $0.07/s exact, so 5 s is $0.35, about 5 min.
- A `price = "base"` row is an estimate. Say "estimate" next to the number.
- Never pass `--approve` on the agent's own choice. The user gives the plan_id approval. Keep `--max-spend` at the plan.

### Licence terms (no flag since 5 Nov 2026)
- No row is refused for its terms. A known limit (`commercial = false`) is data in `providers` and `metadata.json`; no note is printed (13 Nov 2026), and the user decides.
- The fal rows leave `commercial` out (null): not checked. Do not call them safe.
- A row becomes commercial only after someone reads the model licence and puts the link in `terms.note`.

### Key safety
- The agent never asks for a key in chat, never echoes one, never puts one in a command line or a file it writes.
- The user runs `scriptvox keys set <provider>` in their own terminal. Input is hidden. The file is `keys.toml`, mode 0600.
- `--stdin` exists for scripts the user owns (a secrets manager pipe). The agent does not build that pipe with a value.
- Check with `scriptvox keys list --json`: it shows the source (env or keys.toml), never the value.
- The environment variable wins over the file. If a key "will not change", look for a stale env var.
- A pasted key in chat: tell the user to revoke it at the provider and set a new one. Do not use the pasted one.

### Price staleness
- Every row has `price_checked`. Older than 30 days: say the price may be old and link `price_source`.
- Before any paid batch over $2, ask the user to recheck the price page, or update the row first.
- Never edit a price from memory or a blog. Only from the provider's own page, with the new date.

### Exit codes the agent will meet
- 4 approval needed: show the plan cost and plan_id to the user. Wait. Rerun with `--approve <plan_id>` only after yes.
  A changed prompt or model is a new plan_id; the old approval does not carry.
- 5 provider retryable (network, status check, result fetch, download): rerun the same command. It polls or resumes
  the same job. Back off; after three tries, report.
- 6 unconfirmed charge: stop. Never resubmit. Tell the user to check the gateway dashboard. The job record stays in
  the run folder. A new paid try is a new plan the user approves.
- 9 still running: rerun the same command later (or `scriptvox wait <run_id>`). It never submits twice. No sleep loops.
- 10 needs input: usually a missing or refused key, or `--local-only` with a generated scene. Show `needs_input`
  to the user. For a key, tell them to run `scriptvox keys set <provider>`. Do not ask for the value.
- 3 clip check failed: the clip was paid and is kept. Read the findings (length, size, black, frozen). Change the
  prompt or the row; that is a new plan. Do not rerun the same plan hoping for luck.
- 2 usage: unknown row, unfit row, or replay from cache. Fix the command; nothing was charged.

### Add a new row safely
- Data first: one `[[generator]]` block in `registry/generators.toml`. Rust validates it; `providers` shows errors.
- Endpoints from the provider's own API page. `durations` only values the page states.
- `price_source` an https page, `price_checked` today, `price = "base"` unless the page says the price is flat.
- `hosts` only download hosts seen in a real result. `terms.commercial` left out (null) until the licence is read; `false` only for a limit that was read.
- If the model makes audio by default, add `extra` to turn it off (scriptvox adds its own voice).
- Test with the fake queue first. Then one paid smoke call the owner approves, with a ledger check (one commit).
- A row counts as supported only after cassette, smoke, price check and licence flag (PHASE2 2.2).

## 4. Claims to test
- `wan22-ti2v-5b-self` repeats 5 s 704x1280 in under 4 min on one H100, clip checks pass, ledger shows $0.
- Plan for a 5 s t2v scene with all keys set picks the selfhost row, not MiniMax ($0.13).
- Plan for an i2v scene skips the selfhost row (no `i2v`) and picks the cheapest fal row.
- `--local-only` with a generated scene exits 10 even for the selfhost row.
- Kling 2.6 Pro 5 s: one ledger commit of $0.35 after a forced download failure and rerun.
- A submit with no answer exits 6 twice and submits once (P2-002 fake-queue test).
- A provider status with an error field exits 6, not 5.
- `scriptvox keys set selfhost` works, though `keys set --help` lists only five providers. Fix the help text if it works.
- A key never shows in `--json` output, events file, logs, or the embedded recipe (grep the run folder for it).
- An env var set to a wrong key overrides a good `keys.toml` value; `keys list` names `env` as the source.
- No code check flags an old `price_checked` today (only the date format). Decide: warn in `providers` past 30 days?
- `price = "base"` rows: does the plan label the cost an estimate in the envelope? MiniMax may bill more than shown.
- MiniMax fal clips run up to 0.7 s long; the clip check and crop handle it without a fail.
- The skill text passes `just skills-check` with no 8-word run shared with any source.
