# Idea reuse from AGPL, GPL, source-available and closed tools

Owner rules, 1 Nov and 5 Nov 2026. They replace the earlier "no reads" clean-room rule and all older licence text.

## The rule

1. We may read the code of OpenMontage (AGPL-3.0), ComfyUI (GPL-3.0) and Remotion (source-available) to learn how they solve a problem.
2. We never copy, translate or closely paraphrase their code. This also covers prompts, comments, test data, file layouts, assets and docs text.
3. We write our own design for our need, usually in Rust. We take the idea only when our version is better.
4. The same rule applies to text: prompts, agent skills, tool descriptions and docs. Read theirs, learn what works, then write our own text that is better. Never copy their wording.
5. All ScriptVox code and text is Apache-2.0. One licence, no exceptions from these repos.
6. **Models.** Any model at any licence, local or through an API. The licence stays on its row.
7. **Tools.** Any tool, AGPL or closed source: learn the ideas and build a better own version under Apache-2.0. AGPL and GPL code is never copied, translated or closely paraphrased, and runs only as a separate component. Closed source: public docs and product behaviour only, no decompiling and no leaked code. No names, logos or brands copied.
8. **Free image and video.** They come from open models that ScriptVox downloads and runs on the user's hardware, with setup at most a Hugging Face login.
9. **Commercial use.** All rows are allowed, the user decides, and the tool shows no note about it (owner, 13 Nov 2026: "I don't want any label in rendering like no commercial for end users"). No flag and no question. `metadata.json` keeps terms as data. The spend cap and paid approval do not change.
10. **Discovery.** It repeats per `docs/plan/DISCOVERY.md`, and gaps are taken from `docs/plan/GAPS.md`.


HyperFrames (Apache-2.0), MoneyPrinterTurbo (MIT) and Manim (MIT) code may be copied, with the attribution kept.

## How to work

1. Read the source. Write `docs/specs/<feature>.md`: what the feature must do, what we read (repo and path), what is weak in it, and why our design is better.
2. Close the source. Write the code from the spec only. Do not keep the source file open next to our file.
3. Never port a file line by line into Rust. A port is a translation, and a translation is a copy.
4. Facts are free to use: numbers, thresholds, formats, protocols and API names. Expression is not: the exact code, text and structure.
5. The commit message names the spec. The PR checkbox asks: "No code, prompt or text was copied from an AGPL, GPL or source-available repo."
