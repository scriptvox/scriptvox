# Skills for other agents (G043)

`scriptvox skills install --agents` writes the main skill and every topic skill for the coding agents that are not Claude Code. It does not write `~/.claude/skills` (plain `skills install` does that). The answer has one row for each target in `agents`: the agents, the path, `installed` or `skipped`, and a note.

| Target | Agents | When |
|---|---|---|
| `~/.agents/skills/` | Codex, Cursor, Gemini CLI, opencode, Pi | always |
| `<Hermes home>/skills/` | Hermes | only when the Hermes home is there |

- The shared folder: the docs of Codex, Cursor and Gemini CLI, read 7 Nov 2026, name `~/.agents/skills/`. opencode and Pi read it too (13 Nov 2026: `opencode debug skill` listed a skill from it; the Pi docs name it).
- Hermes does not read the shared folder. It reads `skills/` in its home: `HERMES_HOME`, else `%LOCALAPPDATA%\hermes` on Windows, else `~/.hermes` (13 Nov 2026: `hermes skills list` listed a skill from it).
- The command never makes a Hermes home. With none, the Hermes row is `skipped`, and its note says to run `scriptvox skills install --agents` again after Hermes is installed.
- `scriptvox init --agents` adds one short block to the `AGENTS.md` and `CLAUDE.md` of a project, one time. A `CLAUDE.md` that imports `AGENTS.md` is not changed.
- For a later public install: `plugin/.codex-plugin/plugin.json`, `plugin/.cursor-plugin/plugin.json` and `plugin/gemini-extension.json` name the same `plugin/skills/`. The Gemini file starts `scriptvox mcp`. Manifest fields follow the shape in HyperFrames (Apache-2.0); these are facts about our own package, and no text was reused.

Tests: `tests/skills.rs` (the shared folder, Hermes there and not there, `HERMES_HOME`, and `the_spec_names_every_agent_of_the_answer`, which fails when this file does not name an agent of a row); the manifest test in `tests/docs.rs` checks name, version and fields against the Claude manifest.

Real runs:
- Codex CLI 0.164.0, with an empty home and only `skills install --agents` done, named all 12 skills of that day (7 Nov 2026). It listed names only and did not use a skill.
- opencode and Hermes each listed a skill from its folder (13 Nov 2026, as above).
- Not run: Cursor, Gemini CLI and Pi. They are not installed here, so their part stands on their docs. A marketplace install: it needs a public repo.
