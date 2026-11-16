---
name: scriptvox
description: Make narrated, captioned short videos (MP4) from a script with the scriptvox CLI, in English, Arabic, Spanish, French, German, Portuguese or Indonesian, from your own clips, stock footage or a built-in background. Use when the user asks for a video, a short, a reel, a voice-over video or captions. Do not use for video editing of an existing timeline.
---
<!-- scriptvox-skill-version: 0.1.0 -->

# scriptvox

You write the script. scriptvox makes the video. Every command prints one JSON envelope with `--json`. Read `status`, `error.fix`, `needs_input` and `next`.

## Steps

1. Run `scriptvox --version --json`. If it fails, tell the user to install scriptvox, and stop.
2. Run `scriptvox doctor --json`. If exit 1, run `scriptvox doctor --fix --json`. If exit 10, show `needs_input[].human_action` to the user.
3. Write `brief.json` with the full narration (fields below). You are the script writer. For a script that states numbers or dates, research first, then write: collect the facts with their sources, write the script from those facts only, and put each fact in `sources`. With your own web search you do this yourself. With none, and only when the user has set up a text row: `scriptvox research "<topic>" --json`, then `scriptvox write --from research.json --json` (with no text row both stop with exit 10 and the one command for the user to run).
4. Run `scriptvox plan --brief brief.json --json`. With `sources` in the brief, read the `claims` finding: it names each number or date of the script that is in no source (`--strict` makes that a failure). If `needs_input` has an item of kind `approval`, or `metrics.cost_usd` is above 0: show the price to the person and ask. Add `--approve <plan_id>` only after the person says yes. An agent must not approve, or raise `--max-spend`, by its own decision. For other `needs_input` items, take a free alternative from `alternatives`.
5. Run `scriptvox make --brief brief.json --detach --json`, then `scriptvox wait <run_id> --json`. Always `--detach`: a make takes minutes, and without it the run stops when your shell limit stops the command. `wait` returns after 100 s; exit 9 is not a failure, the run is still going: run the command in `error.fix` again (with a 10-minute shell limit, `--timeout 540`). Never use sleep. If you are not sure a run is going, `scriptvox status <run_id> --json` says `alive` (true or false), `stage` and `idle_s`; when `alive` is false and there is no video, `scriptvox resume <run_id> --json` goes on from what is finished. Never start a second `make` for the same video.
6. Run `scriptvox snapshot <run_id> --sheet 9 --json` and Read the PNG at `artifacts.image`. Check the frames and the captions. With scene cards, also `--cards` (one frame per card, after it has landed).
7. To fix, edit `brief.json` and run `scriptvox resume <run_id> --brief brief.json --json`, then `wait` on the new `run_id`. The new video is next to the old one (`name.1.mp4`; the answer's `out` field says where). Do at most 2 rounds. For a video over about 30 s, add `--draft` to the first `make` (half size, 15 fps, about 8 times faster on an 84 s video), and when the frames look right run `scriptvox resume <run_id> --final --json`. A draft has no `words` gate: the final runs it. To check the voice and the fit before any picture, run `scriptvox make --brief brief.json --until voice --json`, then `scriptvox resume <run_id> --json`. With `generate` scenes, run `scriptvox make --brief brief.json --storyboard --json` first: one still per scene on one sheet, and no clip.
8. Report `artifacts.video` (in `scriptvox-out/`), `metrics.duration_s`, the cost, and the voice terms from the result's `explain` field (the `scriptvox explain` command shows only the recipe).

## Rules

- Never ask the user for a key in chat. On `kind:"secret"`, show the `human_action` (the user runs `scriptvox keys set <provider>` in a terminal).
- Never print a key. Never poll with sleep: use `wait`.
- A file is opened only as what its first bytes say: PNG, JPEG, WebP, BMP, MP4, MOV, MKV, WebM, AVI, MPEG-TS, WAV, MP3, FLAC, OGG, M4A, AAC. A GIF, a TIFF, a playlist or a URL as `media`, `logo`, `narration` or `base_video` stops at exit 2. Only `music` takes an https address (after `scriptvox setup fetch`, which the person runs).
- Free first: Edge TTS, own media, the built-in background. Paid stages always need `--approve`. The approval belongs to the person.
- Terms: every row is allowed and the user decides. The terms are data in `providers`, `voices`, `credits.txt` and `metadata.json` (`commercial_ok`). Give them when the person asks.

## Brief fields

- `accent`: The brand colour as `#RRGGBB`: the highlight of the spoken word and the bars of a chart (default yellow).
- `aspect`: 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9. Generated clips are asked as 9:16 (tall shapes) or 16:9 (wide shapes) and cropped to the frame.
- `background`: The plain background, when a scene has no picture: `dusk` (default, purple to yellow), `night`, `ocean`, `forest`, `sand`, `slate`, or 2 or 3 of your colours as `#0b1020,#1b2a4a`. It moves slowly.
- `caption_at`: Where captions sit: `bottom` (default), `middle` (lower middle, a common short-form place) or `top`. `middle` and `top` need a video with no `hook` and no scene `card`, which use that space.
- `caption_size`: How big the captions are: `small` (four fifths), `normal` (default), `large` (five quarters), or a percent of normal from `"50%"` to `"200%"`, for example `"115%"`. The lines are measured again at the new size, so they still fit the frame. The hook, the cards and `captions_second` keep their size. A bare number is read as a percent: 115 is `"115%"`.
- `captions`: Caption style: `karaoke` (default: up to two lines, the spoken word lights up) , `pop` (1 to 3 big words at a time, each page pops in) or `reveal` (up to 3 lines, no box, each word appears when it is said).
- `duration`: Target length in seconds; the narration is fitted before render. A brief with `duration` and no script, script_file, scenes or lines is a video with no words of that length (picture and `music` only).
- `hook`: A headline shown big for the first 1.6 seconds (the hook).
- `lang`: en, ar, es, fr, de, pt or id (`scriptvox voices` lists them). Default: detected from the script.
- `logo`: A logo on every frame: a PNG (transparent background) in a top corner, under the captions.
- `media`: Folder of your own clips and stills, inside the project, relative to the brief.
- `music`: calm, pulse, a music file in the project, an https address, or none. Any song or media file can be used (mp3, m4a, wav, or a video for its sound): the choice and the responsibility are yours. A file shorter than the video starts again, and the music fades out in the last second. An address: `make` downloads its sound one time through `fetch` (run `scriptvox setup fetch` first), and `plan` lists the download.
- `music_level`: How far any bed sits under the voice: `quiet`, `normal` (default) or `loud`.
- `narration`: Your own recorded narration (wav, mp3, m4a or flac) instead of a voice: no TTS. Keep `script` as the words said: captions follow them, exactly with a speech to text row, else at an even pace. The file sets the length, so no `duration`.
- `outro`: An end card after the narration: the picture blurs and darkens, the logo (if any) shows large in the middle, and `text` (a call to action, at most 60 characters) under it. The music runs on.
- `say`: Written forms the voice says another way, `{"ACME": "A C M E"}`, or with IPA between slashes and then the respelled form, `{"Doha": "/ˈdoʊhə/ doh-ha"}` (a voice with `takes_ipa` says the IPA). The captions keep the written form. A brand file may hold this.
- `scenes`: Scenes in order: each with its narration and its own clip, photo or stock term. Cuts fall on scene boundaries. Use this, `script` or `script_file`. A scene with `seconds` and no `script` has no words: its picture shows for that time, with no voice and no caption (a reel with music only: every scene with `seconds`).
- `script`: The narration. Pause tags like `[pause 1.5]` are allowed (0.1 to 10 s each; a pause at the end holds the picture and the music), and delivery cues like `[whisper]` for a voice whose row takes them (`takes_tags` in `scriptvox voices`). Use this or `script_file`.
- `script_file`: A text file with the narration, relative to the brief.
- `sfx`: Built-in sound effects: a whoosh into each scene cut, a pop when a card appears, ticks while a stat counts.
- `sources`: The facts the script states and where each comes from (at most 20). ScriptVox fetches nothing and does not judge them: `export` writes them to `metadata.json` and to the credits, and they travel in the MP4's recipe, so a viewer or an editor can check a number.
- `stock`: Stock search terms, one per shot, or `auto`.
- `transition`: The transition between shots: `fade` (default, 0.25 s), `slide`, `cut`, `wipe`, `push`, `cover`, `dissolve`, `blur`, `circle`, `dip` (through black), `flash` (through white), `pixel`, `radial`, `wind`, `zoom`, `reveal`, `iris`, `split`, `slice` or `rise`. Sideways moves follow the reading direction. A scene's own `transition` wins for the cut into that scene.
- `tts`: TTS engine: edge, elevenlabs, kokoro, kokoro-rust, selfhost (an open voice on your own GPU service), or none: no voice, the script runs as captions at reading pace over the picture and any music. `reveal` captions suit it.
- `voice`: Voice id from `scriptvox voices`.
- More fields, each with one line in the skill `scriptvox-reference` (read it when you need one; `scriptvox schema brief --json` has every field of a scene and of a card): `ambience`, `base_sound`, `base_video`, `brand`, `caption_box`, `caption_outline`, `captions_second`, `card_style`, `character`, `characters`, `cut_on`, `cutaways`, `emphasis`, `font`, `fps`, `lines`, `look`, `loop`, `media_match`, `music_from`, `music_prompt`, `music_search`, `music_to`, `seed`, `size`, `smooth`, `sources_card`, `style`, `upscale`, `voice_rate`, `waveform`.

Example: `{"script": "Dates are an old food of the Gulf. [pause 1] They give fast energy.", "lang": "en", "duration": 20, "media": "clips", "music": "calm"}`

Scene cards: a scene can carry a `card` (motion text over it): `title`, `stat`, `lower_third`, `list`, `bars`, `line`, `formula`, `code`, `diff`, `terminal`, `plot`, `quote`, `compare`, `donut`, `timeline`, `table`, `number_line`, `diagram`, `steps`, `progress` or `gauge`, for example `{"script": "...", "card": {"kind": "list", "items": ["Shade", "Cool air"]}}`. Or `cards`, 1 to 4 cards that show one after the other in the scene, each with `on` (a spoken word) or `at` (seconds from the scene's start) and `hold`. The fields of each kind: `scriptvox schema brief`, or the scriptvox-brief skill.
Pause tags: `[pause 1.5]`, `(silence: 800ms)`, Arabic `[توقف ١٫٥ ثانية]`. Arabic scripts get Arabic voices and correct right-to-left captions. Spanish, French, German, Portuguese and Indonesian scripts get their own voices (`lang` es, fr, de, pt, id).

## Commands

- `scriptvox make`: Make a narrated, captioned video from a script. `scriptvox plan ...` is `make --plan ...`. It takes minutes: with a time limit on your shell, `make --detach`, then `wait`
- `scriptvox wait`: Wait for a run to finish and print its result. Exit 9 is not a failure: the run is still going, run `wait` again
- `scriptvox cancel`: Stop a running run
- `scriptvox explain`: Show the recipe embedded in a ScriptVox MP4
- `scriptvox resume`: Make a child run from a finished or failed run, with a new brief or flags
- `scriptvox templates`: List the video templates
- `scriptvox skills`: The skills for coding agents (Claude Code, Codex, Cursor, Gemini CLI, opencode, Pi, Hermes): install them, or print the main one
- `scriptvox schema`: Print a JSON Schema: brief, result or event; or `openapi`, the OpenAPI 3.1 document of the planned HTTP server
- `scriptvox check`: Run the quality gates on a run or an MP4 (black, slideshow, silence, loudness, A/V drift)
- `scriptvox snapshot`: Write one PNG an agent can read: frames at `--at` times, or a `--sheet N` contact sheet
- `scriptvox providers`: List the HTTP providers with their output terms and key status. `providers test <row>` runs the row's smallest call. Each video row in `generators` has `takes_start_picture`: only a row with `true` can make a scene that starts from a picture (a brief with `character`, or a scene with `generate.image` or `first_frame`)
- `scriptvox keys`: Provider keys: set one, or list names and sources (never values)
- `scriptvox doctor`: Check this machine: tools, fonts, keys (by name), network and TLS
- `scriptvox setup`: Download the pinned tools (FFmpeg, uv) now, instead of on the first run
- `scriptvox media`: Index your own media for `make --media`
- `scriptvox voices`: List the voices, with their cost and terms. `--preview <voice id>` writes one short audio file of a voice, to hear it before a video
- `scriptvox status`: Show the runs in this project, or one run with its last events
- `scriptvox export`: One folder to publish from: video, SRT, cover image (plus one per scene card, at most 3), credits (when stock or music needs them) and metadata.json (title, description, the terms of every part, `commercial_ok`, chapters from the scenes, and a title or caption per platform inside its limits). The video gets the chapter marks inside. Nothing is uploaded
- `scriptvox analyze`: Read a video like an editor: shots, pace, loudness, one frame per shot; `--transcript` adds the words
- `scriptvox judge`: Score frames against the brief with a vision model (your own service, `SCRIPTVOX_VISION_URL`); exit 3 on a weak frame
- `scriptvox edit`: Edit an existing video: cut pauses, crop, captions (local and free); a new background, a cut-out or an upscale go to a row and show a plan
- `scriptvox dub`: Dub a video on its own times. `--plan` alone prints a sheet of timed lines. The agent writes the new text, then `--lines` runs voice, fit, captions and gates as one `make`, at one plan and one cost
- `scriptvox research`: Collect facts about a topic with their sources, before the script: writes `research.json`. Opt-in: needs a text row (`scriptvox connect <name> --url <base> --text --model <id>`); with none it stops with exit 10
- `scriptvox write`: Write a brief (a script in scenes) with a text row, from the facts of a research file or from a topic alone. Opt-in: needs a text row (`scriptvox connect <name> --url <base> --text --model <id>`)
- `scriptvox demo`: Render a short test video: no keys, no Python, no Node
- More commands, each with one line in the skill `scriptvox-reference` (or `scriptvox --help`): `replay`, `remake`, `recipes`, `bench`, `report`, `connect`, `licenses`, `init`, `tools`, `models`, `uninstall`, `batch`, `mcp`, `clips`, `publish`, `fetch`, `compare`, `join`, `generate`.

## Exit codes

- 0: ok, also a detached start (OK)
- 1: error, fixable (ERROR)
- 2: usage error (USAGE)
- 3: a quality gate failed (GATE_FAILED)
- 4: approval needed for this plan (APPROVAL_NEEDED)
- 5: provider error, retry is safe (PROVIDER_RETRYABLE)
- 6: unconfirmed charge, do NOT retry (UNCONFIRMED_CHARGE)
- 7: watchdog killed a stalled step (WATCHDOG_KILL)
- 8: cancelled (CANCELLED)
- 9: still running (wait timeout only) (STILL_RUNNING)
- 10: needs input from a human or the agent (NEEDS_INPUT)
- 70: internal bug (INTERNAL)
