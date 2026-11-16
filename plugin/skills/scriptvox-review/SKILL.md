---
name: scriptvox-review
description: Review a finished scriptvox video against its brief and fix it - read the gate results and fit metrics, look at a contact sheet whose frames map to caption lines, grade the faults, edit brief.json and resume, at most two rounds. Use after `scriptvox wait` returns a video, or when the user says the video looks wrong, the captions are off, a shot does not match, or the voice is too fast or too slow.
---

# scriptvox-review

You check the video a viewer will see, then fix it through the brief. Code has already measured what can be
measured. Your eyes judge the rest. You cannot hear audio, so you judge sound only from numbers.

## 1. Read the result

From the `scriptvox wait <run_id> --json` result (a run that already finished: `scriptvox check <run_id> --json` gives
the same metrics; `scriptvox check <video> --json` for any MP4 gives the gates only):

| Field | What to look for |
|---|---|
| `findings` | any gate with `status` other than `pass`: take its `fix` line as the first finding |
| `not_checked` | what no gate looked at. Report it with any pass: never say "checked" for an item on this list |
| `coverage`, `limitations` | what each gate that ran looked at, and what a pass from it cannot show. Say the limit with the pass: "no black stretch of 0.5 s", not "no black" |
| `metrics.duration_s` | within 5 percent of the brief's `duration` |
| `metrics.voice_rate_pct` | 0 is best. Beyond -15 the read drags, beyond +15 it rushes |
| `metrics.voice_tempo` | 0.92 to 1.08 is the fitter's own safe band. Outside it, fix the word count |
| `warning` | read it out to the user if present |

When `voice_rate_pct` is past 15 either way, the `fit` finding gives the word count to add or cut: take it. By
hand, the words to keep are the words now divided by (1 + rate/100): +48 on 54 words keeps about 36 (cut a third,
not 48 percent); -16 on 60 words wants about 71. Numbers count as the words the voice says ("2005" is three).

### Listen with numbers

You cannot hear, so let the speech to text row hear for you. `make` already does it when the free row has a key
(gate `words` in its findings); for any other video run `scriptvox check <run_id> --hear --json`. Gate `words`
counts the script words heard in order: pass from 90 percent, warn from 75. Below that, a line was dropped or
garbled, or the bed drowns the voice: say which part, then remake the voice or use a calmer bed. Measured 3 Nov:
an English run 25 of 25, an Arabic run 17 of 18, the same English video under loud noise 0 of 25 (fail).
Gate `captions` warns when a caption page has almost no time or one or two words stay over 2.5 s: the captions
lost step with the voice. Open a snapshot at the time it names before you change anything.

## 2. Look at the frames

A `--draft` run has the same layout as the full video, so read its sheet the same way. The plain background moves slower, and the `words` gate waits for `resume --final`.

Run `scriptvox snapshot <run_id> --sheet 9 --json` and Read the PNG at `artifacts.image`. The result also has
`frames`: the time `t` of each frame, in grid order (left to right, top to bottom), and the `caption` on screen at
that time. `null` means the frame falls between two caption pages.

Tie each frame to the brief: the scene whose `script` contains the caption is the scene on screen.

Look closer where needed:

- the hook: `scriptvox snapshot <run_id> --at 0.6 --json`
- the scene cards, each once it has landed: `scriptvox snapshot <run_id> --cards --json`
- a doubtful shot: `--at` with the times of that scene, one frame near its start and one near its end

### A second pair of eyes: the judge

Run `scriptvox judge <run_id> --json`. A vision model scores 6 evenly spaced frames from 1 to 5 against the run's own
script, hook and scenes, and checks the text on screen against the caption. It sends those frames to your vision service
(`SCRIPTVOX_VISION_URL`, or the file `vision-url` in the config folder), and with neither it refuses. Exit 3 means at least one frame scored under 3 on all 3 asks: a low
frame is asked about twice more, because a service can answer the same request in different ways. A `warn` finding
means the asks disagree on that frame, so read that frame yourself. A frame inside a card is taken once the card has landed, so two frames inside one card can be the same frame.

- `findings` holds only the frames under the bar. Each has `frame` ("frame 2 of 6"), the time `t`, the `score`, the
  `detail` (what the model saw) and a `fix`. Every frame, passed or not, is in `frames`; `metrics.frames_scored` says
  how many. `next` holds up to 5 fixes for the whole video, most important first.
- A plain or gradient background is a valid choice when the brief asks for no footage. The judge is told to score
  those frames on the text and cards and not to ask for stock. If a fix still asks for stock in such a run, it is
  not a must-fix: keep the background unless the user wants footage.
- The JPEGs it judged are in the folder at `artifacts.frames`. Read the frame before you act on a finding: the model
  names what it sees well but can be wrong about why it matters.
- Measured on 3 Nov: the same clip scored 5 on every frame with a brief that described it, and 1 on every frame with
  a brief about another subject. Test-pattern placeholders scored 1 to 2. So a low score mostly means the picture
  does not fit the words, which is a must-fix below.
- Titles behind the subject (`"behind": true`): the judge is told the overlap is on purpose and checks only that the
  words read. Measured on 3 Nov: before that rule it scored every behind title 3 with "overlaps the subject" and
  passed an unreadable "ABU DHABI"; with it, "ABU DHABI" fails (2, `text_ok` false) and "CITY" over the same towers
  passes, the same in two runs. Read the `behind` finding of the run as well: it measures the hidden share.
- Captions are paged, so a sentence often goes on to the next page. The judge is told this. If it still says a
  caption is cut off, check the frame: only letters past the edge or clipped by a box are a fault.
- Hand eval 3 Nov (wrong picture on purpose: a beach for "farmers dug stone channels"): the judge failed both
  frames of that scene, the frame confirmed it, one `media` change and `resume` fixed it, and the judge passed.
- A pass from the judge does not cover the hook's line count or the speed of the voice. On 11 Nov it gave 5 to every
  frame of a run with a 4 line hook and a voice at +42 percent. Check `--at 0.6` and `voice_rate_pct` yourself.
- Other briefs: `--brief <file>`. Any OpenAI-compatible vision model: `--endpoint <url>/v1 --model <name>`.

## 3. Grade what you see

**Must fix** (the video does not go to the user like this):

1. A gate failed.
2. A caption does not match the brief's words (beyond punctuation), or is in the wrong language.
3. Arabic letters do not join, the word order reads left to right, or the full stop sits on the wrong side.
4. A picture contradicts its line, shows something irrelevant, or carries another brand's logo or watermark.
5. A caption box covers a face or text that is part of the shot.
6. The length misses the target by more than 5 percent.

**Fix if a round is left:**

1. `voice_rate_pct` beyond plus or minus 15, or `voice_tempo` outside 0.92 to 1.08.
2. The same picture on three or more frames in a row when the brief has more than one visual.
3. The hook wraps to more than two lines or is hard to read over the background.
4. The last frame does not land on the takeaway line.

**Not a finding:** a taste opinion with no rule behind it, the gradient colours, the caption font.

When you find one fault, check every other frame for the same fault before you fix anything.

## 4. Write each finding as a change

One line each: the time, what is seen, what the brief wants, and the exact edit.

```text
12.5 s | a beach, for "dug narrow channels" | scene 3 stock "falaj water channel" becomes "stone irrigation channel aerial"
0.6 s  | the hook wraps to 3 lines         | hook "Water older than the city you live in" becomes "Water older than the city"
metrics | voice_rate_pct is -16            | add about 14 words across scenes 2 and 4
```

The usual edits:

| Fault | Edit in brief.json |
|---|---|
| wrong or weak stock shot | a more concrete `stock` term: subject first, plus aerial, close-up or timelapse |
| no good stock exists | remove `stock` from that scene: the built-in background and caption carry it |
| a file shot is wrong | another `media` path, or remove it |
| caption text wrong | correct the scene `script` (captions come from the script) |
| dragged or rushed read | add or cut the words the `fit` finding names (words now divided by 1 + rate/100) |
| hook too long | 3 to 7 words |

## 5. Fix and look again

1. Save the edited `brief.json`.
2. Run `scriptvox resume <run_id> --brief brief.json --json`, then `scriptvox wait <new run_id> --json`. A look field
   that the new brief leaves out (hook, outro, logo, music, accent, look, captions) is dropped. Or give only the
   fields that change: `resume <run_id> --patch fix.json` with `{"hook": "New hook"}` (`null` drops a field).
   `resume <run_id> --only-scene 2` writes one scene as a short MP4, for a fix loop.
3. Review the new run from step 1.

Do at most two rounds. After that, report what is still wrong in plain words. Do not claim a fix you did not see in
a new frame.

To see what a fix changed, run `scriptvox compare old.mp4 new.mp4 --json`: `differs` lists the times, so you read only those frames. Pictures only; a small change on a fine picture needs `--threshold 0.005`, and a cut moves all later frames.

## 6. Hand it over

`scriptvox export <run_id> --json` writes one folder: the video, the SRT, a cover image, credits and `metadata.json`
(title, description, the terms of every part). `metrics.commercial_ok` and the parts in `metadata.json` are data: give them when the person asks.
A run with scene cards also gets `cover-1.jpg` to `cover-3.jpg` (`artifacts.covers`), each card once it has
landed. Open them and the hook cover, and offer the user the one that says most at thumbnail size.
For more copies add `--format webm,av1,hevc,prores,hls`. For an editor's own cut add `--overlay`: `overlay.mov` and
`overlay.webm` hold the captions and cards alone on a transparent frame. It needs a run id, not a video file.

A cover that is not a frame of the video:

```
scriptvox export <run_id> --cover-prompt "a wooden dhow leaving a harbour at sunrise" --plan --json
```

- Say what the cover shows, by the rules of `scriptvox-prompts`. An image row makes one picture in the video's shape
  (`--cover-model free`, `cheapest` or `best`, or a row name). `--plan` shows the row and the cost. Then run it
  without `--plan` when the cost is 0. A paid row stops at exit 4 and needs the user's yes for `--approve`.
- `cover.jpg` is then the made picture and `cover-frame.jpg` is the frame cover. No title is drawn on it. The same
  text and row again makes no call. The row counts in `commercial_ok`. Open the cover before you offer it.

Other copies for the hand-over: `--max-mb 16` (a copy under a chat or mail limit), `--gif` (`preview.gif`, the first
6 s), `--soft-subs en,ar=ar.srt` (caption tracks a player can switch), `--timeline fcpxml` or `otio` (an editor's
timeline), `--stems` (see `scriptvox-sound`), and `--title` and `--tags` for the texts in `metadata.json`.

## 7. Report

Give the user the video path from `artifacts.video`, the length, the cost, any fault left with its frame time, and
one line on what changed in each round.
