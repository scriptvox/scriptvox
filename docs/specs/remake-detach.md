# `remake --detach`, and `remake` as an MCP tool (G135, part 3 of 3, 10 Nov 2026)

Before: `remake <file.mp4>` ran only in the foreground. It can take minutes and can cost money, so it was not an MCP tool: the server handles one call at a time and it would have blocked it.

## Rules

- `remake <file.mp4> --detach` answers at once: `status: running`, the `run_id`, `next: scriptvox wait <id>`. `wait`, `status` and `cancel` work on the run as for `make`.
- It uses the code of `make --detach` (`detach_start` in `main.rs`). There is no third copy.
- Every check below runs in the calling process, before a run folder or a supervisor exists. Any stop is answered at once with its own exit code, and no run starts.
- A paid plan with no approval, or with the approval of another plan, stops with exit 4, `approval_needed` and the plan id (`make::approval_stop`). A plan over `--max-spend` stops with exit 4 and `SVX_SPEND_CAP`. No run, no call.
- One plan is one run. The run id comes from the plan id and `--idempotency-key`, or the word `remake` when there is no key (a plan with generated scenes or paid music uses `make`'s own id, so it finds that run's provider jobs). The same call again finds the run: `running` while it works, and the stored result with `idempotent: true` when it finished and its video is still on disk. So the same recipe with a different `--out` also gets the finished run, not a second video. Delete the video, or give a new key, to make it again.
- `remake --plan` runs nothing, with or without `--detach`.
- Windows: the run stays in the foreground with a warning, as `make --detach` does.
- MCP: `scriptvox_remake` is a tool. The server adds `--detach`, unless the call is a `--plan`. Its row is `make`'s row: not read only, can replace a file, reaches a network, can cost money. The `initialize` text names `scriptvox_remake` in the loop and in the rule that a paid plan needs the user's yes.

## The recipe is not trusted text

It comes from the `comment` tag of a file that the user may not have made. These checks run before the supervisor starts, and the supervisor runs all of them again when it reads the file (so a file changed in between is checked as a new recipe, and a changed plan has a new id that the old approval does not fit).

| What | Check |
|---|---|
| The file | It must exist and carry a ScriptVox recipe, read with ffprobe. Else exit 2 or 1. |
| The request | It must parse as a `make` request of this version. Else exit 1. |
| Fields that are never taken from a recipe | `out` (only `--out`), `run_id`, `idempotency_key`, `parent_run`, `approve`, `use_system`, `no_embed`, `plan_only`, and since this change `max_spend`. |
| Keys | A request never holds one, and no field of it is read as one. Keys come from the user's key store. |
| Files that go into the video | `narration`, `font`, `logo`, scene `overlay` and `inset`, scene `media`, `media` (the folder), `base_video`, a `music` file, a `look` file, a character picture, a scene's `image` and `last_frame`: each must be inside the project folder after links are resolved. Else `SVX_PATH_OUTSIDE_ROOT`, or a usage error for a picture that goes to a provider. |
| Stock picks | Each must name a known provider, a plain id and an https URL on that provider's hosts. Else `SVX_BAD_RECIPE` with "Only remake videos you trust." This check ran only at the fetch, inside the run. It now also runs when the request arrives. |
| Names and numbers | Aspect, fps, background, transition, look, caption size and `until` are checked by `make` before its plan (read in the code). Every other text field goes the way a brief's field goes in `make --brief`: this change adds no check there, and not every field was read for this spec. The accent is held to `#RRGGBB` before the filter graph (`wave_colour`). |
| Money | The plan is priced from the recipe as it is now. Approval is for that plan id only. |

Found and fixed here: `remake` and `replay` took `max_spend` from the recipe when the user gave none. After an approval the cap of a run is `--max-spend`, else the cost of the approved plan. A recipe with a high cap could let a run spend more than the plan the user said yes to. The cap now comes only from `--max-spend`. Also fixed: `--local-only` on `remake` and `replay` was dropped when the recipe did not ask for it; it now holds.

Side effects of this change: the stock pick check now runs at the start for `replay` and for a foreground `remake` too, before the voice, and both no longer take the recipe's cap.

Limit: with MCP roots, the server checks the MP4's own path and `--out`. It does not read the recipe inside the MP4. The project folder checks above hold the recipe's files, and the project folder must be inside a root.

## Tests

- `replay::remake_detach_answers_running_and_wait_gives_the_video`
- `replay::the_same_remake_detach_twice_is_one_run` (3 calls, 1 run folder)
- `replay::remake_detach_refuses_a_bad_recipe_before_any_run_starts` (a narration file outside the project, a bad stock pick, a request that does not parse, a file with no recipe, a missing file: no run folder each time)
- `elevenlabs::remake_detach_of_a_paid_recipe_needs_approval_and_starts_no_run_without_it`. The fake server counts calls. The first `make` is 1 call. In a new project with an empty cache: no approval 0 more calls, a wrong approval 0 more, the right approval 1 more (one part of speech, nothing cached), the same call again 0 more.
- `elevenlabs::the_spend_cap_of_a_recipe_is_not_used`
- `mcp::tests::make_and_generate_go_to_the_background_and_a_plan_does_not`, `mcp::tests::remake_is_a_tool_that_can_cost_money_and_the_user_is_asked_first`

## Measured, 10 Nov 2026

A real stdio session (a pipe, no real client), free Edge voice, a 6.8 s video made first with `make`:

| Call | Answer |
|---|---|
| `tools/list` | 34 tools, `scriptvox_remake` among them |
| `scriptvox_remake first.mp4 --out again.mp4` | 0.05 s, `running`, run id |
| the same call while it runs | 0.05 s, the same run id |
| `scriptvox_wait <id>` | 2.2 s, `ok`, `again.mp4`, cost 0, 8 gates pass (black, slideshow, silence, loudness, av_drift, words, captions, text_bounds) |
| the same call after it finished | the same run id, `idempotent: true` |
| `scriptvox_remake first.mp4 --plan` | the plan, no run |

The voice came from the cache of the first `make`, which is why the run took 2 s.

## Not run

- No paid call. The paid path ran only against the fake server in the tests.
- No real MCP client (Claude Desktop, Cursor, Codex).
- Windows and macOS.
- A recipe with generated scenes or paid music.
- `--run-id` given through `args` by an MCP client is not refused (see `mcp.md`).
- `cancel` on a detached remake. It is the code path of `make`, which `detach.rs` tests.

## A run id is checked before it names a folder (10 Nov 2026, found by a review before the merge)

`--run-id` is the supervisor's own hidden flag on `make`, `remake` and `generate`. Its value became a folder under `.scriptvox/runs` with no check, so `--run-id ../../x` could name a folder outside the project, and an MCP client could pass the flag in `args`. Now `run::safe_id` accepts only the shape ScriptVox writes (`r_` and letters or digits, then `.` and a number for each resume, at most 64 characters), `Run::create_with_id` and `Run::attach` refuse anything else, and the MCP server refuses `--run-id` (and `--run-id=...`) from a client. Tests: `only_a_run_id_names_a_run_folder`, `a_client_cannot_pass_the_supervisors_run_id`, and `a_run_id_that_is_not_one_is_refused_and_makes_no_folder`.

Not done: commands that read a run by id from the command line (`check <run>`, `status <id>`, `export <run>`) join the id to a path in their own places; they only read, and `report` already checks its id. One shared helper for all of them is open.
