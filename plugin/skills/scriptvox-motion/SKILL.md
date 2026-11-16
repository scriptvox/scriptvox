---
name: scriptvox-motion
description: Plan the movement in a scriptvox video - which scenes get a title, a counting stat, a lower third or a list card, how generated clips move, where the still beat goes - and check it on real frames. Use when a brief needs on-screen text beyond captions, when the user asks for motion graphics, kinetic text, a title card, a big number, a name strap or bullet points, says the video feels static or like a slideshow, or asks for transitions.
---

# scriptvox-motion

Motion is there to carry the story, not to fill time. Every second, one thing on screen must be doing work: a new
fact arriving as the voice says it, a camera move with a goal, a number landing. A picture that only drifts or
pulses tells the viewer to wait.

## 1. What scriptvox moves for you

| Element | How it moves | Set it with |
|---|---|---|
| Hook | big headline pops in, gone at 1.6 s | `hook` |
| Scene change | on the scene's first word: `fade` (0.25 s, default), `slide`, `cut`, `wipe`, `push`, `cover`, `dissolve`, `blur`, `circle`, `dip` (through black), `flash` (through white), `pixel`, `radial`, `wind`, `zoom`, `reveal`, `iris`, `split`, `slice`, `rise`. Sideways moves follow the reading direction, so Arabic goes the other way. One name for the whole video; one scene may differ with its own `transition` (a topic change, a reveal) | `transition`, `transition` on a scene |
| A still photo | slow push-in, 100 to 110 percent | `media` or `stock` with a photo |
| A generated clip | whatever the prompt asks for, but Wan barely moves its camera (measured): add a scene `camera` for a sure move | `generate.prompt` |
| Frame rate | 30 by default; `24` for a film feel and when most scenes are generated clips (they are 24 fps, so no blending); `60` only for fast screen content | `fps` |
| A scene camera move | `push_in`, `pull_out`, `pan_left`, `pan_right`: about 12 percent over the scene, smooth, on any video scene (generated, stock, own). On a scene with no picture it does nothing, and `plan` does not warn (11 Nov: the frames were the same with and without it) | `camera` on a scene |
| Captions | the word being said lights up; with `"captions": "pop"` 1 to 3 big words at a time, each page pops in; with `"reveal"` each word appears on its spoken time, up to 3 lines, no box (use over a calm picture; it can move with `caption_at`) | `captions` (`karaoke` default, `pop`, `reveal`); `emphasis` for key words |
| Card | see section 3 | `card` on a scene |
| The plain background | a slow gradient behind text and cards when a scene has no picture: `dusk` (default), `night`, `ocean`, `forest`, `sand`, `slate`, or your colours. Use a dark one when the accent is yellow | `background` |
| A picture in picture | a clip or photo in a corner of one scene, with a thin white edge; its own sound is not used | `inset` on a scene: `file`, `corner` (`top_right`, `top_left`, `bottom_right`, `bottom_left`), `width` 0.2 to 0.6 |
| A layer of your own | a transparent WebM, MOV (ProRes 4444) or PNG over one scene, for example a Manim render. It starts with the scene, the captions stay on top, and its sound is not used. A file with no alpha is refused | `overlay` on a scene |
| Colour look | the whole picture, not the text: `warm`, `cool`, `film` (softer colour, more contrast, a vignette), `punchy`, `mono`, or a `.cube` LUT file from the user's colourist. One look per video. Pick it from the mood of the script, not by default | `look` (`natural` default) |

ScriptVox has no spin transition, no custom eases, no per-letter animated type, and no
animated 3D or map cards. When the user asks for one of these, say so and offer the nearest card.
Do not build it with raw FFmpeg.

## 2. Plan the motion for the whole video first

Write one line per scene before the brief: what is on screen, what moves, and what the viewer must notice first.
Then check these rules on the list:

1. One idea per scene, 3 to 6 seconds. A scene with two things to show is two scenes.
2. One moving layer with meaning per scene: the clip's own action, the push-in on a still, or one card. A scene
   can hold only one card.
3. Hold facts back and pay them off on the word: a list card shows each item when the voice says it.
4. Before a payoff line, put a short pause in the script (`[pause 0.5]`). The still beat makes the result land.
5. Do not give every scene a card. Plain footage with captions is the rest between cards.
6. A card stays from its scene's start to its end (a title 2.5 s). Put a number card in the scene whose words say
   that number: in one long scene "3.8 million" sat under the 2005 line (4 Nov 2026 eval). Split the script into
   scenes so each card meets its own line. A `stat` counts up for 0.8 s, so a frame in that time shows a smaller
   number.

## 3. Cards

| `kind` | On screen | Rules |
|---|---|---|
| `title` | headline in the middle, pops in, 2.5 s | only at a change of part; at most one per 15 to 20 s |
| `stat` | a number counts up in 0.8 s, label under it | the one number to remember; at most one per 20 s; it counts in the digits you write (`90%` or `٩٠٪`). `from` is the number the count starts at (default 0); a `from` above the number counts down |
| `lower_third` | name and role slide in at the side, 4 s | only over footage of that person; over a plain background the judge scored it 1 of 5 |
| `list` | 1 to 5 points, each appears on its spoken word | write each item with the narration's own words, in the same order |
| `line` | a trend: title from `label`, 3 to 8 points (usually years) joined by a line that draws itself in the reading direction, each value appears as the line reaches it | `items` (point names, at most 12 characters) and `values` (any sign); one series only; in Arabic write the years in Arabic-Indic digits and the values follow |
| `formula` | one equation, typeset by scriptvox (no LaTeX), written left to right in about 1 s on a dark panel; optional `label` under it says what the symbols mean | `text` in a small TeX subset: `^` and `_` with `{}`, `\frac{a}{b}`, `\sqrt{x}`, `\text{words}`, `\pi`, `\theta` and other Greek names, `\times`, `\pm`, `\le`, `\ne`, `\approx`, `\to`, `\infty`, `\sum`, `\int`, `\sin`, `\log`; at most 120 characters. Not supported: matrices, `\left`, alignment, colour. `plan` names an unknown command. To show a step of the working, add `then` (a second formula) and `on` (a word of the narration): the symbols both share glide into place when the voice says that word, the rest fades. Or `mark`: up to 4 pieces take the accent colour on a spoken word, `[{"part": "mc^2", "on": "energy"}]`; not with `then` |
| `code` | a short snippet in a dark editor panel, coloured (keywords, strings, numbers, comments, calls), each line typed in after the one before; optional `label` as the file name on a top strip | `text` with lines split by `\n`, at most 12 lines of 44 characters; indentation kept. Any language: the colouring knows Python, JavaScript, Rust, shell and upper-case SQL words. Keep it to the lines the narration talks about |
| `diff` | a code change in the same panel: a line that starts with `+` gets a green bar (added), one that starts with `-` a red bar (removed), the others are context. Limits as `code` (12 lines, 44 characters) | `{"kind": "diff", "label": "area.py", "text": " def area(r):\n-    return 3.14 * r * r\n+    return math.pi * r ** 2"}` |
| `terminal` | a terminal in the same dark panel: a line that starts with `$ ` is a command and is typed in, the other lines are its output and come whole; `label` is the title (`bash`). Limits as `code`. No scrolling | `{"kind": "terminal", "label": "bash", "text": "$ cargo test\ntest result: ok. 3 passed"}` |
| `plot` | a function of `x` drawn on axes: the function written above, the curve sweeps in left to right in about 1.6 s, the x and y range at the edges; in a wide frame the title and function stand left of the plot | `text` is the function (`x^2 - 2`, `2sin(x)`, `e^(-x^2)`, `sqrt(x)`; names: `pi`, `e`, sin, cos, tan, sqrt, exp, ln, log, abs); `values` is the x range `[from, to]`, default `[-5, 5]`. Up to 3 curves: separate the functions with ";" (`x; x^2 - 4`); each has its own colour and the function line above is the legend in the same colours. Up to 4 points where the first curve crosses zero get a dot and their x value (more get none). `"grid": true` draws light grid lines, `"trace": true` runs a dot along the first curve with its x and y, and `y` is the range `[min, max]`. No other marks, so the narration names any other point. `label` is a title in words ("A parabola"), not the function again: a label equal to the function is left out. Pick a range that shows what the narration says (both zero crossings, one full wave) |
| `bars` | a bar chart: title from `label`, 2 to 6 named bars grow in turn, values at the ends | `items` (names) and `values` (numbers, same count); compare values of one unit only; the narration says the trend, the chart shows it |
| `quote` | a saying, big, in quote marks that suit the language; `label` names the speaker | `text` at most 160 characters; only a real quotation you can source |
| `compare` | two panels side by side, each number counts up, the second is lit | 2 `items`, 2 `values`, the unit in `label`; a before and after, or this against that |
| `donut` | a ring draws itself part by part, a legend gives each share (whole percents that add to 100) | 2 to 6 `items` and `values` (parts of a whole, not a trend); a short word in the middle from `text` |
| `timeline` | events down a line, each comes on its spoken word | 2 to 5 `items`, at most 48 characters; say them in order in the narration |

A `title` with `"behind": true` stands behind the subject: three times the caption size, no box, and the subject
(cut out on every frame by the `birefnet-self` row) passes in front of it. ScriptVox moves the title to the
height where the subject hides about a fifth of it. Use it on a shot whose subject has a top edge or thin parts
(a mast, a head, tower tops). Read the `behind` finding: on 3 Nov "CITY" over two towers hid 24 percent and read
well; "ABU DHABI" wrapped to two lines and the second line sank behind the towers (59 percent, warn above 40). Use
one short word; if the finding warns, shorten the title or drop `behind`. Arabic gets a smaller share (aim 8, warn above 25): the
letters join on the baseline, and an awning across "الخليج" made it unreadable at 21 percent.

Keep card text shorter than the line. The voice says the sentence; the card keeps the takeaway. In Arabic the lower
third and the list move to the right side by themselves.

Any card can wait for its moment: `on` is a word of the scene's script (written as the script writes it), and the
card comes in when the voice says it, so a number lands on its own word. `hold` is how many seconds it stays (default:
to the scene's end). `plan` refuses a word the scene does not say. A formula with `then` keeps `on` for its change.

`list` and `bars` take two motion fields, so cards in one video do not all move the same way:

- `ease`: `out` (fast, then slows as it lands: calm), `spring` (goes about 8 percent past its size and settles:
  lively, for a hook or a short punchy list) or `linear`. Default: list items fade in, bars grow at one speed.
- `stagger`: seconds from one item to the next, 0.05 to 1.5. A list's first item still waits for its spoken word;
  the rest follow at the gap, so use it when the narration names the items quickly ("trees, water, and shade").
  Default: each list item waits for its own word; bars come 0.15 s apart.

Use one style per video, and `spring` on one or two cards at most. Measured 4 Nov: a spring list at 0.35 s grows
from 1.83 s, is past its size at 2.03 s and is still by 2.17 s; the landed frame is the same as without motion.

```json
{"script": "Almost all of the drinking water here comes from the sea.",
 "stock": "desalination plant coast aerial",
 "card": {"kind": "stat", "text": "90%", "label": "of drinking water comes from the sea"}}
```

## 4. Motion inside generated clips

The clip's own motion comes from the prompt. Name one camera move and one action of the subject, both small: "slow
push in, the dhow drifts forward". A big move asks too much of a 5 s clip: on 3 Nov, a subject asked to cross the
whole frame gave a clip that was 74 percent still. If the clip check reports a still clip, ask for less movement,
not more.

For footage wider than the frame, `scriptvox edit <clip> --reframe 9:16 --track` keeps a moving subject in the
crop. That is a separate step after `make`; see the scriptvox-edit skill. For a subject on a clean background, or a
transparent cut-out for an editor, use `scriptvox edit <clip> --background blur --plan` or `--alpha`: same skill,
section 3d.

## 5. Check the motion on real frames

1. `scriptvox snapshot <run_id> --cards --json`: one frame per card, taken after it has landed (all list items in,
   the formula written, the code typed). `frames[].scene` and `frames[].card` say which is which. For the motion
   itself add `--at` with a time early in the card.
2. Look for: text cut off at an edge, a card touching the caption box, a list item that shows before its word, two
   things fighting for the eye.
3. `scriptvox judge <run_id> --json`. A frame with `text_ok` false is a must fix.
4. Fix in the brief (shorter card text, move the card to another scene, split the scene), then
   `scriptvox resume <run_id> --brief brief.json --json`.

## 6. Arabic

Card text in Arabic words; use the same digit style as the captions (a stat counts in either). On 3 Nov at 9:16, a 5-word Arabic stat label fit on one line
and a 7-word English label took two (wrapped evenly). Check one frame of each card scene by eye: joined letters
and the right side.
