# Spanish, French, German, Portuguese and Indonesian (G073, P4.2, 13 Nov 2026)

Before: a script in one of these languages was read by an English voice, with a `lang` warning. Own design; no code of another tool was read.

## What a user does

Nothing new. A script in one of the five languages gets a voice of that language. `lang` in the brief (or `--lang`) sets it by hand: `es`, `fr`, `de`, `pt`, `id`, or the name of the language (`german`, `deutsch`, `português`, `bahasa indonesia`).

## What is there

| Part | Spanish, French, German, Portuguese, Indonesian |
|---|---|
| Voices | Two rows each on the free network voice (`edge`): `es-ES-AlvaroNeural`, `es-ES-ElviraNeural`, `fr-FR-HenriNeural`, `fr-FR-DeniseNeural`, `de-DE-ConradNeural`, `de-DE-KatjaNeural`, `pt-BR-AntonioNeural`, `pt-BR-FranciscaNeural`, `id-ID-ArdiNeural`, `id-ID-GadisNeural`. The ids are from the live voice list of 13 Nov 2026. The first row of a language is its default |
| No voice | `tts: none` takes the five languages |
| Local voices | None. `kokoro-rust` and `kokoro` change text to sounds with an English dictionary only, so a voice file alone is not a voice. `--local-only` stops with "No local voice for 'fr'" and names the network rows |
| Detection | `make::detect_lang`: Arabic by script as before; else the common short words of each language, its own letters (`ñ ¿ ¡`, `è ê œ`, `ß ä ö ü`, `ã õ`) and French elisions (`l'eau`, `qu'en`). The language with the most hits wins when it has 2 hits or more, more hits than the English list (about 90 words), and more than a tenth of the words. A short word counts only when it is written in lower case ("Los" in "Los Angeles" is a name), and no list holds a word that is also English, but for the German "die". A text under 5 words stays English. Italian has no voice row: it stays English with the `lang` warning |
| Rate for `fit` | `fit::words_per_s`, table below |
| Numbers for `fit` | In these languages `1.250` is one number and `3,5` is a decimal. A mark that stands alone (`?`, `!`, `:`, `«`) is not a word |
| Symbols | `≤ ≥ ≠ ≈ × ÷ ± √ ∞` are said in the language of the script (`say.rs`). Captions keep the symbol |
| Captions | Only when the caption words are in one of the five languages (the same detection), so English and Arabic pages are what they were. French sets `? ! : ; » %` and `«` off with a space, so each is a word of its own. A closing mark never starts a line or a page, and an opening mark never ends one (`captions::keep_marks`). No page ends on a short word of the language (`de`, `les`, `und`, `não`, `yang`). A number keeps `mil`, `millones`, `tausend`, `ribu`, `persen` |
| Font | The bundled caption font has every letter and mark of the five languages (test below) |
| Preview | `voices --preview <id>` has a built-in sentence in each language |

## Measured rate, 13 Nov 2026

Three plain texts of 35 to 45 words (12 to 20 s) for each voice, no numbers. Speech time is from the first to the last word event of the engine. English, one text on the same day: 3.46 (table value 3.3).

| Language | Voice | Text 1 | Text 2 | Text 3 | Words a second, all texts of the language | Characters a second | Table value |
|---|---|---|---|---|---|---|---|
| Spanish | Alvaro | 2.93 | 3.14 | 2.86 | 2.98 | 16.7 | 3.0 |
| Spanish | Elvira | 3.02 | 3.14 | 2.79 | | | |
| French | Henri | 3.43 | 3.23 | 3.53 | 3.29 | 19.0 | 3.3 |
| French | Denise | 3.28 | 3.03 | 3.26 | | | |
| German | Conrad | 2.77 | 2.99 | 2.89 | 2.75 | 16.1 | 2.75 |
| German | Katja | 2.57 | 2.72 | 2.64 | | | |
| Portuguese | Antonio | 2.83 | 2.99 | 2.73 | 2.94 | 16.1 | 2.95 |
| Portuguese | Francisca | 3.06 | 3.09 | 3.00 | | | |
| Indonesian | Ardi | 2.64 | 2.66 | 2.32 | 2.53 (Ardi only) | 17.7 (Ardi) | 2.5 |
| Indonesian | Gadis | 2.05 | 2.03 | 1.78 | | 13.6 (Gadis) | |

The Indonesian value is the default voice. The other voice (Gadis) is about 25 percent slower: with `duration` the fitter reads it again at a faster rate, and the `fit` warning can show.

## Real videos, 13 Nov 2026

One video for each language: the worktree build, an empty config folder, no key, `edge` voice, local speech to text (`whisper-small-local`) for the `words` gate. Each script has a year, `1.250` (French `1250`), `87%` and `3,5`, and the letters and marks of its language. The language was detected, not given. Frames of each video were looked at (4 frames each).

| Language | Voice picked | Length | Gates | `words` gate | What speech to text wrote for the numbers |
|---|---|---|---|---|---|
| Spanish, 16:9 | es-ES-AlvaroNeural | 14.1 s | all pass | 38 of 38 | 2005, 1.250, 87%, 3,5 |
| French, 9:16 | fr-FR-HenriNeural | 13.4 s | pass, `words` warn | 33 of 37 | 2005, 1250, 87%, 3,5. It wrote "depuis" for "de pluie" and "canot" for "canaux" |
| German, 16:9 | de-DE-ConradNeural | 16.5 s | all pass | 34 of 35 | 2005, 1250, 3,5. It wrote "7,80%" for "87%": the small model, not the voice |
| Portuguese, 9:16 | pt-BR-AntonioNeural | 15.4 s | all pass | 39 of 39 | 2005, 1.250, 87%, 3,5 |
| Indonesian, 16:9 | id-ID-ArdiNeural | 13.5 s | all pass | 33 of 35 | 2005, 1.250, 87 persen, 3,5 |

Seen in the frames: `¿Sabías`, `niño`, `piña`, `mañana!`, `garçon :`, `« C'est très bien »,`, `fontaines ?`, `ça marche !`, `fließen`, `Große Straßen`, `übrigens`, `Você`, `água`, `ação`, `mãos` are drawn right, each mark with its word.

Faults found by these runs and closed: French was read by the Spanish voice and German by the English voice (the word lists were too short; the `words` gate failed both at 51 and 36 percent). After the fix the 15 rate texts and the 5 video scripts each get their own language.

A fault found by review and closed: short English lines got another voice ("Why do cats purr so much?" got the Portuguese one, because "do" was in its list). The rule is now the one in the table. Checked: 42 short English lines in a test (names of places, "de facto", "en route", "faux pas"), and 749 English lines taken from the tests, the evals and the docs of this repo all stay English.

English and Arabic did not change: the build of `main` and this build made 20 videos with `tts: none` (16 English scripts, 7 of them with names such as "Dan", "Los Angeles", "MIT", with "et al", "de facto", "mil" and with marks set off by a space; 4 Arabic scripts). The 20 SRT files, the 20 plan ids and the findings are the same. The five videos above were made again with the last build: the same SRT files.

Other checks: `plan --local-only` on the French brief is `needs_input` with the two network rows. `voices --preview fr-FR-DeniseNeural` with no text gave 5.1 s of audio.

## Limits

- French writes a thousand with a space (`1 250`). Here that is two words, and the voice reads two numbers. Write `1250`.
- A page can end on a number ("in 3,5" then "Stunden"), as in English.
- The `words` gate knows number words in English only. In the five languages it matches digits to digits, which is what speech to text wrote in each run above.
- Detection reads short words. A text with very few of them (a list of names) stays English: give `lang`.
- With `tts: none` or the local English voice, a mark that stands alone gets a short time of its own in a karaoke caption.

## Not done

- No local voice for these languages. No measure on `selfhost` (the rows `own-es` and so on in `voice-services.md` use the rate of the English open voice, 4.1 words a second, until a service is measured).
- Italian: no row, the warning stays.
- Nobody listened to the five videos: the proof is speech to text and the frames.
- The scriptvox-arabic skill has no sister skill for these languages (word budget per length).

Tests: `make::tests::detects_the_latin_languages`, `make::tests::short_english_lines_stay_english`, `fit::tests::latin_languages_have_their_rate_and_their_numbers`, `say::tests::symbols_become_words`, `captions::tests::a_mark_with_a_space_stays_with_its_word`, `captions::tests::weak_words_of_the_latin_languages`, `captions::tests::the_caption_font_has_the_letters_of_the_latin_languages`.
