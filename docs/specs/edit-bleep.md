# `edit --bleep`: hide a spoken word (G102, 10 Nov 2026)

Why: a recording can hold a word that must not go out: a swear word, a name, a number said by mistake. The idea is our own, from the one-line fact that some clip tools can bleep words; no other tool's code or docs were read.

## The rule

- `scriptvox edit talk.mp4 --bleep "word,two words"`. Each time the speech says an entry, that stretch of sound is replaced. The length does not change: this is not a cut. The picture is copied when no other option changes it.
- `--bleep-with tone` (default) or `silence`. The tone is 1 kHz. Its peak is the level (RMS) of the whole sound, so the tone is 3 dB under it; the master step then brings the file to -14 LUFS as before.
- The source is not mixed under the tone. Inside the stretch the source is exactly zero, except in a 10 ms fade at each end, so there is no click.
- Air: 0.05 s before the heard word and 0.2 s after it. `--bleep-pad <seconds>`, 0 to 1, sets both sides. Stretches that meet are joined. The task asked for about 0.04 s. The measurement below shows 0.04 s is not safe.
- The result has `bleep`: `matches` (the entry, the heard start and end), `replaced` (the stretches), `not_heard`, `with`, `pad_s`, `fade_s` and a note.

## Matching

- By the heard word times: the transcript of `analyze --transcript` (the same cache), or `--words <file>` (that transcript file, or JSON with `words`: text, start, end). With `--words` no speech model runs.
- Words compare as in the `words` gate (`gates::norm_word`): case, punctuation, Arabic marks and letter forms do not count. "Three" matches "3".
- An entry of several words matches heard words in a row, in that order.
- Whole words only. "pineapple" does not match "pineapples": give each form.
- An entry with no letter or digit is exit 2.
- An entry that was not heard is a `bleep_not_heard` finding, `warn`, with its name. The file is still written, with the other entries hidden.
- When no entry at all was heard: exit 3 and no file. A file at `--out` that still holds the word, from a run that ended with 0, is the dangerous case.
- With no speech row and no `--words`: exit 10 with the tool's usual message.

## Captions

- With `--captions` in the same call, each letter and digit of a hidden word is `*` (Arabic too). End punctuation stays, so sentences and pages do not move, and karaoke timing stays. `captions.masked` is the count. The SRT and VTT files hold the mask too.
- The captions use the same heard words as the bleep: one speech pass.
- With `--text` or `--captions-file` the captions can spell other words. The mask is set by the entry's text. A heard place with no such caption word on it masks the caption word whose middle is in the heard word's own time.
- A hook, a label and text already burned in the source are not changed.

## Other options in the same call

- Exit 2, with "use two calls": `--cut-silence`, `--cut-still`, `--cut-fillers`, `--cut-retakes`, `--keep`, `--keep-words`, `--speed`, `--duration`, `--from`, `--to`, `--lipsync`. They move the times or change the voice. Bleep first, then the other edit on the result.
- Allowed: `--captions`, `--captions-file`, `--text`, `--reframe`, `--clean-voice`, `--logo`, `--music`, `--hook`, `--label`, `--blur`, `--zoom`, `--broll`.

## How

- The normal `edit` pass writes into the work folder, not to `--out`. A second pass decodes its sound once to raw 48 kHz stereo samples; ScriptVox changes the samples of each stretch in that file (one stretch in memory at a time); FFmpeg then copies the picture and encodes the new sound into `--out`. So `--out` never holds the word, even when a run stops half way. If the second pass fails, no file stays.
- No word and no time reaches an FFmpeg filter or a shell. FFmpeg gets file paths and fixed arguments only. No new graph branch.
- Mono sound comes out as stereo.

## Run on 10 Nov 2026, FFmpeg held to 8 GB, free

Tests failed first ("unexpected argument '--bleep'", 4 of 4), then passed:
- `tests/edit_bleep.rs` (4, with `--words`, no model): silence is 60 dB under the sound around it and the rest keeps its level; the tone has 1 kHz and the 440 Hz source is 40 dB under it; the length is the same; "pine apple" matches only in that order; "banana" gives the warning; the SRT and ASS hold `**** *****.` and not the word; `--speed`, `--keep`, `--cut-silence`, `--from` are exit 2; nothing heard is exit 3 with no file; no speech row is exit 10.
- Unit tests in `edit.rs` (4, written after the code): matching in English and Arabic, exact samples of a replaced stretch, the mask, the mask on captions with other words.

Real runs with the local speech row and the Edge voice (`make`, then `edit --bleep ... --captions`, then `analyze --transcript` on the result):
- English, 9.73 s, "pineapple" said twice: matches 3.96 to 4.42 s and 6.56 to 6.94 s. The result's transcript has no "pineapple": "Today, I bought a fresh and some bread. My sister says the ____ is the best fruit". Length 9.73 s before and after. `check`: all gates pass, loudness -14.0 LUFS, silence 0.0 s, av_drift 0.3 ms. The same with `silence`, and "mango" gave the warning.
- Arabic, 14.10 s, "القهوة" said twice, asked as "القهوه": matches 5.38 to 5.94 s and 9.52 to 10.06 s. The result's transcript has no "القهوة". Length the same, all gates pass. But two words after the bleeps lost their first sound: "وبعض" was heard as "بعض", "أفضل" as "ضل". In this voice the heard ends were late (45 and 73 ms), not early, so the 0.2 s after the word was too much. With `--bleep-pad 0.05` the marker is still gone and both words are whole; the short "إن" in front of the second bleep was then not heard.
- A caption file of our own (`--captions-file own.srt`, the word in capitals in cue 1, not in cue 2): the word is `*********` in cue 1, and in cue 2 the word on the heard time is masked.
- Both voices are the free Edge voice (cost 0 in both receipts). The speech row was the local one (`setup speech-local`): no self-host key is set, and a paid row needs `--approve`, which was not given.

Measurement that set the air: the speech row's times against the exact word times of the made voice, 32 English words. Starts were up to 82 ms late, ends up to 185 ms early. With 0.04 s each side the second "pineapple" was heard again as "people". With 0.1 s before, "fresh" in front was heard as "f***". With 0.05 s before and 0.2 s after, the word is gone and the words around it stay.

## Not done, not run

- The air is fixed. A short word next to a bleep can lose a part (the Arabic run). A snap to the sound's own quiet edges is not built.
- No match by word stem or sound: another form of the word, or a word the speech row spelled another way, is not hidden. Read `not_heard`, and listen.
- Text burned in the source picture is not hidden (a `make` video shows the word in its own captions).
- Not run: a real speaker, a noisy room, a talk longer than 15 s, a paid speech row, 5.1 sound, Windows, the MCP tool call itself (only the tool list was read).
- Sound that starts later than the picture in the source is not handled apart.
