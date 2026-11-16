# make --topic: one command with no agent (13 Nov 2026)

Plan item P2.3 of `docs/plan/MILESTONE-PARITY.md`. It uses `research` (`docs/specs/research.md`) and `write` (`docs/specs/write.md`).

```
scriptvox make --topic "<text>" [--lang en|ar] [--duration N] [every other flag of make]
```

Why: a user with no agent had three commands to run and two files to carry. This is one command.

## 1. What it does

1. `research "<text>"` with depth `normal`: facts with their sources, each checked in code.
2. `write --from` that file: a brief with `scenes` and `sources`.
3. The normal `make` with that brief: the same plan, the same gates (also `claims`), the same approval for a paid plan.

Opt-in, as `research` and `write` are. With no text row nothing is sent: exit 10, `needs_input`, and `next` is the one command `scriptvox connect local --url http://127.0.0.1:11434 --text --model <id>`.

`make "<text>"` with no flag stays as it was: it asks the agent for a script and calls no model.

## 2. The files

`scriptvox-out/topic-<8 hex>/research.json` and `brief.json`. The name is a hash of the topic, `--lang` and `--duration`.

- The second run of the same command finds the brief there and calls no model. This is needed: a run with `--approve`, and the child of `--detach`, are the same command again, and a model does not give the same words two times. Other words would be another plan id.
- To get a new script: remove the folder. To change the script by hand: change `brief.json` there, or run `make --brief` with your own copy.
- With no `--duration` the brief has no `duration`: the video is as long as the script that the model wrote. With `--duration N` the script is written for N seconds, and `make` checks the length as always.

## 3. Stops

Each stop has the command name `make`. The fix is always: the same `make --topic` command again.

| Case | What happens |
|---|---|
| No text row, or its key is not set | Exit 10, the one command. No request. |
| A text row on this machine | No stop for the text steps. A free plan of the video then runs at once; a paid one stops with exit 4. |
| A paid text row (or a network row with no price) | Up to three stops with exit 4, each with its own `plan_id`: the research, the script, then the video if its plan is paid. Each needs its own yes of the person. |
| `--plan` and no script yet | Nothing is sent. The answer is the plan of the research step, with `topic_steps`. The plan of the video exists only after the script. |
| `--plan` and the script is there | The plan of the video, as `plan --brief`. |
| `research` finds no page or no fact | Its error (exit 1 or 5). No script is written from the model's memory in its place: the fix names `write --topic` for a person who wants that. |
| `--local-only` | `research` needs the network, so the command stops with its usage error. |
| `--topic` with `--script`, `--brief` or a topic word | Exit 2. |

## 4. Limits

- No real model was run: this machine has no local text server. The test uses the fake text server of `tests/research.rs`.
- `write` gives words only. For pictures add `--stock auto` or `--media <folder>` to the same command.
- The built-in search rows are three publisher families; the three new rows are for science and in English only (see `research.md`, section 2). For another kind of topic most facts stay `single`.
- `--tts` of the command sets the word target of the script (`write.md`, "The length"). The folder of the topic does not depend on `--tts`: a second run with another voice finds the same script.
- The script step sends at most 3 calls (was 2).
- A paid row needs three runs of the command. One approval for all three steps is not done: each id is made from other inputs.
- `--detach` with `--topic` was not run. By the code the text steps run before the detach, and the child finds the brief.
- The result does not name the brief file in the envelope; the path is printed on stderr.
