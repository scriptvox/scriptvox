# Notes: scriptvox-brief

Written 2 Nov 2026 before the skill, from a full read of these source files (OpenMontage, AGPL, ideas only):
`skills/creative/storytelling.md`, `skills/creative/short-form.md`, `skills/meta/creative-intake.md`,
`skills/creative/broll-planning.md`, `skills/pipelines/explainer/script-director.md`. No text is carried over.

## Ideas worth keeping (our words)

1. The first one to two seconds decide if a viewer stays. The opening line must give a reason to stay: a surprise, a
   problem, a promise of a result, or a question. Greetings and "in this video" openings lose viewers.
2. Sections join by cause, not by sequence. Each step either explains the one before it or breaks it. A list of facts
   joined by "and" feels flat.
3. Start from what the viewer already believes, then correct it. A corrected belief is remembered better than a plain
   fact. This is an old finding from science education research.
4. One idea per section. Many new ideas in a short time make the viewer forget all of them.
5. Narration and picture must say the same thing at the same moment. A picture that shows something else competes with
   the voice.
6. A shot description names what is visible (subject, action, framing), not a feeling. "Close shot of hands opening a
   date box" guides a search or a generator. "A warm moment" does not.
7. Stock search works with two to four plain nouns, subject first, plus a framing word (aerial, close-up, timelapse).
   Real places and people come from stock. Abstract ideas come from a generated or built-in visual.
8. Ask only for what is missing. If the request already states the purpose, audience or platform, do not ask again.
9. Count words against the target length before any voice is made. Too many words is the most common failure.
10. Captions matter because many people watch with the sound off.

## Weak points of the sources

- Numbers without a source we can check, and numbers that disagree with each other: one file says 85 percent watch
  muted, the same file later says 80 percent. Retention multipliers (for example "2.8x") have no method.
- Speaking rates disagree across files (150, 155, 180 and 180 to 200 words per minute) and none was measured on the
  voices the tool actually uses. Word budgets for 30 seconds disagree (65 to 75 against 70 to 80).
- No language other than English is considered. An Arabic script built on those budgets is about twice too long.
- Long prose and large JSON schemas (delivery cues, SSML, enhancement cue types) that most engines ignore. The rules
  live in Markdown and nothing checks them.
- Seven intake questions and a human approval stop after every stage: slow for a 30 second video.
- Platform safe zones and caption styles are left to the writer, although a tool can enforce them in code.

## What our skill does better

- Word budgets come from measurement on our own voices (2 Nov 2026, one 88 word English and one 69 word Arabic
  paragraph, speech time from the caption file): Edge English 190 to 202 words per minute, Kokoro 191 to 212, Edge
  Arabic 105 to 110. So about 3.2 English words or 1.8 Arabic words per second of speech.
- The tool, not the writer, enforces the hard rules: `--duration` fits the voice or stops with "cut about N words",
  caption layout and safe placement are measured in code, loudness is fixed at -14 LUFS, gates check every frame.
  The skill only teaches what code cannot decide: the words and the shot choices.
- The output is a real `brief.json` that `scriptvox plan` validates, not an informal note.
- At most one question to the user, and only when the purpose or the language is unknown.
- Arabic gets its own rules (budget, Gulf tone, numbers, names), because no source covers it.
- A short self check before `plan`, with checks a reader can apply in seconds.
