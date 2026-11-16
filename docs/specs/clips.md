# `scriptvox clips`: a long talk into ranked shorts (5 Nov 2026)

Gap: OpenMontage #24, a clip factory left to the agent's judgement; the MoneyPrinterTurbo audience asks for it. ScriptVox had the parts (`analyze --transcript`, `edit`) and a skill section with hand steps.

- `scriptvox clips talk.mp4 [--count 3] [--min 20] [--max 60] [--aspect 9:16] [--track] [--captions pop] [--lang ar] [--plan]`.
- The free speech row gives the sentences. The free text row picks parts that stand alone, with a score, a hook in the language of the talk, and a reason. The reply is cached by the sentences, so a rerun picks the same.
- Rust then checks each part: inside the list, stretched or cut by whole sentences to `--min` and `--max`, no overlap, best first, at most `--count`. A part that cannot fit is left out, so `count` is a maximum. No part left is exit 3.
- `edit` makes each short (`from` 0.15 s before the first word, `to` 0.2 s after the last): crop, captions, hook, -14 LUFS, one encode. Names: `scriptvox-out/<video>-clip<rank>.mp4`.
- `--plan` lists the parts with their times and words, and cuts nothing.
- `edit` refuses a ScriptVox MP4 made in another frame, so `clips` does too, before the transcript and with `--plan`. A hook the fonts cannot draw is left out. A short is never written over an earlier one (`talk-clip1-1.mp4`).
- Up to 20 minutes, the limit of `transcribe`.

Measured: 116 s English talk with 3 strong parts between a slide check, an annex note and budget lines: all 3 strong parts picked, none of the rest, hooks "One third of water vanishes", "Rain gardens beat expensive pipes", "The 24-hour storm test", scores 9, 9, 8. Plan 9 s (transcript included), cut of all 3 in 21 s, 22 s each, av_drift up to 40 ms, 5 gates pass, `export` works. Arabic, 59 s talk: picked the strong part, hook in Arabic ("ثلث مياهك يضيع قبل الصنبور"), 27 s, hook and captions checked by eye. The score is the model's opinion, not a measure of views.

Real talk (5 Nov 2026, rerun): TEDxGeneva 2014, 13.6 min, CC BY 3.0, from Wikimedia Commons, not in the tree. Plan 46 s; 3 shorts with `--track` 219 s. See daily item 159 for the 3 faults it found and the limits left.

