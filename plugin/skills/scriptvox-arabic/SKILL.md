---
name: scriptvox-arabic
description: Write and check the Arabic narration of a scriptvox video - word budget, numbers, names, mixed Arabic and English text, punctuation, pause tags and voice choice, based on measured scriptvox renders. Use whenever a scriptvox video is in Arabic, a user writes the request in Arabic, or an English script must become an Arabic video. Load it together with scriptvox-brief.
---

# scriptvox-arabic

Arabic videos fail in ways English ones do not: the script is too long, numbers drag, and mixed text reads in the
wrong order. Every rule here comes from a scriptvox render that you can repeat.

## 1. Budget: half the words of English

The Edge Arabic voices read 105 to 110 words a minute. English voices read about 200.

| Length | Arabic words |
|---|---|
| 15 s | 22 |
| 30 s | 48 |
| 45 s | 72 |
| 60 s | 95 |

- Count a number as the words it is spoken as: "3999" is about six spoken words, not one.
- To turn an English script into Arabic, keep the ideas that matter most and drop the rest. A word-for-word
  translation runs about twice as long, and with `duration` set scriptvox stops it with "cut about N words".

## 2. Numbers

| Write | Not | Why |
|---|---|---|
| "نحو أربعة آلاف درهم" | "3999 درهمًا" | a long exact number is one caption word that stays for seconds while it is spoken |
| "عام 2026" | "عام ألفين وستة وعشرين" | years in digits are short on screen; spelled out they can break across caption pages |
| "ثلاث سنوات" | "3 سنوات" | small counts read better as words |

- Round when the exact value does not matter. Keep the exact value when it is the point (a price in an offer).
- Pick one digit style for the whole video: Western (2026) or Arabic-Indic (٢٠٢٦). Both render and both are read
  correctly. Do not mix them.
- Check every number against the source before you plan.

## 3. Names and English words

- Keep a short list of protected terms (people, brands, products) and spell each the same way every time.
- Write foreign names in Arabic letters the way a newsreader says them ("مايكروسوفت"). The voice reads them more
  naturally than Latin letters.
- Keep a brand in Latin letters only when it has no common Arabic form. scriptvox shows a line that starts with a Latin
  word or a number in the right order.
- Do not mix English words into the sentence for style.

## 4. Sentences and punctuation

- Under about 12 words a sentence. Long chains joined with "و" lose the listener.
- Use Arabic punctuation: the comma "،" and the question mark "؟". scriptvox treats ". ! ؟" as sentence ends for
  caption pages.
- One idea per sentence. Join beats with "لكن" (but), "لذلك" (so) and "لأن" (because), as in any scriptvox script.
- Pauses: `[توقف ١ ثانية]` and `[pause 1]` give the same silence. Use one after the key point only.

## 5. Tone and voice

| Audience | Voice | Tone |
|---|---|---|
| UAE | `ar-AE-HamdanNeural` (male), `ar-AE-FatimaNeural` (female) | Modern Standard Arabic, plain and warm, light Gulf word choice |
| Saudi | `ar-SA-HamedNeural` (male) | Modern Standard Arabic |
| Government or formal | any of the above | formal Modern Standard Arabic, no slang, full titles |

- Let the user hear the voice first: `scriptvox voices --preview ar-AE-HamdanNeural --brief brief.json --json` reads
  the first sentence and makes no video (see `scriptvox-brief`, section 7).
- The voices are labelled by country, not by dialect. Write Modern Standard Arabic unless the user asks for a dialect.
- If one word is misread, short vowel marks on that word only may help. Do not mark the whole script. Or give the spoken form in the brief's `say`: the captions keep the written form.
- ElevenLabs is the paid choice and needs approval. The terms of each voice are data in `scriptvox voices`.
- The open voice (`"tts": "selfhost"`, Chatterbox on your own GPU service, MIT, commercial use allowed) reads Arabic fast,
  about 175 words a minute, and blurs some words. Measured 3 Nov on 3 scripts: Whisper heard 83 to 95 percent of
  the words, Edge 100 percent of the same script (17.9 s against 10.1 s). No setting fixed it. Use the open voice for
  drafts and tests, read gate `words` in the result, and for a clear final Arabic voice use Edge (personal) or
  ElevenLabs (paid, with approval). Its fast rate also means a script written to the Edge word budget comes out
  shorter: set `duration` and let the fitter slow it.

## 6. Check before `scriptvox plan`

1. Word count at or under the budget, with numbers counted as spoken.
2. No long exact numbers unless the number is the point.
3. One digit style.
4. Every protected term spelled the same way each time.
5. Arabic punctuation, sentences under about 12 words.
6. `lang` is `ar` and the voice is an `ar-` voice from `scriptvox voices --lang ar --json`.

## 7. Check after the render

Run `scriptvox snapshot <run_id> --sheet 9 --json` and look at the frames:

- letters join inside each word
- each line reads from right to left, and the full stop sits at the left end
- numbers and Latin words sit where the sentence puts them, not at the wrong end
- the `caption` text in `frames` matches the brief

A fault here is a "must fix" in the scriptvox-review skill.
