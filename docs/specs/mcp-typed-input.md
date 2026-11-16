# Named tool input (G032, 7 Nov 2026)

All tools shared `{"args": [string]}`. A client could not check a call, and an agent had to read help text. `ffmpeg-skill` documents typed tools; no code read.

- `mcp::input_schema` builds each tool's `inputSchema` from the clap tree: boolean, number, integer, string or array, with the flag's help. It comes from the commands, so it cannot drift, and CI has no separate file to go stale.
- A required argument says "Required." in its description but is not in JSON `required`, because a call in the older `args` form would then fail in the client. `args` stays for one version. clap still refuses a missing one.
- `mcp::typed_args` makes the command line: positional fields first by index, then flags, then `args`. A field the tool does not have gives `isError`, with its name. `additionalProperties` is false.
- A command with subcommands (`media`) has `subcommand` and takes the rest in `args`. `setup` also has `yes`, and the rule for a download does not change.
- `--json` and `--use-system` are not offered.
- The tool description now holds only the command's own text, as each flag's help is in the schema.

## Measured, 7 Nov 2026, real stdio session

| Call | Result |
|---|---|
| `tools/list` | 25 tools, 319 fields |
| `scriptvox_edit {input, zoom: [...], out}` | ran, wrote the file |
| `scriptvox_check {target}` and `{args: [...]}` | the same findings |
| `scriptvox_edit {input, wrong: 1}` | `isError`, "edit has no field wrong" |
| `scriptvox_setup {what}` | refused, needs a dry run or a yes |
| `scriptvox_setup {what, dry_run: true}` | ran |

Not run: Cursor, Claude Desktop or any other client: only a pipe.
