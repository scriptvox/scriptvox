---
name: scriptvox-sound
description: Choose and check the sound of a scriptvox video - the music bed (built-in, the user's file, or a generated bed from `music_prompt`), how it sits under the voice, and the final loudness. Use whenever a brief needs music, the user asks for a mood or style of music, says the music is too loud or wrong, asks for sound effects or a music-led edit, or a check reports loudness or silence.
---

# scriptvox-sound

ScriptVox mixes one voice and one bed. It always lowers the bed while the voice speaks, and brings the whole video
to -14 LUFS (the usual level for short-video apps). You choose the bed. You do not tune the mix.

## 1. Choose the bed

| Video | Brief |
|---|---|
| narrated explainer, calm | `"music": "calm"` |
| fast promo | `"music": "pulse"` |
| serious, sad or official message | `"music": "none"` |
| a style the built-in beds do not have | `"music_prompt": "..."` (generated; needs a music row) |
| a style the built-in beds do not have, and no GPU service | `"music_search": "calm piano"` (a free bed found with no key; it keeps its own licence in the credits file, so tell the user) |
| the user's own track | `"music": "track.mp3"` (inside the project) |

- Built-in beds have no licence question. For a user track, ask where it comes from and note its licence. A short
  track loops.
- `music` and `music_prompt` cannot both be set.


`music_level` sets how far any bed sits under the voice: `quiet`, `normal` (default) or `loud`. ScriptVox measures the voice and the bed, so the setting means the same with any voice and any music file. Measured 5 Nov on an Edge voice: `calm` sits 21, 17 and 15 dB under the speech; `pulse` 16, 12 and 9 dB, because its energy is low bass. All words were still heard at `loud`. To add a bed to a video that exists: `scriptvox edit talk.mp4 --music calm --json`.

## 2. Write a `music_prompt`

One line, in this order: style, tempo, mood, two or three instruments, then "instrumental, steady, under a voice".

Example: `"calm cinematic ambient, 80 bpm, warm and hopeful, soft piano, oud and low strings, instrumental, steady, under a voice"`

- Never name an artist or a song.
- Send no lyrics. With no lyrics, the row makes an instrumental.
- Avoid words that bring busy, bright sound (lead synth, hi-hats, vocals, choir). Prefer pads, soft piano, low bass
  and gentle percussion.
- Run `scriptvox plan --brief b.json --json` first. It shows a `music` stage with the row and the cost.
  - With `--local-only`, or with no `selfhost` key, the run stops with exit 10 and offers the built-in bed. Take that
    offer, or ask the user. With `--local-only` and a network voice, the exit 10 is about the voice first.
  - The generated bed row is MIT.

Measured on 2 Nov 2026: a 15 s bed took 5.7 s. But the piece faded out after about 9.5 s, so scriptvox asks for a bed
10 s longer than the video and cuts it.

## 3. Check the result

- Run `scriptvox check <video> --json` or `scriptvox analyze <video> --json`. The loudness must be -14 LUFS, within
  1 LU. Report any other value as a fault.
- A generated bed that ended early leaves a quiet tail. Signs are a `silence_share` well above the voice's own
  pauses, or the silence gate in `check`. If seen, make it again with another prompt.
- Do not run the video through another loudness tool after scriptvox. It changes the measured level.

## 4. When the user wants a change

| User says | Do |
|---|---|
| "music too loud" | `"music_level": "quiet"`, a calmer prompt, or `none`. |
| "wrong mood" | rewrite the prompt: change the mood word and the instruments, keep the order |
| "keep the original sound under the new voice" (re-voice) | `"base_sound": "background"` with `base_video`: a stem row removes the old voice and keeps its music and ambience under the new one; `"keep"` keeps all of it (old voice too). It replaces `music`. |
| "add a whoosh, some clicks" | `"sfx": true` in the brief: a whoosh into each scene cut, a pop when a card or list item appears, ticks while a stat counts. Measured 3 Nov: effect peaks about 15 dB under the voice peaks, master still -14 LUFS. On/off only in a brief. On a finished video: `scriptvox edit v.mp4 --sfx whoosh@12.0 --sfx pop@3 --json` (also `tick`, or a sound file in the project; `--sfx-level quiet\|normal\|loud`); a whoosh peaks at its time, the length stays. |
| "the scenes should sound like the place" | `"ambience": true` in the brief: each video scene (generated, stock or own clip) gets its own sound from an MMAudio row, about 12 dB under the voice. The terms of the row are in `providers`. Another choice is a built-in bed (`calm`, `pulse`) or `"sfx": true` (whoosh, pop, ticks) instead; a real recording the client owns can be the bed as `"music": "sea.wav"` (a file inside the project, one for the whole video). |
| "let my clip be heard" | `"sound": "under"` on a scene with the user's own `media` clip: its own sound plays 12 dB under the voice. `"keep"`: at the voice's level, lowered while the voice speaks; give that scene a short script with a pause, `"Listen. [pause 3]"`. The same part of the file as the picture, never slowed. Not for stock, found or generated clips, and a clip with no sound is refused. |
| "add a crowd, rain, a door" | not for stock or own media. `scriptvox generate --model mmaudio-v2a-self --video clip.mp4 --prompt "rain on a tin roof"` gives sound that follows the clip; `mmaudio-t2a-self` takes text only. Weights are non-commercial: tell the user. Do not add sounds by hand with FFmpeg. |
| "no voice, just music" | `"tts": "none"` with `music`: the script runs as captions at reading pace over the picture and the bed. |
| "I want to mix it myself" | `scriptvox export <run_id> --stems` also writes `narration.wav` and `music.wav` (the bed before its level and ducking). It needs a run id |
| "cut to the beat" | `"cut_on": "beat"` in a brief with `scenes` and `music` as `pulse` or a music file in the project: each scene boundary moves to the nearest beat when it is at most 0.25 s away; the voice and the captions do not move. Read `cut_on` in the result: `moved`, `stayed` and the reason for each boundary that stayed. A bed with no steady beat (`calm`) moves nothing and warns. Music from an address is refused: `fetch <address> --audio` first. Check a track before: `scriptvox analyze <file> --beats`, and use it only when `steady` is true. Say "near the beat", not "exactly on it": the grid is within about 40 ms. |

## 5. Report

Report the bed (its name, file, or prompt and row), the final loudness, and every sound finding from `check`.
