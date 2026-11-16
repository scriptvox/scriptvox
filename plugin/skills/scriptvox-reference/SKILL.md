---
name: scriptvox-reference
description: The less common brief fields and commands of the scriptvox CLI, one line each. Read it when the scriptvox skill names a brief field or a command and does not describe it, for example a dub on a base video, generated clips, a brand file, a batch, joining clips or the MCP server.
---
<!-- scriptvox-skill-version: 0.1.0 -->

# scriptvox: more brief fields and commands

The `scriptvox` skill has the steps, the rules, the common brief fields and commands, and the exit codes. This file has the rest. Both are generated from the tool. For the fields of a scene and of a card: `scriptvox schema brief --json`. For the flags of a command: `scriptvox <command> --help`.

## Brief fields

- `ambience`: Sound for each video scene from its own picture (MMAudio on your own GPU service), low under the voice. The terms of the model are in `providers` and in `metadata.json`.
- `base_sound`: The base video's own sound under the new voice: `none` (default), `keep` (all of it, low and ducked) or `background` (its voice removed by a stem row, so the music and ambience stay: a dub). Replaces the music bed.
- `base_video`: A video to re-voice: it plays from the start under the new narration, and its own sound is dropped (translate: transcribe it with `scriptvox analyze --transcript`, write the new script, set `lang`). Give the narration as `script` or `script_file`, not `scenes`: the base video replaces the other visuals.
- `brand`: A brand file (JSON, path relative to the brief) with the look of a series: lang, tts, voice, aspect, size, fps, captions, caption_at, caption_size, caption_outline, caption_box, font, style, background, music_level, transition, look, sfx, music, music_prompt, logo, accent, outro, card_style. A field in the brief wins.
- `caption_box`: The black box behind the captions, as its opacity: 0 is no box, 1 is solid black. Default 0.65. Not for `captions` `reveal`, which has no box. With no box, the gate `contrast` warns when the letters cannot be read on the picture.
- `caption_outline`: A black outline around the caption letters, in px on a 1080 px short side: 0 to 12 (it follows the frame's size). Default: none inside the box; with `caption_box` 0, a tenth of the caption size.
- `captions_second`: A small second caption line under the main one, for example a translation: one entry per sentence of the script, in order. Each shows while its sentence is said. Not with `caption_at` `top`.
- `card_style`: The colours of the cards, the hook and the end card: `dark` (default, light text on dark boxes) or `paper` for a light `background`: dark text, light boxes, thin rules, and the `accent` for headers, marks and numbers.
- `character`: The same person or thing in every generated scene: each still and first frame starts from one reference picture, and a clip with no first frame gets one. Name them in the scene prompts as "the same woman".
- `characters`: Two or more people in one story, each with a `name`. A scene names who is in it with `generate.who`.
- `cut_on`: `beat`: each scene boundary moves to the nearest beat of the music bed when that beat is at most 0.25 s away; the picture, the scene's card and the whoosh move, the voice and the captions do not. Needs `scenes` and `music` as a built-in bed with a beat (`pulse`) or a music file in the project. A boundary with no beat near, or a bed with no steady beat, stays where it was; the result's `cut_on` lists each boundary.
- `cutaways`: Pictures over `base_video` at its own times, each a file or a generated shot: one plan prices them all, then they go over the video with its own sound and captions on top. Needs `base_video`, and no script, scenes or lines.
- `emphasis`: Key words or phrases of the script that stay in the accent colour on their caption page, for example `["90 percent", "free"]`. Few of them, or nothing stands out.
- `font`: The brand's font for captions, cards and the hook: a TTF or OTF file inside the project, relative to the brief. It must hold every letter of the script. You answer for its licence.
- `fps`: Frames per second of the video: 24 (film; generated clips at 24 play frame for frame), 25, 30 (default) or 60.
- `lines`: A dub on the times of the base video: each line starts at its own `start` and fits its own time. Needs `base_video`. Use this in place of `script`, `script_file` and `scenes`.
- `look`: The colour look of the picture: `natural` (default), `warm`, `cool`, `film` (softer colour, more contrast, a vignette), `punchy` (more contrast and colour), `mono`, or a `.cube` 3D LUT file relative to the brief (a grade from your colourist). Captions and cards keep their colours.
- `loop`: `true`: the video is meant to play in a loop. It only asks for the gate `loop_seam`, which compares the last frame with the first and warns when the jump shows. It changes nothing in the video.
- `media_match`: With `media`: each line of the script gets the file from the folder that shows it, instead of the folder order. A vision service you set looks at each file once. `plan` sends nothing and names the host; `make` sends the frames and lists the picks in `media_match`, and a later `plan` shows them. To change one, write `scenes` yourself.
- `music_from`: The start of the cut of a music file or of music from an address: seconds as a number (`12.5`) or as text (`"12.5"`), or a clock (`"0:12"`).
- `music_prompt`: Or a music bed made by a music row from this description (style, instruments, mood, tempo; no vocals).
- `music_search`: Or a free music bed found with no key through `media search`, from these search words. It keeps its own licence.
- `music_to`: The end of that cut, in the same forms.
- `seed`: A seed for every generated still and clip: the same seed and brief give the same pictures on rows that take one (`takes_seed` in `scriptvox providers`, today the local image row). The plan says when a row does not.
- `size`: The short side of the frame in px: 720, 1080 (default), 1440 or 2160, for every `aspect` (16:9 at 2160 is 3840x2160). Text, cards, logo and outro follow it; a smaller clip is scaled up. `plan` gives the render time and disk use.
- `smooth`: Smooth motion for each generated clip with an interpolate row (RIFE on your own GPU service): new frames between the clip's own, 24 to 48 fps. A clip a little short for its scene then plays up to 2 times slower without a stutter (24 fps clips: 1.43 times at most). Adds about half a minute per clip.
- `sources_card`: `true`: a card "Sources" on the last scene with the names of the publishers of `sources` (each `publisher`, else `title`, else the host of `url`), in English or Arabic. Needs `scenes` and `sources`. Default: no card.
- `style`: A look added to the end of every `generate` prompt, so the clips of a video or a series look alike. For example "ink illustration, muted colours, soft morning light". Name the result, not the making.
- `upscale`: Upscale each generated clip with an upscale row (SeedVR2 3B on your own GPU service): a 704x1280 Wan clip comes back as 1080x1920 with real detail instead of plain scaling. Adds about a minute per clip.
- `voice_rate`: How fast the voice speaks: 1.0 is the voice's own pace, 0.75 is slower, 1.3 is faster (ElevenLabs: up to 1.2). For `tts` edge, elevenlabs, kokoro-rust and none. With `duration` the voice keeps this pace, and the length is reached by a tempo change of at most 16 percent, or the run stops and says how many words to add or cut.
- `waveform`: `true`: a waveform that follows the video's own sound (voice and music as heard), in the `accent` colour, over the plain background of the scenes with no picture and no card. For a podcast clip or a voice-only video. It stays clear of the captions. Needs sound: a voice, a narration or music.

## Commands

- `scriptvox replay`: Rebuild a ScriptVox MP4 from its recipe, from the local cache only (no provider calls)
- `scriptvox remake`: Remake a ScriptVox MP4 from its recipe (provider calls allowed; paid stages need --approve)
- `scriptvox recipes`: Starter briefs per kind of video: `recipes` lists them, `recipes show <name>` gives one whole brief and how to change it
- `scriptvox bench`: ScriptVox's own speed on this machine (hardware recorded)
- `scriptvox report`: Write a text about this machine and one run, to send with a problem. Made here; nothing is sent. Key values, the home folder, the user name and the script, prompt and card texts of the request are taken out. An error message can still quote a few of your words: read it before you send it
- `scriptvox connect`: Connect your own OpenAI-compatible image service (rows filled from its `GET /v1/models`) or, with --workflow, a ComfyUI graph. https, or http only on this machine. Its price is unknown, so `generate` needs --max-spend. Keys go in only through `scriptvox keys set`
- `scriptvox licenses`: Third-party licences: crates, fonts, Python packages and every file setup can download (`--full` prints the texts)
- `scriptvox init`: Add a short scriptvox block to this project's AGENTS.md and CLAUDE.md (for any coding agent). Needs --agents
- `scriptvox tools`: List the tools a plan can use (engine, TTS, stock), with cost and network
- `scriptvox models`: Model files already on this machine, so setup need not download them
- `scriptvox uninstall`: Remove ScriptVox data (tools, worker, cache). Projects are not touched
- `scriptvox batch`: One video per row: a brief with {{key}} placeholders and a JSON list of rows that fill them
- `scriptvox mcp`: Serve the commands as MCP tools over stdio (for Claude Desktop, Cursor, Codex and other MCP clients)
- `scriptvox clips`: A long talk into ranked shorts (needs speech to text and a vision service): picks the parts that stand alone, writes a hook for each, then cuts, crops and captions them
- `scriptvox publish`: Upload an export folder to your own YouTube channel or Hugging Face repo. Opt-in, private by default, and each upload stops with exit 4 until the person approves it (`--approve <publish_id>`)
- `scriptvox fetch`: Get a video or audio file from an https address: a media file or an HLS playlist. Off until you run `scriptvox setup fetch` one time. You can get a file from any address: the choice and the responsibility are yours. fetch never signs in to a site. When a site refuses the download, save the file yourself and give the local file to `scriptvox analyze <file>`, `scriptvox make --music <file>` or `--media <folder>`
- `scriptvox compare`: Compare 2 videos frame by frame: how alike they look and the times that differ. Picture only, free and local
- `scriptvox join`: Join clips of any size and rate into one video, locally and free, with a transition and mastered sound
- `scriptvox generate`: Make one AI video clip with a generator from `scriptvox providers` (paid: plan, then --approve)
