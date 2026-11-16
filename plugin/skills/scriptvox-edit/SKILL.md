---
name: scriptvox-edit
description: Work on a video that already exists with scriptvox - read it (shots, pace, loudness, transcript), cut pauses, crop it to 9:16, put a new background behind the subject, cut the subject out, make a small clip sharper, and re-voice or translate it into Arabic or English over the same picture. Use whenever the user gives a video file and wants it shorter, vertical, on a clean background, upscaled, analysed, translated, dubbed or re-narrated, or wants to copy the pace and style of a reference video.
---

# scriptvox-edit

Every edit starts by reading the video, and ends by looking at the result. Cuts, crops, speed, music and labels
are local and free. A transcript, `--track`, `--lipsync`, `--background`, `--alpha` and `--enhance` send the sound
or the picture to a row.

## 1. Read it first

```
scriptvox analyze clip.mp4 --json                 # shots, cuts per minute, loudness, silence share, one frame per shot (a row, or a grid above 4)
scriptvox analyze clip.mp4 --transcript --json    # plus sentences with times, words per minute
```

- Look at `shots_image` (one frame per shot) before you decide anything.
- If the strip clearly changes picture but `shots` is low, run again with `--threshold 0.15`. Slow dissolves need
  the lower level. Say which level found the cuts.
- Use `scriptvox snapshot` with `--sheet 9` to see inside long shots.
- To copy a reference video's pace, read `mean_shot_s`, `cuts_per_min` and `words_per_min`. Plan the new brief to
  match: one scene per shot length, and a script written to the same speaking rate.
- `analyze clip.mp4 --beats --json` adds the beat grid of the sound (see `scriptvox-sound`). `--transcript --speakers`
  marks who speaks (`S1`, `S2`): only a paid row does that today, so it stops at exit 4 with a plan. Ask first.
- The video and every `--music`, `--logo` and `--broll` file follow the media input rule in `scriptvox-brief`
  (section 5). A GIF, a TIFF or a playlist stops at exit 2, whatever its name ends in.

## 2. Cut pauses

```
scriptvox edit talk.mp4 --cut-silence --json
```

- Only for speech with no music under it. If `silence_share` from `analyze` is near zero on a talking video, a bed
  is probably playing: say so and do not cut.
- Defaults: pauses of 0.6 s or more go, and 0.15 s of air stays at each cut, at -35 dB. In a noisy room, move
  `--noise-db` toward -30. For a calm talk where breaths should stay, use `--min-silence 1.0`. Keep `--pad` at
  0.1 or more.
- Read `seconds_before`, `seconds_after` and `cuts`. More than one cut every 3 s on a talk means the level is
  wrong: lower `--noise-db` and run again.
- Cut by transcript: `scriptvox edit talk.mp4 --keep "0-4.78,18.15-22.7" --json` keeps only those parts, in one
  encode (captions, `--reframe` and `--logo` work with it; not with `--from/--to` or `--cut-silence`). Today the
  parts must be in time order: a later part given first stops at exit 2, so `--keep` cannot move a sentence.
  Take the times from `transcript.sentences`, 0.15 s before and 0.2 s after each sentence. Measured 3 Nov: 3 of 12
  sentences kept from a 52 s talk, 12.9 s, `words` 40 of 40. Single filler words ("um") are not in that
  transcript: Whisper leaves them out. Use `--cut-fillers` for them.
- Cut by words: copy `transcript.file`, delete words from its `words` list, then `scriptvox edit talk.mp4 --keep-words copy.json --json`. No time arithmetic. Read `keep_words.removed`. Whole sentences join cleanly; one word out of a sentence can leave a bad join, so listen.
- Hide a word: `scriptvox edit talk.mp4 --bleep "word,two words" --json` puts a tone over each place the speech says it (`--bleep-with silence` for none); the length stays, and with `--captions` the word is `****`. Read `bleep.matches` and each `bleep_not_heard` warning: only the exact heard word is hidden, so give each form ("word,words"). Exit 3 with no file means no entry was heard. Not with a cut or `--speed`: bleep first, then cut. Then `analyze --transcript` on the result, and ask the user to listen. `--bleep-pad 0.3` when a piece of the word stays. `--words t.json` (the `transcript.file` of `analyze --transcript`) gives the heard words, so no speech row is called again.
- A screen recording that stands still: `scriptvox edit demo.mp4 --cut-still --json` cuts the parts where the picture does not change for `--min-silence` or more (with sound, only where it is silent too).
- A face, a plate or a screen: `--blur "x,y,w,h@12-15"` (pixels and seconds of the source) blurs a fixed box. Read `blur_check`, a frame from each box. A blur is not proof of redaction: say so.
- Cut retakes: `scriptvox edit talk.mp4 --cut-retakes --plan --json` lists each sentence that a later take replaces. Read it, and ask about each `retake_differs`, because two meant sentences can look alike. Then run it without `--plan`.
- Pictures over a talk: `scriptvox edit talk.mp4 --broll road.mp4@12.0:4 --broll map.png@20:3 --captions --json`. The talk's sound stays, and captions stay on top. Cut first. A generated clip comes from `scriptvox generate` first, with its own plan.
- A slow push in: `scriptvox edit talk.mp4 --zoom 12-15@1.2 --json` (seconds of the result, zoom up to 2, or `12-15@1.2,0.3,0.4` with a centre). Cut first, then zoom. On spoken words: `--captions --emphasis "water" --zoom-on-emphasis`. New captions stay still. Burned text in the source moves with the picture, and `base_text` warns.
- Noise under a voice: `scriptvox edit talk.mp4 --clean-voice light --json`, then `medium` or `strong` only if it stays noisy. Read `clean_voice` for the floor before and after. A strong level can thin a voice and no gate hears that, so ask the user to listen. Every level also looks for mains hum (50 or 60 Hz and harmonics) and low rumble, and removes only what it finds: read `clean_voice.hum` (`removed_hz`, `db_before`, `db_after`) and `clean_voice.rumble`. A harmonic under the voice's own pitch can stay.
- Cut fillers: `scriptvox edit talk.mp4 --cut-fillers --json` (Arabic speech: add `--lang ar`). It cuts "um", "uh",
  "erm", "hmm", "اممم", "ااا", and "يعني" only when it has a pause on both sides, each with the pause around it
  (`--pad` of air stays). Each cut is snapped to the filler's own sound, because Whisper places a filler in the
  pause before it. Read `fillers.cut`, `fillers.words` (the times of the cut sounds) and `fillers.skipped` (a
  filler with no short sound of its own stays: it is said quickly between words and a cut would clip a word).
  Measured 4 Nov: English talk 5 of 5 cut, 14.0 s to 10.15 s, all 25 words remain, a second run finds none.
  Arabic 2 of 2 cut, all words remain. Not with `--keep` or `--cut-silence`; `--captions` and `--reframe` work with it.

## 3. Make it vertical

```
scriptvox edit wide.mp4 --reframe 9:16 --json
scriptvox edit talk.mp4 --cut-silence --reframe 9:16 --json   # both in one encode
scriptvox edit wide.mp4 --reframe 9:16 --track --subject "the camel" --json
```

- Without `--track` this is a centre crop that fills the frame, with no black bars. Check the strip first: if the
  subject sits off centre or moves across the frame, add `--track`. With a vision service set, a warning
  `centre_crop` says when the crop cuts the face or subject on the frames it read.
- `--track` asks your vision service for the subject's box about twice a second and moves the crop along it,
  holding still for small moves. Name the subject with `--subject` when there is more than one candidate. The
  frames it looked at are in `scriptvox-out/<name>.track/`, and `track.x` in the result is the path (0 left, 1 right).
- The path restarts at each cut that `analyze` finds (the crop jumps with the cut, it never pans across one), so
  `track.shots` should match the shot count of `analyze`. If it is lower (two shots with the same colours), the crop
  pans for up to half a second after that cut: look at a sheet of frames just after each cut before you hand it over.
- Text burned into the wide picture (subtitles, a lower third, a slide) is cut at both edges by a 9:16 crop. Check the
  strip for text before you crop; if it is there, ask for a clean copy, or tell the user the text will be cut.
- A ScriptVox MP4 made in another frame: `--reframe` refuses (exit 2), because its captions and cards are burned in for
  that frame. Make it again in the new frame: `"aspect": "9:16"` in the brief, then `scriptvox resume <run_id> --brief
  <file>`. `--crop-text` crops anyway. Found 4 Nov: a 16:9 run cropped to 9:16 lost the caption edges and the list card.
- Nothing may be lost: `--reframe 9:16 --fit blur` keeps all of the picture over a blurred copy. `join --fit blur` does the same.
- Do not crop twice. If the next step uses `base_video`, skip `--reframe` and set `aspect` in the brief instead.

## 3a. A video from a link, and which clean-up to use

scriptvox does not download from a site. The user runs this once, and only for a video they may use:

```
yt-dlp -f "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]" -o source.mp4 "<link>"
```

yt-dlp is a separate tool that the user installs. Its result is an ordinary file for every command here. Do not run it for the user on a video whose rights are not clear, and do not pass a site's terms off as cleared.

Pick one clean-up by what is wrong, and start with the smallest:

| What is wrong | Use | Not this |
|---|---|---|
| Long gaps, the talk is good | `--cut-silence` | `--cut-fillers`: it needs a speech row |
| "um" and "uh" | `--cut-fillers` | `--keep-words`: more work for the same cut |
| A sentence said twice | `--cut-retakes --plan` first | `--cut-silence`: it keeps both takes |
| Wrong or spare sentences | `--keep-words` | time arithmetic with `--keep` |
| A hiss or a hum under the voice | `--clean-voice light` | any level on clean sound |

For a whole talk in one order, read `scriptvox-talking-head`.

To mark a video as made with AI, in the user's own words: `scriptvox edit v.mp4 --label "Made with AI" --json` burns a small line in the corner. `export` lists the parts a model made as `ai_parts` in `metadata.json`. These are tools for the user: do not say that they meet a law.

## 3b. Caption a talking video, or cut a clip


You have an SRT or a word JSON? `--captions-file x.srt` uses its times, with no speech to text.

You have the script? `--captions --text script.txt` spells the captions from it, on the heard times. A text that was not said stops at exit 3.

Names in the captions come out wrong? Give `--terms terms.json` with `{"terms": ["ACME"], "fix": {"ak me": "ACME"}}`. It goes as a hint, and every change is listed in `term_fixes`. Tell the user what changed.

```
scriptvox edit talk.mp4 --captions --json                       # keeps its own sound, adds word captions
scriptvox edit long.mp4 --from 754 --to 801 --reframe 9:16 --track --captions pop --json   # a short from a long video
```

- `--captions` transcribes the video's own speech with a speech to text row and burns the same captions as
  `make` (`pop` for 1 to 3 big words). Give `--lang` when you know it; detection is right for clean Arabic and
  English. Check one sheet: Whisper can misspell rare names; if a name matters, re-voice with `make` instead.
- Brand it in the same pass: `--logo logo.png` (top right) and `--accent "#RRGGBB"` (the spoken-word colour; a
  dark colour is refused because it cannot be read on the caption box).
- Add a bed in the same pass: `--music calm` (or `pulse`, or a music file in the project) with `--music-level`. The speech ducks it, and with no other edit the picture is copied. A sound effect at a set time of the result: `--sfx whoosh@12.0` (or `pop`, `tick`, or a sound file in the project; repeat for more; `--sfx-level quiet|normal|loud`). Terms of a music file stay your duty: `export` marks the result as not cleared.
- A clip needs a reason to stop scrolling: `--hook "The one rule nobody follows"` shows a big headline for 1.6 s, and `--emphasis "ten minutes,free"` marks words of the speech in the captions. The words of `--emphasis` must be the words Whisper hears, so read `analyze --transcript` first.
- For shorts from a long talk, start with `scriptvox clips talk.mp4 --plan --json`: it lists the parts that stand alone, each with a score, a hook, a reason and its words. Read them, then drop `--plan` to cut. The score is the model's opinion. It knows the words only, so it cannot see a bad picture: look at each sheet.
- To pick clips yourself from a long video: `scriptvox analyze long.mp4 --transcript --json`, read `transcript.sentences`
  (start, end, text; the word list stays in `transcript.file`), choose parts that start and end on a sentence,
  20 to 60 s, then one `edit --from --to` per clip. Start about 0.15 s before the first sentence and end about
  0.2 s after the last, so no word is clipped. A sentence also ends at a pause over 1.2 s, for speech without
  punctuation.

## 4. Re-voice or translate

1. `scriptvox analyze source.mp4 --transcript --lang <source language> --json`
2. Write the new script from the transcript. You are the translator:
   - Keep names, product words and numbers exactly as in the source.
   - Arabic runs longer than English for the same idea, so write it tight (scriptvox-arabic).
   - Where the transcript has a gap over 0.8 s, put a `[pause N]` tag of that length, so each sentence stays near
     its picture.
3. Write a brief with `script`, `lang`, `base_video` (a file inside the project), and `duration` set to the source
   length from `analyze`. Add a voice (`tts`, `voice`, or `tts: "selfhost"` for the free open voice), and
   `music_prompt` or `music` if wanted.
4. `scriptvox plan --brief b.json --json`, then `scriptvox make --brief b.json --json`.

What to know:
- The base video plays from the start; its own sound is dropped unless `base_sound` is set. If the voice is longer than the video, the last
  frame holds; if shorter, the video ends early. `duration` prevents both.
- **Burned-in captions:** if the source already has captions in the picture, the new captions land on top of them.
  This was seen on 2 Nov 2026. `make` now warns with the finding `base_text` (a vision model looks at 3 frames);
  report it to the user. Use a clean source. For a video scriptvox made, use
  `scriptvox resume <run_id> --brief <translated brief> --json`, which rebuilds from the clean clips: a clip or still with the same row and prompt comes back free from the cache, the voice too if its text is the same (`reused_scenes`, `voice_reused`). Change a word of a prompt for a new take.
- People talking on screen will not match the new words. Warn the user. Lip sync is an option where a `latentsync-16-self` row shows in `scriptvox providers`.
- The short way to dub on the source's times: `scriptvox dub video.mp4 --plan --json` writes a sheet of timed lines. Copy
  it, put the new language in each `text`, then `scriptvox dub video.mp4 --lines ar.json --lang ar --plan --json`, and the
  same without `--plan` (with `--approve` if it costs money). You write the words: no text model runs. The result has the
  source's size, gate `source_size`. Change one line and run again: only that line goes to the voice. `--lipsync` needs a
  free lip sync row.
- To keep each line on the source's own time, write `lines` in place of `script`: `[{"start": 5.3, "end": 12.76, "text": "..."}]`,
  with times from `analyze --transcript`. A line reads up to 10 percent faster, then borrows the gap before the next
  line, then reads up to 25 percent faster. Past that, `make` stops with exit 3 and `line_budgets` gives the
  characters each line may have, before anything renders. `metrics.lines` says what each line did.
- The original's own sound: add `"base_sound": "background"` for a dub that keeps its music and ambience (the old voice
  is removed by a stem row, Demucs), or `"keep"` for all of it. Measured 3 Nov: an English video with music,
  dubbed to Arabic in 29 s; Whisper heard 0 of the 20 old English words in the kept background.
- `make` writes the SRT next to the video and `export` copies it; `edit --captions` writes one next to its output.

Measured on 2 Nov 2026: a 6.8 s Arabic video was transcribed (17 of 17 words right), re-voiced in English with the
open voice and a generated bed. It took 13 s and cost $0. The voice was slowed 15% to fill the length.

Clip factory hand eval, 3 Nov: a 4 min 53 s talk (no burned captions), `analyze --transcript` in 59 s gave 40
sentences; sentences 3 to 7 cut with `--from 14.21 --to 51.16 --captions --reframe 9:16` in 6 s, 36.95 s long, `words`
heard 96 percent, -14.9 LUFS. `edit` now brings the sound to -14 LUFS like `make` (it came out at -20.6 before).

## 3b2. Change the speed

`scriptvox edit in.mp4 --speed 1.25 --json` plays faster or slower, 0.5 to 2. For one part only: `--speed "12-15@0.5"` (seconds of the video, then the speed; commas for more) with a short ease in and out; read `speed.ranges` for where the part is in the result, and the `.srt` and `.vtt` beside the input are written again beside the result on the new times. The voice keeps its pitch. It mixes with cuts, `--reframe` and `--captions` (the captions are heard after the speed change). Not with `--bleep`, `--zoom` or `--broll`: use two calls. For an exact length, `--duration 30` sets the speed for you, inside the same 0.5 to 2.

## 3c. Join clips

`scriptvox join a.mp4 b.mp4 c.mp4 --transition fade --json` gives one video, free and local. The first clip sets the frame and rate (or `--aspect`). A clip of another shape gets bars, or use `--fit crop`. A clip with no sound gets silence, and the result is mastered to -14 LUFS. Read gate `av_drift` in the result: exit 3 means the sound and the picture differ in length. The file is kept, but do not hand it over as good. Report it.

## 3d. A new background, a cut-out, a sharper clip

```
scriptvox edit talk.mp4 --background "#102040" --plan --json   # or `blur`, or a picture or a clip in the project
scriptvox edit talk.mp4 --alpha --plan --json                  # the subject alone, for an editor
scriptvox edit small.mp4 --enhance --plan --json               # larger and sharper
```

- Each sends the picture, with no sound, to a row: a matte row for `--background` and `--alpha`, an upscale row for
  `--enhance`. Run `--plan` first. Tell the user the row, the cost and the time, then run the same command without
  `--plan`. A paid row stops at exit 4 and needs the user's yes for `--approve`. No row with a key: exit 10.
- `--background`: the subject stays and the rest is replaced. At most 60 s. Not with a cut, `--from/--to`, `--speed`,
  `--reframe` or `--lipsync`: cut and crop first, then the background on the result. Captions, `--hook`, `--label`,
  `--logo` and `--music` work in the same call. A second background on the same clip makes no new call.
- `--alpha`: a ProRes 4444 `.mov` with the sound and a WebM with alpha beside it. Alone: no other edit option.
- `--enhance`: the short side goes to the row's size (1080 on the built-in row), the sound and the length stay. At
  most 10 s. Alone: no other edit option. Read `enhance` in the plan for the sizes and `takes_s_about`.
- No gate judges the mask (`not_checked` names `matte`). Look at a sheet: the old background's colour can stay in
  fine hair, and a chair can count as the subject. Measured 11 Nov: a 5 s clip, one person, a clean edge, 14 s.

## 4b. Make one like this

When the user gives a video and asks for the same style with other content:

1. `scriptvox analyze reference.mp4 --like new.json --json` (about 10 s, free rows, needs a local file).
2. Open `new.json`. Frame shape, rate, length, scene breaks and transition come from measurement. `look`, `captions` and each
   `generate.prompt` come from the vision row. A `like` finding says which parts are missing.
3. Replace every `script` with new words. The file holds the reference's words only to show length and tone. Do not
   publish them.
4. Change the visuals you want: `media` for the user's clips, `stock` for stock, or keep `generate`. `plan` prices it and shows the time: add `"still": true` to a `generate` for a picture in about 10 s instead of a clip in minutes.
5. `scriptvox plan --brief new.json --json`, then `make`. Compare `snapshot --sheet` with `shots_image`.

Measured on 5 Nov: a 55 s video gave 5 scenes in 9 s; with new words and FastWan the result kept the warm look and the kind of shots.
Copy a style, not a person, a brand or a protected scene. The prompts name none of these; keep it so.

## 5. After every edit

- `scriptvox check <output.mp4> --json` and one `scriptvox snapshot <output.mp4> --sheet 6 --json`.
- Report the output path, the length before and after, the cuts, the loudness, and every gate finding.
- Never report a fix you did not look at.
