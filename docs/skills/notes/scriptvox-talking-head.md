# Notes: scriptvox-talking-head

Written 7 Nov 2026 before the skill. Read in full: BaoCut `talking-head-cut` and its `picture-and-sound` reference (Community License, source-available). Read in part: OpenChatCut `talking-head-guide` (AGPL-3.0), the treatment list, the order, and the start of its speech section, about 15 percent of 9,550 words; the motion graphics part was not read. Ideas only. No text kept open while the skill was written.

## What the sources teach (our words)

- In a talk, the words set the cuts. The picture comes after the words are final, because each later layer hangs on the final times.
- Read the whole transcript before the first cut. A sentence that looks spare early may carry a later point. Make one list, sentence by sentence, keep or cut, with a reason, and apply it at once.
- Keep the speaker's order. Moving a sentence changes an argument, so ask first. The one usual exception is a strong line moved to the start, and then it moves whole, with its limits ("for beginners"), because a cut limit makes a new claim.
- Pauses are the voice's tone. Cut waiting, not breathing. Keep the pause after a turn or a joke.
- With two takes, the last is often better, and this is not a rule. If two takes differ in a number, a name or a "not", find out which one is right.
- A zoom on a stressed line also hides a jump cut. On every line it stresses nothing.
- B-roll shows what the words do not say. It starts on the word and returns to the face in a pause, not in mid sentence. If no real picture exists, stay on the face.
- There are two clocks, source and result. Mixing them puts a cut in the wrong place.
- Report what was measured, what was seen, and what a person must still hear. Never call a cut smooth with no listener.

## Weak points of the sources

- Each is tied to its own timeline editor, and a large part of the text is tool steps that mean nothing elsewhere.
- Their pause and zoom numbers are starting points with no stated measure.
- The AGPL guide is long: 9,550 words, and an agent must load all of it for a small cut.
- BaoCut is good on honesty, but gives no failure path when no transcript comes back.

## What our skill does better

- One page, commands that exist, each with a figure we measured on 7 Nov or a plain "not measured".
- It cuts by deleting words from a JSON file, so the agent does no time arithmetic.
- It gives two passes, so times for zoom and B-roll are read from the cut result and the two clocks cannot mix.
- It names what the tool does not do: no multi-camera, no face-following by speaker, and retake finding only by near-same words.
