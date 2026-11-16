---
name: scriptvox-dub
description: Dub an existing video into another language with scriptvox - get a timed line sheet, write new lines that fit each source time, keep terms and names, plan, run, keep the background sound, optional free lip sync, and report what a person must still hear. Use when the user says dub, re-voice, translate the voice, Arabic version or English version of a video they have. Not for a new video (`scriptvox-brief`). For Arabic text rules, also load `scriptvox-arabic`.
---

# scriptvox-dub

You write the new words. No text model runs inside `dub`. Each line must be true to the source and must fit its time.

## 1. Get the sheet

```
scriptvox dub video.mp4 --plan --terms terms.json --json
```

- It costs nothing and writes `.scriptvox/dub/<stem>-lines.json`: `start`, `end`, `text`, `room_s` per line.
- No speech row: exit 10. `scriptvox setup speech-local` (about 490 MB) or `setup speech-large` (about 1.6 GB, better Arabic: 48 of 49 words against 45 on one test voice) gives one. Tell the user the size and wait for a yes.
- `terms.json` holds words speech to text must get right. They come back as `keep_terms`.
- Read the sheet. Fix any word speech to text got wrong before you translate it. Ask the user if a word is not clear.

## 2. Write the lines

Copy the sheet to `ar.json`. Change only each `text`.

Read all the lines first. Fix one form for each term and name, and use it in every line. A term the user named goes in exactly. In Arabic lines, a term written in Arabic letters gives a `terms` warning at `plan` ("not in the new lines"): say in the report that the change is on purpose.

- One source line gives one new line. Do not join, split, drop or reorder lines. The times belong to the source.
- When two wordings read equally well, take the one whose clauses come in the source's order. The viewer sees and hears the same point together.
- Spoken words, not written prose. Keep the speaker's tone.
- Short is part of correct: remove fillers, spare links and repeated subjects first.
- Never remove a "not", a number, a limit, a name, or a cause or condition, to save time. If a line needs its length, say so and let `plan` decide.
- One wording per line. No slashes, notes or the source text inside `text`.
- A number the voice must say: write it the way it is spoken. A short name that the voice reads wrong: use `say` in a brief, as in `scriptvox-brief`.

Arabic reads slower than English. Measured on our voices: English 190 to 212 words a minute, Arabic 105 to 110. Expect to cut more than you think.

## 3. Plan

```
scriptvox dub video.mp4 --lines ar.json --lang ar --terms terms.json --plan --json
```

- `line_fit` warns of a line that may run long. A warning for a term means that it is not in the new lines.
- A line reads up to 10 percent faster, then borrows the gap before the next line, then reads up to 25 percent faster.
- Past that, `make` stops with exit 3 before anything renders, and `line_budgets` gives the characters each line may have. Shorten those lines only. Do not slow the whole voice.
- Choose the voice with `--tts` and `--voice` from `scriptvox voices`. One voice, or one per speaker with `--voices` (see below). A paid voice needs `--approve` with the plan id, and the user's yes.

## 4. Run

```
scriptvox dub video.mp4 --lines ar.json --lang ar --json
```

- `--base-sound background` removes the old voice and keeps the music and ambience, on a free stem row. It is the default when that row has a key. `keep` holds all the old sound, low. `none` drops it.
- The result has the source's size, gate `source_size`. A frame far from a model's shape gives exit 2: use `edit --reframe` first.
- `dub` runs in the foreground. The 30 s eval promo took 9 min 50 s with lip sync. Warn the user first.
- `--lipsync`: moves the mouths on a free row (`latentsync-16-self`). It sends the video and the voice to that row. With no free row, `dub` says so. It does not pick a paid row.
- Burned text: warning `base_text`. New captions land on top of the old ones. Use a clean source, or report it.
- `--captions-second src.txt`: a small second caption line under the captions, one line of the file per sentence of the new lines, in order. Use it for the source's words.
- Two speakers: write `"speaker": "S1"` or `"S2"` on each line of the sheet, then add `--voices "S1=ar-AE-HamdanNeural,S2=ar-AE-FatimaNeural"`, both from one engine. `dub video.mp4 --plan --speakers` marks the lines for you, but only a paid row tells speakers apart today: it stops at exit 4 with a plan, so ask first. Checked to the plan stage only (11 Nov).
- Change one line: edit it and run again. Only that line goes to the voice, so the other lines stay as they were. The picture renders again.

## 5. Check and report

```
scriptvox snapshot <artifacts.video> --sheet 9 --json
```

The video path is in `artifacts.video`. Open the sheet and look at it. Then read `metrics.lines`: how fast each line read and what it borrowed. Report in three parts:

- **Measured:** gates, `words`, each line's rate, the size, the cost.
- **Seen:** the frames you read: captions, and any old text.
- **A person must hear and watch:** the voice, the meaning in the new language, the mouths. A still sheet cannot show that mouths match words. You cannot hear. Say both plainly.

Name each line you shortened, and each term you were not sure of.

## What this does not do

- It does not translate for you.
- It does not find a second speaker for free. Tell the user when a video has two speakers, and mark the lines by hand (section 4).
- It has no picture slowdown for a long line.
- The output voice follows that voice's terms. `scriptvox voices` shows them as data. The choice is the user's.
