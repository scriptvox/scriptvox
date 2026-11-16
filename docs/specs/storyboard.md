# `make --storyboard`: stills first, then pay for clips (11 Nov 2026)

Gap G054. A clip is the part of a video that costs the most money and time. Before it is paid for, a person or an agent wants to see what each generated scene would look like. The idea is from one line of observation that other tools show a sheet of stills for approval before any clip; no other tool's code or docs were read.

## The command

```
scriptvox make --brief b.json --storyboard
```

For every scene whose `generate` makes a clip, one still is made with an image row, in the video's shape. The stills are written as one PNG sheet with the scene number on each. Then the command stops. No clip is made, no voice is made, nothing is rendered.

The form is `make --storyboard`, not only `plan --storyboard`, because `plan` promises that nothing is called. `plan` is `make --plan`, so `plan --brief b.json --storyboard` exists too and keeps that promise: it shows the storyboard's plan and makes no call. `--storyboard` does not go with `--detach` or `--until`.

## Which picture a panel is

- A scene with `first_frame`: the still is that first frame. The same image row and the same request as `make` uses (`from: "first_frame"`).
- A scene with only a prompt (text to video): the still is made from the scene's own prompt, with the brief's `style` (`from: "prompt"`). The image row is chosen by the rule of first frames (`make::frame_row`: the cheapest image row that can run and offers the shape, so a free row comes first).
- A scene with its own `image`: that file is the panel. No call (`from: "image"`).
- A scene with `still: true` or with no `generate` is not on the sheet: it makes no clip.
- No clip scene in the brief: exit 2, `nothing to storyboard`.

## Is a still used again by `make`?

Each still is kept in the content store under a key of its image row and its request (`make::frame_key`).

- **A `first_frame` scene: yes.** `make` looks in the store for the first frame before it makes one. The storyboard's still has the same row and request, so the clip starts from the picture that was on the sheet, and the image row is not called again. `stills[].make_reuses` is true.
- **A text to video scene: no.** The brief did not ask for image to video, so `make` gives the row the prompt only, as before. The still is a look at the idea, not the first frame of the clip. `make_reuses` is false. To start the clip from the still, put the still's `prompt` (it is in the result) in the scene as `first_frame`.
- **A scene with a character: no.** `make` gives the image row the character's reference picture, which the storyboard does not make. `make` does not look in the store for such a first frame. The still shows the words only.

This lookup is also a change to `make` with no storyboard: a first frame of a scene with no character, with the same row and request as one made before (an earlier run, the same seed or none) now comes from the store, the rule that generated clips already had (`gen-cache.md`). Change a word of `first_frame` for a new picture.

The plan of the video does not take this discount: it still shows the full price of a first frame that is in the store, and the run then spends less. The plan id must not change when a picture is made (a run id comes from it), and a price that is too high is the safe direction.

## Money

- The storyboard has its own plan and plan id: one stage per still. The clips, the voice and the music are not in it, so an approval of it never covers a clip.
- A free image row runs at once. A paid image row stops with exit 4, `approval_needed` and the storyboard's plan id; `--approve <that id>` runs it. The plan id of the video is not accepted.
- `--max-spend` under the price stops it. Each paid still goes through the ledger's reserve against the cap, as every generated picture does (`genjob::run_clip`).
- A still that is in the store costs 0 and makes no call. A storyboard that is all in the store needs no approval.
- `next` names the video: the same command without `--storyboard`, with the cost of the video's own plan (`make_plan.cost_usd`, `make_plan.plan_id`). That plan needs its own approval.

## The result

- `artifacts.storyboard`: the sheet, `scriptvox-out/<run>-storyboard/storyboard.png`. `artifacts.stills`: the files, `scene<N>.png` in the same folder.
- `stills`: for each, `scene`, `file`, `row`, `prompt`, `from`, `cost_usd`, `reused` (from the store), `make_reuses`.
- `metrics.cost_usd`: what the stills cost in this call. `stages`: the storyboard's plan. `make_plan`: the id and cost of the video's plan.

The sheet is made by the tile graph of `snapshot --sheet` (`gates::tile_graph`): tiles 360 wide, up to 3 in a row, then a square grid. The scene numbers are an ASS file with digits only, drawn with the embedded font. A prompt never reaches the FFmpeg graph.

## Measured

11 Nov 2026, a 16:9 brief of 3 scenes, each with `generate` and `"model": "wan22-ti2v-5b-self"`, scene 2 with `first_frame`. `plan --storyboard --max-spend 0`: 3 stages on `flux2-klein-4b-self`, $0, no call. `make --storyboard --max-spend 0`: 3 stills and the sheet, $0, 3 image calls, no clip job, no voice. It took 4 min 2 s, nearly all of it waiting for the shared GPU service, which had another job. The sheet is 1116x214: scene 1 a dhow with white sails leaving a harbour wall at sunrise, scene 2 a falcon over red dunes under a blue sky, scene 3 a city skyline at night with its lights in the water, with the numbers 1, 2 and 3 in the top left of the tiles. Looked at by the agent that built this; no person has seen it. The clips were not made, so the reuse of the scene 2 still by a real `make` is proven only by the test.

Tests (`tests/generate.rs`, fake queue): `a_storyboard_makes_one_still_per_clip_scene_and_no_clip`, `a_paid_storyboard_needs_its_own_approval_and_never_the_clips_cost`, `a_brief_with_no_clip_scene_has_nothing_to_storyboard`.

## Not done

- One image call per still. One call that returns 4 to 9 panels in one picture (the words of the gap row) needs a row that makes a grid and a cut of it: not built.
- Characters on the stills (the reference picture is not made here).
- No paid storyboard was bought: the paid path ran only against the fake queue.
- `resume` and `batch` do not take `--storyboard`.
