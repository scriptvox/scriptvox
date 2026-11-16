# Row gateway `openai-speech` (G049, 8 Nov 2026)

Many local voice servers (Kokoro-FastAPI, openedai-speech, Piper wrappers, and others) speak the OpenAI speech API. Public API docs only; no code read.

- A row with `gateway = "openai-speech"` and `kind = "tts"`, in `<config>/generators.toml`. `base` is the service: `https://...`, or plain `http://` only on 127.0.0.1 or localhost. `t2v` is the model name the service gets. `extra = { voice = "..." }` sets its voice. `key = "none"` sends no Authorization header; a declared key sends `Bearer <key>`.
- One call: `POST <base>/v1/audio/speech` with `{model, input, voice, response_format: "wav"}`. An answer that is not a WAV file (an error page, JSON) is exit 5 with the service named. A finished file is kept, so a rerun asks nothing.
- The row speaks for the `selfhost` voice engine, as the rows on your GPU service do: `make --tts selfhost` uses it. The `selfhost` voice is now available when such a row is ready, not only when the `selfhost` key is set.
- `scriptvox providers test <row>` on a voice row adds gate `listen`: a free speech row hears "This is a test." and each word must come back, else exit 3. With no free speech row the gate is a `warn` that says it did not run.
- Price: these rows may be $0 (your own service).

## Measured, 8 Nov 2026, 2 cores

A real local service: a 30-line Python server (in the session scratch folder, not in the repo) on the Kokoro model that `setup voice-local` installed, on 127.0.0.1.

| Check | Result |
|---|---|
| `providers` | the row listed, "no charge, on your own speech service at http://127.0.0.1:18891 (OpenAI speech API)" |
| `providers test` | 10.2 s, exit 0, `listen` pass: heard "this is a test." The service got model, input, voice `am_michael`, `wav`, no Authorization header |
| `make --script --tts selfhost` | 27 s, 7 gates pass, `words` 13 of 13; sheet of 4 frames read by eye |
| Fake service, tests | a WAV is saved and the body is right; JSON instead of audio gives exit 5; `http://example.com` and a non-voice kind are refused at load |

## Not done

A real third-party server (Kokoro-FastAPI and others). The `listen` fail path with a real wrong voice (only a fake). Word times still come from the speech row, as for the other open voice rows. A voice list from the service.
