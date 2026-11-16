# Audiogram: a waveform on the plain background (10 Nov 2026, G107)

Before: a voice-only video (a podcast clip, a narration with no footage) sat on the slow gradient and nothing followed the sound.

- Brief field `"waveform": true`. Off by default. A brief without it gives the same plan id, the same render key and the same frames as before.
- Where: only where the plain background shows with nothing in the middle. That is a video with no picture at all, or the scenes with no `media`, `stock`, `generate` or `open_image` and no `card`. Not during the first 1.6 s of a `hook`. A brief whose every scene has a picture or a card gets no wave and no message.
- Look: one filled line across the full width, a quarter of the height tall, in the `accent` colour (default the captions' yellow, `#FFE500`). The background colours do not change. The wave is under the captions, cards, insets, overlays, end card and logo, and over the `look`, so the accent stays true.
- Placement rule, as shares of the height: captions at the bottom (default) put the wave at 28 to 53 percent; `caption_at` `middle` at 12 to 37 percent; `caption_at` `top` at 40 to 65 percent. Captions never cover it.
- Sound: the final mix as heard (`master.wav`: voice, music, effects, after loudness), mixed to mono for the drawing. Not the voice alone.
- No sound: with `tts` `none` and no `narration`, `music`, `music_prompt`, `music_search`, `base_sound`, `sfx` or `ambience`, `plan` and `make` stop with a usage error (exit 2): "waveform needs sound".
- How: before the render, one small FFmpeg pass draws the wave from the mix into `wave-<hash>.mov` (QuickTime RLE with alpha, band size only) in the run folder. The render reads it as one more input and overlays it. The file is removed when the video is done.

## Chunks and resume

- The render still runs in chunks. A resumed render reads the wave file from its start and trims, as it does for every other input, so each chunk shows the sound of its own time.
- The file name is a hash of the mix's bytes and the wave settings (size, rate, colour). The render key holds the input names, so a changed sound, accent, size or `caption_at` is a new key and renders again; the same sound finds its chunks, also in a new run folder (no idempotency key needed).
- The wave pass always runs again on a rerun (about 4.5 s for 25 s of sound at 1080 px), so a file cut short by a kill is never used.

## Fault found

The first version drew the wave inside the render graph (`showwaves` beside the `gradients` source, then `overlay`). The two branches ran at different speeds and FFmpeg kept full frames in front of `overlay` with no limit: two renders reached 19 GB each and the kernel stopped the session. Measured by the coordinator on a 5.4 s render with FFmpeg held to 3 GB: "Cannot allocate memory" 3 times of 3 with `enable`, 1 of 3 without. With the wave as its own file: 3 of 3 pass, peak 112 to 125 MB, wave pass under 190 MB. Rule: no audio-to-video filter in the render graph.

Test `a_30_s_waveform_render_stays_inside_a_memory_cap` (FFmpeg held to 2.5 GB of address space) on the old graph, 3 runs: 1 gave "Cannot allocate memory"; the other 2 rendered and failed only the colour count, as that version also drew the wave pale (no `draw=full`). On the new design: 3 of 3 pass.

## Run on 10 Nov 2026

All with FFmpeg held to 4 GB, free, no key. Tests in `tests/audiogram.rs`. The first three failed first with "unknown field `waveform`"; the memory cap test was written later and failed first on the old graph, as above: the wave shows and the gates pass (accent pixels in the band, none without the field; `captions` and `text_bounds` read the same); a render stopped after 2 of 4 chunks resumes with no idempotency key and matches a clean render (PSNR 50 or more); the no-sound refusal; the memory cap.

Real runs, Edge voice, `music: calm`, sheets of 9 frames read by eye:
- 9:16 English, 9.8 s, accent `#40E0D0` on `night`: wave in the accent colour, above the captions, different in every frame. All gates pass.
- 16:9 Arabic, 13.1 s, defaults: the same; all gates pass. The default yellow wave on the yellow part of `dusk` is hard to see: pick an `accent` or a `background` that differ.
- 1:1 `--draft`, `tts: none`, `music: calm`, `caption_at: middle`: wave above the captions, clear at the bed's level.
- 16:9, three scenes, the middle one with a title card, `music: pulse`: wave on scenes 1 and 3 only.
- Kill and resume, 9:16, 25.1 s, 3 chunks, with `--idempotency-key`. Killed 4 s after "rendering": that was still the wave pass; the rerun finished from chunk 0, gates pass. Killed after chunk 1 started: the rerun gave `resumed_from_chunk` 1, gates pass, PSNR 54.7 dB against the clean render. `master.wav` had the same bytes in all 3 run folders.
- Peak FFmpeg memory, 25.1 s at 1080x1920: 747 MB with the wave, 719 MB without. Three plain scenes at 1920x1080, `tts: none`: 9.8 s gave 1097 MB with and 1063 MB without; 49.0 s gave 1086 MB with and 1062 MB without. It does not grow with the length.

## Not done, not run

- One shape only (`cline`). No bars or spectrum.
- The wave starts and stops at a scene cut with no fade.
- `waveform` is not a brand-file key and has no CLI flag.
- A long video: the wave file is about 1.6 MB a second at 1080 px while the render runs (41 MB for 25 s). Not run above 30 s.
- Not run: 4:5, a paid voice, `narration`, `base_video`, a `hook` with the wave, Windows and macOS.
- A killed run with no idempotency key leaves its wave file in the old run folder.
- The slideshow gate passed before and passes now; it was not shown to pass "better".

## The colour is checked before it reaches FFmpeg (10 Nov 2026, found by a review before the merge)

The accent can come from a recipe inside an MP4 (`remake`), so it is not trusted text. `wave_colour` gives the filter graph only a checked `#RRGGBB`; any other text gets the default yellow. Before the fix the accent went into `showwaves=...:colors=0x<accent>` as written, so a crafted value could add filter text. Unit test `only_a_checked_hex_colour_reaches_the_filter_text`.
