# Hear a voice before a video: `voices --preview` (10 Nov 2026)

The voice list showed names, cost and terms, and no sound: the only way to hear a voice was to make a video with it. Some video tools let a user hear a voice first. The idea is from that one observation; own design, no other tool's code or text was read.

```
scriptvox voices --preview <voice id> [--text "..."] [--brief b.json] [--lang ar|en] [--tts <engine>] [--out file]
```

It writes one short audio file and makes no video. `scriptvox voices` with no `--preview` gives the list, as before.

## Rules

- **One synthesis path.** `voices::preview` calls `make::synthesize` with one speech part. So the worker, the cache key, the cache terms and the ledger are the ones `make` uses, and a line heard in a preview is not paid or fetched again in `make` when the text and voice are the same.
- **The voice.** The id must be in the registry, else a usage error (exit 2). `voices::choose` then gives the same answers as `make`: a missing key, a local voice that is not installed, `--local-only` with a network voice (exit 10 with `needs_input`). `--tts` picks the engine when two engines share an id (`kokoro`, `kokoro-rust`). The `no-voice` row is refused: there is nothing to hear.
- **The text.** `--text`, else the first sentence of the brief, else a built-in sentence in English or Arabic. The brief's text comes from `brief::script_text` (script file, `script`, `lines` or `scenes`), which `make` now uses too. Pause tags are removed, and the brief's `say` forms and the symbol words reach the voice as in `make`. `text` in the result is the written line; the voice hears the `say` form.
- **The language.** `--lang`, else the brief's `lang`, else the script of the text when the voice speaks it, else the first language of the voice's row. A language with no built-in sentence needs `--text`.
- **The length.** 1 to 200 characters (`PREVIEW_MAX_CHARS`), counted as characters, not bytes. More is a usage error: a whole script is heard with `make`. A brief whose first sentence is longer gets the same error.
- **Money.** A free voice runs at once. A paid voice (ElevenLabs, or any row with a price) goes through the plan rule of the tool: the price is characters × the row's price, rounded up to a cent, as the voice stage of `make`. The `plan_id` covers the engine, voice, model, spoken text and price. Without `--approve <plan_id>`: exit 4, `approval_needed`, and no cache read, no worker and no call. A stale id is refused. `--max-spend` under the price gives `SVX_SPEND_CAP` and no id to approve. `--plan` gives the plan and runs nothing, also with `--approve`. With approval, the worker's `ledger.reserve` is the guard, and the cost is in `.scriptvox/state.db`. A paid row with no price is refused, not run.
- **The file.** Default `scriptvox-out/voice-<id>.<ext>`. The same sound again keeps that name. A different file at that name stays, and the preview goes to `voice-<id>-2.<ext>` and so on. A named `--out` that exists with other content is refused (exit 2). The extension is from the bytes: `wav` for a RIFF file (the local voices), else `mp3`. The worker's files stay in `.scriptvox/preview/`, which each preview clears.
- **The result.** `artifacts.audio`, `metrics.duration_s`, `metrics.chars`, `metrics.cost_usd`, `text`, `voice`, `explain` (who gets the text), and `next` with the `make --tts <engine> --voice <id>` line. A terminal shows a `result:` line with the file and its length.
- **MCP.** `scriptvox_voices` gets `preview`, `text`, `brief`, `tts` and `out` from the clap tree, with `approve`, `max_spend`, `plan` and `local_only`. Its row in `ANNOTATIONS` changed: not read only, reaches a network, can cost money, not destructive (it never replaces a different file). A side effect: under `mcp --write-root`, the plain list now needs the project inside a write root.
- **`selfhost`.** The open voice runs as in `make`: only free rows (`opentts::rows`), so there is no plan stop for it.

## Tests

`tests/voices_preview.rs` (stub worker, no network) and `a_voice_preview_makes_no_paid_call_without_approval` in `tests/elevenlabs.rs` (the real worker and the fake ElevenLabs API, which counts calls). All failed first with "unexpected argument '--preview' found".

- A free voice gives one file, the metrics, the terms note and the `make` line; the same call gives the same name; a different file at the name stays and the preview takes `-2`; a named `--out` with other content is refused.
- A brief gives its first sentence with the pause tag removed, and the stub got the `say` form. No text gives an Arabic sentence for an Arabic voice and an English one for an English voice.
- 204 characters, an unknown id, `--text` with no `--preview`, and `no-voice` are refused and write nothing. `--plan` writes nothing.
- Paid, no approval: exit 4, 0 calls. `--plan`, a stale id and a low cap: 0 calls. Approved: 1 call and a `committed` ledger row, and the key is in no output. The same call again: still 1 call (the cache). Its plan then shows cost 0 and it needs no approval (`voice-plan-cache.md`).

## Measured, 10 Nov 2026, real Edge voices, a scratch folder outside the repo

| Call | Result (ffprobe) |
|---|---|
| `--preview en-US-AvaNeural` | mp3, 1 audio stream, 24 kHz, 3.624 s, 21,744 bytes, 55 characters |
| `--preview ar-AE-FatimaNeural` | mp3, 1 audio stream, 24 kHz, 5.352 s, 32,112 bytes, 39 characters |
| `--preview en-US-AndrewNeural --brief b.json` (a pause tag, `say` 3 as three) | text "Rain fell on the city for 3 hours.", 2.352 s |
| `scriptvox_voices {preview: "ar-AE-HamdanNeural", text}` over a stdio pipe | not an error, 2.256 s; `tools/list` shows the new fields and hints |

## Not run

- Nobody listened: the files were read by ffprobe only. Whether the Arabic sentence sounds right is not known.
- No real paid call: ElevenLabs ran only against the fake API.
- `kokoro`, `kokoro-rust` and `selfhost` previews (they use the same path; the `wav` extension rule is not seen on a real file).
- No real MCP client: only a pipe. The terminal `result:` line was not seen on a real terminal, as this shell runs in agent mode.

## Project boundary (10 Nov 2026, found by a review before the merge)

A brief given to `--preview` goes through the same gate as in `make`: a `script_file` (or any other file of the brief) outside the project folder is exit 2, "is outside this project folder", and nothing is read or written. Before the fix the preview read such a file and sent its first sentence to the voice. Test `a_brief_cannot_make_the_preview_read_a_file_outside_the_project` failed on the code before the fix with exit 0 and an audio file.
