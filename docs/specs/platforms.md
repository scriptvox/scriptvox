# Platform limits and screen zones (G057)

`registry/platforms.toml` holds, per platform: frame shapes, maximum length, maximum file size, the screen zones the platform's own controls cover, the date the row was read, and `source` (`official`, or `secondary` for third-party guides that quote the platform).

- `check <run or file> --platform <name>` adds a finding `platform`. A shape is right when the width over the height is within 1% of a listed shape (an encoder may round 1920 to 1918). A wrong shape, or a length or size over the row's limit, fails with exit 3, as any gate does. The finding says the date and the source, and which limits have no value on record and so were not checked.
- `snapshot <target> --safe <name>` shades the row's zones in red on each frame. A row with no zones refuses with exit 2, and says so.
- An unknown name is exit 2 and lists the names.

Rules of the table: a limit nobody could read is left out, never guessed. Only YouTube's two rows come from the platform's own page. On 10 Nov 2026 the Meta and TikTok help pages could not be read from this machine (expired certificate), so their rows are `secondary`, and each `note` says what is weak. TikTok publishes no fixed zone: the row gives one guide's reading.

Not in this slice: the caption margin taken from this table, and more platforms. Tests: `src/platforms.rs` (table, rules, a shape within 1%, filter), `tests/gates.rs` (a real file through `check` and `snapshot`).
