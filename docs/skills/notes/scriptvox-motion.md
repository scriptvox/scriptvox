# Notes: scriptvox-motion

Written 3 Nov 2026 before the skill. Read in full: HyperFrames (Apache-2.0) `.claude/skills/motion-doctrine`
(SKILL.md, references/seam-gate.md; the two scripts skimmed for what they measure), `.claude/skills/cut-the-curve`
(catalog, Z sign, sections 6 and 7); Remotion (source-available, ideas only) `remotion-best-practices`,
`remotion-markup` (SKILL.md, timing.md, arabic-text.md, multi-scene-video.md), `remotion-create/video-layout.md`;
OpenMontage (AGPL, ideas only) `manim-composer` (SKILL.md, references/visual-techniques.md). No text carried over.
Ours: `make --help`, `schema brief`, `docs/specs/cards.md`, `hook-and-topic.md`, `RVX-013.md` (crossfade, Ken Burns),
P3-006 (tracked reframe), P3-008 (judge).

## 1. What the sources teach (our words)

1. Plan the whole film's motion before any one scene. Scenes animated alone feel like a slide deck: the eye stops
   at every cut.
2. Pick one main direction of travel for ordinary changes and keep it. Other directions carry a meaning (up for a
   conclusion, zoom for "deeper" or "arrival"), so spend them rarely and on purpose. Back-and-forth reads as a bug.
3. Something must be doing work at every moment: information arriving on the beat of the narration, a camera move
   with a goal, a number ticking. Idle loops (breathing, floating) look like the video is waiting.
4. Hold content back and pay it off when the voice says it. Staged reveal is the default for two or more items.
5. A short still beat (about a third to three quarters of a second) before the result makes the result land.
6. A single entry is short (under about 0.8 s); longer build-ups come from several short entries, not one slow
   one. Exits are a bit shorter than entries. Similar elements share one timing and one ease. No bounce or elastic.
7. Use only two or three transition types in one film and repeat them.
8. Audio is the clock: time motion to the real word times of the voice, and re-check after any re-voice.
9. Frame layout: decide the one thing the viewer must see first; keep key text inside a safe margin (on a 1080 px
   wide frame roughly 80 px from the sides and 100 px from top and bottom); headline about 84 px and supporting
   text about 44 px at that width, scaled with the frame.
10. Arabic: set the base direction, animate whole words or lines, never single letters (they lose their joins);
    keep digits and Latin runs whole.
11. Explainers: progressive disclosure (build an idea part by part), colour that keeps one meaning all through,
    position that encodes relation (left to right is time or cause, centre is focus).
12. Check motion with real frames, not by reading code: the HyperFrames gate measures direction, overlap and speed
    at each cut from rendered positions.

## 2. Weak points of the sources

- HyperFrames' rules need GSAP, Chrome, a ledger file and two Node scripts; most of the numbers (eases, pixel
  offsets, frame overlaps) only apply to its HTML engine. An agent with an FFmpeg pipeline cannot follow them.
- The seam law assumes every cut is a motion-graphics cut. For footage and generated clips, the motion is inside
  the clip; a crossfade there is normal, not a fault.
- Remotion's skill is a router into React code and its Studio; it gives almost no motion judgement beyond eases,
  and prefers a preview server over a check the agent can read.
- Manim composer asks the user many questions and plans 5 to 30 minute math videos; it does not fit 15 to 60 s
  narrated shorts. Its colour rules are generic.
- None of them measures the result against the brief or checks that text is readable at the size it ends up.

## 3. What scriptvox-motion must do better (rules)

What exists in scriptvox: the hook (1.5x caption size, pops in, 1.6 s); scene cuts on the first word of each scene
with a 0.25 s crossfade; stills get a slow push-in (100 to 110 percent); captions highlight the word being said;
scene `card` (title 2.5 s, stat counting up in 0.8 s, lower third 4 s, list items on their spoken word; Arabic on
the right); `generate` clips whose motion comes from the prompt; `edit --reframe --track`; `snapshot`, `judge`.
What does not exist: custom transitions, custom eases, per-word kinetic text, charts, a motion direction setting.

1. Motion budget per scene: one moving layer that carries meaning (the clip's own action, the push-in, or a card)
   plus the captions. Two cards in one scene do not exist; split the scene instead.
2. Staged reveal through scenes: one idea per scene, 3 to 6 s each; a list card for 2 to 5 parallel points, its
   items written with the narration's own words so each appears on its word.
3. A number the viewer must remember is a `stat` card in the scene that says it, never a caption only. One stat
   card per 20 s at most, or none stands out.
4. Title cards mark a change of part, not every scene: at most one per 15 to 20 s.
5. Lower third only over footage of that person (measured: over the plain background the judge scored it 1 of 5).
6. Clip motion comes from the `generate.prompt`: name one camera move and one subject action. Wan 2.2 5B gave a
   nearly still clip when asked for a subject to cross the whole frame in 5 s (74 percent still); ask for a small
   move instead.
7. The still beat before a payoff is made with a pause tag before the payoff line (`[pause 0.5]`), not with motion.
8. Check by eye on frames, not by trusting the brief: `snapshot --at` the start of each card scene and 0.9 s later
   (a stat has landed by then), then `judge` for text that is cut off or covered.
9. Arabic: write card text in Arabic words; numbers in the script's digit style (a stat counts in Western or Arabic-Indic digits, added the same night). Lower third and list move
   to the right side by themselves.
10. Say plainly what scriptvox cannot do (kinetic per-word type, charts, custom transitions) and offer the nearest
    card instead of faking it with raw FFmpeg.
