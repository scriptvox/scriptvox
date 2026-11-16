# Plan: Chinese, Japanese, Korean and Hindi (G108, P4.3)

Status: a plan only. Nothing here is built. Each fact marked "to check" was not checked on 13 Nov 2026.

## Why these four are not a row change

The five Latin languages (`latin-languages.md`) needed rows, a rate and a few caption rules, because a word is the text between two spaces and the bundled font has the letters. These four break one or both of those:

| Language | Words between spaces | Bundled font has the script | Shaping |
|---|---|---|---|
| Chinese | no | no | none |
| Japanese | no | no | none |
| Korean | yes | no | none (syllable blocks are single characters) |
| Hindi | yes | no | yes: conjuncts, vowel signs that sit before or above the letter |

## 1. Fonts

| Script | Font | Licence | Size | Note |
|---|---|---|---|---|
| Chinese (Simplified) | Noto Sans SC | SIL OFL 1.1 | about 8 to 16 MB a weight (to check) | Traditional is Noto Sans TC, a different file: Han characters have regional forms |
| Japanese | Noto Sans JP | SIL OFL 1.1 | about 5 to 9 MB a weight (to check) | A Chinese font draws some kanji in the Chinese form: the font follows `lang`, not the character |
| Korean | Noto Sans KR | SIL OFL 1.1 | about 5 to 10 MB a weight (to check) | |
| Hindi | Noto Sans Devanagari | SIL OFL 1.1 | under 1 MB a weight (to check) | Small: can be bundled as the Arabic font is |

- The three CJK fonts are too large to put in the binary (the binary holds 9 font files today). Plan: a pinned download by sha256 through `setup` (`scriptvox setup fonts-cjk`, or one target for each language), as the voice files are. `make` with `lang` zh, ja or ko and no font: exit with `needs_input` and that command in the fix line. Devanagari is bundled.
- Each font file gets a row in `PROVENANCE.tsv` and its licence text in `LICENSES/`. OFL allows bundling and embedding; the font is not sold by itself.
- Code: `captions::Measurer` picks a second font by script today (Arabic or not). It becomes a choice by `lang` and script: Han with `lang` ja takes the Japanese font. `covers()` already tells when a font has no glyph; the `textlint` check that stops boxes must know the new fonts.
- Cards use the same fonts through `template.rs`: each card kind needs one look in each script.
- Check before code: libass in the pinned FFmpeg shapes Devanagari (it needs HarfBuzz; the Arabic captions already prove complex shaping works, to check for Devanagari conjuncts with a rendered frame).

## 2. Line breaking and caption pages

- Chinese and Japanese: no spaces. `split_whitespace` gives one "word" for a sentence, so the pager, the karaoke highlight, `fit`, the `words` gate and scene word counts are all wrong. Plan: one tokenizer function for the whole tool (today about 20 places call `split_whitespace` on the script), then:
  - the caption unit is a character group, not a word: break between any two Han or kana characters, except the rules below;
  - line start and line end rules (kinsoku): no line starts with a closing mark or a small kana (`、。，．）」』！？ー ゃゅょっ`), no line ends with an opening mark (`（「『`); digits and Latin words stay whole;
  - line width in full-width characters: about 12 to 16 a line on 9:16 and 20 to 24 on 16:9 (to measure with the real font, as the Latin widths were);
  - punctuation: full-width marks `。，！？` end a sentence; `captions::ends_sentence` knows only the Latin and Arabic marks today.
- A segmenter for real words (for karaoke by word and for the `words` gate) is a second step. A dictionary segmenter is a new dependency with its own licence: choose one with an MIT or Apache licence, or keep character groups and time them from the engine's events (the free network voice sends word events for Chinese, to check what unit they hold).
- Korean: spaces are there, so the pager works. Check the page end rule: a particle is joined to its word, so the "weak word" list is not needed.
- Hindi: spaces are there. Rule to add: never break inside a syllable cluster. Text measuring must shape the text (it does, through the same shaper as Arabic). The danda `।` ends a sentence.
- Vertical text and ruby (furigana): out of scope.

## 3. Voices

Free network voice (`edge`), ids from the live voice list on 13 Nov 2026 (not run):

| Language | Ids |
|---|---|
| Chinese (Mandarin, mainland) | zh-CN-XiaoxiaoNeural, zh-CN-YunxiNeural (also Xiaoyi, Yunjian, Yunxia, Yunyang) |
| Chinese (Taiwan) | zh-TW-HsiaoChenNeural, zh-TW-YunJheNeural, zh-TW-HsiaoYuNeural |
| Japanese | ja-JP-NanamiNeural, ja-JP-KeitaNeural |
| Korean | ko-KR-SunHiNeural, ko-KR-InJoonNeural |
| Hindi | hi-IN-SwaraNeural, hi-IN-MadhurNeural |

- Local voices: the Kokoro model has voice files for Mandarin, Japanese and Hindi (to check on its model card), but the text to sounds step for each needs its own part (a Mandarin and a Japanese reading dictionary, a Hindi one). The Rust voice has English only. So a local voice is a separate item for each language, with a licence check of each dictionary.
- The open voice on a GPU service (Chatterbox Multilingual): its card lists 23 languages (the row note in `generators.toml`); which of the four are in it is to check before a row.
- Detection: by script, as Arabic is today. Kana means Japanese; Hangul means Korean; Devanagari means Hindi; Han with no kana means Chinese. Simplified or Traditional cannot be told from short text: `lang` `zh` is Simplified and mainland voices, `zh-TW` is by hand.
- `voices::lang_code` already knows "hindi"; add the names of the other three in English, Arabic and the language itself.

## 4. Rate measures for `fit`

- Chinese and Japanese: characters a second, not words. Measure three texts of about 15 s for each voice, as for the Latin languages, and count Han and kana characters without marks. `fit::spoken_words` already has a character count for one engine: the same shape, with a rate for each language. Expected range 4 to 6 characters a second (to measure, not to use).
- Korean: syllable blocks a second is steadier than words a second (words are long, as in Indonesian, where words a second moved by 14 percent between three texts of one voice and characters a second by 3 percent).
- Hindi: words a second, as Latin.
- Numbers: a digit group is read as several syllables (2005 in Mandarin is 4 or 5 characters of speech). `fit::read_units` needs a count for each language, measured.
- The reading pace with no voice (`tts: none`): characters a second for Chinese and Japanese, to set by a reading test.

## 5. Other parts that read the script

- `say::spoken`: the symbol table (`≤`, `×`) needs a column for each language.
- `pauses`: Chinese, Japanese and Korean pause words are in the keyword list already.
- The `words` gate: compare character sequences for Chinese and Japanese. Speech to text writes Han numbers or digits by its own choice: the gate needs a number form for each language.
- Local speech to text (`whisper-small-local`) lists all four languages; the small model is weak on some (Arabic was "usable with a few wrong words"). Measure each before the gate may fail a run.
- `textlint`, `claims`, `research`, `write`: each has English and Arabic rules. They are out of scope for the first step: the first step is `make` from a finished script.

## 6. Test plan

For each language, in this order:

1. Unit: font coverage of a sample of the 3,000 most used characters (Chinese, Japanese), all Hangul syllables used in a test text, all Devanagari letters and signs; the tokenizer; the line start and end rules at 7 line widths (the shape of `a_mark_with_a_space_stays_with_its_word`); detection on 5 texts and on mixed text (a Japanese text with many kanji must not be Chinese).
2. Render with no voice: one `tts: none` video, 9:16 and 16:9, frames looked at by a person or a model that reads the script. Stop here if a glyph is a box or a conjunct is broken.
3. Rate: the three texts for each voice, the table in the spec.
4. Real video: 10 s with the free network voice, a year, a large number, a percent and a decimal in the script; gates pass; `words` gate with local speech to text; frames looked at.
5. `fit`: one video with `duration`, the estimate within 10 percent of the read.
6. Regression: the English, Arabic and Latin caption tests and the plan ids of old briefs do not change.

## Size and order

1. Hindi (font bundled, spaces, shaping check): S to M.
2. Korean (font download, spaces): M.
3. Japanese and Chinese (font download, tokenizer through the whole tool, kinsoku, character rates): L, and the tokenizer is the large part.
