# write: a brief from facts, and the text rows (13 Nov 2026)

Plan items P2.1 and P2.2 R6 of `docs/plan/MILESTONE-PARITY.md`. The research step is in `docs/specs/research.md`.

Until now ScriptVox had no text model inside: the agent wrote the script. That stays the main way and does not change. `write` is an opt-in part for a user with no agent. Nothing calls a text model unless the user added a text row and ran `research` or `write`.

```
scriptvox connect <name> --url <base> --text --model <id> [--key-name <key>] [--usd-in N --usd-out N] [--dry-run]
scriptvox write --from research.json [--duration N] [--lang en|ar] [--tts <engine>] [--sources-card]
                                     [--out brief.json] [--text-row <name>]
scriptvox write --topic "<topic>"     [--duration N] [--lang en|ar] [--tts <engine>] [--out brief.json] [--text-row <name>]
```

## 1. Text rows

A text row is data: one `[[text]]` block in `generators.toml` in the config folder (the file of the user's rows and key names). No row is built in, so no row is on by default.

```toml
[[text]]
name = "local"
base = "http://127.0.0.1:11434"
model = "<the model id of the server>"
key = "none"
usd_per_1m_in = 0.0
usd_per_1m_out = 0.0
```

| Field | Meaning |
|---|---|
| `name` | 1 to 48 of a-z, 0-9, `.`, `_`, `-`. |
| `base` | The base address of a service with the OpenAI chat API, without `/v1`. `https`, or `http` only on `127.0.0.1` or `localhost`: a key must not cross a network in clear text. |
| `model` | The model id that the service asks for. |
| `key` | `none`, or a key name. The value goes in only through `scriptvox keys set <name>`. |
| `usd_per_1m_in`, `usd_per_1m_out` | The user's price for one million tokens in and out. More than 0: a paid row. Both 0: free on this machine, "price not known" on a network service (section 3). |

`connect --text` writes the block. It uses the rules of `connect`: the address rule above, a key name that belongs to another service is refused, and a new key name is declared. It asks `GET <base>/v1/models` and refuses a model that the service does not list. `--dry-run` shows the block and writes nothing.

So a local server (Ollama, llama.cpp, LM Studio, vLLM) works with no key, and any hosted service with this API works with the user's own key.

The request is `POST <base>/v1/chat/completions` with `model`, `messages`, `temperature: 0`, `max_tokens` and `stream: false`. The answer is read from `choices[0].message.content`. Text around the JSON (a code fence, a thinking part) is allowed: the first complete JSON object is taken. Time limit 180 s for one call.

The first row is used. `--text-row <name>` picks another.

## 2. No row

`research` (without `--pages-only`) and `write` stop before any request:

- exit 10, status `needs_input`, one `needs_input` item;
- the fix is the one command: `scriptvox connect local --url http://127.0.0.1:11434 --text --model <id>`.

## 3. A paid row

A row with a price goes through the plan, the spend cap and the approval, as every paid step does.

- The plan id is made from the command, the row, its model and prices, and the inputs that set the size of the work (`research`: topic, language, depth, `--url` list; `write`: the hash of the research file or the topic, duration, language). It does not depend on the text of a page, so the id of the stop is the id of the next run.
- The cost of the plan is an upper limit: the largest number of calls of the command, each with the largest input (2 characters counted as 1 token) and the largest answer (`max_tokens`).
- Not approved: exit 4, `approval_needed`, with `plan_id` and `metrics.cost_usd`. The envelope tells an agent to ask the person (`Envelope::ask_before_paying`). An agent never approves by itself.
- Above `--max-spend`: exit 4, `SVX_SPEND_CAP`.
- At run time the cost is counted from the `usage` of each answer. `research` does not send a call that could pass the approved cost; it then ends with what it has and a `budget` finding. `write` sends at most the 3 calls of its plan (section 4, "The calls").
- A row on a network service with no price (both prices 0 and a base that is not on this machine): the cost is not known. Each command then stops with exit 4 and a `plan_id`, and runs only with `--approve <plan_id>`. So a row with the price left out is never a way around the approval. A row on this machine with no price is free.
- `--plan` shows the cost and sends nothing.
- `--local-only`: `write` runs only with a row on this machine. `research` always stops.

## 4. `write --from research.json`

1. Read the file (`scriptvox.research/v1`). The facts go to the model in this order: `confirmed`, `single`, `conflict`, at most 40.
2. The model gets the topic, the language, the target duration (default 45 s, 10 to 600), the word range (see "The length"), and the facts. Each fact has its id and its marks: its status, `[number 450 nm]` when its value is a number, `[quote by <name>]` when it is a quotation. The model must answer with JSON: `hook` and `scenes`, each scene with `script`, `facts` (the ids it uses) and, if it wants, `card` (a kind and fact ids).
3. The code builds the brief. The model gives only words, fact ids and the kind of a card. It cannot set any other field of a brief (no file, no address, no music, no generator, no text of a card), because the code copies nothing else from its answer.
4. The brief gets `lang`, `duration`, `hook`, `scenes` (each with `script`, some with `card`) and `sources`: one entry for each fact that a scene or a card uses, in the order of first use, with the fact's `text`, the `url` of the page, `title` (the page title and the publisher) and `publisher`. The addresses come from the file, never from the model. `--tts` puts `tts` in the brief, and `--sources-card` puts `sources_card: true` (`sources-card.md`).

The answer is valid when all of these are true (checked in code):

- It is JSON of the form above, with 1 to 12 scenes (more for a long video: one for each 5 s); each `script` is 1 to 600 characters; `hook`, if given, is at most 80 characters.
- Each fact id of a scene is in the file. At most 20 facts are used (the limit of `sources`).
- Each number and date of the scripts and the hook is in a fact that is used.
- A fact with status `conflict`: when the script or the hook says its value, they also say the value of each fact it is in conflict with, and both facts are listed. Else the answer is not valid, and the model is told the two values.
- The brief that the code built passes the brief schema (the same parser as `make --brief`), the form check of `sources`, and gate `claims` as `plan` runs it (the script, the hook and the cards).

### The length

The code owns the length, not the model.

- The target is `duration` times the words a second of the voice, from the rate table that `plan` uses for its `fit` finding (`fit::words_per_s`: English 3.3 on Edge and ElevenLabs, 3.0 on `kokoro-rust`, 4.1 on the open voice; Arabic 1.5 on Edge). The voice is `--tts`, else the engine that `make` picks on this machine for the language.
- The words of an answer are counted as `fit` counts them (`fit::spoken_words`): a number counts as the words that say it, and Arabic counts letters.
- The prompt states the range: the target minus and plus 6 percent ("Write 93 to 105 words").
- An answer more than 15 percent from the target is a miss. The model is asked one more time, with the count: "Your last answer had 56 words in its scripts. That is too short. Write 93 to 105 words".
- After that the answer nearer to the target is taken. The envelope has the finding `fit`: `pass`, or `warn` with the count and the target. `plan` then shows what the voice does with the script.
- An answer that is too long is no error any more; it is a miss as a short one is.

### The cards

A scene can have one card from a closed list. The model names the kind and the fact ids. The code fills the card from the facts.

| Kind | Facts | What the code puts on the card |
|---|---|---|
| `title` | none; the first scene only | The hook. The brief then has no `hook`, so the words are not drawn two times. |
| `stat` | one fact whose value is a number or a date | `text`: the value, cut out of the sentence of the page in the page's own digit form (at most 24 characters). `label`: the `subject` of the fact, when it holds no number. |
| `quote` | one fact with `quote` (`research.md`, section 4) | `text`: the words; `label`: the speaker. Both must be in the sentence of the page. |
| `bars` | 2 to 6 facts whose values are numbers of one unit, each with another subject, and the subjects start with the same words ("wavelength of blue light", "wavelength of red light") | `items`: what is left of each subject ("blue light", "red light"); `values`: the numbers; `label`: the shared words and the unit ("wavelength (nm)"). |

A card is dropped, and the script stays, when: a fact id is not in the file; a fact is in `conflict` (a card shows one value); the fact has no number or no quotation; the value is not in its sentence; the kind is not in the list; `Card::problem` names a fault; or a number of the card is not in the `text` of its fact (gate `claims` would warn). A dropped card costs no call. The envelope names each one in `cards_dropped` with its cause, and `scenes[].card` has the kind of each card that was made.

The order of the facts and the marks tell the model to prefer `confirmed`, then `single`. The code does not judge which fact is the best for a card.

### A number and a card with a number

When the file has a fact with a number or a date that is not in conflict, the script must say the value of such a fact, and one scene must have a `stat` or `bars` card that the code could build. A valid answer without them is asked for one more time; the message names what is missing and the facts that can give it. After that the answer is taken with the finding `cards` (`warn`: "no card with a number"), never in silence. The full rule is in `research-search.md`, section 4.

### The calls

At most 3: one answer that is not valid is asked again with the cause, and one valid answer that misses the length, or has no number or no card with a number, is asked again. That one question holds the word count and what is missing: the length and the numbers share it. A third answer is not asked for a second miss. Of two valid answers the one with fewer things missing is taken (the length is one thing), then the one nearer to the length. When the answer to the length question is not valid, the valid answer before it is taken. Two answers that are not valid: exit 1, `SVX_WRITE_INVALID`, with the cause, and no file is written.

A service that refuses the request shape (`max_tokens`, section 1) gets one more request, one time for the command: the refused request costs nothing, the later calls start with the newer shape, and `metrics.calls` counts it (so 4 at most with such a service).

The plan says "at most 3 calls". Its cost is `3 * (characters_in_at_most / 2 * usd_per_1m_in + max_tokens * usd_per_1m_out) / 1,000,000`. `max_tokens` is 2,000, or more for a long video: `duration * (words a second of the fastest voice of the language) * 4 + 600`. It does not depend on `--tts`, so `--tts` does not change the plan id.

The envelope gives `artifacts.brief`, the list `scenes` (number, fact ids, card), and in `metrics`: `scenes`, `words`, `words_target`, `facts_used`, `facts_left_out`, `cards`, `cards_dropped`, `calls`, `cost_usd`. `--plan` gives `length` (`words_target`, `words_asked`, `words_per_s`, `tts`). `next` is `scriptvox plan --brief <file> --json`.

## 5. `write --topic`

It does not run `research`. The model writes from what it knows, so the brief has no `sources`, and no code checks its facts. The envelope says so with a `claims` finding of status `warn`: "the facts of this script are not checked". The prompt asks the model to leave out numbers and dates that it is not sure of. For checked facts: `research`, then `write --from`.

## 6. Errors

| Cause | Exit | Code | Fix |
|---|---|---|---|
| No text row | 10 | `SVX_NEEDS_INPUT` | Section 2. |
| The row's key is not set | 10 | `SVX_NEEDS_INPUT` | `scriptvox keys set <name>` in the user's terminal. |
| Both or none of `--from` and `--topic`; bad `--duration` or `--lang`; `--out` exists | 2 | `SVX_USAGE` | Names the flag. |
| The research file cannot be read, is not `scriptvox.research/v1`, or has no usable fact | 2 | `SVX_USAGE` | Run `scriptvox research` first, or correct the file. |
| A paid row, not approved or above the cap | 4 | Section 3 | Section 3. |
| `--local-only` with a row that is not on this machine | 2 | `SVX_USAGE` | Use a local row, or drop the flag. Nothing was sent. |
| The row does not answer, or answers with an HTTP error | 5 | `SVX_PROVIDER_RETRYABLE` | Start the server or check the row; run again. |
| The answer is not valid two times | 1 | `SVX_WRITE_INVALID` | Run again, give a longer `--duration`, or use a larger model. |
| `--sources-card` with `--topic` | 2 | `SVX_USAGE` | A script from a topic has no sources: `research` first. |

An error message holds the host of the row's base and the HTTP code. It never holds the key, and the body of an error answer is not printed.

## 7. Not done

- Done on 13 Nov 2026 in `make-topic.md`: `make --topic` end to end with no agent (P2.3). `make` with a topic word and no flag still asks for a script.
- The pictures of a scene (stock terms, generated clips): `write` gives words and cards only. `make --stock auto` or an agent adds the pictures.
- A card kind outside the four (`list`, `line`, `compare`, `timeline`, `donut`, `table`, `number_line`) is not made. Two cards in one scene are not made.
- `bars` reads "the same kind of thing" from the first words of the subject names, which the model gave at research time. Two names for one kind ("height of X", "X's height") make no chart.
- The heading of a `bars` card loses a short last word by its length ("wavelength of" becomes "wavelength"), not by a word list.
- The length target is for one voice. `make --tts <other>` on the same brief reads at another rate; `fit` then says so.
- No streaming, no tool calls, no vision input.
- Which facts a scene uses is in the envelope, not in the brief: brief `sources` is one list for the video.
- The quality of the script depends on the model. The code checks the form and the numbers, not the style.
