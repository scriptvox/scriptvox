# Stock video: MoneyPrinterTurbo ideas (MIT)

Source: `app/services/material.py` (Copyright (c) 2024 harry0703, MIT): search terms in script order, Pexels and Pixabay search, the best file per clip, and a Range header to resume a download. No code copied: ScriptVox runs declarative rows in Rust (`registry/providers.toml`, `src/http_rows.rs`).

ScriptVox additions: providers as data rows (a new provider is a new row), shots cut on caption pages, a 429 or 5xx fallback to the next row with a `fallback` event, clips through the content-addressed cache with each provider's output terms (`redistribution:false`), a credits file per video, and NEEDS_INPUT with free alternatives when no key is set.
