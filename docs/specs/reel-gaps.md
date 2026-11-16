# Gaps of the reel test (14 Nov 2026)

A user test on 14 Nov 2026 ("copy one short reel: a cat video with a song and no words") failed. `no-words.md` has the first three gaps (no words, the pause at the end, the plan that said ok). This spec has the other six.

## 4. `cut_on: "beat"` with music from an address

Before: exit 2, "cut_on needs the music before the voice is made, and music from an address ... comes later".

Now: when the brief has `cut_on` and `music` is an https address, `make` gets the music first, before the voice. It is the same download as before (`fetch.md`): `setup fetch` must be on, one download for an address and a cut, the store keeps it, one line in `credits.txt`. A free download needs no approval. A download that fails stops the run before the voice is made, so nothing is paid.

```json
{"music": "https://example.org/song.m4a", "music_from": 0, "music_to": 12, "cut_on": "beat", "scenes": [...]}
```

- The plan line of the music says "before the voice: the cuts follow its beat".
- `music_search` and `music_prompt` with `cut_on` are still exit 2. The fix line: get the file first and give it as `music`, or use a built-in bed, a file or an https address.
- Not run against a real site: the test serves a click file from a loopback server (`tests/fetch.rs`).

## 5. Types in a brief

- `music_from` and `music_to` take a number (`12.5`) or text (`"12.5"`, `"0:12"`). Before, a number was a type error. The schema shows both. A number and its text give the same plan id.
- A type error names its field. Before: `invalid type: integer 0, expected a string`. Now: `music_from: ...`, `duration: invalid type: string "12", expected f64`, `scenes[1].media_from: ...`. An unknown field keeps its message with the closest name.

## 6. A site that asks to sign in

`SVX_FETCH_BOT_CHECK`: the site asked this network to sign in. ScriptVox does not sign in, uses no cookies of a browser, and has no way around the check of a site. This does not change.

- The fix line now has the route that always works: "save the file yourself (a way that the site allows) and give the local file: `scriptvox analyze <file>`, `scriptvox make --music <file>` or `--media <folder>`". `fetch --help` says the same.
- After such an answer, a plan with music from the same site says for 30 minutes that the download will likely fail, with the same route. The record is one small file in the data folder (`fetch-sign-in.txt`: the host name only). The plan sends nothing.

## 7. Rows that take a start picture

A brief with `character` makes each generated clip start from a picture. A scene with `generate.image` or `first_frame` does too. Only a video row with image to video can do that.

- `scriptvox providers` (and its JSON): each video row in `generators` has `takes_start_picture`, true or false. The help of `providers` says what it is for.
- The plan error for a row that cannot: `scene 1: kling-3.0-pro does not fit: the scene starts from a picture (the brief's character, or the scene's image or first_frame), and this row takes no start picture`. The fix line names the rows that fit the scene.

## 8. Every service call is a stage, and a service that does not answer is not chosen

Before: the plan had no stage for the picture of a `character` and none for the first frame of a clip. A free row on the user's own GPU service was chosen when it had a key, also when the service was off.

Now, in `plan`, `plan --storyboard` and `make`:
- Stage `character`: one line for each character whose picture must be made, with its image row and price. This price is now in the plan's cost.
- Stage `first_frame`: one line for each clip that starts from a made picture, with its image row. Its price stays in the line of the scene.
- The rows are picked as before. What the content store has (a clip, a still, a character's picture) comes from there on the row that made it, and its service is not asked: a `resume`, a `replay` or a new plan with the service off stays at "already made".
- For what must still be made, a row on the user's own GPU service (`gateway` selfhost) is asked one time if it answers: one TCP connect to its host, 1.5 s at most after the name lookup. Nothing of the brief is sent. Its plan line ends "your own service answers now". Not asked with `--local-only` and for a run from the cache only.
- When it does not answer, the row is not chosen. The next row that fits is used, and the plan has the finding `services` (warn) that names the row that was left out. When no other row fits, the plan stops (exit 10, needs input) and the message names the row and says: start the service, `scriptvox providers test <row>` checks it.
- A row that the brief names as `model` is never replaced: when its service does not answer, the plan stops with the same message.
- Not asked: paid rows (their service is not the user's own), local rows, and any row when a proxy variable is set (a direct connect says nothing then).

Limits: the check is the TCP port only, not the health of the service. The name lookup before it has the time limit of the system. When a clip must be made again on another row and its first frame was in the store on the row that does not answer, a new first frame is made. A voice row on the same service (`tts: selfhost`) is not checked here.

## 9. No terms note in the flow

Owner rule: the user sees no licence or commercial-use note in the terminal, the plan or the video. Licence data stays in `metadata.json`, `credits.txt` and the listing commands (`providers`, `voices`, `tools`, `licenses`).

Removed from the output of `plan` and `make`:
- Music from an address: "you must have the right to use it; its source goes to credits.txt and export".
- Open search: "each file keeps its own licence in credits.txt and export".
- Stock: "credits file written, ship it where the terms ask; clips are not redistributed".
- The error of scene `sound`: "(their sound has its own licence question)".

Revised by the owner on 14 Nov 2026 (`user-music.md`): the user can load any song, and the choice and the responsibility are the user's. So there is exactly one line of information, one time: the notice of `setup fetch`, now short and neutral ("fetch gets a file from any address that you give. The choice and the responsibility are yours: ScriptVox does not check the rights to a file."). It was printed two times in a JSON run (stderr and the `notice` field). Now a JSON run has the field only, and a person at a terminal gets the line. The help texts no longer say "you must have the right to use it"; they say that any song can be used.

Left as they are, on purpose:
- The notice of `setup voice-local` (GPL parts): a one-time setup notice about the software, not about a video.
- Terms as data in JSON (`media_sources`, `music.source.terms`, the receipt) and in `credits.txt`, `metadata.json`.
- The listing commands (`providers`, `voices`, `tools`, `licenses`).

## Checked

`tests/reel_gaps.rs` (types, no terms note, start picture, stages and the service check with a loopback port) and `tests/fetch.rs` (`make_cuts_on_the_beat_of_music_from_an_address`, `a_plan_says_that_the_site_asked_to_sign_in_a_short_time_ago`). No network.

Real checks on 14 Nov 2026, `plan` only, on copies of the briefs of the failed test (nothing was approved or made):
- Three clips with a `character` on `kling-2.6-pro`: the plan has the stage `character` and three `first_frame` stages on the free image row of the owner's own GPU service, each with "your own service answers now", then the three clips at $0.35. Before, the plan had the clips only.
- The same brief with `kling-3.0-pro`: exit 2, and the fix line names the 7 rows of this install that take a start picture.
- `plan --storyboard`: three stills on the same free row, each line with "your own service answers now".
- Music from an address with `cut_on: "beat"`: the plan is ok and the music line says "before the voice". Not made: no download from a real site in this work.
