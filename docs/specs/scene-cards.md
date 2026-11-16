# More than one card in a scene: scene field `cards` (P1.2, G058)

State on 13 Nov 2026: built. Code: `crates/scriptvox/src/scene_cards.rs` (the rules and the times), the field `cards` on a scene and the field `at` on a card. Tests: unit tests in `scene_cards.rs`, `crates/scriptvox/tests/scene_cards.rs`. One `card` per scene with `on` and `hold` (`card-on-word.md`) works as before: same `captions.ass`, same `cards.json`, same plan id.

## Owner's answers, 13 Nov 2026

1. One card at a time. A scene can have several cards, one after the other, never two together.
2. A card's `at` counts from the start of its scene, not from the start of the video.

## Why

A scene often needs two things in turn: a title, then a number; or a stat, then a list. Today that needs two scenes, and a scene cut changes the picture. The idea is ours from a one-line note that an overlay can have its own start and end; no other tool's code or docs were read.

## Brief

A scene takes `cards`: a list of 1 to 4 cards. Each entry is a card as today, with its own start and length:

```json
{"script": "Revenue grew by forty two percent. Three things made it.",
 "cards": [
   {"kind": "stat", "text": "42%", "label": "revenue growth", "on": "forty", "hold": 2.5},
   {"kind": "list", "items": ["Faster delivery", "Lower prices"], "on": "three"}
 ]}
```

- `on`: a word of the scene's script, as today. The matching does not change: the first word of the scene that starts with it.
- `at`: new, for every card, also a single `card`: seconds from the scene's start (the scene's cut), 0 or more. A card takes `on` or `at`, not both.
- `hold`: as today. Default: to the next card's start, and for the last card to the scene's end.
- A card with no `on` and no `at`: the first card starts with the scene. A later card starts 0.2 s after the card before it ends, when that card has a `hold`. When it has no `hold`, the cards with no start share the time evenly, from the last card that has a start to the next card that has one (or to the scene's end). So two cards with no times get half of the scene each.
- `card` and `cards` in one scene are refused. One entry in `cards` is allowed and is the same as `card`.

## Rule: one card at a time

Two cards of a scene are never on screen together. This keeps the layout rules of every card as they are (each card is laid out for the whole frame), and it is what a viewer can read.

- The cards must be in time order in the list.
- A card's end is cut at the next card's start, less 0.2 s, so the fade out of one does not lie under the fade in of the next.
- After the cut each card must still show for 0.9 s, or the run stops: the message names the two cards and their times, and the fix line says what to change.
- On scene one with a `hook`, a card waits for the hook to go (1.6 s), as a single card does. This wait is applied first; the order check and the 0.9 s check use the times after it.

Not in this step: two cards on screen together (a lower third under a chart). That needs a place rule for each pair of kinds and is a later row.

## Refusals, exit 2, with the scene number and the card's place in the list

By `plan` and `make`, from the brief alone:
- `cards` with 0 entries or more than 4;
- `card` together with `cards`;
- `on` together with `at`; an `at` that is negative or not a number;
- an `on` word the scene's script does not have (today's rule, per card);
- two cards whose `on` words are spoken in the wrong order for the list, or are the same word (checked on the script's word order, with the first match of each word, as the voice's times are read);
- two cards whose `at` is not later than the `at` of a card before it;
- a card with `behind`, or a formula with `then`, in `cards` (each owns the whole scene);
- `caption_at` `middle` or `top` with `cards` (today's rule for `card`);
- every refusal a single `card` has (text too long, a `mark` the card does not have, and so on).

By `make`, after the voice is made (the real times are known only then). Each has a `fix` line:
- an `at` at or after the scene's end;
- a card that does not start after the card before it (an `at` card against an `on` card);
- a card that has less than 0.9 s after the cut at the next card.

## What the code does

- `scene_cards::of(scene)` gives the cards of a scene from `card` or `cards`. Every place that asks "has this scene a card" reads it: the text lint, the `claims` check, `caption_at`, the waveform, the chapters of `export`. `resume --patch` with `scenes` runs the same check of the cards.
- `scene_cards::problem(scene)` is the check of the brief. `scene_cards::windows(...)` gives each card's start and end from the voice's word times; `make` then draws each card with `cards::events`, as before.
- `cards.json` has one row per card. A row of a `cards` list has the field `card`, its place in the list from 1. A row of a single `card` has no such field (unchanged). `snapshot --cards` gives one frame per row; `judge` and the covers of `export` read the rows as before.
- `cards` and `at` are written into the plan id only when they are set, so the id of an old brief does not change.
- Gates: `text_bounds` and `contrast` read the burned events and did not change. The overlap check in `text_bounds` is per time, so two cards in turn do not count as one over the other.
- Not built: a pop sound per card. The `sfx` field's text speaks of one, but `make` does not call `cards::cues` for any card today, single or in a list.

## Tests

- Unit (`scene_cards.rs`): the windows of three cards from word times (order, the 0.2 s gap, the 0.9 s floor, `at`, the even share), and every refusal with the index in the message.
- `tests/scene_cards.rs`: a real `make` with `tts: none`, English and Arabic, cards in one scene: the events of one card end before the events of the next start, `cards.json` has one row per card, `snapshot --cards` gives one frame per card; `plan` refusals with exit 2; a brief with `cards` of one entry gives the same `captions.ass` as `card`.
- Measured on 13 Nov 2026 with the build before this change and the build after it, on 5 briefs with `card` only (English, Arabic, a hook, `on`, `hold`, `mark`, a list, bars, a formula with `then`): the plan id, `captions.ass` and `cards.json` are the same, byte for byte.
