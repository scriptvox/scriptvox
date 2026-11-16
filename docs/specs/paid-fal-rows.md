# Paid image and speech to text rows through fal (5 Nov 2026)

Gap: a user with no GPU service has no image row and no speech to text row. Rows are data, but the row rules refused any price not by time.

- New field `usd_each`: the price of one picture, image rows only. On a speech to text row `usd_per_s` is per second of the audio sent. Other non-timed kinds stay free. A hosted row with no price is refused.
- `flux-pro-ultra-fal`: `fal-ai/flux-pro/v1.1-ultra`, $0.06 an image, `price = "base"` because the page also speaks of rounded megapixels. `scribe-fal`: `fal-ai/elevenlabs/speech-to-text`, $0.03 a minute. Fields read from fal's OpenAPI document on 5 Nov.
- `unverified = true`: **no call was ever made to these rows.** `providers`, `tools` and `plan` say "not run live yet". The tests use a fake queue. A data URI as `audio_url` is not stated on fal's page, so that may need a fix on the first real run.
- Not purely data: fal returns a transcript in the result itself, so `output = "inline"` keeps the result JSON as the file (one branch in `genjob`). The `generate` path passes the estimate for the audio's length, so the spend cap holds; a paid transcript with an unmeasured length is refused.
- `scribe-fal` works only through `generate --model`, with a plan and approval. The result is the provider's JSON. `edit`, `clips`, `analyze` and `check --hear` have no approval step, so they use only self-hosted rows. Local speech to text is the way to free them (RELEASE row 8).

Found and fixed, money:
- `analyze --transcript` took any speech row with a key. A fal row and a fal key would have paid with no plan. Now self-hosted only.
- A paid first frame would not have been on the plan's bill. Now it is.
- The image row choice took the first in the file. Now the cheapest, so a free row goes before a paid one.

Audit fixes, 9 Nov 2026, from fal's OpenAPI schema and model pages read directly, not through a summary. No real call has confirmed a field or a price.

- `grok-imagine`: `resolution = "720p"`, fal's default and its highest. $0.07/s plus $0.002 for an image input, as $0.0704/s, so 6 s plans at $0.43.
- `minimax-h3-max-turbo`: `resolution = "768P"`, fal's default. $0.04/s is the rate from 16 Nov 2026 ($0.028 before), and the output can run 0.7 s long, so $0.046/s and 5 s plans at $0.23. Until 15 Nov the plan is above the bill.
- `kling-2.6-pro`: new row field `image_field = "start_image_url"`. The image to video schema requires that name and has no `image_url`.
- A resolution of 480p on these rows would be cheaper, but a setting the service ignores would then bill above the plan.
