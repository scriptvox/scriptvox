# Stop after the voice: `make --until voice` (10 Nov 2026)

Before: `make` ran to the video. A user or an agent could not hear the narration and read the caption times before a long render, or before paying for clips.

- Flag `make --until <stage>`. The one stage is `voice`. Another value is exit 2 and names it and the stages offered (`make::UNTIL` is the list; `captions` or `clips` would be added there, each with its own stop). The MCP `make` tool has the flag too, because the tool list comes from the same command tree.
- `--until voice` runs the voice, the fit to `duration` and the caption timing, and stops with status `ok`. The run folder holds what those steps write in any run: the voice file (`voice_fit.wav` after a tempo change of a script of one part; with `scenes` or pause tags the fitted voice is `voice.wav`, and `pace.json` holds the pace), `words.json`, `captions.srt`, `captions.vtt`, `captions.ass` (with the hook, the cards and the end card text), the fonts and `request.json`. The answer names them in `artifacts` (`audio`, `words`, `captions_srt`, `captions_vtt`, `captions_ass`), and has no `video`.
- It does not run: clips, stock and open search, music, sound effects, the render, the gates, and the `media_match` pick (which sends frames to the vision service). No output folder is made and no file is written next to `--out`.
- The plan of `--until voice` lists the `voice` stage alone, and its cost is the voice's cost. It has its own plan id (`until` is part of it), so an approval of it is for the voice alone, and a later whole video of the same brief is another plan and another run. A paid voice needs `--approve` as in any run.
- Metrics: `until` (`voice`), `voice_s` (the voice file's length), `duration_s` (the length the video will have), `words`, `caption_pages`, `voice_rate_pct`, `voice_tempo`, `voice_reused`, `cost_usd`.
- `next` says how to go on: `scriptvox resume <run> --wait --json`. `resume` needs no new flag: it makes a child run from the saved request without `until`, and the voice comes from the cache (`voice_reused` true, no voice call). `resume --patch`, `--brief`, `--duration` and `--final` work as on any run; new words or another voice or pace make a new voice.
- `--out`: the stopped run writes nothing there. Its child writes the video to that name (`final.mp4`, not `final.1.mp4` as the child of a run that has a video).
- A paid voice and `resume`: the plan of the resume is the plan of the whole video. Its voice is in the cache, so the plan prices it at 0 and the resume asks for no second approval (see "Closed later on 10 Nov 2026" below). The voice comes from the cache: no new voice call, and the child's `cost_usd` has no voice in it. A resume with paid clips or paid music still asks, for those.
- `status`, `wait` and `--detach`: the stopped run is a finished run with status `ok`, exit 0 and no video. `wait` gives the same answer with the same `next`. The same command again with the same idempotency key gives the same stopped run.
- The brief checks before the voice are the checks of a whole run: a missing media folder, an unknown generator row or a missing key for a clip still stops the run before the voice. With `cutaways` (no voice is made) the flag is exit 2.

Run on 10 Nov 2026, worktree build, free Edge voice, 9:16, 31 words, `duration` 12, a hook:
- `plan --until voice`: one stage, `voice`, cost 0.
- `make --until voice`: status `ok` in 16 s. The run folder has `voice.mp3` (12.05 s after the fit read it at -15 percent), `words.json`, the three caption files, the fonts, `request.json`, `result.json` and `events.jsonl`, and no video; `scriptvox-out` has no file of the run. `status` shows `ok` with no video, and `wait` gives the same `next`.
- `resume <run> --wait`: the child made the 12.0 s video, `voice_reused` true, cost 0, all 8 gates pass.
- Tests: `tests/make_until.rs` with the stub worker (the plan has one stage and its own id; no MP4 anywhere and no `credits.txt` with `--stock`; the audio and caption files exist; `wait` and `status`; `resume` and `resume --patch` give a video with `voice_reused`; the fit before the stop; `--until clips` is exit 2 in `make` and `plan`; `--detach` then `wait`), and `tests/elevenlabs.rs` `until_voice_then_resume_is_one_paid_voice_call` with the fake API: no call before approval, one call at `--until voice --approve`, and the resume with no approval makes the video with one voice call in all and a child cost of 0.

Not done: `captions` and `clips` as stages; the `text_bounds` check at the stop (it runs in the resume). Not run: `--until voice` with generated clips, stock or `media_match` in a real run (the stub test with `--stock` and the plan cover them); a dub (`lines`) or a recorded `narration` with the flag; the flag through an MCP client.

## Closed later on 10 Nov 2026: the second approval

Seen by the owner: a first `make` paid $0.06 for the voice and stopped for another reason. The second `make` of the same script planned $0.06 again, asked for a new `--approve`, then made no call and cost $0. The `resume` after `--until voice` did the same.

Now the plan looks in the voice cache (`docs/specs/voice-plan-cache.md`). A voice that is there costs 0 in the plan, and a plan that costs 0 needs no approval. So `make --until voice --approve <id>`, then `resume <run> --wait` with no `--approve`, gives the video with one paid call in all. With `duration`, the stopped run also writes down how its fit ended, so the resume finds the re-read too.
