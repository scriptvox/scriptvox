---
name: scriptvox-talking-head
description: Turn a recording of a person talking into a video ready to post with scriptvox - read the transcript, make one keep and cut list, cut retakes, fillers and pauses, set the opening, then add zoom, B-roll, a vertical crop, clean sound and captions. Use when the user has a talk, lecture, interview, podcast video or a to-camera recording and says clean it up, cut the mistakes, remove the ums, tighten it, or make it postable. Not for picking short clips from a long video (`scriptvox clips`), and not for a video with new narration (`scriptvox-brief`).
---

# scriptvox-talking-head

The words decide the cuts. Fix the words first, then the picture, then the sound. Each later step reads its times from the cut result.

The viewer must not notice the edit, and the speaker must still say what they meant.

## 1. Read everything first

```
scriptvox analyze talk.mp4 --transcript --json
```

- Add `--lang ar` for Arabic. No speech row: exit 10, and `scriptvox setup speech-local` (about 490 MB) or `setup speech-large` (about 1.6 GB, better Arabic) gives one. Tell the user the size and wait for a yes.
- A name or a term that speech to text gets wrong: give `--terms terms.json`.
- Read the whole transcript before you cut. A sentence that looks spare may carry a later point.
- Write one list, by the speaker's points: each sentence, keep or cut, and why. Show the list to the user when the cuts change the meaning.

## 2. Cut the words, in one pass

Pick one way. These do not mix with each other, except as the table says.

| Goal | Command | Know this |
|---|---|---|
| Delete any words | copy `transcript.file`, delete items from its `words`, then `edit talk.mp4 --keep-words t.json` | No time arithmetic. Measured: 346 of 351 kept words heard again. One join read wrong ("their water" as "the rainwater"), so check a cut inside a sentence |
| Keep whole sentences | `edit talk.mp4 --keep "3.0-9.5,14.2-51.1"` | The parts must be in time order: a new order stops at exit 2. For a new order, cut each part as a file and `join` them |
| Retakes | `edit talk.mp4 --cut-retakes --plan --json`, read it, then run without `--plan` | Finds a sentence said again within 10 s, with near-same words. Read each `retake_differs`: a number, a name or a "not" may differ, so ask which take is right. Takes in different words are not found: use `--keep-words` |
| Ums | `edit talk.mp4 --cut-fillers --json` | Read `fillers.skipped`: a filler said fast between words stays |
| Long pauses | `edit talk.mp4 --cut-silence --min-silence 0.6 --pad 0.15 --json` | Those are the defaults. A longer `--pad` keeps more air |

For more than one goal, run them in this order, each on the file before it: retakes, fillers, pauses. Name each result with `--out` (`--out scriptvox-out/cut.mp4` on the last), because the default name grows to `talk-edit-edit-edit.mp4`. Read gate `av_drift` after each call. A hidden word (`--bleep "word"`, see `scriptvox-edit`) comes before any cut.

Rules for the list:

- Keep the speaker's order.
- Cut between words, never inside one. Cut a false start, the talk before the take, and a part off the subject.
- Cut waiting, not breath. A pause after a turn or a joke is part of the talk. Raise `--min-silence` if the result runs like a machine.
- With two takes, the last is usually better, not always. If you cannot tell, keep both and ask.

## 3. The opening

- `--hook "text"` shows a headline for the first 1.6 s, and moves nothing.
- To move a line to the front: `--keep` refuses a new order (exit 2), so cut the line as its own file (`edit talk.mp4 --keep "20.4-23.4" --out open.mp4`), cut the rest (`--out rest.mp4`), then `scriptvox join open.mp4 rest.mp4 --transition cut`. Read gate `av_drift` of the join.
- With no line that stands alone, cut the greeting and start at the first point.

## 4. The picture, on the cut result

Run this on the output of step 2 (`scriptvox-out/talk-edit.mp4`, or the `--out` name of a chain). Read the new times with `analyze scriptvox-out/talk-edit.mp4 --transcript --json`. Times in `--zoom` and `--broll` are seconds of the result.

- `--zoom "12-15@1.2"`: an eased push in. Use it on a stressed line and to hide a jump cut. On every line it stresses nothing. `--zoom-on-emphasis` gives 1.12x on each `--emphasis` word.
- `--broll "road.mp4@12.0:4"`: fills the frame for 4 s, the speech continues. Start on the word it shows, and end in a pause. Show what the words do not say. With no real picture, stay on the face. Paid or generated pictures need the user's yes first.
- `--reframe 9:16`: a centre crop. Warning `centre_crop` means that the face is cut. Then use `--track --subject "the speaker"`, or `--fit blur`, which keeps the whole picture over a blurred copy.

- A bad room behind the speaker: `--background blur --plan` or a colour. It sends the picture to a matte row, at most 60 s, and not in one call with `--reframe`: see `scriptvox-edit`, section 3d.

Not available: multi-camera, and a crop that follows whoever speaks. Say so. For two people, `--fit blur` is the safe choice.

## 5. Sound

- `--clean-voice light`: start here. Read `clean_voice.noise_floor_db_before` and `_after` (null when no noise floor was found). Measured on added noise: -44 dB to -49 (light), -50.5 (medium), -52 (strong), no words lost. A strong level can change a voice, and nobody has listened to it on a real talk. Do not run it on clean sound.
- `--music calm`: only if the user asks. The speech ducks it. `edit` brings the result to -14 LUFS.

## 6. Captions, last

- `--captions`: from the final speech.
- `--captions --text script.txt`: the speaker's script gives exact spelling, on the heard times. Gate `words` says how much matched.
- `--captions-file talk.srt`: the user's own file, with no speech to text.
- `base_text` warning: the source has burned text. New captions land on top. Report it.

## 7. Check and report

```
scriptvox check scriptvox-out/talk-edit.mp4 --hear --script keep.txt --json
scriptvox snapshot scriptvox-out/talk-edit.mp4 --sheet 9 --json
```

Open the sheet and look at it. Then report in three parts:

- **Measured:** length, cuts, gates, noise floor.
- **Seen:** the frames you read, and what they showed. `not_checked` lists what no gate covered. `limitations` says what a pass cannot show.
- **A person must hear:** each join, the pace, the music level. You cannot hear. Never write "smooth".

Say also what opens the video: a `--hook` headline, or the first point after a cut greeting.
