# The plan prices a paid voice by what the run must buy (10 Nov 2026)

Before: the plan priced a paid voice by the script and did not look in the voice cache. A second `make` of the same script, or the `resume` after `make --until voice`, showed the price again and asked for `--approve`. Then it made no call and cost $0. A plan that asks a person to approve money that is not spent teaches them to approve without reading.

## The rule

- A voice is made in parts. A script with no pause tag is one part. Each pause tag (`[pause 0.5]`) cuts it. A brief with `scenes` is cut at each scene too (since 13 Nov 2026 for every such brief with a made voice and no `media_match`, before only when a scene named its own `voice`; `docs/specs/resume-scene-voice.md`).
- Each part has one key in the voice cache: the engine, the text the engine gets (after `say` and the symbol words), the voice id, the rate, and the engine marks. One function, `make::voice_key`, makes this key. `synthesize` and the plan both call it, so the plan looks for the same keys the run asks for.
- The plan asks the store for each key. It makes no call, no download and no write.
- Nothing in the cache: the price is the one of every earlier version (the characters of the script, times the price, up to the next cent).
- Some parts in the cache: only the characters of the other parts are priced. The `voice` stage says `1 of 2 parts already made: only the rest is priced`.
- All parts in the cache: cost 0. The stage says `already made: no call and no charge`. A plan that costs 0 needs no `--approve`.
- Not looked up, so priced in full as before: dub `lines` (each line may be read again at a pace that is known only after its first read) and a brief with `narration`. Free engines are not looked up: their plan text and plan ids do not change. One exception since 13 Nov 2026: the plan of a `resume` child with `scenes` looks each scene up, free or paid, to say which scenes are voiced again (`docs/specs/resume-scene-voice.md`).
- `voices --preview` has the same rule: a preview that is in the cache costs 0 and needs no approval.

## The re-read for `duration`

With `duration` and no `voice_rate`, the fit may read the voice one more time at another pace. That pace comes from the measured length of the first read, so the plan cannot know it before the first read exists.

- First read not in the cache: 2 reads are priced, as before.
- First read in the cache, and no run has fitted it to this `duration` yet: the first read costs 0, and one whole re-read is priced. The stage says `a re-read at another pace may be needed to reach the duration: up to $X`. This plan asks for approval. If the fit then needs no re-read, nothing is spent.
- First read in the cache, and a run has fitted it to this `duration`: that run wrote a small note in the store (key: the keys of the first read, the pauses, the duration, the word count). The note says "no re-read" or "re-read at N percent". The plan then prices 0 for "no re-read", or looks up the parts at N percent. This is true for a `resume` and for the same command again. The note is written before the re-read is asked, so a run that stops there still leaves the true pace.
- A run made before this change left no note: its next plan prices one re-read, one time.

## The plan id and an older approval

The plan id does not hold the cost of the voice. It holds the script, the voice and the listed price. This is the rule of generated clips (`docs/specs/gen-cache.md`: the id holds the full price, not the store's discount). So:

- A cached voice does not change the plan id. Only the cost changes.
- An `--approve` of that id from before still matches. The run spends what the plan shows now, which is less or 0.
- So one plan id can show two prices: the listed one, and a lower one when parts are in the cache. An approval of the id is for the listed bill, as for clips. If the cache loses a part between the plan a person saw and the approved run, that run pays for the part again: more than the lower price it showed, never more than the listed price of the id.
- Nobody is asked to approve again for something that costs 0.

## A run with no approval can not spend more than its plan showed

- The voice's spend cap is the plan's cost when the run has no `--approve` of this plan id. `--max-spend` alone is not an approval. Before, `--max-spend` was the cap in that case; no path got there with a paid voice, because every paid plan needed an approval.
- So when the plan said 0 and a cache file can not be read back, the paid call is refused before any request (`SVX_SPEND_CAP`, exit 4). The store drops an entry whose file is gone, and the next plan shows the price again.
- The same holds when the fit chooses another pace than the note said: the note is corrected first, the call is refused, and the next plan shows the price.
- The ledger: a cached voice reserves nothing and commits nothing. The run that paid keeps its row. `metrics.cost_usd` of the later run is 0.

## What was run

Tests in `tests/elevenlabs.rs`, with the real worker and a fake ElevenLabs server that counts calls. Six of them failed before the fix (exit 4 where 0 is expected, or cost 0.01 where 0 is expected) and pass after it:

- `a_cached_paid_voice_costs_0_in_the_plan_and_needs_no_approval`: 1 call with approval; `--plan` gives the same id, cost 0; the same command with no approval gives a video, 0 new calls, `cost_usd` 0, 1 ledger row. Other words: the full price, exit 4, 0 new calls.
- `until_voice_then_resume_is_one_paid_voice_call`: `--until voice --approve`, then `resume` with no approval: a video, 1 call in all.
- `only_the_changed_part_of_a_voice_is_priced`: 2 parts, 2 calls; the second part changes; the plan prices less than the whole, exit 4 with no approval, and 1 new call after approval.
- `the_re_read_for_a_duration_is_priced_until_its_pace_is_known`: `--until voice` with `duration`, 2 calls (read and re-read at +11 percent); the `resume` with no approval makes the video with 0 new calls; the whole `make` of the same brief costs 0 and makes 0 calls; another `duration` prices one re-read and stops at exit 4.
- `a_cached_voice_that_cannot_be_read_back_is_not_bought_without_approval`: the cache files are deleted; with `--max-spend 5` and no approval the run stops at exit 4 with 0 calls; the next run asks for approval with the price.
- `a_voice_preview_makes_no_paid_call_without_approval`: the preview's plan after a paid preview: same id, cost 0, and it runs with no approval and no call.
- `a_free_voice_keeps_its_plan_id`: 2 free briefs give `p_e0dbc84426203d19` and `p_f7d1a72f7463daa5`, the ids the build before this change gave.

One real run with the free Edge voice, two times: see the end of this file.

## What was not run

- No call to the real ElevenLabs API. Every paid path ran against the fake server only.
- Dub `lines` with a paid voice in the cache: not looked up, so its second run still shows the full price and asks for approval, then spends nothing for cached lines. Left open.
- A scene with its own paid `voice` (the cut at scenes): the code path is the same list of parts, but no test has two paid speakers.
- A paid `selfhost` or `kokoro-rust` row: none has a price today.
- In the case of a cache file that can not be read back, the fix text of `SVX_SPEND_CAP` still says to raise `--max-spend`. That does not help there: the next run of the same command shows the price and asks for approval. The text is not changed here.
- The store now drops an entry whose file is gone for every kind of entry (free voices and clips too), not only for a paid voice. Before, the entry stayed and each read was a miss.
- Generated clips keep `--max-spend` as their cap without an approval. A clip the plan found in the store that can not be read back could be made again inside `--max-spend`. Not changed here.

## The real free run (10 Nov 2026)

Worktree build, free Edge voice, 9:16, 2 scenes with a pause tag, `duration` 8, its own empty data folder:

- Plan ids of 2 free briefs, from the main tree's build and from this build: `p_e0dbc84426203d19` and `p_f7d1a72f7463daa5` both times. The `voice` stage text has no new words.
- `make`: status `ok`, 8.0 s, `voice_rate_pct` -35, cost 0, 7 gates pass, and `fit` warns about the slow pace (the script is short for 8 s).
- `make` again: the same, with `voice_reused` true.
