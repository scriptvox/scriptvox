# ScriptVox

ScriptVox is a command-line tool that makes a narrated and captioned MP4 from a script. You can use it directly, or a coding agent such as Claude Code can operate it for you.

| Item | Data |
|---|---|
| Licence | Apache-2.0 |
| Version | 0.1.0, the first public release |
| Platforms | Linux and Windows. macOS is a preview: see [its limits](#macos-preview) |
| Form | One binary. No account and no telemetry |
| Cost control | A plan that costs money stops until you approve it |
| Recovery | A run that stops continues from its last finished chunk |

![A real session: a 30 s render killed with SIGKILL, then the same command resumes from the finished chunks](docs/launch/kill-and-resume.gif)

```text
you:     "make a 30 s vertical short in Arabic about dates, free tools only"
claude:  scriptvox plan, make --detach, wait, snapshot
result:  scriptvox-out/r_7f3c9a.mp4, 30.0 s, $0, Edge voice, -14 LUFS, all gates passed
```

## Install with your coding agent

Copy this block into Claude Code, Codex, Cursor, Gemini CLI, opencode, Pi or Hermes. The agent installs ScriptVox, proves that it works, and then asks you what video to make.

```text
Install ScriptVox, a command-line tool that makes narrated, captioned videos, and prove that it works.

Rules: no sudo. Do not change a shell profile. Do not ask me for a key or a token in chat.
Give each command --json and read the answer: ok, error.fix, needs_input, next.

1. Get the binary.
   Linux or macOS:  curl -fsSL https://github.com/scriptvox/scriptvox/releases/latest/download/install.sh -o install.sh && sh install.sh
   Windows (PowerShell):  irm https://github.com/scriptvox/scriptvox/releases/latest/download/install.ps1 -OutFile install.ps1; powershell -ExecutionPolicy Bypass -File install.ps1
   Each install script checks the sha256 of the archive, uses no sudo, and prints the folder of the binary.
   Call the binary by its full path, or add its folder to PATH for this session only.
2. Run `scriptvox doctor --json`. On exit 1, run `scriptvox doctor --fix --json`. On exit 10, show me each needs_input[].human_action and stop.
3. Run `scriptvox demo --json`. It makes a short test video with no key. Tell me the path in artifacts.video.
4. Install the skills for yourself: `scriptvox skills install --json` if you are Claude Code, `scriptvox skills install --agents --json` if you are any other agent.
5. Ask me what video I want. Then follow the scriptvox skill, or `scriptvox --help`.

Money: when a plan costs money the tool stops with exit 4. Show me the price and the plan_id. Add --approve <plan_id> only after I say yes.
Keys: when the tool needs a key, tell me the one command to run myself (`scriptvox keys set <provider>`).
```

## For coding agents

Read [`llms.txt`](llms.txt) first. It contains the same data in one file. Each command accepts `--json` and prints one JSON document on stdout. This is also the default when stdout is not a terminal.

| Step | Command | What you read |
|---|---|---|
| 1 | `scriptvox doctor --json` | Exit 1: run `doctor --fix`. Exit 10: show `needs_input[].human_action` to the person |
| 2 | `scriptvox plan --brief brief.json --json` | `plan_id`, `metrics.cost_usd`, `needs_input`. Nothing runs |
| 3 | `scriptvox make --brief brief.json --detach --json` | `run_id`. The command returns at once. The run continues in the background |
| 4 | `scriptvox wait <run_id> --json` | The result, with `artifacts.video`. Exit 9 is not a failure: the run is still going, run the command in `error.fix` |
| 5 | `scriptvox check <run_id> --json` | `findings`: one row for each quality gate. Exit 3 when a gate failed |
| 6 | `scriptvox snapshot <run_id> --sheet 9 --json` | `artifacts.image`: one PNG with 9 frames. Look at it |
| 7 | `scriptvox resume <run_id> --brief brief.json --json` | A child run `<run_id>.1` from what is finished. Then `wait` on it |

**The envelope.** The name is `scriptvox.result/v1` ([schema](schemas/result.json)). Its fields are `schema`, `ok`, `status`, `command`, `run_id`, `plan_id`, `needs_input`, `findings`, `next`, `error` (`code`, `message`, `fix`, `retryable`, `retry_after_s`), `artifacts`, `metrics`, `events_file`. On an error, do what `error.fix` says. `next` holds the next commands.

**Exit codes.**

| Code | Meaning |
|---|---|
| 0 | OK. Also a detached start |
| 1 | Error with a fix |
| 2 | Usage error |
| 3 | A quality gate failed |
| 4 | Approval needed |
| 5 | Provider error. A retry is safe |
| 6 | Unconfirmed charge. Do not retry |
| 7 | The watchdog stopped a stalled step |
| 8 | Cancelled |
| 9 | Still running. Only `wait` gives this code, at its timeout |
| 10 | Input needed from a person or the agent |
| 70 | Internal bug |

**Money.** A plan that costs money stops with exit 4 and status `approval_needed` until the command has `--approve <plan_id>`. The approval belongs to the person. An agent shows the price and the `plan_id`, asks, and adds the flag only after a yes.

> **CAUTION:** An agent must not approve a plan by itself. An agent must not raise `--max-spend` by itself.

**Keys.** Keys stay out of chat. When a key is missing, the answer has a `needs_input` item of kind `secret`. Show its `human_action` to the person. The person runs `scriptvox keys set <provider>` in a terminal.

## What you get, and what it needs

This table shows what operates with no additions, and what each addition needs.

| With | You get |
|---|---|
| **No key and no service** | A narrated video from your script with an Edge voice, in English and Arabic. The picture is your own clips and photos, or a moving colour background. Also: fitted captions, cards, formulas, plots, 15 transitions, synthesized music beds, gates over each frame, and resume after a crash. The commands `edit` (cuts, speed, crop, logo), `join`, `export` and `analyze` are included. The Edge voice needs the network, because the script text goes to Microsoft's read-aloud service. Its terms are for personal use. `export` writes the terms of each part to `metadata.json`, and the decision is yours |
| **A free download** | `scriptvox setup speech-local` (about 490 MB to download, about 880 MB on disk, MIT, runs on your CPU) gives captions from the speech of a video, `--cut-fillers`, `check --hear`, `analyze --transcript` and the listening gate. Arabic is weaker than English on this small model, so read the captions. `scriptvox setup speech-local --arabic` adds an Arabic model ([scriptvox/whisper-small-ar](https://huggingface.co/scriptvox/whisper-small-ar), about 485 MB, Apache-2.0, used only when the language is `ar`) that makes fewer word errors on Gulf and Emirati speech: in tests, 28 in 100 words wrong on Gulf speech in place of 41, and 21 in place of 27 on Emirati speech. `scriptvox setup voice-local` (about 180 MB with its packages, includes GPL-3.0 parts in a separate process) gives a local English voice. With it, `--local-only` makes a full video with no network. `scriptvox setup image-local` (about 5 GB, MIT and Apache-2.0, no token) makes pictures on your own machine. It operates on the CPU and is slow: 446 s at 512x512 on 2 cores. The Linux build needs glibc 2.38 (Ubuntu 24.04 or newer). Its speed on a graphics card is not measured, and NVIDIA on Linux has no CUDA pack at this time. There is no local clip model at this time |
| **No key** | Stock clips from Wikimedia Commons and the Internet Archive. The credits give the terms of each file |
| **A free key** | Pexels and Pixabay stock clips |
| **A vision service that you run or rent** (any OpenAI-compatible URL, `SCRIPTVOX_VISION_URL`) | `judge`, `media_match`, `clips`, `edit --track`, and the look part of `analyze --like`. With no service set, nothing is sent and the step tells you. `plan` names the host before frames go |
| **Your own GPU service** (`services/gen`, see [self-hosted](docs/specs/self-hosted.md), key `selfhost`) | Open models at no charge for each call: Wan video, FLUX.2 klein image, ACE-Step music, Chatterbox voice, Whisper large-v3, upscale, interpolate and lip sync. This is the free route to AI video that ScriptVox supplies, and it is a real GPU server to operate. A ComfyUI video graph that you run and add with `scriptvox connect --workflow` also has no provider charge. Free AI pictures also come from `scriptvox setup image-local`, above. Each model keeps its own licence, and `scriptvox providers` shows it |
| **A paid key of your own** | ElevenLabs voices. Through fal: Kling, MiniMax and Grok video, a FLUX image row and a transcript row. A plan shows the price, and a paid step runs only with `--approve <plan_id>`. Kling 2.6 Pro, Kling 3.0 Pro and an ElevenLabs voice were run live. `scriptvox providers` marks the rows that were not run live |

Music: `library:<name>` is a song from the music library. Each song has a traced source and the CC0 licence, so it is free for any use with no credit. `scriptvox media music` lists 11 of them, by mood. You can also use any song or media file. The normal route is a file on your computer: `--music song.mp3`, or a video for its sound. An address also works, but a site can refuse an automatic download (for example a bot check). Then download the file yourself and give its path. The choice and the responsibility are yours. ScriptVox does not check the rights to a file.

## Installation

| System | Command |
|---|---|
| Ubuntu and Debian | Download `scriptvox_0.1.0_amd64.deb` from the [release page](https://github.com/scriptvox/scriptvox/releases/latest). Then: `sudo apt install ./scriptvox_0.1.0_amd64.deb` |
| Linux, with the script | `curl -fsSL https://github.com/scriptvox/scriptvox/releases/latest/download/install.sh -o install.sh && sh install.sh` |
| macOS (preview) and Linux, with Homebrew | `brew install scriptvox/tap/scriptvox`. On macOS it also installs FFmpeg (`ffmpeg-full`) and uv |
| macOS (preview), with the script | The same command as Linux. Then install FFmpeg with libass, and uv: `brew install ffmpeg-full uv` |
| Windows, with Scoop | `scoop bucket add scriptvox https://github.com/scriptvox/scriptvox`, then `scoop install scriptvox` |
| Windows, with the script | `irm https://github.com/scriptvox/scriptvox/releases/latest/download/install.ps1 -OutFile install.ps1; powershell -ExecutionPolicy Bypass -File install.ps1` |

Each route was installed and run on a real machine of that system. The AUR package comes in a later release.

The Windows program is not signed. Windows can show a warning for a file that you download in a browser. The script installs it without that warning.

### macOS preview

ScriptVox uses the FFmpeg (`ffmpeg-full`) and the uv of Homebrew on macOS. The install script tells you the two commands. The demo video, captions, English cards, the default voice and the quality check were run on Apple silicon and on Intel. The limits at this time are:

- `scriptvox setup image-local` has no build for Intel Macs. It refuses there with a clear message.
- `scriptvox setup voice-local` can refuse a long data folder path. Set `SCRIPTVOX_DATA` to a short path.
- ScriptVox has no pinned FFmpeg for macOS. It uses the FFmpeg of Homebrew.
- The program is not signed.

### Build from source

1. Install [Rust](https://rustup.rs).
2. Build the binary and read its version:

   ```sh
   cargo build --release
   ./target/release/scriptvox --version
   ```

3. Put `target/release/scriptvox` on your PATH.

### Installation from a release archive

| Platform | Command | Default folder |
|---|---|---|
| Linux | `sh install.sh --from <tarball>` | `~/.local/bin`, or `SCRIPTVOX_BIN_DIR` |
| Windows | `powershell -ExecutionPolicy Bypass -File install.ps1 -From <zip>` | `%LOCALAPPDATA%\scriptvox\bin`, or `-Dest <folder>` |

Each script checks the sha256 file that is next to the archive. The scripts make no change to your PATH. Each script prints the line to add. With no `--from`, a script downloads from the release address, or from `SCRIPTVOX_BASE_URL` (`-BaseUrl` on Windows) for a mirror.

### Installation test

```sh
# exec
scriptvox --version
scriptvox demo
scriptvox doctor --offline
scriptvox voices --lang ar
```

`scriptvox demo` renders a test video of 8 seconds with no keys, no Python and no Node.

### First-use downloads

On first use, ScriptVox downloads a pinned FFmpeg build into its data folder and checks its sha256. On the first voice, it downloads a pinned uv and Python. This takes about 1 to 2 minutes, one time. The installation runs without `sudo` and makes no change to your shell profile.

Sizes, measured on 13 Nov 2026:

| Part | Size |
|---|---|
| FFmpeg | 152 MB download on Linux, 194 MB on Windows, about 520 MB on disk |
| uv | 15 to 18 MB |
| Python with its packages | About 100 MB on disk |
| Data folder, total | About 600 to 700 MB |

### Your first video

```sh
echo "ScriptVox turns a script into a video. It reads your words aloud and writes captions that fit." > script.txt
scriptvox make --script script.txt --music calm
```

Add `--media my-clips` to use your own clips and photos.

## Use with Claude Code and other agents

```sh
scriptvox skills install            # Claude Code: writes the skills to ~/.claude/skills/
scriptvox skills install --agents   # Codex, Cursor, Gemini CLI, opencode and Pi: ~/.agents/skills/. Hermes: ~/.hermes/skills/, when Hermes is installed
scriptvox init --agents             # this project: a short block in AGENTS.md and CLAUDE.md
```

Then ask your agent for a video. The skill writes a brief, makes a plan, makes the video, waits, looks at a contact sheet of the result, and corrects what it sees. With `--json`, each command prints one JSON envelope with fixed exit codes and a `fix` line, so any coding agent can operate it. `init --agents` makes no change to a `CLAUDE.md` that imports `AGENTS.md`.

## Functions

### Picture

| Function | Description | Needs |
|---|---|---|
| Shot for each line | With `media_match`, a vision service looks at your folder one time and gives each sentence the file that shows it. `plan` sends nothing and names the host. `make` sends the frames | A vision service |
| Your clips as the shot list | Point `--media` at a folder of clips and photos. Shots cut on the rhythm of the speech, each file is used, and stills get a slow zoom | Nothing |
| Scenes | In a brief, `scenes` pairs each line of narration with a clip, a photo, a stock term or the built-in background. Cuts occur on the first word of each scene, with short crossfades | Nothing |
| Hook | `--hook "Stop scrolling"` puts a large headline on screen for the first 1.6 s, measured and wrapped to fit | Nothing |
| Cards | A scene can have a `card`: title, stat, lower third, list, bars, line, plot, donut, timeline, compare, quote, formula (a TeX subset, no LaTeX) or code. Cards are drawn in the same pass as the captions. Arabic mirrors the layout | Nothing |
| Brand and look | A brief accepts `logo`, `accent`, `font`, `background`, `look`, an `outro` end card and 15 transitions, one for the video or one for each scene | Nothing |
| Picture in picture | `scenes[i].inset` puts a clip or a still over the scene | Nothing |
| Same person in each scene | Brief `character` makes one reference picture, and each still and first frame starts from it. `characters` does the same for two or more persons | Your own GPU service |
| AI shots | `generate.still` makes a picture, and a video row makes a clip. A clip that was made comes back at no cost on `resume` | Your own GPU service, or a paid key |
| Draft | `make --draft` renders at half size and 15 fps (an 84 s video in 9 s, not 71 s). Then `resume --final` makes the full video | Nothing |

### Voice and captions

| Function | Description | Needs |
|---|---|---|
| Captions that fit | Line breaks are measured with real text shaping (within 0.5% of the renderer). Arabic is shaped and right-to-left, with Arabic voices and no English fallback | Nothing |
| Target length | `--duration 30` fits the voice before the render starts. The options are: keep, a small tempo change, or a new reading at a different speaking rate. A script that cannot fit stops with "cut about N words" | Nothing |
| Caption styles | `karaoke` (the spoken word is lit), `pop` (1 to 3 large words on a page) and `reveal` (each word appears when it is said). `caption_at` sets the position and `emphasis` marks key words | Nothing |
| Speakers | `scenes[i].voice` gives a scene a second voice. `tts: none` shows the words at reading pace with music or silence | Nothing |
| Pause tags | `[pause 1.5]`, and in Arabic `[توقف ١٫٥ ثانية]` | Nothing |

### Work on a video that you have

| Function | Description | Needs |
|---|---|---|
| Edit | `edit` cuts silence and parts of a clip, crops to a new shape and changes speed. `join` joins clips of any size and rate. `export` makes one folder to publish from, with the terms of each part | Nothing |
| Captions from speech | `edit` burns captions from the speech of the video | Speech to text |
| Subject tracking | `edit --track` keeps the subject in frame | A vision service |
| Make one like this | `analyze ref.mp4 --like new.json` writes a starter brief from a reference video. Its cuts and pace come on any installation. Its words need speech to text. Its look needs a vision service | Nothing for cuts and pace |
| Shorts from a long talk | `clips talk.mp4` selects the parts that stand alone, writes a hook for each, then cuts, crops and captions them | Speech to text and a vision service |

### Control and safety

| Function | Description |
|---|---|
| Resume after a crash | Renders run in chunks with a watchdog. Stop the run, give the same command again, and it continues from the last finished chunk |
| Self-check | Each video goes through gates over each frame: black stretches, slideshow risk, silence, loudness (-14 LUFS) and audio/video drift |
| Money control in code | A plan lists stages, cost and downloads. A plan that costs more than $0 needs `--approve <plan_id>`. A spend cap is checked in a database transaction before each paid call |
| Recipe in each video | The MP4 metadata holds the brief and the plan, and no keys. Use `scriptvox explain`, `replay` (cache only, zero provider calls) and `remake` |
| Keys | Keys come from the environment or from a `keys.toml` that only your user can read (`scriptvox keys set pexels`). Arguments, logs, results and the MP4 contain no key. An agent gets `kind:"secret"` and tells you the command to run |

## Commands

| Command | Function |
|---|---|
| `make --script f` / `--brief f` | Make a video (`--detach` returns at once) |
| `plan` | Stages, cost, downloads and `plan_id`, with nothing run |
| `wait` / `status` / `cancel` / `resume --brief` | Follow and revise runs |
| `check [--hear]` / `snapshot --sheet 9` / `judge` | Quality gates. One PNG that an agent can read. A vision model scores frames against the brief |
| `analyze [--transcript] [--like f]` | Read a video as an editor does. Write a starter brief in its style |
| `clips` | A long talk into ranked shorts |
| `providers` / `voices` / `tools` | Each row tells you who runs it and who pays: your own GPU service, or a paid external API with your own key and its price |
| `voices --preview <id>` | One short audio file of a voice, to hear it before a video. A paid voice needs `--approve` |
| `edit` / `join` / `export` | Change a video, join clips, make a publish folder (`--format webm,av1,hevc,prores,hls`, `--overlay` for captions and cards on a transparent frame) |
| `generate` / `batch` | One AI clip from a row. One video for each row of data |
| `compare` | How alike 2 videos look, and the times that differ |
| `mcp` | Most commands as MCP tools ([the list, and the ones not offered](docs/specs/mcp.md)). `setup mcp --client cursor` connects a client |
| `explain` / `replay` / `remake` | Read and use again the recipe inside an MP4 |
| `doctor` / `setup` / `uninstall --purge` | Machine check, pinned tools, clean removal |
| `templates` / `schema` | Lists of templates and of the data formats |
| `keys set --stdin` / `keys list` | Provider keys. The values are not shown |
| `dub` | A new voice in a different language on the times of the video, with captions. A line that cannot fit stops with the number of characters to cut |
| `media add` / `media search` | Your own files into the project. Free pictures and sound with no key, each with its own licence |
| `connect` / `models add` / `models scan` | Your own image service or ComfyUI graph as a row. A model from Hugging Face or from a file. Models that are on this machine |
| `licenses [--full]` / `bench` | Each part and its licence. How fast this machine renders |
| `demo` | The test with no key |

`scriptvox --help` gives the full list.

## Construction

One Rust binary does the planning, captions, rendering (FFmpeg), cache, ledger and gates. A small Python worker does text to speech. It starts only when a voice is necessary, and `uv` runs it. Optional plugins (local voice, local speech to text) operate the same way, each in its own process. The parts communicate with JSON-RPC over stdio. State is in one SQLite file for each project. Generated files are in a content-addressed cache.

## Credits

ScriptVox learned from OpenMontage, MoneyPrinterTurbo, HyperFrames, ComfyUI, Manim and Remotion. [CREDITS.md](CREDITS.md) shows what came from where.

## Licence

The code of ScriptVox is Apache-2.0. Models and components are used as separate parts, as an API call is. Each keeps its own licence unchanged, and ScriptVox claims no ownership of them. `scriptvox providers` shows the terms of each row, and `export` writes them to `metadata.json` (`commercial_ok`). The decision is yours.

Fonts: SIL OFL 1.1. FFmpeg is downloaded at first use (GPL build) and is not part of this repository.
