# Price shapes

Gap G014. A row held only `usd_per_s` and `usd_each`, so a row with a minimum charge, a source-length price, a reference price or a sound price could not be priced, and the spend cap could not hold.

Four optional fields in `generators.toml`, all from the provider's page: `usd_min`, `usd_per_input_s`, `usd_per_reference`, `usd_per_s_sound`. `cost_for(seconds, input_s, references)` adds them and applies the minimum. `generate`, `make` and a paid first frame use it. A row with any of these is paid. `plan` and `providers` (`price_also`) name them. `generate` refuses a paid source-length row when it cannot measure the source.

One row uses these fields today: `sync-lipsync-2-fal` has `usd_per_input_s = 0.05` (G035). It is `unverified`, so this ran only in unit tests and in its plan, and no provider call was made. A row with a sound price needs `generate_audio = true` in its `extra`.
