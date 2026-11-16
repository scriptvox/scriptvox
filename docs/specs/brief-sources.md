# Brief `sources`: the facts of a script and where they come from (10 Nov 2026)

Why: a video that states a number should carry where the number comes from. Seen in the 10 Nov comparison pass: one tool has a research step with cited facts, and ScriptVox had no place for a source.

- `brief.sources`: a list of at most 20 entries `{text, url?, title?}`. `text` is the fact as the script states it, 1 to 300 characters. Each entry needs a `url` (`https://` or `http://`, no spaces), a `title`, or both.
- ScriptVox fetches nothing and does not judge a fact. It checks the form only, before any work: a bad entry is exit 2 and names the entry.
- `sources` does not change the picture, the sound or the plan id.
- `export` writes the list to `metadata.json` as `sources`, and one line per entry to `credits.txt`: `Source: <text> (<title>, <url>)`.
- The list is in the run's request, so it is in the recipe inside the MP4: `explain` shows it, and `remake` keeps it.

The agent's part: collect the facts and their sources before it writes the script, and put every number of the script in `sources`.

Run on 10 Nov 2026, free, at home: a brief of 2 scenes with 2 sources (one with a URL, one with a title only). `make` ok, `export` wrote both to `metadata.json` and 2 `Source:` lines to `credits.txt`, and `explain` on the MP4 showed them. Test `sources_of_the_brief_reach_the_metadata_and_the_credits`: a source with no URL and no title is exit 2; a good one reaches the metadata, the credits and the recipe.

Not done: no source shown on screen (a card or a caption line).

Since 13 Nov 2026 (`docs/specs/research.md`): gate `claims` in `plan` and `check` warns for a number or date in the script that is in no source (`--strict` makes it a failure), and `export --check-links` names a source address that does not answer. `research` and `write` can fill `sources`.

Since 13 Nov 2026 (`recipes.md`): the gate also reads a table's cells and a number line's `points` and `from`. The two ends of a number line's scale (`values`) are not facts and are not read.
