---
name: scriptvox-brief
description: Write the brief.json for a scriptvox video - the narration, the hook and the scenes - sized to a target length, in English or Arabic. Use before `scriptvox plan` whenever the user gives a topic, an idea or rough notes instead of a finished script, or asks for a better script, a stronger opening or a shot list. Not for editing an existing video.
---

# scriptvox-brief

You turn a request into one `brief.json`. scriptvox does the rest: it reads the narration, fits it to the length,
lays out captions, renders and checks. So your job is only the words and the choice of picture for each line.

The fields are in `scriptvox schema brief`. The ones you use most: `script` or `scenes`, `lang`, `duration`, `aspect`,
`hook`, `music`, `voice`.

## 1. Fix four facts first

| Fact | Default when the user does not say |
|---|---|
| Goal: teach, sell, announce, tell a story | teach |
| Language | the language of the request |
| Length | 30 s |
| Shape | 9:16 (phone). 16:9 for a screen, a slide or YouTube. 1:1 for a feed post |

Ask the user one question only when the goal or the language cannot be inferred. Never ask about the voice, the
music or the font: pick from `scriptvox voices` and keep the defaults.

## 2. Word budget (measured on scriptvox voices, 2 Nov 2026)

| Language | Speech rate | 15 s | 30 s | 45 s | 60 s |
|---|---|---|---|---|---|
| English (Edge, Kokoro) | 190 to 212 words a minute | 45 | 92 | 140 | 185 |
| Arabic (Edge) | 105 to 110 words a minute | 22 | 48 | 72 | 95 |
| English, open voice (`"tts": "selfhost"`) | about 245 words a minute (4 Nov) | 60 | 120 | 185 | 245 |
| Arabic, open voice (`"tts": "selfhost"`) | about 175 words a minute (3 Nov) | 43 | 87 | 130 | 175 |

- Aim within 10 percent of the number. Too few words makes scriptvox slow the voice down to fill the time, which
  sounds dragged. Too many makes it speed up, then stop. The open voice takes no speaking rate, so it only gets
  the small speed change (up to 16 percent) before it stops: keep its scripts close to the number. `plan` warns
  (gate `fit`) with the words to add or cut when the open voice is likely outside that band, or when another voice
  would need a speaking rate change of about 12 percent or more (the band `make` warns at); `make` warns again if it did. Take the words it gives. Each `[pause N]` costs N seconds of the budget.
- Arabic carries about half the words of English in the same time. Never translate an English script word for word
  into an Arabic video of the same length. Cut ideas, not only words.
- Set `duration` to the target. scriptvox fits small overruns. A large overrun stops with "cut about N words": cut
  that many and plan again.

## 3. Shape the narration

Pick the shape by length. Each beat is one or two sentences.

| Length | Beats |
|---|---|
| 15 s | the claim, one proof, the payoff |
| 30 s | the opening, the problem, the answer in two or three steps, the result |
| 45 to 60 s | the opening, what people believe, why that is wrong, the real reason, one example, the takeaway |

Rules for the words:

1. The first sentence carries the whole reason to watch: a surprise, a cost, a result, or a sharp question. No
   greeting, no "in this video", no title read aloud.
2. Join beats with cause: "but", "so", "because". If two sentences only add facts side by side, drop one.
3. One new idea per beat. A second idea gets its own beat or leaves the script.
4. Write for the ear. Short sentences, common words, numbers a listener can hold ("about half", not "47.3 percent").
5. Every fact must be true and checkable. If you are not sure of a number, leave the number out.
6. End on the takeaway or one clear action. Do not add a new fact in the last sentence.
7. Use `[pause 0.5]` to `[pause 1]` only after the key reveal. Pauses elsewhere waste the budget.

## 4. The hook

`hook` is a headline on screen for the first 1.6 s while the voice already speaks. Many people watch muted, so the
hook must work alone. A card on the first scene waits until the hook is gone (1.6 s).

- 3 to 7 words. Same promise as the first sentence, other words. Never a copy of it.
- Concrete: a number, a place, an outcome. "Water older than the city" works. "Amazing facts" does not.

## 5. Scenes: a picture for every line

Use `scenes` when the user gives media or when the topic has clear pictures. Each scene is one beat with its own
visual. scriptvox cuts on the first word of each scene.

| What the line talks about | Visual |
|---|---|
| a file the user gave | `media`: the path inside the project; `media_from` and `media_to`: the part of a long clip |
| a real thing, and one picture is enough | `open_image`: one or two plain words (`camel`). A free picture found with no key; its own licence goes to the credits file |
| a real place, object, person or action | `stock`: two to four plain nouns, subject first, plus a framing word if it matters (`date palm farm aerial`, `hands pouring coffee close-up`) |
| an idea, a number, a rule | no visual field: the built-in background, so the caption carries it, plus a `card` |

- Which files are taken: a file is opened only as what its first bytes say, whatever its name ends in. Pictures: PNG,
  JPEG, WebP, BMP. Videos: MP4, MOV, MKV, WebM, AVI, MPEG-TS. Sounds: WAV, MP3, FLAC, OGG, Opus, M4A, AAC. Anything
  else stops at exit 2 at `plan`: a GIF, a TIFF, an SVG, a playlist, a URL. `"logo": {"file": "logo.gif"}` is refused,
  so convert it to PNG first. The rule holds for `media`, `logo`, `music`, `narration`, `base_video`, `overlay`,
  `inset` and a `generate` scene's `image`, and for the files of `edit`, `join`, `dub`, `check` and `snapshot`.
- Describe what a camera would see, never a mood. `old irrigation channel stone` finds footage. `ancient wisdom` does
  not.
- Aim for a new visual every 3 to 6 seconds. A scene of 20 words or more needs a split.
- With no Pexels or Pixabay key, stock comes from Wikimedia Commons and the Internet Archive. Those search titles, so give one or two plain words (`camel`, `palm trees`). Each file has its own licence, written in the credits file, and most ask you to credit the author. Do not ask the user for a key in chat.

A folder of the user's clips, and no time to pair them by hand: `"media": "clips", "media_match": true` with a plain `script`. Each sentence gets the file that shows it, from the vision service the user set. `plan` sends nothing: its `vision` stage names the host. Tell the user before `make`, which sends the frames. Read `media_match` in the `make --draft` result (a later `plan` shows it too). It names each scene's file, what the row saw, and the `unused` files. If one is wrong, write `scenes` and set that scene's `media`. Measured 5 Nov: 11 of 11 right in English and Arabic on 6 pictures, where the words did not repeat the pictures ("saffron and cardamom" for a market). A clip over 12 s is looked at in up to 4 parts, and the pick plays only that part (`from` and `to`, or set `media_from` and `media_to` yourself in a scene).

### Caption style

`"captions": "pop"` shows 1 to 3 big words at a time, the usual look of fast vertical shorts. Keep the default
(`karaoke`, two lines) for calm explainers, Arabic with long words, and any video watched with the sound off,
where a whole phrase on screen is easier to read.

The look of the letters: `"caption_box": 0` takes the black box away (0 to 1 is its opacity, default 0.65), `"caption_outline": 4` gives a black outline (0 to 12 px on a 1080 px short side), and `"caption_size"` takes `small`, `large` or a percent such as `"115%"`. With no box, read the `contrast` finding: on a light picture the letters need the outline.

Mark 1 or 2 key words on a page at most: `"emphasis": ["ten minutes", "free"]`. They get the accent colour and a size step. Write them as the script says them; `plan` warns about one it cannot find.

### Cards: motion text over a scene

A scene can carry one `card`. It stays from the scene's first word to the next scene, above the captions.
For two to four cards in one scene, one after the other (a title, then a number, then a list), give `cards`, a list, instead of `card`: `"cards": [{"kind": "stat", "text": "42%", "on": "forty", "hold": 2.5}, {"kind": "list", "items": ["Faster delivery", "Lower prices"], "on": "three"}]`. Each card starts at its `on` word or at `at` seconds from the scene's start, in the order of the list; with no times the cards share the scene evenly. Only one card shows at a time: a card ends 0.2 s before the next, and each needs 0.9 s on screen, or `make` stops with the fix. Not for a title with `behind` or a formula with `then`.
On a `title`, `stat`, `quote` or `list` card, `"mark": [{"part": "forty percent", "on": "forty", "style": "ring"}]` puts an `underline` (default), a `ring` or a `highlight` on words of the card when the voice says the `on` word; at most 4, each `part` once in the card.

| `kind` | Fields | Use it for |
|---|---|---|
| `title` | `text` (at most 60 characters) | a section start; it pops in for 2.5 s |
| `stat` | `text` with the number and unit (`"90%"`, `"$1.2 billion"`), `label` | one number the viewer must keep; it counts up in 0.8 s |
| `lower_third` | `text` (a name), `label` (a role) | the first time a person speaks or is named; it slides in for 4 s |
| `quote` | `text` (at most 160 characters), `label` (the speaker) | a real quotation you can source |
| `compare` | 2 `items`, 2 `values`, `label` (the unit) | a before and after, or this against that |
| `donut` | 2 to 6 `items` and `values`, optional `text` in the middle | parts of a whole, not a trend |
| `timeline` | 2 to 5 `items` (at most 48 characters) | events said in order; each comes on its word |
| `list` | `items`: 1 to 5, each at most 40 characters | steps or reasons; each item appears when the voice says its first longer word, so use the narration's own words in the same order. An item with no word of the narration shows at the scene's start, and `plan` does not warn |
| `bars` | `items` (2 to 6 names, at most 24 characters), `values` (same count), `label` (the unit, as the title) | a trend or a comparison of one unit, for example litres a day by year |
| `table` | `rows`: 2 to 8 rows of 2 to 5 cells, the first is the header; optional `label`; `mark` `[{"part": "<one cell>", "on": "<word>"}]` | a few numbers side by side; a mark on a first-column cell lights its row, on a header cell its column, on any other cell that cell |
| `number_line` | `values` `[from, to]`, `points` (1 to 4 values), optional `items` (their names), `from` (the first marker travels from it), `label` | where a value is on a scale, or a move from one value to another |
| `diagram` | `nodes` (2 to 8 labels, at most 18 characters), `edges` (`["from", "to"]` pairs), optional `label` | how parts feed each other; at most 4 steps in a row and 3 boxes in a step, no circle, no arrow over a step |
| `steps` | `items`: 2 to 5, at most 40 characters | a process in order; the step the voice names is lit, so use the narration's own words |
| `progress`, `gauge` | `values` `[value]` out of 100 or `[value, top]`, optional `text` (in place of the percent), `label` | one share of a whole, on a bar or on a ring |

`list` and `bars` also take `ease` (`out`, `spring`, `linear`) and `stagger` (seconds between items, 0.05 to 1.5);
see the scriptvox-motion skill.

- Keep the card shorter than the line: the card is the takeaway, the voice says the rest.
- A `stat` counts up in the digits you write: `"90%"` or `"٩٠٪"`. Use the same digit style as the script.
- Put a `lower_third` on footage of that person. Over the plain background a vision check (`scriptvox judge`) scored
  it 1 of 5 on 3 Nov: a name with no face looks unfinished.
- In Arabic the lower third and the list move to the right side by themselves.
- A `title` can stand behind the subject with `"behind": true` (see the `scriptvox-motion` skill): one short word,
  on a shot whose subject has a top edge or thin parts.

When the user gives a logo, add `"logo": {"file": "logo.png"}` at the top level: a PNG with a transparent
background, top right, 16 percent of the width (`width` 0.05 to 0.4, `corner` `left`, `opacity`). It stays the same
place in Arabic. Give the brand colour as `"accent": "#RRGGBB"`: it colours the spoken word and the bars. A dark
colour is refused (contrast on the black caption box under 3); use a lighter shade of the brand colour. For an end
card add `"outro": {"text": "the web address or the call to action"}`: 2.5 s after the narration, the logo large
in the middle and the text under it. Keep `seconds` at 2.5 or less (the text shows for 2.5 s).

A name the voice reads wrong: `"say": {"ACME": "A C M E"}`. The voice gets the spoken form, and the captions keep the written one. A brand file may hold it.

For a series (several videos with one look), put these fields once in a brand file, for example `kit/brand.json`
with `accent`, `logo`, `look`, `voice`, `outro`, `aspect`, and give each brief `"brand": "kit/brand.json"`. A field
in the brief wins over the brand. A brand file holds only look and sound fields (`lang`, `tts`, `voice`, `aspect`, `fps`, `captions`, `caption_at`,
`caption_size`, `caption_outline`, `caption_box`, `font`, `style`, `background`, `music_level`, `transition`, `look`, `sfx`, `music`, `music_prompt`, `logo`, `accent`, `outro`, `say`); the script and
scenes stay in each brief. The logo path in the brand file is relative to the brand file.

## 6. Arabic

For any Arabic video, also load the `scriptvox-arabic` skill: budget, numbers, names, mixed text and voice choice.

## 7. Check before `scriptvox plan`

Answer each with yes, or fix the brief:

1. Does the first sentence alone make a stranger want the second?
2. Is the word count at or under the budget for the language and length?
3. Does every beat lead to the next with "but", "so" or "because"?
4. Does every scene name something a camera can show, or rely on the background on purpose?
5. Is every fact one you are sure of?
6. Does the hook work with the sound off, in 7 words or fewer?

Then run `scriptvox plan --brief brief.json --json` and continue with the `scriptvox` skill.

Three cheap stops before a long or paid run. Each has its own plan, and a free one runs at once:

```
scriptvox voices --preview en-US-AvaNeural --brief brief.json --json   # hear the voice: one short audio file, no video
scriptvox make --brief brief.json --until voice --json                 # the voice, the fit and the caption files only
scriptvox make --brief brief.json --storyboard --json                  # one still per `generate` scene, on one sheet
```

- `voices --preview <voice id>` reads the first sentence of the brief (or `--text "..."`, at most 200 characters) and
  writes `artifacts.audio`. Give the file to the user: you cannot hear it. The same line costs nothing again in `make`.
- `--until voice` stops with `voice_rate_pct`, `duration_s` and the caption files in `artifacts`, and no video. Fix the
  words, then `scriptvox resume <run_id> --wait --json` makes the video with the same voice (`voice_reused`).
- `--storyboard` is for briefs with `generate` scenes: see `scriptvox-prompts`.

## Examples

English, 30 s, 9:16, 89 words:

```json
{
  "lang": "en",
  "duration": 30,
  "hook": "Water older than the city",
  "music": "calm",
  "scenes": [
    {"script": "Most people think a desert has no water. That is not true. There is plenty, but it is hidden.", "stock": "desert dunes aerial"},
    {"script": "Under the sand, old rain sits in deep layers of rock, some of it for thousands of years.", "stock": "rock layers close-up"},
    {"script": "Farmers found it long ago, so they dug narrow channels to carry it, mile after mile, to their palms.", "stock": "falaj water channel"},
    {"script": "Some still work today, with no pumps and no power, only a gentle slope.", "stock": "water stream stone channel"},
    {"script": "So the next green oasis you see may be drinking water that is older than the city around it."}
  ]
}
```

Arabic, 30 s, 9:16, 46 words:

```json
{
  "lang": "ar",
  "duration": 30,
  "voice": "ar-AE-HamdanNeural",
  "hook": "ماء أقدم من المدينة",
  "scenes": [
    {"script": "يظن كثيرون أن الصحراء بلا ماء. وهذا غير صحيح، فالماء موجود لكنه مخفي.", "stock": "desert dunes aerial"},
    {"script": "تحت الرمال مياه أمطار قديمة في طبقات الصخر منذ آلاف السنين.", "stock": "rock layers close-up"},
    {"script": "لذلك حفر المزارعون أفلاجًا تحمل الماء إلى النخيل بلا مضخات ولا كهرباء.", "stock": "falaj water channel"},
    {"script": "فالواحة التي تراها قد تشرب ماءً أقدم من المدينة حولها."}
  ]
}
```

## No voice

`"tts": "none"` makes a video with text on screen and no speech: the script runs as captions at reading pace (2.5 words a second, Arabic 1.8), with cards and music if the brief has them. `duration` fits the pace. `"captions": "reveal"` suits it. Gates for silence and spoken words are skipped.

## Two speakers

A dialogue is scenes with `voice` on the second speaker: `{"scenes": [{"script": "..."}, {"script": "...", "voice": "en-US-AvaNeural"}]}`.
Every speaker comes from one engine (`scriptvox voices`); the open `selfhost` voice has one speaker and stops at `plan`. ScriptVox adds a 0.25 s breath where the speaker changes, and the second speaker's words light up in light blue instead of the accent. A third voice shares that blue, so keep a dialogue to two.

## Many versions of one video

For the same video per name, district or language, write one brief with `{{key}}` placeholders and a rows file,
then `scriptvox batch --brief b.json --rows rows.json --json`: one video per row, each checked like `make`. Plan one row
first with `scriptvox plan` on a filled copy if any stage costs money. `scriptvox batch --brief b.json --describe --json`
makes nothing and shows the keys and a sample row. `--rows` also takes a CSV file whose first line names the columns.

The user recorded the narration: `"narration": "me.wav"` with the same words in `script`. No voice is made, and the file sets the length. Do not give `duration`.
