# `scriptvox mcp`: the commands as MCP tools

Source: OpenMontage cherry #1 (agent-native surface including MCP stdio) and ComfyUI A1 (headless API, the most
asked-for item). Claude Code uses the CLI and the skill; other agents (Claude Desktop, Cursor, Codex) speak MCP.

- `scriptvox mcp` serves newline-delimited JSON-RPC 2.0 on stdio: `initialize` (with short instructions: the loop,
  never ask for keys, a paid plan needs the user's yes), `ping`, `tools/list`, `tools/call`.
- The tools are `scriptvox_<command>` for the 34 commands in `mcp::TOOLS`. Input: `{"args": [...]}`, the command line after the
  command. The server runs this same binary with `--json` (no shell) and returns the envelope as the tool text;
  `isError` follows the exit code. Descriptions are each command's clap help, so they cannot drift.
- Not offered: `keys` (keys go in only through `scriptvox keys set --stdin`, typed by the user), `uninstall`, `init`, `mcp`, `bench`, `licenses`, `skills`, `fetch` (it downloads from an address that the caller gives, and its opt-in must come from the person: `scriptvox_setup` with the target `fetch` is refused with "Run `scriptvox setup fetch` in a terminal yourself.", see `docs/specs/fetch.md`). The skills reach a client as `scriptvox_guide`, resources and prompts. The tool list is `mcp::TOOLS`, 34 commands plus `scriptvox_guide` and `scriptvox_page`, so 36 tools. `research` and `write` are in it since 13 Nov 2026 (`docs/specs/research.md`); they call a text model only when the user has set up a text row.

Client config example: `{"mcpServers": {"scriptvox": {"command": "scriptvox", "args": ["mcp"]}}}`.

Measured 3 Nov 2026: a stdio session initialize, tools/list (18 tools), tools/call `scriptvox_check r_4ea377` returned
the five gates. Test `mcp::tests::every_tool_has_a_command_and_keys_are_not_offered`.

G011 and G012 (6 Nov 2026). `tools/list` adds `scriptvox_guide {topic}`, `initialize` says to read it. `resources/list` and `resources/read` give each skill at `scriptvox://skills/<name>`, and `prompts/list` and `prompts/get` give the same text. All come from `skill::all()`, the text that `skills install` writes. `scriptvox_snapshot` adds the picture as image content, up to 5 MB, as well as the path.

G013 (6 Nov 2026). `scriptvox_setup` and `scriptvox_media` are tools. `scriptvox_setup` runs only with `--dry-run`, or with `--yes` after the user agreed to the size; the server removes `--yes` before it runs the CLI. That rule holds for an agent, not for a person at the CLI.

G032 (7 Nov 2026). Named fields from the clap tree, see `mcp-typed-input.md`.

G161 (7 Nov 2026). Big results: summary first, then pages. Idea from the public docs of Craft Anything; own design, no code read.

Measured before any code: `analyze --transcript` on 25 min gives 73 KB, about 2.9 KB a minute, so a 1 hour talk is about 176 KB. The next largest are `providers` 19 KB, `schema brief` 19 KB, `tools` 7 KB. So only long transcripts are large today.

- Above 40 KB of JSON, `tools/call` returns a summary. A big string shows its length and start, a big list shows its count and first 2 items, and `page` gives `result_id`, the size and how to read on.
- `scriptvox_page {result_id, path, offset, limit}` reads a list by items or a text by characters at a JSON pointer. A page stays near 40 KB.
- The server keeps the last 8 results in memory, so a read never runs the command again. After a restart, or after 8 more big results, the tool says so.
- CLI output does not change. A smaller result stays whole, so a client with a limit above 40 KB loses nothing it had under that size.

G164: read and write roots, see `mcp-roots.md`.

G088 (8 Nov 2026). `scriptvox_explain` (the recipe in a ScriptVox MP4) and `scriptvox_templates` are tools too, both read-only, idempotent, no network, no cost. Their arguments come from the clap tree like the others; `explain` reads its file inside the read roots. Real stdio session: `tools/list` shows both with `readOnlyHint`, `scriptvox_explain` on a made MP4 gives its request and plan id, `scriptvox_templates` gives 4 templates. No real client.

E7 (9 Nov 2026). `make` and `generate` always go to the background over MCP, see `mcp-detach.md`.

G135, first part (10 Nov 2026). `scriptvox_demo` and `scriptvox_replay` are tools. `demo` writes one short test video and can reach a network on a machine's first run (the pinned FFmpeg download). `replay` renders a ScriptVox MP4 again from the cache of its run and makes no provider call. Measured over a stdio pipe: `tools/list` has both, `scriptvox_demo` gave a video, and `scriptvox_replay` of a made video gave a video with cost 0. `remake` is still not offered: it can run for minutes and has no `--detach`, so it would block the server as `make` did before it went to the background. Its recipe can also come from a file the user did not make, so it needs the same care as `make` for approval.

G135, last part (10 Nov 2026). `scriptvox_remake` is a tool. `remake` gained `--detach`, and the server adds it as it does for `make` and `generate`, so the call answers at once with a run id. The recipe is checked before any run starts, and a paid recipe needs the user's yes. Rules, checks and the measured session: `remake-detach.md`. A flag that clap hides (`--run-id` for the supervisor, `--accept-gpl`, `--golden`) is no longer a named field of a tool. `args` still passes it: a `make` or `remake` with `--run-id` in `args` runs in the foreground and holds the server, as before. Open.

## Size of the tool list (G186, 13 Nov 2026)

A client reads `tools/list` at every start, so its size is a cost for every session. It was 80,771 bytes for 34 tools. It was 47,701 bytes after that work. With `research` and `write` (13 Nov 2026, 36 tools) it is 51,679 bytes, and a unit test fails above 54,000.

- Each field holds the first sentence of its help, at most 160 characters. The values a field takes stay (they are in the first sentence).
- Each tool holds the first paragraph of its command's help.
- The whole help of a command is one call: `scriptvox_guide` with topic `help-<command>`, for example `help-edit`. It is the text of `scriptvox <command> --help`.
- The flags every command accepts (`plan`, `approve`, `max_spend`, `idempotency_key`, `local_only`, `no_embed`) and `args` have no text in a tool. Their meaning is said once, in the description of `scriptvox_guide`.
- 16 tools that make no provider call and take no plan (`wait`, `status`, `cancel`, `snapshot`, `join`, `compare`, `tools`, `doctor`, `schema`, `setup`, `media`, `explain`, `templates`, `models`, `demo`, `replay`) do not list those flags. Given through `args`, they still work.

Not done: the rival's claimed 21.7 KB. The rest is the JSON of 300 fields and the annotations of each tool. Not measured: if a real client picks the right field as often with the short text as with the long one.
