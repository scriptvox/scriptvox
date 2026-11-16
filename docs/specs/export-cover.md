# `export --cover-prompt`: a cover made by an image row (11 Nov 2026)

Gap G116. `export` took the cover from a frame of the video. A creator often wants a cover made for the thumbnail: a picture that shows the subject, not the first second.

## The command

```
scriptvox export <run or video> --cover-prompt "a dhow leaving a harbour at sunrise" [--cover-model <row>]
```

- `--cover-prompt` is what the cover shows. One picture is made with an image row.
- `--cover-model` is a row name from `scriptvox providers --kind image`, or `free`, `cheapest` or `best`. The default is `cheapest`, the default of `generate --model`. The choice is made by the same function (`generators::choose_by_word`).
- The shape is the video's. Rows offer 9:16, 16:9 and 1:1, so a run of another shape asks for the nearest (`make::gen_aspect`, the rule of generated scenes). A video file with no run: wide gives 16:9, square gives 1:1, else 9:16.
- `cover.jpg` is the made picture at the video's size (scaled to fill, the middle kept). `cover-frame.jpg` is the frame cover that `cover.jpg` was before. The card covers `cover-1.jpg` to `cover-3.jpg` do not change.
- No title is drawn on the cover: `export` draws no text on any cover.
- With no `--cover-prompt`, `export` writes the same files and the same `metadata.json` keys as before.

## Money

The picture is made by `generate::run`, the code of `scriptvox generate --kind image`. `export` has no money code of its own. So the rules are the same:

- A free row runs at once.
- A paid row stops with exit 4, `status: approval_needed` and the `plan_id`. Nothing is written, not the folder either. `--approve <plan_id>` runs it. The plan id is the one `generate` gives for the same row, text and shape.
- `export ... --cover-prompt ... --plan` shows the stage and the cost and makes no call. Without `--cover-prompt`, `--plan` does nothing on `export`, as before.
- `--max-spend` under the price stops at plan time (`SVX_SPEND_CAP`). The ledger holds the reserve at run time.
- A row that cannot run (no key, a placeholder host) says so before any approval.
- `export --help` now lists the money options, and the MCP row for `export` says it can reach a network and cost money.

## The store

The picture is kept in the content store under a key of the row, the prompt and the shape. The same cover again (the same run, another run, another folder) comes from the store: no call and no charge, and no approval is asked for a picture that is already paid. `covers[0].reused` is true then, and `cost_usd` is 0. Change a word of the prompt for a new picture: that is a new plan and needs its own approval.

## The prompt

The prompt is user text. It goes into the JSON request of the row and into `metadata.json`. It is never part of a shell line or an FFmpeg filter: the only filter here is `scale` and `crop` with the numbers of the video's size. No `next` line repeats the prompt.

## `metadata.json` and the result

- `covers` starts with two entries when a cover is made: `{"file": "cover.jpg", "kind": "generated", "row", "prompt", "aspect", "cost_usd", "reused", "plan_id"}` and `{"file": "cover-frame.jpg", "kind": "frame", "at"}`. `row_choice` says why a word picked the row.
- `parts` has the cover's row as a `generator` part with its terms, by the same code as the row of a generated scene. So it counts in `commercial_ok`, in `ai_parts`, in the terms note, and in `credits.txt` when the row asks for attribution. A row that is also a scene's row is one part.
- `metadata.cost_usd` is the cost of the video, as before. The cover's cost is `covers[0].cost_usd` and `metrics.cover_cost_usd`.
- `artifacts.cover` is the made cover; `artifacts.cover_frame` is the frame.

## Measured

11 Nov 2026, run r_319969 (16:9, Edge voice, free): `export --cover-prompt "a wooden dhow with a white sail leaving a harbour at sunrise ..." --cover-model flux2-klein-4b-self --max-spend 0`. The plan said $0. The export took 5.2 s with one call to the self-hosted row. `cover.jpg` is 1920x1080 and shows a wooden dhow with a white sail on calm water, a harbour town behind and a low sun: looked at by the agent that built this, no person has seen it. The same command again: 0.3 s, `reused: true`, no call. `commercial_ok` stayed false because of the Edge voice.

Tests (`tests/generate.rs`, fake queue): `export_makes_a_cover_with_a_free_row_and_keeps_the_frame`, `export_with_a_paid_cover_row_needs_approval_and_pays_once`, `a_cover_row_with_a_use_limit_makes_the_export_personal_use`.

## Not done

- No paid cover was bought: the paid path ran only against the fake queue.
- No title on the cover. Add it when `export` gets a text path for covers.
- One picture per call. Several candidates to choose from is the storyboard's sheet idea (G054), not built here.
- A reference picture for the cover (`--cover-image`): add when someone asks.
