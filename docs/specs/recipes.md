# Recipes per kind of video: `scriptvox recipes` and the skill `scriptvox-genres` (P7.5, G083, 13 Nov 2026)

Why: an agent that gets "make an ad" or "make a news short" had the brief fields and the card kinds, but no start that has the right shape for that kind of video. The idea is ours from one-line notes that other tools have a skill per genre; no other tool's code or skill text was read.

## What

- **A recipe is data.** It is one complete brief (a JSON file in `crates/scriptvox/recipes/`) and a few lines of notes in `crates/scriptvox/src/recipes.rs`. There is no new render code and no new brief field.
- **Every recipe is free to run.** The voice is the local `kokoro-rust` voice or `tts: none`. No recipe has `media`, `stock`, `open_image`, `generate`, a logo or a music file, so a saved recipe works in any folder with no network and no key.
- **The words are an example, not a placeholder.** A brief refuses unknown fields and shows `{{key}}` text on the picture, so each recipe is a whole small video with invented names. The agent replaces the words and keeps the shape.

## Commands

- `scriptvox recipes --json`: the list. Each row has `name`, `kind`, `seconds`, `aspect`, `lang`, `voice`, `scenes`, `use_for` and `command`.
- `scriptvox recipes show <name> --json`: the row, plus `brief` (the whole brief as an object), `shape` (hook, beats, ending, cards, music, transition, captions, one line each) and `adapt` (how to change it). `next` says: save `brief` as `brief.json`, change the words, then `scriptvox plan --brief brief.json --json`.
- An unknown name: exit 2, the message lists the names, and the fix line is `scriptvox recipes --json`.
- Both make nothing, read no file and reach no network. They are an MCP tool (`scriptvox_recipes`), read only.

Why a new command and not `templates`: a row of `templates` is a way to build the picture and names a `make` command line; its list has a snapshot test and is one MCP tool with no argument. A recipe is a brief. One command with both would need a second shape in one list.

Not the same thing as "the recipe inside the MP4" (`explain`, `remake`, `--no-embed`): that is the brief and plan of one finished video. The help of `recipes` says so.

The seven older files in `recipes/` at the top of the repo stay as they are. They are examples for the repo's own fixtures (some need the network or files of the repo) and are not listed by the command.

## The twelve recipes

| Name | Kind | Length | Shape | Voice | Main cards |
|---|---|---|---|---|---|
| `explainer` | explainer | 60 s | 16:9 | local | title, mark, steps, diagram, list, two cards in one scene |
| `explainer-short` | explainer | 30 s | 9:16 | local | title, diagram, list |
| `explainer-ar` | explainer, Arabic | 60 s | 16:9 | none | as `explainer`, right to left |
| `explainer-short-ar` | explainer, Arabic | 30 s | 9:16 | none | as `explainer-short`, right to left |
| `product-demo` | product demo | 30 s | 16:9 | local | title, list with marks, table with marks, outro |
| `ad` | ad | 15 s | 9:16 | local | list (spring), title, outro; `cut_on: beat` |
| `news-brief` | news | 30 s | 9:16 | local | title, stat, timeline, `sources` and the Sources card |
| `tutorial` | tutorial | 40 s | 16:9 | local | steps, terminal, code, progress |
| `data-story` | data story | 40 s | 16:9 | local | bars, number line, table, `sources` and the Sources card |
| `listicle` | top 5 | 30 s | 9:16 | local | a title per item, a list to close; `cut_on: beat` |
| `quote-short` | quote or motivation | 10 s | 9:16 | none | quote with a mark; `captions: reveal` |
| `micro-drama` | dialogue | 25 s | 9:16 | two local voices | title; scene `voice` |

Arabic has no free local voice (`voice-local-first.md`), so the two Arabic recipes are `tts: none`. Their notes say how to give them the Edge voice when a network is allowed.

## Content rules of a recipe

- Invented names only: no real person, no brand, no logo, no model name.
- `news-brief` and `data-story`: every number of the script and the cards is in a `sources` entry. The facts are invented, so each entry's `title` starts with "PLACEHOLDER" and the publishers are "Example Council" and the like. The notes say to replace all of them.
- A `quote` card shows an invented line with no speaker.

## Checks

- `tests/recipes.rs`: every recipe loads as a `Brief`, `plan --local-only` gives no finding with status `warn` or `fail`, the list and `show` have the fields above, an unknown name is exit 2, the news and data recipes have `sources`, and the skill names every recipe.
- A machine with no local voice installed (the test's empty data folder) cannot plan a made voice with `--local-only`: `plan` stops with "Install the local voice". The test then plans the same brief with `tts: none`, no `voice` fields and no `duration` (the words are sized for the voice, not for reading pace), so the scenes and the cards are still checked, but not the ten voiced briefs as shipped. On a machine with the voice the brief is planned as it is.
- The skill `scriptvox-genres` is written by hand (its rules are prose, not help text), is one of the topic skills in `skill.rs`, so `skills install` writes it to every folder, and it passes `scripts/skills_check.py`.

## Limits

- No `make --recipe`, and no fill-in of a topic: the agent edits the words. `write --from research.json` makes a brief from facts; a recipe gives the shape.
- No recipe has a picture. Each note says which field adds one (`media`, `stock`, `generate`).
- No HTTP route in the OpenAPI document yet.
- The word counts are sized for the local voice (measured in these runs: about 3.0 words a second in prose, about 2.3 in a dialogue of short lines). Another voice needs the words that `plan` names in its `fit` finding.
- `micro-drama` reads 8 percent fast to fit 25 s (27.0 s at the voice's own pace). A longer `duration` makes `plan` warn, because its estimate for short lines is about 18 percent under the real read. Both stay inside the bands, so neither warns.

## Real runs, 13 Nov 2026

All with `make --local-only`, 2 cores, the capped FFmpeg, no key and no network. A recipe over 20 s was run in parts of whole scenes, each under 18 s, with nothing dropped: every scene and every card of the shipped brief was rendered. For six recipes a part's `duration` is its share of the recipe's own voice run (`make --until voice`), so each part asks for the pace of the whole. The parts of `explainer` and `product-demo` (and the Arabic ones, which have no voice) were cut by word count; their rates were 0, 0, 0, -8 and -13, 0, 0 percent.

| Recipe | Parts | Video of the parts | Whole voice: rate | Gates | Read on sheets |
|---|---|---|---|---|---|
| `ad` | 1 (whole) | 14.5 s with the outro | 0 percent | 10 pass; `cut_on` moved 2 of 3 cuts | list at the left, title, outro text |
| `explainer` | 4 | 15.6, 13.9, 15.6, 14.9 s | 0 (60.0 s) | 10 pass each | title, highlight, lit steps, diagram left to right, list, two cards in turn |
| `explainer-short` | 2 | 15.7, 14.3 s | 0 | 10 pass each | diagram top to bottom, list |
| `explainer-ar` | 4 | 16.0, 12.9, 16.0, 15.0 s | no voice | 10 pass each | right to left, joined letters, diagram from the right, steps at the right |
| `explainer-short-ar` | 2 | 15.6, 14.4 s | no voice | 10 pass each | as above, tall |
| `product-demo` | 3 | 11.9, 13.3, 7.3 s | 0 | 10 pass each | marks on the list, the light on the table rows, outro |
| `news-brief` | 2 | 16.8, 13.2 s | 0 | 11 pass each (`claims`: each number in a source) | stat, timeline, "Sources: Example Council, Example News" |
| `tutorial` | 3 | 16.2, 14.0, 9.8 s | 0 | 10 pass each | lit steps, two terminals, code, progress bar |
| `data-story` | 4 | 9.7, 11.5, 13.1, 5.6 s | 0 | 11 pass each | bars, the marker at 15 on its way to 17, table rows lit, underline, Sources card |
| `listicle` | 2 | 17.3, 12.7 s | 0 | 10 pass each | a numbered title per item, the closing list |
| `quote-short` | 1 (whole) | 10.0 s | no voice | 10 pass | reveal captions, the quote with its highlight |
| `micro-drama` | 2 | 16.1, 9.7 s | +8 | 10 pass each | the second speaker in light blue |

Found by the runs and fixed in the recipes: yellow captions on the default yellow background (`listicle`, now a dark one); a Sources line of 32 characters cut to "and more" on a tall frame (shorter publisher names); a mark whose word came after a 2.5 s title had gone (`data-story`); scripts 15 to 19 percent short for the real voice (`explainer-short`, `tutorial`).

A part made from a share by word count warned (`fit`, -16 to -22 percent) in the first runs of `explainer-short`, `tutorial` and `data-story`, and one part of `micro-drama` read at +16 percent at its share of 8.9 s; it passed at 9.7 s. These are faults of the cut into parts, not of the whole briefs.

## Tool faults found

Fixed here, each small:
- A scene `voice` with `tts: kokoro-rust` was refused ("not usable with engine kokoro-rust"): the id was looked up in the first engine that has it. Now the video's engine first (`make.rs`).
- `plan --local-only` with `tts: kokoro-rust` and no voice installed named `setup voice-local`, the other engine's setup. Now `setup voice-rust` (`voices.rs`).
- Gate `claims` read the two ends of a number line's scale as facts and did not read its `points`, its `from`, or a table's cells. Now it reads the points, `from` and the cells, not the scale. The cells are new behaviour: a brief with `sources` and a table whose numbers are in no source now gets the warning (a failure with `--strict`) (`claims.rs`, test `a_number_line_gives_its_points_not_its_scale_and_a_table_gives_its_cells`).

Not fixed, for the owner of each part:
- `plan`'s length estimate for `kokoro-rust` is off in both directions: prose reads about 8 percent faster than the estimate (a script that `plan` passes reads at -17 percent in `make`), and a dialogue of short lines reads about 18 percent slower.
- Gate `contrast` warns on a comment line of a `code` card (2.6 to 1, the grey comment colour on the panel), and its fix line speaks of the captions. The recipe has no comment line.
- Gate `claims` reads a month at the end of the script as a date when the first card text starts with a number (the texts are joined). Seen on a part only.
- The MCP tool list is 54,453 bytes with `recipes`; the test limit went from 54,000 to 55,000.
