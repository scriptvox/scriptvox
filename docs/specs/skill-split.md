# Skill split: a short main skill and a generated reference (P8.5)

State on 13 Nov 2026: built. Code: `crates/scriptvox/src/skill.rs`, the lists `COMMON_FIELDS` in `brief.rs` and `COMMON_COMMANDS` in `cli.rs`.

## Why

The main skill, `plugin/skills/scriptvox/SKILL.md`, is generated from the tool. It had one line for each brief field and each command, so it grew with every feature: 152 lines on 13 Nov, against a test limit that was raised from 150 to 160. An agent reads the whole file before every video. Most videos use the same 20 to 25 fields and commands.

## What

Two generated skills:

- `scriptvox` (the main one): the steps, the rules, the common brief fields, the common commands, the exit codes. After the common fields, one line names every other field. After the common commands, one line names every other command. Both lines name the second skill.
- `scriptvox-reference` (new): one line for each brief field and each command that the main skill names but does not describe. It has its own front matter, so every agent that reads a skills folder finds it by name.

An agent that needs a field that is not in the main skill has three ways: read `scriptvox-reference`, run `scriptvox schema brief --json`, or run `scriptvox <command> --help`.

## What is "common"

Two lists in code, next to the things they name:

- `brief::COMMON_FIELDS`, next to the brief's other field lists;
- `cli::COMMON_COMMANDS`, next to the `Cmd` enum.

A name in a list goes to the main skill. Every other field or command goes to the reference. So a new field or command lands in the reference with no other change, and the main skill grows only when a person adds a name to a list. No skill text is edited by hand.

## Rules, each with a test

- Every field and every visible command is described in exactly one of the two skills (`skill::tests::every_field_and_command_is_in_one_of_the_two_generated_skills`).
- A name in a list that the tool does not have fails the same test.
- Every command that the steps of the main skill use is described in the main skill (same test).
- Both committed files are the same as what the tool renders; the main skill is under 150 lines and the reference under 300 (`tests/docs.rs`, `committed_skill_matches_the_binary_and_is_short`). On 13 Nov the main skill has 106 lines and the reference 63.

## Commands

- `scriptvox skills render --json`: the field `skill` is the main skill, as before; the new field `reference` is the second one. `just docs` writes both files.
- `scriptvox skills install` and `skills install --agents`: write `scriptvox-reference/SKILL.md` beside the other skills, in the Claude Code folder, in `~/.agents/skills` and in the Hermes folder. The main skill stays first in the answer.
- `scriptvox mcp`: the reference is one more topic of the guide and one more resource, with the other skills.

## Limits

- The reference describes top-level brief fields only, as the main skill does. The fields of a scene and of a card are in `scriptvox schema brief --json` and in the `scriptvox-brief` skill.
- Which names are common is a choice of 13 Nov 2026, not a measurement of use.
