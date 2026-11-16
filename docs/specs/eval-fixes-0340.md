# Fixes from the 4 Nov 2026 hand eval of the formula and code cards

The eval agent (skills only) made both videos but hit these faults. Each fix was checked on a real run.

1. **Hook over a scene-one card.** The 1.6 s hook and a card on the first scene were drawn together (a code card
   half under the hook at 0.8 s). A card now starts when the hook is gone (`HOOK_S`, make.rs), for title, code
   and every other card; behind titles too. Checked by eye at 0.8, 2.6 and 6 s on the agent's code brief.
2. **`resume --brief` brought a deleted hook back.** The skills say "edit brief.json and resume", so the revised
   brief is the look of the new run: hook, outro, logo, music, music prompt, accent, look, captions, transition,
   sfx, upscale, smooth and ambience left out are dropped. Language, voice, length, aspect and media keep the
   parent's. Checked: resume without `hook`, the child request has none and its captions have no Hook line.
3. **`plan` did not warn about a short script on the open voice.** The open voice takes no rate and fits only
   within 16 percent, so `make` stopped after TTS (0.75x and 0.81x). `plan` now adds a `fit` warn finding with the
   words to add or cut (`fit::plan_warn`). Checked: the agent's 14 s brief warns ("Add about 14 words"), its
   11.5 s brief (which made a good video) does not.
4. **Open voice rate.** English `selfhost` measured 98 words in 23.9 s of speech over three runs on 4 Nov (4.1 words a
   second, 245 a minute; the 4.5 of item 64 came from one short run). Fitter and the brief skill table changed.
5. **Help names every voice engine** (edge, elevenlabs, kokoro, selfhost); the `scriptvox` skill listed two.

Not fixed: the sheet misses a card landing (the agent guessed `--at` times); a caption line may end on "pi" before
"r squared"; open voice word times are estimated.
