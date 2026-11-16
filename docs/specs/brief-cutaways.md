# Brief `cutaways`: generated pictures over a base video, one plan (G027, 8 Nov 2026)

Before this, an agent ran `generate` for each picture and then `edit --broll`: two or more plans and approvals. Public docs of Descript, OpusClip and OpenChatCut describe one step for this; no code read.

```json
{"base_video": "talk.mp4", "captions": "karaoke", "style": "ink illustration",
 "cutaways": [
   {"at": 6.0, "seconds": 4, "prompt": "a drone shot over a desert road", "model": "fastwan22-self"},
   {"at": 12.0, "seconds": 3, "prompt": "a hand-drawn map of the coast", "still": true},
   {"at": 16.0, "seconds": 2, "file": "logo-card.png"}
 ]}
```

- Each cutaway has `at` and `seconds` (seconds of the base video) and one of `file` (a clip or still in the project, relative to the brief) or `prompt`. `model` names the row; default is the cheapest ready row that fits the shape (`image.gen` with `still: true`, else `video.gen`). That default can be a paid row when a paid key is set: name a free `-self` row to be sure.
- The brief's `style` goes at the end of each prompt. Rows are asked for 16:9 or 9:16 by the base video's shape; `edit` crops each picture to fill.
- `plan`: each generated cutaway is priced as one `generate` plan. Its stage is `cutaway<n>`, and the last stage is `edit` ($0). The plan id covers the base video's hash, the times, each part's plan id, the file hashes, captions, hook and logo. One `--approve` covers all of it. $0 needs none.
- `make`: each part runs `generate` on its own plan id, so a rerun resumes its provider job and pays nothing. Then `edit --broll` lays them over the base video: its own sound stays, captions (from its own speech), hook and logo stay on top. The result lists `generated` and `broll`.
- Refused with exit 2: cutaways with a `script`, `scenes` or `lines` (a re-voice with cutaways is not built), no `base_video`, both or neither of `file` and `prompt`, a missing file, times that overlap, run under 0.5 s or past the end.

## Measured, 8 Nov 2026, 2 cores

Base: 17.5 s, 1280x720, a test pattern with a Kokoro voice. Cutaway 0: `fastwan22-self` clip at 3 s for 4 s. Cutaway 1: `flux2-klein-4b-self` still at 9 s for 3 s. `captions: karaoke`, `style`.

| Check | Result |
|---|---|
| `plan --brief` | 0.08 s, $0, stages `cutaway0`, `cutaway1`, `edit`, one `plan_id` |
| `make`, first run | 53 s, $0, both rows answered, gate `av_drift` failed: my base file had video 18.77 s and sound 17.47 s (the base was wrong, not this step) |
| `make` on a fixed base | 10.8 s, $0, both parts resumed from their jobs (no new generation), `av_drift` pass, 17.5 s, -14.17 LUFS |
| Sheet of 9 frames, read by eye | the clip (a wet street with a drain) at about 4 and 6 s, the map still at about 11 s, the test pattern elsewhere; captions on top in every frame with the talk's words |
| Fake server, paid row | plan > $0, `make` without approval exit 4 and no submit, approved run 1 submit, a rerun no second submit; a `script` with cutaways exit 2 |

## Not done

A re-voiced base video with cutaways. `--detach`. A crossfade into a cutaway. A paid row in a real run (only the fake server).
