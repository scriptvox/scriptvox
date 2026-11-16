# Notes: scriptvox-prompts

Written 2 Nov 2026 before the skill. Full read of (OpenMontage, AGPL, ideas only): `skills/creative/video-gen-prompting.md`,
`skills/creative/prompting/` (sora, seedance, ltx, grok, veo, hunyuan), `.agents/skills/` (kling-official, minimax-h3,
seedance-2-5, ltx2, ai-video-gen), `.claude/skills/flux-best-practices/` (SKILL.md, rules/t2i-prompting.md),
`.claude/skills/create-video/references/` (prompt-optimizer.md, prompt-examples.md). Also the Wan 2.2 TI2V 5B model
card on Hugging Face (Apache-2.0). No text is carried over.

## 1. What the sources teach (our words)

### Anatomy of one clip prompt
- A clip prompt has six parts: who or what is in frame, what it does, where the camera is and how it moves, the
  light, the look (grade, lens, film or digital), and the motion of the world around the subject.
- Models fill in subject and place well on their own. They miss motion, layout and camera. So spend words there.
- Test for a good prompt: a stranger who reads it can picture the shot. If not, the model will not either.
- Put events in the order they happen. When order does not matter, put the most important subject first.
- Swap feelings for things a camera sees. "Tired" becomes "eyes half shut, head resting on a fist".
  "Epic" becomes a low angle, a wide lens and the sun behind the subject.
- "Cinematic" alone adds nothing; every model already aims for it.
- One light setup per clip. "Bright noon" plus "deep shadows" makes the model drop one of them.
- Name a few anchor colours (three to five) instead of "warm tones".
- Camera words fall into three families and must not be mixed up:
  the rig travels (dolly, truck, pedestal), the rig turns in place (pan, tilt, roll), or only the lens
  changes (zoom, focus pull). Dolly is not zoom. Pan is not truck.
- "Static" means no move, no zoom and no focus change at all. Do not write "static" and then a move.
- Bird's-eye means straight down. A drone at 45 degrees is a high angle from altitude, not bird's-eye.
- A camera move renders better when the prompt says what the move reveals at its end.
- For several subjects across cuts, repeat the same few visual traits word for word each time. Pronouns lose
  the identity.
- Start small (subject, action, place), check, then add camera, then light, then style. Change one thing per try.
- Keep a good seed and reuse it for variants, where the route takes a seed.

### Per-model points (only what helps our four rows)
- Kling 2.6 Pro (fal): likes a clear order of actions, plain camera verbs and a short list of things to avoid.
  The fal guide (linked, not read here) claims a `++word++` emphasis syntax. Kling makes audio by default; our row
  already sends `generate_audio = false`.
- MiniMax H3 (fal): responds to explicit camera paths. Suggested order: subject, action, camera path, place, light,
  audio. Our fal row gives 5 s only, and the fal page says output can run up to 0.7 s long. Trim in the edit.
- Grok Imagine (fal): two input modes. Image to video uses the image as the opening frame. Reference to video uses
  images as influence only (placeholders like `<IMAGE_1>`), so the layout is not locked. Suggested order: shot,
  camera move, subject, one main beat, place, light, tone. Common faults: several scene changes in one clip, many
  style labels with little scene detail. Our row is 6 s, not 5.
- Wan 2.2 TI2V 5B (ours, free): the card shows 1280x704 or 704x1280, 121 frames at 24 fps (about 5 s), guidance 5,
  50 steps (our server runs 40). The card says training used labels for light, composition, contrast and colour
  tone, so those words should steer it. The card's own negative prompt is in Chinese; ours is English. The card
  gives no camera control claim and no prompt length advice. A prompt extension step exists upstream; we do not run
  it.
- General families: Sora and Veo reward a prose paragraph plus a short camera block and plateau around 100 to 250
  words. LTX degrades past about 80 words and takes "static" literally. Hunyuan for image to video wants motion
  only. Seedance rewards long structured prompts with labelled sections and named "locks". Runway prefers motion
  over looks and very short prompts.

### Image to video, first frames
- When an image is the first frame, the prompt must describe only motion: how the subject moves, how the scene
  changes, and the camera. Restating what the image shows wastes words and can fight the image.
- Make the first frame with an image model first if the look matters. Image prompts follow the same anatomy plus
  lens and composition words (rule of thirds, leading lines, negative space).
- FLUX takes no negative prompt. Say what you want instead.
- A start image carries its own faults into the clip. Use it to fix one shot, not as the way to keep a series
  consistent.

### Negative cues
- Some routes take a negative prompt (Veo, LTX, Wan, Kling); FLUX does not; Grok and MiniMax via fal: not stated.
- Keep it short. Typical targets: blur, warped hands and faces, extra limbs, flicker, watermark, logos, text,
  subtitles, frozen frames.
- Better than "no X": name the thing that replaces X. A ban without a substitute is weak.
- Veo may add subtitles when there is speech. Our clips have no speech, which removes that cause.

### Length and pacing for a 5 s clip
- One subject, one action, one camera move, one light. Four actions in one clip breaks motion.
- No cuts inside a 5 s clip. Multi-shot prompts are for 10 s and longer on models built for them.
- Simple physics only. Walking, pouring, cloth in wind: fine. Explosions, splashes, crowds in contact: risky.
- Slow moves read best at this length: a slow push in, a slow truck, a gentle drift.

## 2. Weak points of the sources

- Vendor bias. Seedance is called the "preferred default" in four files, with an Elo figure and no date or link
  to the test. The selector is said to be tuned toward it. None of this is checked against our use.
- Word counts are presented as findings from a paper, but the paper is about captioning, not generation. The
  table (Wan 2.2: 200 to 400 words) has no test behind it and the Wan card says nothing about length.
- Contradiction on length: "LTX under 80 words" in one file, "under 200 words" in the LTX skill.
- Contradiction on constraints: one file says negatives go in a short trailing list, the Seedance 2.5 file says
  they belong in named positive blocks.
- Audio is treated as a selling point everywhere ("keep generate_audio true"). For us audio is waste and may cost.
- Kling and MiniMax files are mostly API wiring and routing. Prompt advice is two or three lines each.
- No Wan 2.2 guide at all. Wan is lumped under "use the generic guide".
- The HeyGen prompt optimizer is about an avatar agent that renders text graphics. Its rules (every element must
  move, on-screen text lists, B-roll never 5 s or less) are wrong for diffusion clips. Its "5 s B-roll goes black"
  claim is about that agent only, but sits next to 5 s clip advice elsewhere.
- Style lists lean on named studios and artists (anime studios, Van Gogh). That is a rights risk for a government
  user and gives no control the model is known to honour.
- Examples are Western and generic (fishermen, astronauts, cartoon heroes). Nothing on Gulf light, desert haze,
  Arabic signage or local dress.
- "About 30 per cent of outputs show stray logos" is stated for LTX with no sample size.
- No guidance on vertical 9:16 composition, on cost per try, or on how to judge a clip besides "watch the whole
  thing".
- Long examples (the 7-shot trailer, the 90 s CEO report) are far longer than any clip we make.

## 3. What our skill must do better

### Fit the job: B-roll under narration
- Each clip illustrates one narration line. The prompt starts from that line's visual noun, not from the script.
- Fixed tail on every prompt: no people speaking, no on-screen text, no signs with letters, no captions, no logos.
  ScriptVox adds voice, captions and music. Audio is off where the route has the switch.
- Avoid readable surfaces in frame: shop fronts, screens, number plates, documents. Choose angles where they are
  out of frame or out of focus.
- Leave the lower third calm. Captions sit there in 9:16. Put the subject in the upper two thirds.

### 9:16 composition
- Vertical framing favours one tall subject: a person standing, a tower, a palm, a street receding.
- Prefer moves that suit a tall frame: tilt up, pedestal up, slow push in. Avoid wide pans; they show little.
- Ask for the aspect in the request field, not in prose.

### UAE and Arabic settings
- Describe place by what a camera sees: sand tone, low winter sun, haze, palm shade, Corniche water, wind towers,
  modern glass towers, mosque domes in the far distance. Do not name real landmarks unless the brief needs them.
- People: modest dress described plainly (white kandura, dark abaya), adults, faces small or turned away to avoid
  look-alike problems. No children unless the brief needs them.
- Light in the Gulf is hard at noon. Prefer early morning, late afternoon or blue hour for flattering clips.
- No Arabic text in frame; models write broken script. The caption track carries the Arabic.

### One look across all scenes
- Write a style line once per video (grade, colours, lens feel, time of day) and append it unchanged to every
  clip prompt. The brief stores it.
- Same model for every clip in a video when possible. Mixing models changes grain, colour and motion.
- Same time of day and weather in every clip unless the story moves.

### Cost
- Order of preference: wan22-ti2v-5b-self (free) for drafts and for any shot it handles; a paid row only for shots
  Wan fails twice, or when image to video is needed (Wan row is text only).
- Price per clip from the registry: minimax 5 s about 0.13 USD, grok 6 s about 0.30 USD, kling 5 s 0.35 USD.
  A plan must show the total before approval.
- Draft every prompt on Wan first. Move the same prompt to a paid row only after the composition reads.
- At most two paid tries per shot. After that, simplify the shot (less action, simpler camera) or use stock.

### Judge a clip with `scriptvox snapshot`
- Take frames at start, middle and end of each clip. A good first frame can hide a melt at second 4.
- Fail on: any letters or logo shapes, warped hands or faces, the subject changing identity, a frozen clip, a
  camera move that is not the one asked, clutter in the caption zone, a look that breaks the style line.
- Check that each frame matches the narration line shown with it (snapshot returns time and caption).
- Record the verdict and the prompt change in the brief, then `scriptvox resume`. One change per retry.

## 4. Claims to test with real renders

1. Wan 2.2 5B follows light and colour words better than camera words (card hints at the first, silent on the
   second). Test five camera moves on the same scene.
2. Best prompt length for Wan: 40, 80 or 150 words. The 200 to 400 word claim is untested.
3. Our English negative works as well as the card's Chinese one for text and watermark leaks.
4. Does "no text, no signs" in the positive prompt reduce letters on Kling, MiniMax and Grok, which may lack a
   negative field via fal?
5. Kling `++emphasis++` syntax: does fal pass it through and does it help?
6. Dolly versus zoom, pan versus truck: do our four models really keep them apart?
7. MiniMax 0.7 s overrun: how often, and does it break our 5 s timeline.
8. One shared style line keeps a consistent look across five clips on one model, and across two models.
9. Gulf scenes (abaya, kandura, wind towers): rate of wrong or offensive dress, fake Arabic letters, wrong skyline.
10. Lower-third calm: does "subject in upper frame" actually keep the bottom free in 9:16.
