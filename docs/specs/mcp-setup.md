# `scriptvox setup mcp --client <name>` (G163, 7 Nov 2026)

A user pastes JSON by hand to connect an MCP client, and a wrong edit can lose other servers. The behaviour is an idea from the public docs of Craft Anything (`docs/plan/research/discovery-craft-suite-20261107.md`): dry run, backup, read back. Own design, no code read.

It is a target of `setup`, not a new command, so `scriptvox_setup` carries it with the rule that it already has: `--dry-run`, or `--yes` after the user's yes.

Clients: `claude-desktop`, `cursor`, `codex`. `--config` names another file. The usual paths are from each client's docs and were not checked on a running Desktop or Cursor.

- `--dry-run` writes nothing. It gives `action` (add, update, none), the file, the entry, the backup name and the names of other servers.
- A real run writes a backup copy first (`<file>.scriptvox.bak`, then `.bak.1`, so the first one is never replaced), writes through a temp file, reads the file back, and checks that our entry is there and that the rest of the document is the same. A failed check restores the old state.
- Values of other entries never reach the output, because an `env` can hold a key. Names only.
- A file that does not parse stays as it is, with no backup. Exit 2.
- The entry holds the full path of this binary, because a client's `PATH` is often not the shell's.

Limits, on purpose:
- JSON: keys come out sorted, with our indent. The data stays.
- TOML (Codex): the block is added as text, so comments stay. A `scriptvox` entry with other values is refused, and the error says what to write by hand.

## Measured, 7 Nov 2026

Temp folders only. No real config of any client was read or written. Tests set `HOME` and every config path to a temp folder.

| Check | Result |
|---|---|
| Real run on a temp `config.toml` with a comment | comment stays, block added, backup holds the old text |
| Real Codex CLI 0.164.0, `CODEX_HOME` on that folder, `codex mcp list` | lists `scriptvox`, its full path, `mcp`, enabled |
| Second run | `action: none` |
| Test with a made-up value in another server's `env` | not in the output, the entry stays |
| Test where the read back fails | old text back; with no file before, no file after |

Not run: Claude Desktop and Cursor reading the file; Windows and macOS paths; a client calling a tool after this setup.
