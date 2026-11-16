---
name: scriptvox-genres
description: Pick the right kind of scriptvox video and start from its recipe - explainer, short explainer, product demo, ad, news brief, tutorial, data story, top 5 list, quote short, micro-drama with two voices, and Arabic explainers. Use when the user names a kind of video ("make an ad", "a news short", "a tutorial", "a top 5", "a reel with a quote", "a product video from this page", "ad variants"), or gives a topic and you must choose the shape, the scene count, the cards, the music and the caption style before you write the brief.
---

# scriptvox-genres

A kind of video is a promise to the viewer: an ad sells one thing, a news brief reports, a tutorial lets them
repeat a task. Each kind has its own script shape, pace, cards and sound. ScriptVox holds one starter brief for
each, called a recipe. A recipe is a whole small video with invented words. You keep its shape and replace its
words.

```
scriptvox recipes --json                 # the list: name, kind, seconds, aspect, voice
scriptvox recipes show news-brief --json # recipe.brief, recipe.shape, recipe.adapt
```

## 1. How to use a recipe

1. Pick the kind from the table in section 2. If two fit, pick by the goal, not by the topic.
2. Run `scriptvox recipes show <name> --json`. Save `recipe.brief` as `brief.json`. Read `recipe.adapt`.
3. Replace the words scene by scene. Keep the number of scenes, the order of the beats and the card kinds.
   Change a card's text together with its scene's words: a `steps` card, a `list`, a card `on` and every `mark`
   follow words that the voice says.
4. `scriptvox plan --brief brief.json --json`. Take the words that a `fit` finding names. A `claims` finding names
   a number that is in no source.
5. Then the usual loop of the `scriptvox` skill: make, wait, look at a sheet, fix.

Every recipe runs free: the local voice, or no voice, and no picture. A recipe has no picture on purpose, so it
works in any folder. Add pictures after the words are right: `media`, `stock` or `generate` on a scene.

## 2. Which recipe

| The user wants | Recipe | Length, shape |
|---|---|---|
| to teach how or why something works, for a screen | `explainer` | 60 s, 16:9 |
| one small question answered, for a phone | `explainer-short` | 30 s, 9:16 |
| the same two in Arabic | `explainer-ar`, `explainer-short-ar` | 60 s 16:9, 30 s 9:16 |
| to show what a product does and costs | `product-demo` | 30 s, 16:9 |
| to sell one thing fast | `ad` | 15 s, 9:16 |
| to report an event, with its sources | `news-brief` | 30 s, 9:16 |
| to teach a task at a keyboard | `tutorial` | 40 s, 16:9 |
| to say what some numbers mean | `data-story` | 40 s, 16:9 |
| five tips, places or things | `listicle` | 30 s, 9:16 |
| one line to remember, text and music | `quote-short` | 10 s, 9:16 |
| a tiny story in dialogue | `micro-drama` | 25 s, 9:16 |

Near misses: a product with a story of numbers is a `data-story` with an outro, not a `product-demo`. A "how
to" with no keyboard is an `explainer` with a `steps` card. A news item with an opinion is not a `news-brief`:
tell the user, and write it as an explainer in the user's own name.

## 3. The rules of each kind

### Explainer (`explainer`, `explainer-short`)

- Open with a claim that turns a common belief around. Never open with the topic's name or a greeting.
- One idea a scene, each joined to the next by "but", "so" or "because".
- Give the real cause in two or three steps, then one thing the viewer can try or see for themselves.
- Cards do the pointing: `steps` for a process, a `diagram` for cause and effect, a `list` for the result.
  Leave two scenes with no card, so the eye rests.
- Calm music, a soft transition, `karaoke` captions at 16:9. On a phone: `pop` captions and one card a scene at most.
- End on the opening claim, now understood. No new fact in the last sentence.

### Arabic explainer (`explainer-ar`, `explainer-short-ar`)

- Write in Arabic from the start. Arabic takes about half the words of English for the same time, so cut ideas.
- Every card in Arabic words, with one digit style in the script and the cards.
- Lists and steps move to the right side, and a wide diagram runs from the right, with no field for it.
- Keep `karaoke` captions: Arabic words are long, and a whole phrase reads better than three big words.
- The recipes have no voice, because no free local Arabic voice exists. With a network, give them the Edge
  voice as their `adapt` note says. Load the `scriptvox-arabic` skill.

### Product demo (`product-demo`)

- Start with the user's problem in the user's words, not with the product.
- Name the product once in a sentence that says what it does. Then show three things it does, no more.
- One card for each proof: a `list` whose points are marked as the voice names them, a `table` for the plans.
- Give prices and limits only as the user gave them. Do not round a price or invent a plan.
- One call to action, said once, and the same words on the `outro` card. The outro adds 2.5 s.
- Use the brand colour as `accent` and the user's logo as `logo`. Put a screen recording on each feature scene.
- From a product page: copy the name, the benefits and the action by hand. ScriptVox does not read the page.

### Ad (`ad`)

- 15 s is about 30 spoken words. The viewer's problem comes in the first 2 s, as a question or a picture.
- One product, one promise, one action. A second promise is a second ad.
- Three short points at most, on a `list` that springs in. No number unless the user can prove it.
- Fast sound and picture: `pulse` music, `cut_on: beat`, `pop` captions, a hard transition, `sfx`.
- The last spoken line says where to get it. The `outro` card repeats it for viewers with the sound off.
- Variants: keep the scenes and change one thing per version (the hook, the points or the outro). One brief
  with `{{key}}` text and a rows file goes to `scriptvox batch --brief b.json --rows rows.json --json`.

### News brief (`news-brief`)

- The first sentence says what happened, where and when. The hook is the event, not a tease.
- Every number and date is in `sources`, with the real address. `plan` warns about one that is not.
- Say who says so, in the sentence: "the council says". Do not add your view, and do not guess a cause.
- Time order after the first sentence. A `timeline` card for what comes next, a `stat` for the one key number.
- No music, plain cuts, a calm background. A bed or a bright look tells the viewer how to feel.
- `sources_card` puts the publishers on the last scene. Fill each source's `publisher` with a short name.
- For real facts: `scriptvox research "<topic>" --json`, then `scriptvox write --from research.json --sources-card --json`.

### Tutorial (`tutorial`)

- Say the problem, then the whole plan in one `steps` card before the first command.
- One command a scene. A `terminal` card shows the line to type and what it prints; a `code` card shows a file.
- Show only the lines the voice talks about: a panel holds 12 lines of 44 characters.
- The voice says what the command does, not its letters. A name it reads wrong goes in `say`.
- Small captions, so the panel has the room. Slow pace: 5 to 9 s a scene.
- Run every command yourself first. End with the result and one caution.

### Data story (`data-story`)

- One chart a scene, and one message a chart. Say the message; let the chart show the values.
- Pick the card by the question: `bars` for a trend or a ranking, `number_line` for a move from one value to
  another, `table` for a few values side by side, `donut` for parts of a whole.
- One unit a chart. Say at most two numbers in a sentence, and round them for the ear.
- Every number on a card and in the script is in `sources`.
- Mark the row or cell the voice is at (`mark` on a table), so the viewer looks where the voice is.
- End with the reason or the lesson, with no new number.

### Listicle (`listicle`)

- The hook gives the count and the gain: "5 habits for a calm morning".
- Count down, and keep the best item for number one. Say so at the start, so the viewer stays.
- One sentence an item, about 12 words, one scene each, with a `title` card that carries the number.
- Even pace: scenes of the same length, `pulse` music with `cut_on: beat`, one transition for all.
- Close with a `list` that brings all five back, and a question to the viewer.

### Quote short (`quote-short`)

- Text and music, no voice: `tts: none` with `reveal` captions.
- Three beats: the feeling the viewer has, the line, one small action.
- The line goes on a `quote` card, with a highlight on its key words. Leave a still beat after it.
- A line from a real person needs the name in `label` and a source. An invented line has no name.
- Slow transition, dark background, calm music. Nothing else moves.

### Micro-drama (`micro-drama`)

- Two people, one conflict, one turn. The turn is a fact that changes what the first line meant.
- One line of speech a scene, 5 to 14 words. The speakers take turns; nobody explains the plot.
- The second speaker is a scene `voice` of the same engine. The captions give that speaker a second colour.
- Names come out in the lines ("I had to, Mira"), not on cards.
- Plain cuts and quiet music. End on a short line that answers the first one.
- It needs pictures to work: a clip for each line, or `generate` shots with `characters`.

## 4. Before `plan`

- Does the first scene do what the kind promises (the event, the problem, the claim)?
- Is every card of the recipe still fed by words its scene says?
- News and data: is every invented fact and every placeholder source replaced? The recipes mark them "PLACEHOLDER".
- Did you keep the length? A recipe's words are sized for the free local voice; another voice needs the count
  that `plan` gives.
