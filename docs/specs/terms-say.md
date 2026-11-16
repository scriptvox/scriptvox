# Terms and spoken forms (G022, 7 Nov 2026)

Speech to text writes names wrong, and a voice can read an acronym wrong. Ideas from public docs of MioSub, BaoCut, HeyGen and Synthesia; no code read.

## `--terms terms.json`

On `analyze`, `clips`, `dub` and `edit --captions`:

```json
{"terms": ["ACME", "Al Ain"], "fix": {"ak me": "ACME"}}
```

- All right spellings go to the speech to text row as a hint (Whisper's prompt).
- Then a fix-up on the word list: `fix` as written, and any case of a term. Several heard words become one, on their times. Each change is in `term_fixes`: `at`, `heard`, `wrote`. A word that was right is not listed.
- `analyze` writes the fixed words to a second file, so `timed_words` and `sentences` get them.
- `dub --plan` lists `keep_terms` and names them in `next`. `dub --lines` warns (`terms`) for a term not in the new lines.

## Brief or brand `say`

`"say": {"ACME": "A C M E"}`. The voice gets the spoken form, whole words only, longest first. Captions keep the written form. Word counts for the fit, the music length and the price count the spoken form. The `words` gate, also in `check --hear`, compares with the spoken form.

## Measured, 7 Nov 2026, Edge, large-v3 on `-self`

The runs used a real three-letter acronym; this page writes ACME for it.

| Check | Result |
|---|---|
| `say` ACME to "A C M E", 24 words | captions "ACME", gate 24 of 24, 9.93 s. Speech to text wrote "ACME" as without `say`, so this run alone does not prove what the voice said |
| `say` ACME to the long name | 14.53 s, speech to text wrote the long name 3 times, captions "ACME" read by eye. So the voice gets the spoken form |
| The words gate on that run, before the fix | false warning, 19 of 24. After: 34 of 36, in `make` and `check --hear` |
| `analyze --transcript` with the hint | "Al-Ain" became "Al Ain", no fix-up needed; one sentence, so one case of the hint working |
| `edit --captions --terms`, the long name to ACME | 3 listed changes, 24 words, 6 pages, sheet read by eye. The source had its own burned captions, and the `base_text` warning named that |
| `dub --plan --terms` on the eval promo | `keep_terms`, `next` names them; `--lines` with Arabic lines warns "not in the new lines" |

## Not done

- The `clips` path was not run with `--terms`; it uses the same two functions.
- `line_seconds` for a dub line in `plan` counts the written form. `make` measures the read, so only the plan's warning can be off.
- An Arabic term file and a real misheard Arabic name were not run. A person did not listen.
