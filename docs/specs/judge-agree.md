# Judge: a frame fails only when every reply agrees (G148, G149, 6 Nov 2026)

## Faults

- G148: a list card shows each item when the voice says it. A frame 1.4 s into the card scored 2, "missing list card", exit 3. `settled` moved a frame only in the first second of a card.
- G149: the same run gave `555553` exit 0 and `334452` exit 3, at temperature 0. The replies fall in two fixed modes, so this is not sampling noise.

Cause of G149: the service URL has two pods behind one selector, `qwen38-27b-b` and `qwen38-27b-heavy`. They serve the same weights with `rope_scaling` factor 2.0 and 4.0. A client cannot choose a pod. The pods were not changed.

## Change

- `settled`: a frame anywhere between a card's start and its `landed` time moves to `landed`.
- `agree`: a frame under the bar gets 2 more asks. A frame fails only when every reply has it under the bar, and each frame takes its best reply. Replies on both sides of the bar give a finding with `status: warn`, and the exit stays 0. A retry that fails gives no opinion.
- Rubric: a plain or gradient background behind clear text scores 3 at the least, also in one scene of a brief that has footage elsewhere.
- New metric `judge_asks`. A failing judge now takes about 3 times as long and, on a hosted vision service, makes 3 requests.

## Measured, 8 runs on each run, 4 on the control

| Case | Before | After |
|---|---|---|
| `r_5b5f2f`, list card, no real fault | 0 of 8 pass | 8 of 8 pass |
| `r_b1c00e.1`, one plain scene, no real fault | 4 of 8 pass | 7 of 8 pass |
| Same video with a wrong brief (Formula 1 race), real fault | not run | 4 of 4 fail, score 1 |

## Limits

- The exit is still not fully stable: 1 of 8 runs failed on `r_b1c00e.1` (`442253`). Three replies from the same pod can agree.
- A frame that only one pod marks down passes when the first reply is from the other pod, because a first reply with no low frame is not asked again. This is the cost of no extra asks on a passing video.
- A frame can move to the same `landed` time as another frame, so a long card may use 2 of the 6 frames.
