# Add your own voice service (P4.1, 13 Nov 2026)

Any service that speaks the OpenAI speech API can be a voice: a local server on your machine, or a server on your own host. The gateway is `openai-speech` (`openai-speech-gateway.md`). This page is the how-to, and it says why no hosted paid service was added as a row.

## Steps

1. Add the row:

   `scriptvox providers add my-speech --gateway openai-speech --kind tts --t2v <model name> --base http://127.0.0.1:8880`

   The file is `<config>/generators.toml`. `base` is `https://...`, or plain `http://` only on 127.0.0.1 or localhost. `scriptvox providers template openai-speech` prints the same block to edit by hand.
2. State the price in the row. For your own free service add these lines to the row:

   ```toml
   usd_per_s = 0.0
   price = "exact"
   price_source = "https://the page of your service or model"
   price_checked = "2026-11-13"
   ```

   Without them the price is not known: `providers test` and `generate` then need `--max-spend` and an approval.
3. The voice of the service: `extra = { voice = "the voice name of your service" }`.
4. A service that wants a token: `key = "selfhost"` in the row, then `scriptvox keys set selfhost` in your terminal. For a second service with its own token, declare a key name in the same file (`[[key]]` with `name = "myspeech"` and `env = "MYSPEECH_KEY"`, see `key-names.md`) and write `key = "myspeech"` in the row. The call sends `Authorization: Bearer <key>`. With no `key` line (or `key = "none"`) no header is sent.
5. Prove it: `scriptvox providers test my-speech`. It makes one short call and, when a free speech to text row is there, listens to the answer (gate `listen`).
6. Use it: `scriptvox make --tts selfhost ...`. The row speaks for the `selfhost` voice. Languages: `en`, `ar`, and the rows `own-es`, `own-fr`, `own-de`, `own-pt`, `own-id` for the five languages of `latin-languages.md`. The word times come from the speech to text row (`scriptvox setup speech-local` is a free one).

The request: `POST <base>/v1/audio/speech` with `{model, input, voice, response_format: "wav"}`. The answer must be a WAV file.

## Hosted paid services: no row added

No hosted paid speech service was added as a row, and no pricing page was read for this item. The row format was checked first, and it stops here:

- A speech row carries a price per second of sound (`usd_per_s`) and a least price per call (`usd_min`). It has no field for a price per character of text. The one paid voice in the registry (`elevenlabs` in `voices.toml`) is priced per 1000 characters, and a row for a service that prices by text would need the same unit. A price changed by hand to another unit would be an invented price.
- `make` picks only speech rows with no charge (`opentts::rows`): its plan does not price a speech row. A paid speech row works through `generate` and `providers test` only (the fake-service tests of `openai_speech.rs` prove that path).

So no row is marked "not run live", and nothing is claimed here about the price or the request shape of any hosted service. A user can still add one by hand with the steps above, a declared key and the price of that service in the row, and use it through `generate`.

What a paid hosted row in the registry needs first (not done): a per-character price field on a `tts` row, the voice stage of `make` priced from it, and then one docs read for each service with the price, the terms and the date in its row.

## Checked, 13 Nov 2026 (a fake service on 127.0.0.1, no network)

| Check | Result |
|---|---|
| `providers add`, then `providers test` with no price in the row | exit 2, the fix names `--max-spend`; the service got no call |
| The price lines added, `providers test` | exit 0; the service got `POST /v1/audio/speech`, model `my-model`, `wav`, no Authorization header |
| `plan --tts selfhost` on a Spanish script with that row | voice stage `tts.selfhost`, row `own-es` |
| A row with `key = "selfhost"` and `extra.voice`, no key set | not 0, the message has `keys set selfhost`; no call |
| The same with a key in the environment | exit 0; the service got `Authorization: Bearer <the key>` and the voice name; the key is not in the output |

## Not done

No real third-party server was run for this page (the gateway page has one real run with a local server, 8 Nov). `make --tts selfhost` in the five languages was not run for real: it needs a speech service in that language. A row with a declared `[[key]]` name was not run (a row with an undeclared name is refused at load: "key ... is not known; add a [[key]] block").

Tests: `tests/voice_services.rs` (2), `tests/openai_speech.rs` (the gateway, as before).
