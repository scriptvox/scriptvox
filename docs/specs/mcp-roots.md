# MCP read and write roots (G164, 7 Nov 2026)

A client can name any path in a tool call, so an agent can read or overwrite a file far from the project. Idea from the public docs of SoundCraft (`docs/plan/research/discovery-craft-suite-20261107.md`); own design, no code read.

`scriptvox mcp --read-root DIR --write-root DIR`, each more than once. No flag: no limit, as before. A `--write-root` can also be read. Each root must exist at the start.

With a root flag, before a tool runs:

- The project folder must be inside a root, and inside a write root for any tool that writes (the annotation table says which).
- `--out`, `--dir`, `--config`, `--like` must be inside a write root, also for a file that does not exist yet.
- Each value of the command line that names a file that exists must be inside a root. Links and `..` are resolved first. A value such as `a.mp4@12:4,b.mp4` is split.
- A JSON file on the command line is read, and each string in it is checked the same way, so a brief cannot name a file outside.
- `setup mcp` with no `--config`, and `snapshot` of a file outside the write roots with no `--out`, write outside the project, so they are refused.

The refusal is `isError` with the path, the folders and who sets them.

## Limits

This is a check in the server, not a sandbox. It guards the paths that a client names.

- The tools still use their own data folder, config and caches outside the roots.
- A path in a text file, or in a file a tool finds by itself (a `media add` folder's contents), is not read by this check.
- A file created between the check and the run is not seen.
- A client that starts `scriptvox` directly, with no `mcp`, has no limit. The user chooses the flags in the client's config; `setup mcp` does not write them yet.

## Measured, 7 Nov 2026

Real stdio session (a pipe, no real client), project in `rw`, `--read-root ../ro --write-root .`:

| Call | Result |
|---|---|
| `check ../ro/a.mp4` | runs |
| `check ../out/a.mp4` | refused |
| `edit ../ro/a.mp4 --out e.mp4` | runs, file in `rw` |
| `edit ../ro/a.mp4 --out ../out/e.mp4` | refused, nothing in `out` |
| `setup mcp --client cursor --dry-run` | refused |
| `doctor --offline` | runs |
| `--read-root /nope` | exit at the start, with the fix line |

Tests also cover a link out of a root, `--logo=` form, a file in a `--broll` value, `..` on a new file, a brief with a nested path, and a server with only read roots.
