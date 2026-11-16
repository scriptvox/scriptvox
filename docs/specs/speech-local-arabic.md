# Local Arabic speech to text (16 Nov 2026)

Gap: the free local speech model (`setup speech-local`, Whisper small) writes many wrong words in Arabic. A fine-tuned
Whisper small for Arabic exists (version r12-a70). It is better on Gulf, Emirati and standard Arabic and the same on
English. It is tested only with the language given (`ar` or `en`), so it runs only for Arabic.

## Command

`scriptvox setup speech-local --arabic [--from <folder>] [--dry-run]`

- It installs the present model first (as plain `setup speech-local`, a no-op when it is there), then the Arabic model
  in the folder `model-ar` beside `model` in the same plugin.
- Source of the Arabic model: the pinned address in `toolchain.lock` (rows `whisper-ar-*`), else `--from <folder>`, a
  local folder with the 5 files `config.json`, `model.bin`, `preprocessor_config.json`, `tokenizer.json`,
  `vocabulary.json`. Each file is checked against its size and sha256 pin before anything is copied or downloaded.
  The files are copied: the source folder is only read.
- `--from` wins over the address when both are there.
- Plain `setup speech-local` does not change: the present model stays the default.

## Use

One place selects the model: `genjob::run_local`, which every caller of the local plugin goes through. The request's
language picks the folder (`provision::whisper_model_folder`):

| Language of the request | Arabic model installed | Folder |
|---|---|---|
| `ar`, `ar-AE`, `AR` (any `ar` tag) | yes | `model-ar` |
| `ar` | no | as before (`model-large` or `model`) |
| `en`, any other, or none (auto-detect) | any | as before |

- The Arabic model wins over `speech-large` for Arabic. Open decision for the lead: `large-v3-turbo` was not measured
  against r12-a70.
- A transcript of the Arabic model is cached under its own file name (`<shot>.model-ar.json`), so audio heard with the
  present model is heard again, and the old cache names stay valid.
- A self-hosted speech row with a key still goes first, as before; then the plugin is not called.
- The Arabic model alone is not "speech local installed": the present model must be there for other languages.

## Errors (each with a fix line)

| Case | Exit | Message names |
|---|---|---|
| No address in the pin and no `--from` | needs input (10) | `--from <folder>` |
| `--from` folder has no file of the 5 | error (1) | the file and the folder |
| A file has the wrong size or sha256 | error (1) | the file, expected and got; nothing is copied |
| Any pin with an empty address asked to download | error (1) | the pin and `--from` |

## Listing

- `providers`: `speech_local.arabic` gives `installed`, `version`, the licence data and the add command.
- `licenses`: the 5 `whisper-ar-*` rows with their licence and an empty `url`.
- `doctor`: `speech_local.arabic_installed` in the extra data. No warning when it is not installed.
- Credits and licence data: base model `openai/whisper-small` (Apache-2.0 on the model card, MIT code); training data
  Meta Omnilingual ASR Corpus (CC BY 4.0), Google FLEURS (CC BY 4.0), LibriSpeech (CC BY 4.0), Mozilla Common Voice 18
  (CC0). No note is shown in the flow.

## Numbers of record (word errors, present model / Arabic model)

Gulf 41.3 / 27.8 %, FLEURS Arabic 26.4 / 17.7 %, Emirati 26.5 / 20.6 %, English 6.52 / 6.44 %. Long recordings (533 s
and 513 s): no gap over 5 s. Measured with the language given only.

## Limits

- No public address yet. The publication step fills `url` in the 5 rows (and `mirror` if there is a second host). No
  code change.
- Auto-detect and languages other than Arabic and English are not tested, so they keep the present model.
- No GPU path.

## Real run (16 Nov 2026, 2 cores, scratch data folder)

`setup speech-local` (40.7 s), `make --script script.txt --lang ar --tts edge` (18.7 s video, Edge Arabic voice), then
`analyze ar.mp4 --transcript --lang ar` with the present model (3.9 s), then `setup speech-local --arabic --from
~/work/scriptvox-whisper-arabic/models/r12-a70` (11.3 s, 5 files checked and copied), then the same `analyze` (3.9 s).

| | Text |
|---|---|
| Script | في فصل الشتاء تهطل الأمطار على مدينة أبوظبي. تعمل فرق البلدية ليلا ونهارا لتصريف المياه من الشوارع. ونحن نستخدم الذكاء الاصطناعي لمعرفة الأماكن التي تتجمع فيها المياه قبل وصولها. |
| Present model | في فصل الشتاء تهتل الانطار على مدينة أبوظبي. تعمل فرق البلدية ليلاً ونهاراً لتصريف المياه من الشوارع. ونحن نستخدم الذكاء الاستناعية لمعرفة الأماكن التي تجمع فيها المياه قبل وصولها. |
| Arabic model | في فصل الشتاء، تهطل الأنطار على مدينة أبوذبي. تعمل فرق البلدية ليلاً ونهاراً لتصريف المياه من الشوارع. ونحن نستخدم الذكاء الاصطناعية لمعرفة الأماكن التي تتجمع فيها المياه قبل وصولها. |

By eye, 29 words: 4 wrong words with the present model (تهتل, الانطار, الاستناعية, تجمع), 3 with the Arabic model
(الأنطار, أبوذبي, الاصطناعية). One synthetic voice and one script: this shows the path works, not the size of the gain;
the numbers of record above are the measure. With no `--lang`, the present model's transcript came back (from the cache of the first run). Since 16 Nov the cache key of `analyze` has the language when one is given, so `--lang en` and `--lang ar` of one file are two transcripts for every row, not only for the local one. With no language the old cache name stays valid.

Tests: `crates/scriptvox/tests/whisper_arabic.rs` (no network) and the unit tests in `provision.rs`.

## Ready checks

Each of the three local models (`model`, `model-large`, `model-ar`) is ready only when its tool files are there and its `model.bin` is linked in the present plugin folder (`provision::whisper_model_ready`). A new plugin version has no links until `setup` runs again, so `doctor` and `providers` no longer say "installed" for a model that the plugin cannot load.
