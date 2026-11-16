---
name: scriptvox-prompts
description: Write the prompt for an AI-generated clip in a scriptvox video - one shot per narration line, 9:16 framing, no text in frame, one look across scenes, Gulf settings, and the free model first. Use whenever a brief scene has `generate`, when `scriptvox generate` is called, when a generated clip came out wrong, or when the user asks for AI footage, B-roll or a shot that stock cannot give. Load it together with scriptvox-brief; scriptvox-providers picks the model.
---

# scriptvox-prompts

A generated clip in scriptvox is B-roll under narration. ScriptVox adds the voice, the captions and the music, so the clip
only has to show one thing well for about 5 seconds. Most bad clips come from asking for too much.

## 1. One clip, one shot

Each clip shows the visual idea of one narration line. Start from the noun of that line, not from the script.

A 5 s clip holds:

| One | Not |
|---|---|
| subject | a group that interacts |
| action | a chain of actions |
| camera move | a move, then a cut, then another move |
| light | noon sun and deep night shadows together |

No cuts inside a clip. Simple physics only: walking, water moving, cloth in wind, traffic flowing, clouds. Crowds in
contact, splashes and explosions melt.

## 2. The prompt, in order

Write one short paragraph, 40 to 90 words, in this order:

1. **Shot and camera:** framing and one move. "Slow aerial push in", "low angle, slow tilt up", "locked-off wide".
2. **Subject:** what is in frame, with two or three visible traits.
3. **Action:** what moves, and how fast.
4. **Place:** what the camera sees around the subject.
5. **Light and colour:** time of day, light direction, three to five anchor colours.
6. **Style line:** the same line for every clip in the video (section 5).
7. **Clean tail:** "no people speaking, no text, no signs, no logos, no captions".

Write things a camera can see. "A calm morning" becomes "low sun from the left, long soft shadows, still water".
"Epic" becomes "low angle, wide lens, sun behind the tower". The word "cinematic" alone adds nothing.

## 3. Camera words mean one thing each

| Family | Words | What happens |
|---|---|---|
| The camera travels | push in, pull back, truck left, pedestal up, aerial drift | position changes, the view changes depth |
| The camera turns | pan, tilt | position stays, the view swings |
| Only the lens | zoom, focus pull | nothing moves, the image scales or the focus shifts |
| Nothing | static, locked-off | no move, no zoom, no focus change |

- Do not write "static" and a move in the same prompt.
- A move works better when the prompt says what it ends on: "slow push in that ends on the door".
- Bird's-eye means straight down. A drone at 45 degrees is a high angle.

## 4. Built for 9:16 and for captions

- Tall subjects suit a tall frame: a person standing, a tower, a palm, a street that recedes.
- Prefer vertical moves: tilt up, pedestal up, slow push in. Wide pans show little in 9:16.
- Keep the lower third calm: captions sit there. Put the subject in the upper two thirds.
- Set the frame with `--aspect` (or the brief `aspect`), not in the prompt text.
- Keep readable surfaces out of frame or out of focus: shop signs, screens, number plates, papers. Models write
  broken letters, and Arabic letters most of all. The caption track carries every word.

## 5. One look across the video

- Write one style line before the first clip: grade, colours, lens feel, time of day. Example: "soft warm grade,
  sand, teal and white, 35 mm look, late afternoon". Paste it unchanged at the end of every clip prompt.
- Use one model for all clips in a video when you can. Two models give two looks.
- Keep the time of day and the weather the same unless the story moves on.
- With no `character` in the brief, repeat a subject's traits word for word in each prompt; "he" or "the same man" gives another person. With `character`, see "The same person in every scene".

## 6. UAE and Gulf scenes

- Describe what the camera sees: pale sand, haze, palm shade, wind towers, Corniche water, glass towers, a dome far
  away. Name a real landmark only when the brief needs it, and check the clip against a photo.
- Gulf light is hard at noon. Early morning, late afternoon and blue hour look better.
- People: adults, plain dress words ("white kandura", "black abaya"), faces small or turned away. No children unless
  the brief needs them. No real person.
- For government work, no flags, emblems or uniforms in generated clips. Use real footage for those.

## 7. Model notes

Check rows and prices with `scriptvox providers --json`. As of 11 Nov 2026:

| Row | Cost | Notes |
|---|---|---|
| `wan22-ti2v-5b-self` | free (your own GPU service) | text to video, and image to video with `--image`; 5 s, 704x1280. The server adds a negative prompt. About 4 min a clip. Light, colour and "no people, no text" land well. Camera words change the framing, not the motion: measured 3 Nov on one scene, "slowly pans right" 0.0 px/s, no camera words 0.5, "handheld tracking" 0.9, "fast drone shot flying forward" 3.4 (an aerial view, almost still), against 11.5 for a Kling clip. For a real move use the scene's `camera` (scriptvox moves the clip) or a paid row. |
| `fastwan22-self` | free (your own GPU service) | text to video, and image to video with `--image`; 5 s. About 35 s a clip, so use it for fast drafts. It loses the exact colours of a `character` by frame 15: keep such a scene on `wan22-ti2v-5b-self`. |
| `minimax-h3-max-turbo` | about $0.23 for 5 s | strong on an explicit camera path. Order: subject, action, camera path, place, light. |
| `grok-imagine` | about $0.43 for 6 s | weak when a prompt stacks style labels or scene changes; give one beat and real scene detail. |
| `kling-2.6-pro` | $0.35 for 5 s | clear action order and plain camera verbs. Image to video available. Audio is turned off by the row. |

`kling-3.0-pro`, `kling-4.0-fal`, `seedance-2-fal`, `luma-ray-3.2-fal` and `gemini-omni-flash-fal` (added 7 Nov, paid,
text to video) have no prompt notes yet: nobody here has judged their clips. Read `scriptvox providers --json` for
their lengths and prices, and see `scriptvox-providers`, section 3.

For image to video (`--image`, or `image` in a brief scene): the image is the first frame. Describe only the motion
and the camera, never what the image already shows.

## 8. Spend little

1. Draft every new prompt on `wan22-ti2v-5b-self`. It costs nothing.
2. Move the same prompt to a paid row only when the free clip shows the composition works but the quality is not
   enough. Image to video is free on the Wan row too.
3. At most two paid tries per shot. Then make the shot simpler (less action, a simpler camera move), or use `stock`.
4. Always run `scriptvox plan` (or `scriptvox generate ... --plan`) and show the cost before `--approve`.

## 9. Judge the clip

ScriptVox checks every clip for length, size, black frames and frozen frames. A failure is exit 3, and the paid clip
stays in the run folder. The rest is your job:

1. `scriptvox snapshot <run_id> --sheet 9 --json` and look at frames from the start, middle and end of each clip.
   Today `snapshot` refuses the run id of a clip from `scriptvox generate` (exit 2): give it the file in
   `artifacts.video`.
   A good first frame can hide a melt at second 4.
2. Fail the clip on: letters or logo shapes, warped hands or faces, a subject that changes identity, a different
   camera move than asked, clutter in the lower third, or a look that breaks the style line.
3. Check that each frame fits the caption shown with it (`frames` gives the time and the caption).
4. Change one thing per retry and write down why: subject words, camera, light, or the model. A new prompt is a
   new plan, so the old paid clip is not reused by mistake.

## Stills or clips

`"generate": {"prompt": "...", "still": true}` makes one picture from an image row in about 10 s, with a slow push-in or the scene's `camera`, and it lasts as long as the scene. A free video clip takes about 35 s with an empty queue (`fastwan22-self`; 49 s on 11 Nov while another job ran) to 226 s (`wan22-ti2v-5b-self`, the default) for 5 s of motion. `plan` shows the time per row. Measured 5 Nov 2026: 2 scenes with stills, 31 s in all, real film-photo pictures. Use stills for an explainer, a draft, and any scene longer than its clip; use a clip when motion is the point.

## Plan the shots before you pay

- Write the shots as a list first: one line per scene with subject, action, framing and what ends the shot. Read it against the script. A wrong shot costs a line here and money later.
- Make one still per scene first: `scriptvox make --brief brief.json --storyboard --json`. The brief does not change. Every scene that would make a clip gets one still from an image row, on one sheet with the scene numbers. No clip, no voice, no render. A free image row runs at once. A paid one stops at exit 4 with the storyboard's own plan, which never covers a clip. `plan --brief brief.json --storyboard` shows it first.
- Read the sheet at `artifacts.storyboard` (the files are in `artifacts.stills`), fix the prompts, and run it again. A still that did not change comes from the store, free. Measured 11 Nov: 2 stills in 16 s, cost 0.
- Read `stills[].make_reuses`. For a scene with `first_frame` (a prompt for the first picture) it is true: the clip starts from the still you saw. For a scene with only `prompt` it is false: the clip is made from the words, and can differ from the still. To keep a good still, give its file to the scene as `"image": "scriptvox-out/<run>-storyboard/scene1.png"`, and write only the motion in `prompt`.
- A scene with a `character` shows the words only on the sheet: the reference picture is not made here.

## Two clips that must join

To make the next clip start where the last one stopped:

1. Take the last frame: `scriptvox snapshot clip1.mp4 --at <its length minus 0.1> --out last.png --json`.
2. Give it to the next scene: `"generate": {"prompt": "...", "image": "last.png"}`, on a row whose `modes` in `scriptvox providers` have `i2v`. (`first_frame` is a prompt for a new picture, not a file.)
3. In the prompt, write only what changes next. Do not describe the picture again.

To set the picture a clip ends on: `"last_frame": "end.png"` beside `image` or `first_frame` (or `generate --last-image`). It needs a row with `takes_last_frame` in `scriptvox providers`. No built-in row has it today, so `plan` refuses it until you add such a row.

Each link adds some drift in faces and colours, so do not chain more than 2 or 3 clips. This is a method, and scriptvox has not measured its drift. Check each join on a sheet.

## The same person in every scene

Text alone changes the clothes and the face from shot to shot, however exact the words. Add `"character": {"prompt": "a young woman engineer with short curly red hair, round glasses, green jacket"}` to the brief, or `{"file": "hero.png"}` for a picture of a real subject you have the right to use. The image row makes one reference picture once, and every still and first frame starts from it. A clip with no first frame gets one from its own prompt.

- Two people: `"characters": [{"name": "Sara", "prompt": "..."}, {"name": "Omar", "prompt": "..."}]`, and `"who": ["Sara"]` in a scene's `generate` (default: all of them). Write "the woman and the man" in the prompt. Measured 5 Nov: both hold alone and together.
- In each scene prompt, write "the same woman" and say only what changes: place, action, framing, light. Do not repeat the clothes.
- A scene without the person (a skyline, an empty room) needs `"character": false` in its `generate`, or the person can walk in.
- Read `.scriptvox/runs/<run>/character.png` first: the model can miss a colour you asked for (a yellow coat came out khaki), and that miss then carries through every scene. Name the colour again in the words, or give your own picture. A small colour drift between stills still happens (a cap went from blue-grey to brown-grey, 5 Nov).
- `resume` keeps the person when the revised brief keeps `character`. A changed one makes new pictures, and one taken out is dropped.
- A clip row reads words only: scriptvox puts the character's words in front of a clip prompt. Measured 5 Nov: the base Wan row (about 4 min) holds the person and the colours to the last frame. FastWan (35 s) holds the hat, glasses and scarf, but a navy cloth fades toward grey. For a character, use base Wan or stills. Check the last frame with `snapshot --at`.
- With `{"file": ...}`, add `prompt` too if the video has clips.

## One look across clips

Brief `style` (also in a brand file) goes at the end of every `generate` prompt, so write the look once: `"style": "ink illustration, muted colours, paper texture"`. Name the result, not the making: with "hand-drawn" FastWan painted a real hand and a pen into the shot (5 Nov 2026, r_922a33).

## 10. Example

Narration line: "By sunset the old harbour comes alive."

Prompt: "Slow push in at eye level toward a row of wooden dhows tied at a stone quay, ropes swaying, gulls crossing
the frame, calm water with small ripples, low sun from the right, long shadows, gold, teal and white, soft warm
grade, 35 mm look, late afternoon, no people speaking, no text, no signs, no logos, no captions"

Measured on `wan22-ti2v-5b-self` (2 Nov 2026, free, all clip checks passed): boats, quay, water and light came out as
asked, with no text. Three faults: the push in barely moved; the gulls landed on the water in the lower third, where
the captions go; the dhows looked like models. Fixes, one per retry: "birds high in the sky" instead of "gulls
crossing the frame"; "aged teak dhows with worn paint"; for a camera move that must read, a paid row (section 7).
Lesson: on Wan, light and colour words land well and camera moves land weakly. Write the shot so it still works if
the camera barely moves.
