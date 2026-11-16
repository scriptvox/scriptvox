# A row priced by the user's source says so (section 6 row 10, L7)

A paid row whose whole price is per second of the source (lipsync) printed `0 s at $0/s`, which reads as free. `Generator::priced_by_source` makes `price_words` and `cost_words` say `$0.05/s of the source`, and the plan's real cost stays computed from the source length as before. Test `a_row_priced_by_the_source_never_says_zero_per_second`. Real run: `generate --model sync-lipsync-2-fal --plan` on a real clip: $2.22 and the new words; `tools` no longer shows `$0/s`.
