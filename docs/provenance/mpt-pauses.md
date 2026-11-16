# Pause tags: MoneyPrinterTurbo (MIT)

Source: `app/utils/utils.py` (`PAUSE_TAG_KEYWORDS`, `PAUSE_TAG_PATTERN`, `parse_script_with_pauses`), Copyright (c) 2024 harry0703, MIT licence.

Ported into `crates/scriptvox/src/pauses.rs` as rules, not as code: the keyword list, units, the 1 s default, the 0.1 s to 10 s clamp, merged runs, and invalid tags removed. The Rust scanner is our own (no regex).

ScriptVox additions: Arabic keywords (توقف، وقفة، صمت، سكوت، استراحة، مهلة), Arabic units (ثانية، ثوان، ث، مللي), Arabic-Indic and Persian digits and the Arabic decimal separator, an invalid tag that does not split the speech, exact silence by sample count, and trimmed TTS tails so the tag is the gap the listener hears.
