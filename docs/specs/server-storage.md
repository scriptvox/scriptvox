# Server storage and platform folders (G165, 8 Nov 2026)

First part of `docs/plan/MILESTONE-API.md`. The plan says this part can start before the owner's Go for the milestone; `scriptvox serve` itself is not built.

- `run::open_db(path, rollback)`: one way to open a SQLite file that the CLI and a server share. Busy timeout 10 s, and WAL, or the DELETE journal where WAL is not safe (`needs_rollback_journal`: 9p, drvfs, SMB, NFS, FUSE, OneDrive). `state.db` now opens through it.
- `run::migrate(conn, steps)`: step `i` runs when `PRAGMA user_version` is below `i + 1`, inside one `BEGIN IMMEDIATE` transaction that also sets the version. A failed step leaves the version as it was. A file newer than the binary is refused, not changed. Steps are only added at the end.
- `run::open_server_db()`: `<data>/server.db` at the newest schema. Version 1: `tokens` (id, sha256, scopes, label, created, revoked; never the token), `jobs` (run, project, token, status), `idempotency` (key and token to run), `spends` (plan, run, token, max_usd, time).
- Folders: `SCRIPTVOX_CONFIG` and `SCRIPTVOX_DATA` still win. Windows as before. Linux as before (XDG). macOS: `~/Library/Application Support/scriptvox` for both, but a set XDG variable wins, and a `~/.config/scriptvox` or `~/.local/share/scriptvox` that already exists stays in use, so keys, consent records and tools set up before are not lost.

## Measured, 8 Nov 2026

Unit tests only: migrations run once and in order, a newer file is refused, a failed step keeps the version, the 4 tables exist, `state.db` still uses WAL here, and the folder rule for Linux, Windows and the 3 macOS cases. Not run: a real macOS machine, `server.db` on a drvfs or OneDrive folder, two processes writing at once.
