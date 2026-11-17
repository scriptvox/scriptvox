//! Command grammar. Verb first, flat (PLAN-R1 §2.3). The clap tree is the
//! command registry: schema, skill and docs generation read it in later tickets.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "scriptvox",
    disable_version_flag = true,
    about = "Agent-first AI video CLI"
)]
pub struct Cli {
    /// Print one JSON envelope on stdout (default when stdout is not a TTY).
    #[arg(long, global = true)]
    pub json: bool,

    /// Use the system FFmpeg (PATH) instead of the pinned download.
    #[arg(long, global = true)]
    pub use_system: bool,

    /// Approve a plan that costs money: the plan_id from `make --plan` or exit 4. The approval belongs to the
    /// person: an agent shows the price, asks, and gives this flag only after a yes.
    #[arg(long, global = true)]
    pub approve: Option<String>,

    /// Spend cap for one run in USD (default: the approved plan cost).
    #[arg(long, global = true)]
    pub max_spend: Option<f64>,

    /// The same command with the same key returns (or resumes) the same run.
    #[arg(long, global = true)]
    pub idempotency_key: Option<String>,

    /// Do not embed the recipe (brief and plan, never keys) in the MP4.
    #[arg(long, global = true)]
    pub no_embed: bool,

    /// Print the plan (stages, cost, downloads, plan_id) and run nothing.
    #[arg(long, global = true)]
    pub plan: bool,

    /// Never use a network engine (no Edge TTS, no stock, no cloud).
    #[arg(long, global = true)]
    pub local_only: bool,

    /// Print the version, protocol and tool pins.
    #[arg(long)]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Cmd>,
}

/// The commands the main skill lists (`skill.rs`). Every other command is listed in the second skill,
/// `scriptvox-reference`, so a new command goes there until its name is added here. Keep this list short: the main
/// skill has a length test.
pub const COMMON_COMMANDS: [&str; 25] = [
    "make",
    "wait",
    "cancel",
    "resume",
    "status",
    "snapshot",
    "check",
    "judge",
    "analyze",
    "doctor",
    "setup",
    "keys",
    "voices",
    "providers",
    "media",
    "templates",
    "research",
    "write",
    "edit",
    "dub",
    "export",
    "explain",
    "schema",
    "skills",
    "demo",
];

// The CLI is parsed once per process: boxing the large `Make` variant buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Make a narrated, captioned video from a script. `scriptvox plan ...` is `make --plan ...`. It takes minutes: with a time limit on your shell, `make --detach`, then `wait`.
    ///
    /// An agent, or any caller with a time limit on a command: use `make --detach`, then `scriptvox wait <run_id>`.
    /// A make takes minutes. Without `--detach` it runs inside this command and stops when your shell stops it; then
    /// `scriptvox status <run_id>` says `alive: false`, and `scriptvox resume <run_id>` goes on from what is finished.
    #[command(visible_alias = "plan")]
    Make {
        /// A topic only: scriptvox asks the agent to write the script (it makes no LLM call).
        topic: Option<String>,
        /// With no agent: a text row finds facts with their sources and writes the script, then the video is made
        /// (`research`, `write`, `make` in one command). Opt-in: with no text row it stops with exit 10 and the one
        /// `connect` command. The script stays in `scriptvox-out/topic-<id>/brief.json`.
        #[arg(long = "topic", value_name = "TEXT", conflicts_with_all = ["topic", "script", "brief"])]
        from_topic: Option<String>,
        /// The narration text, one file.
        #[arg(long)]
        script: Option<PathBuf>,
        /// A brief (JSON, see `scriptvox schema brief`). Flags override its fields.
        #[arg(long, conflicts_with = "script")]
        brief: Option<PathBuf>,
        /// Language code (en, ar). Default: detected from the script.
        #[arg(long)]
        lang: Option<String>,
        /// TTS engine (edge, elevenlabs, kokoro, kokoro-rust, selfhost: an open voice on your own GPU service).
        #[arg(long)]
        tts: Option<String>,
        /// Voice id from `scriptvox voices`.
        #[arg(long)]
        voice: Option<String>,
        /// Where to write the MP4 (default: scriptvox-out/<run id>.mp4).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Target length in seconds. The narration is fitted before render.
        #[arg(long)]
        duration: Option<f64>,
        /// Frame shape: 9:16 (shorts, default), 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9.
        #[arg(long)]
        aspect: Option<String>,
        /// Folder of your own clips and stills (inside this project), used in name order.
        #[arg(long)]
        media: Option<PathBuf>,
        /// Stock video for the shots: comma-separated search terms (one per shot), or `auto`.
        /// With a Pexels or Pixabay key those come first; with none, Wikimedia Commons and Internet Archive, terms per file in the credits.
        #[arg(long)]
        stock: Option<String>,
        /// A fast look at half the size and 15 fps, without upscale or smoothing. `resume <run> --final` makes the full video.
        #[arg(long)]
        draft: bool,
        /// A headline shown big for the first 1.6 s (the hook).
        #[arg(long)]
        hook: Option<String>,
        /// Music under the voice, ducked: calm, pulse (built in), library:<name> (a song with a free licence, `scriptvox media music` lists them), a file in this project, an https address, or none.
        /// Any song or media file can be used (mp3, m4a, wav, or a video for its sound): the choice and the responsibility are yours.
        /// A file shorter than the video starts again; the music fades out in the last second.
        /// An address: its sound is downloaded one time through `fetch` (run `scriptvox setup fetch` first).
        #[arg(long)]
        music: Option<String>,
        /// The start of the cut of a music file or of music from an address: seconds or a clock (0:12).
        #[arg(long, value_name = "T")]
        music_from: Option<String>,
        /// The end of that cut.
        #[arg(long, value_name = "T")]
        music_to: Option<String>,
        /// Stop after a stage, to check its result before the long steps. Only `voice` for now: the voice, the fit to
        /// `duration` and the caption files, with no clips, stock, music or render. `resume <run>` then makes the video
        /// with the same voice.
        #[arg(long, value_name = "STAGE")]
        until: Option<String>,
        /// Stills first: for every scene that generates a clip, make one picture with an image row and write them as
        /// one sheet with the scene numbers, then stop. No clip, no voice, no render. A free image row runs at once; a
        /// paid one needs `--approve` of the storyboard's own plan, which holds the pictures only (`plan --storyboard`
        /// shows it). `make` then starts a `first_frame` scene from its picture.
        #[arg(long, conflicts_with_all = ["detach", "until"])]
        storyboard: bool,
        /// Return at once with the run id; the run continues in the background, also when your shell stops. Then
        /// `scriptvox wait <run_id>`.
        #[arg(long)]
        detach: bool,
        /// Fail when a number or date of the script is in no entry of `sources`. Gate `claims`, exit 3, and no work
        /// starts. Without this flag it is a warning. A brief with no `sources` has no such gate.
        #[arg(long)]
        strict: bool,
        /// Internal: the detached supervisor attaches to this run.
        #[arg(long, hide = true)]
        run_id: Option<String>,
    },
    /// Wait for a run to finish and print its result. Exit 9 is not a failure: the run is still going, run `wait` again.
    Wait {
        id: String,
        /// Seconds to wait (default 100, under an agent's 2-minute tool limit). Use more when your shell allows a
        /// longer command (540 under a 10-minute limit). At the timeout: exit 9, and `error.fix` has the next command.
        #[arg(long, default_value_t = 100)]
        timeout: u64,
    },
    /// Stop a running run.
    Cancel { id: String },
    /// Show the recipe embedded in a ScriptVox MP4.
    Explain { file: PathBuf },
    /// Rebuild a ScriptVox MP4 from its recipe, from the local cache only (no provider calls).
    Replay {
        file: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Remake a ScriptVox MP4 from its recipe (provider calls allowed; paid stages need --approve).
    Remake {
        file: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Return at once with the run id; the run continues in the background. Then `scriptvox wait <run_id>`.
        #[arg(long)]
        detach: bool,
        /// Internal: the detached supervisor attaches to this run.
        #[arg(long, hide = true)]
        run_id: Option<String>,
    },
    /// Make a child run from a finished or failed run, with a new brief or flags.
    ///
    /// The child `<run>.1` runs in the background: `scriptvox wait <run>.1`. Its video is written next to the parent's,
    /// not over it, so the two can be compared: `--out name.mp4` of the parent gives `name.1.mp4` (the answer's `out`
    /// field says where). A parent that wrote no video (it failed or was stopped) leaves the name to the child.
    Resume {
        id: String,
        /// The revised brief. Its fields replace the parent's. A look field it leaves out (hook, outro, logo, music,
        /// accent, look, captions, emphasis, character, transition, sfx, upscale, smooth, ambience) is dropped; language,
        /// voice, length, aspect and media keep the parent's.
        #[arg(long)]
        brief: Option<PathBuf>,
        /// Only the fields that change, as JSON (`{"hook": "New hook"}`, or `null` to drop one): every other field keeps
        /// the parent's. Paths are relative to the project folder.
        #[arg(long, conflicts_with = "brief", value_name = "FILE")]
        patch: Option<PathBuf>,
        /// A new target length in seconds.
        #[arg(long)]
        duration: Option<f64>,
        /// Make the full video from a `--draft` run: same words, cards, clips and music. A made clip comes back free.
        #[arg(long = "final")]
        final_video: bool,
        /// Run in the foreground instead of the background.
        #[arg(long)]
        wait: bool,
        /// Run in the background (the default; accepted so `make` and `resume` take the same flag).
        #[arg(long, conflicts_with = "wait")]
        detach: bool,
        /// Write one scene as a short MP4, for a fix loop. Alone: cut from this run's video, no plan and no call.
        /// With --brief, --patch, --duration or --final: the normal resume first (its plan and approval), in the
        /// foreground, then the cut from the new run.
        #[arg(long, value_name = "N", conflicts_with = "detach")]
        only_scene: Option<usize>,
    },
    /// Internal: the supervisor runs a saved request.
    #[command(name = "_run", hide = true)]
    RunRequest { id: String },
    /// List the video templates.
    Templates,
    /// Starter briefs per kind of video: `recipes` lists them, `recipes show <name>` gives one whole brief and how to change it.
    ///
    /// The kinds: explainer (60 s and 30 s, English and Arabic), product demo, ad, news brief, tutorial, data story,
    /// top 5, quote short and a dialogue of two voices. Every recipe is free to run: the local voice or no voice, no
    /// picture, no key, no network. Not the recipe inside an MP4: that is `explain`.
    Recipes {
        #[command(subcommand)]
        action: Option<RecipesCmd>,
    },
    /// ScriptVox's own speed on this machine (hardware recorded).
    Bench,
    /// Write a text about this machine and one run, to send with a problem. Made here; nothing is sent. Key values,
    /// the home folder, the user name and the script, prompt and card texts of the request are taken out. An error
    /// message can still quote a few of your words: read it before you send it.
    Report {
        /// The run to describe. Default: the newest run that failed, else the newest run.
        run: Option<String>,
        /// Where to write it (default: scriptvox-out/report-<run>.md, never over an earlier file).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Connect your own OpenAI-compatible image service (rows filled from its `GET /v1/models`) or, with --workflow, a
    /// ComfyUI graph. https, or http only on this machine. Its price is unknown, so `generate` needs --max-spend. Keys go
    /// in only through `scriptvox keys set`.
    Connect {
        /// A short name of a-z, 0-9, '.', '_' or '-' for the row, or the start of the rows' names.
        name: String,
        /// The service's base address, without /v1.
        #[arg(long)]
        url: String,
        /// A name for the service's key: then run `scriptvox keys set <name>`. Leave it out for a service with no key.
        #[arg(long)]
        key_name: Option<String>,
        /// An id from the service's list, when its name does not say image. Repeat for more.
        #[arg(long = "model", conflicts_with = "workflow")]
        models: Vec<String>,
        /// A ComfyUI graph in API format (Export (API) in ComfyUI). One row runs it. Prompt, seed, size, length and output
        /// are found by role, every node class is checked against the server, nothing else in the graph changes.
        #[arg(long)]
        workflow: Option<std::path::PathBuf>,
        /// A data file that describes a queue service (submit, poll, result). See docs/specs/connect.md. One row, no code runs.
        #[arg(long, conflicts_with_all = ["workflow", "models"])]
        spec: Option<std::path::PathBuf>,
        /// With --workflow: `role=<node id>.<input>` (`output=<node id>`) when a role is not found or is ambiguous. Roles:
        /// prompt, seed, width, height, length, image, output. Repeat for more.
        #[arg(long = "map", requires = "workflow")]
        maps: Vec<String>,
        /// Read the list and show the rows, and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Add a text row for `research` and `write`, not image rows. The service has the OpenAI chat API: a local
        /// server such as Ollama or llama.cpp, or a hosted one with your own key. Needs one `--model`. No text row
        /// exists until you add one, and only `research` and `write` call it.
        #[arg(long, conflicts_with_all = ["workflow", "spec"])]
        text: bool,
        /// With --text: your USD price for a million tokens in. Leave it out for a free (local) row. A row with a
        /// price is a paid row: `research` and `write` then stop for --approve.
        #[arg(long, requires = "text", value_name = "USD")]
        usd_in: Option<f64>,
        /// With --text: your USD price for a million tokens out.
        #[arg(long, requires = "text", value_name = "USD")]
        usd_out: Option<f64>,
    },
    /// Third-party licences: crates, fonts, Python packages and every file setup can download (`--full` prints the texts).
    Licenses {
        #[arg(long)]
        full: bool,
    },
    /// The skills for coding agents (Claude Code, Codex, Cursor, Gemini CLI, opencode, Pi, Hermes): install them, or print the main one.
    Skills {
        #[command(subcommand)]
        action: SkillsCmd,
    },
    /// Add a short scriptvox block to this project's AGENTS.md and CLAUDE.md (for any coding agent). Needs --agents.
    Init {
        /// Required: write the block into AGENTS.md and CLAUDE.md (a CLAUDE.md that imports AGENTS.md is not changed). Without it, init changes nothing and stops with a usage error.
        #[arg(long)]
        agents: bool,
    },
    /// Print a JSON Schema: brief, result or event; or `openapi`, the OpenAPI 3.1 document of the planned HTTP server.
    Schema { name: String },
    /// Run the quality gates on a run or an MP4 (black, slideshow, silence, loudness, A/V drift).
    Check {
        target: String,
        /// Also listen: a speech to text row hears the final audio, and gate `words` counts the script words
        /// heard in order (pass from 90 percent). Needs a speech row: the free local one (`scriptvox setup
        /// speech-local`, also with `--local-only`) or your own GPU service with its `selfhost` key.
        #[arg(long)]
        hear: bool,
        /// The script to compare with `--hear` (default: the run's own script).
        #[arg(long)]
        script: Option<PathBuf>,
        /// Language of the speech for `--hear` (default: the run's language, else detected).
        #[arg(long)]
        lang: Option<String>,
        /// Stop reading after this many seconds (1 to 3600) and report on the part that was read: for a long file
        /// under an agent's tool time limit. The result is `partial`, and the loudness is not measured.
        #[arg(long)]
        budget: Option<f64>,
        /// Also compare the frame shape, length and file size with this platform's limits: youtube-shorts, youtube,
        /// instagram-reels, tiktok. The finding says when the limits were read and from where.
        #[arg(long)]
        platform: Option<String>,
        /// Fail when a number or date of the run's script is in no entry of its `sources`. Gate `claims`, exit 3.
        /// Without this flag it is a warning. A run with no `sources` has no such gate.
        #[arg(long)]
        strict: bool,
    },
    /// Write one PNG an agent can read: frames at `--at` times, or a `--sheet N` contact sheet.
    Snapshot {
        target: String,
        /// Comma-separated seconds, for example 1,5.
        #[arg(long)]
        at: Option<String>,
        /// Evenly spaced frames in a grid.
        #[arg(long)]
        sheet: Option<u32>,
        /// One frame per scene card of a run, taken once the card has landed (times from the run's cards.json).
        #[arg(long, conflicts_with_all = ["at", "sheet"])]
        cards: bool,
        /// Shade the zones this platform's own screen covers: instagram-reels or tiktok.
        #[arg(long)]
        safe: Option<String>,
        /// Where to write the PNG (default: next to the video).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List the HTTP providers with their output terms and key status. `providers test <row>` runs the row's smallest call. Each video row in `generators` has `takes_start_picture`: only a row with `true` can make a scene that starts from a picture (a brief with `character`, or a scene with `generate.image` or `first_frame`).
    Providers {
        #[command(subcommand)]
        action: Option<ProvidersCmd>,
        /// Only rows of this kind: video, image, music, sfx, tts, stt, upscale, interpolate, lipsync, matte, stems.
        #[arg(long)]
        kind: Option<String>,
        /// Only rows that can run now.
        #[arg(long)]
        ready: bool,
        /// Only rows with no charge.
        #[arg(long)]
        free: bool,
    },
    /// List the tools a plan can use (engine, TTS, stock), with cost and network.
    Tools,
    /// Model files already on this machine, so setup need not download them.
    Models {
        #[command(subcommand)]
        action: ModelsCmd,
    },
    /// Provider keys: set one, or list names and sources (never values).
    Keys {
        #[command(subcommand)]
        action: KeysCmd,
    },
    /// Check this machine: tools, fonts, keys (by name), network and TLS.
    Doctor {
        /// Run the safe fixes (download the pinned tools).
        #[arg(long)]
        fix: bool,
        /// Hide home paths and the user name, for pasting into an issue.
        #[arg(long)]
        redact: bool,
        /// Skip the network and TLS checks.
        #[arg(long)]
        offline: bool,
    },
    /// Remove ScriptVox data (tools, worker, cache). Projects are not touched.
    Uninstall {
        /// Really delete. Without it, only show what would go.
        #[arg(long)]
        purge: bool,
    },
    /// Download the pinned tools (FFmpeg, uv) now, instead of on the first run.
    Setup {
        /// `voice-local`: the opt-in local English voice (Kokoro; includes GPL-3.0 parts). `voice-rust`: the same voices in pure Rust on the CPU, no GPL part (about 332 MB), used with `--tts kokoro-rust`. `speech-local`: opt-in local
        /// speech to text, free on the CPU (faster-whisper, about 490 MB to download, about 880 MB on disk), for captions, fillers and listening checks; with `--arabic` also an Arabic model with fewer word errors in Arabic. `speech-large`: the larger model (large-v3-turbo, about 1.6 GB, better Arabic), used when 3 GB of memory is free. `image-local`:
        /// opt-in local pictures, free on your own hardware (FLUX.2 klein 4B through sd.cpp, about 5 GB).
        /// `mcp`: add scriptvox to an MCP client's config (with `--client`), with a backup copy first; other entries stay.
        /// `fetch`: show the notice of `scriptvox fetch` and turn that command on; it downloads nothing.
        what: Option<String>,
        /// Show what would be downloaded or changed, and do nothing.
        #[arg(long)]
        dry_run: bool,
        /// Ignored, kept for one version: `setup voice-local` shows its GPL-3.0 notice and installs.
        #[arg(long, hide = true)]
        accept_gpl: bool,
        /// With `mcp`: the client whose config gets the scriptvox entry: claude-desktop, cursor or codex.
        #[arg(long)]
        client: Option<String>,
        /// With `mcp`: the client's config file, in place of its usual path.
        #[arg(long)]
        config: Option<PathBuf>,
        /// With `speech-local`: also install the Arabic model (Whisper small fine-tuned for Arabic, about 485 MB), used only when the language is `ar`.
        #[arg(long)]
        arabic: bool,
        /// With `speech-local --arabic`: a local folder with the 5 model files (config.json, model.bin, preprocessor_config.json, tokenizer.json, vocabulary.json), checked by sha256, in place of the download.
        #[arg(long, requires = "arabic", value_name = "FOLDER")]
        from: Option<PathBuf>,
    },
    /// Index your own media for `make --media`.
    Media {
        #[command(subcommand)]
        action: MediaCmd,
    },
    /// List the voices, with their cost and terms. `--preview <voice id>` writes one short audio file of a voice, to hear it before a video.
    Voices {
        /// A language (ar, en): the list shows only its voices, and a preview speaks in it.
        #[arg(long)]
        lang: Option<String>,
        /// Hear this voice (an id from the list): writes one short audio file and makes no video. A free voice runs at once; a paid voice stops with a plan and needs `--approve <plan_id>`.
        #[arg(long, value_name = "VOICE_ID")]
        preview: Option<String>,
        /// With `--preview`: the words to hear, at most 200 characters. Default: the first sentence of `--brief`, else a built-in sentence in the voice's language.
        #[arg(long, requires = "preview", conflicts_with = "brief")]
        text: Option<String>,
        /// With `--preview`: a brief; the voice reads its first sentence as `make` would (pause tags out, `say` forms in).
        #[arg(long, requires = "preview")]
        brief: Option<PathBuf>,
        /// With `--preview`: the engine, when two engines have the same voice id (kokoro, kokoro-rust).
        #[arg(long, requires = "preview")]
        tts: Option<String>,
        /// With `--preview`: where to write the audio (default: scriptvox-out/voice-<id>.<ext>, never over a different file).
        #[arg(long, requires = "preview")]
        out: Option<PathBuf>,
        #[command(subcommand)]
        action: Option<VoicesCmd>,
    },
    /// Show the runs in this project, or one run with its last events.
    ///
    /// For one run: `alive` (true while its process works), and while it has no result `stage` (its last progress
    /// line) and `idle_s` (seconds since its last event). `next` is `wait` for a live run and `resume` for a dead make.
    Status { id: Option<String> },
    /// One video per row: a brief with {{key}} placeholders and a JSON list of rows that fill them.
    Batch {
        /// The brief with placeholders such as {{name}}.
        #[arg(long)]
        brief: PathBuf,
        /// A JSON list of objects, one per video, for example [{"name": "Al Ain"}], or a CSV file whose first line names
        /// the columns. A row whose video exists from the same filled brief is skipped.
        #[arg(long, required_unless_present = "describe")]
        rows: Option<PathBuf>,
        /// Make nothing: print the brief's {{keys}}, one sample row (JSON and a CSV header line), and whether the
        /// brief is valid once every key has a value.
        #[arg(long, conflicts_with = "rows")]
        describe: bool,
        /// Rows at the same time: 1 (default) or 2.
        #[arg(long, default_value_t = 1)]
        jobs: usize,
    },
    /// Serve the commands as MCP tools over stdio (for Claude Desktop, Cursor, Codex and other MCP clients).
    Mcp {
        /// A folder that the tools may read. Give it more than once for more folders. With either root flag, a path outside them is refused.
        #[arg(long)]
        read_root: Vec<PathBuf>,
        /// A folder that the tools may read and write. With a root flag and no write root, a tool that writes is refused.
        #[arg(long)]
        write_root: Vec<PathBuf>,
    },
    /// One folder to publish from: video, SRT, cover image (plus one per scene card, at most 3), credits (when stock or music needs them) and
    /// metadata.json (title, description, the terms of every part, `commercial_ok`, chapters from the scenes, and a
    /// title or caption per platform inside its limits). The video gets the chapter marks inside. Nothing is uploaded.
    Export {
        /// A finished run id, or a video file (from `edit` or `join`; its terms are not known, so `commercial_ok` is null).
        run: String,
        /// Where to write (default: scriptvox-out/<run>-export).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Also write `preview.gif`: the first 6 s, 360 px wide, 12 fps, one palette for the clip (for a README,
        /// a chat or an issue where video does not play).
        #[arg(long)]
        gif: bool,
        /// The title for metadata.json and the platform texts. Default: the hook, else the script's first clause.
        #[arg(long)]
        title: Option<String>,
        /// Hashtags for the platform texts in metadata.json, comma separated, without '#' (`abudhabi,islands`).
        /// Each platform gets as many as it allows. Nothing is uploaded.
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// More copies of the video, comma separated: webm (VP9), av1, hevc, prores (for an editor), hls (a playlist
        /// with 6 s parts).
        #[arg(long, value_delimiter = ',')]
        format: Vec<String>,
        /// Also write the captions and cards alone on a transparent frame, for an editor: overlay.mov (ProRes 4444)
        /// and overlay.webm (VP9 with alpha). Needs a run id.
        #[arg(long)]
        overlay: bool,
        /// Also write the video as an editor's timeline, comma separated: fcpxml (Final Cut Pro, DaVinci Resolve) or
        /// otio (OpenTimelineIO: Kdenlive, Resolve). The picture is cut at its shot changes; the sound is the stems
        /// with --stems, else the video's own.
        #[arg(long, value_delimiter = ',')]
        timeline: Vec<String>,
        /// Also write the sound parts as WAVs of the video's length: narration.wav, and music.wav (the bed before its
        /// level and ducking). Needs a run id.
        #[arg(long)]
        stems: bool,
        /// Also write a copy of at most this many MB (1 MB = 1,000,000 bytes) for a chat or mail limit: two passes at the
        /// bitrate that fits the true length, then the size is measured. Refused when the picture would get under 150
        /// kbit/s.
        #[arg(long, value_name = "MB")]
        max_mb: Option<f64>,
        /// Also write video-subs.mp4 with soft caption tracks a player can switch, no re-encode, comma separated: `en`
        /// is this run's own captions, `ar=ar.srt` adds an SRT file in that language. The first is the default.
        #[arg(long, value_delimiter = ',', value_name = "LANG[=FILE]")]
        soft_subs: Vec<String>,
        /// Make the cover with an image row from this text (what the cover shows), in the video's shape. `cover.jpg`
        /// is then this picture and the frame cover is `cover-frame.jpg`. A free row runs at once; a paid row stops
        /// with a plan and needs `--approve <plan_id>` (`--plan` shows the price). The same text and row again comes
        /// from the store: no call, no charge.
        #[arg(long, value_name = "TEXT")]
        cover_prompt: Option<String>,
        /// The image row for --cover-prompt: a name from `scriptvox providers --kind image`, or `free`, `cheapest`
        /// (default) or `best`, as `generate --model`.
        #[arg(long, value_name = "ROW", requires = "cover_prompt")]
        cover_model: Option<String>,
        /// Ask each `sources` address one time and name the dead ones. Finding `links`; it needs the network. A dead
        /// link is a warning and does not fail the export.
        #[arg(long)]
        check_links: bool,
    },
    /// Read a video like an editor: shots, pace, loudness, one frame per shot; `--transcript` adds the words.
    Analyze {
        /// The video to read.
        input: PathBuf,
        /// Scene change level that counts as a cut, measured at 4 frames a second (0 to 1; lower finds more cuts).
        #[arg(long, default_value_t = 0.25)]
        threshold: f64,
        /// Also transcribe the speech with a speech to text row (needs a row: `scriptvox providers`).
        #[arg(long)]
        transcript: bool,
        /// Also find the beat grid of the sound: `beats` in the result has the tempo, the beat times and `steady`
        /// (false for speech alone or a tempo that changes). No row and no network: FFmpeg decodes, the rest is local.
        #[arg(long)]
        beats: bool,
        /// With --transcript: who speaks, `S1`, `S2` on each word and sentence. Needs a row that tells speakers apart
        /// (`scribe-fal`, paid, after `--approve`); no free one yet.
        #[arg(long, requires = "transcript")]
        speakers: bool,
        /// Language of the speech (ar, en); default: detected.
        #[arg(long)]
        lang: Option<String>,
        /// Write a starter brief in this video's style to this file ("make one like this"): shape, rate, pace, scene
        /// breaks, and, from your rows, the words as a guide, the look and a shot prompt per scene.
        #[arg(long, value_name = "BRIEF")]
        like: Option<PathBuf>,
        /// A JSON file of words speech to text must get right: `{"terms": ["ACME"], "fix": {"ak me": "ACME"}}`.
        /// Goes as a hint to the row, and each change is listed in `term_fixes`.
        #[arg(long, value_name = "FILE")]
        terms: Option<PathBuf>,
    },
    /// Score frames against the brief with a vision model (your own service, `SCRIPTVOX_VISION_URL`); exit 3 on a weak frame.
    Judge {
        /// A video file or a finished run id.
        target: String,
        /// The brief the video should follow (default: the run's own request).
        #[arg(long)]
        brief: Option<PathBuf>,
        /// Evenly spaced frames to score (2 to 12).
        #[arg(long, default_value_t = 6)]
        frames: u32,
        /// An OpenAI-compatible base URL ending in /v1. Default: SCRIPTVOX_VISION_URL or the file vision-url in the config
        /// folder. Frames of the video are sent there; a key comes from `scriptvox keys set vision`.
        #[arg(long)]
        endpoint: Option<String>,
        /// The vision model at that endpoint.
        #[arg(long, default_value = crate::judge::DEFAULT_MODEL)]
        model: String,
        /// A frame under this score (1 to 5) fails.
        #[arg(long, default_value_t = 3)]
        min_score: u8,
    },
    /// Edit an existing video: cut pauses, crop, captions (local and free); a new background, a cut-out or an upscale
    /// go to a row and show a plan.
    Edit {
        /// The video to edit.
        input: PathBuf,
        /// Remove pauses longer than --min-silence.
        #[arg(long)]
        cut_silence: bool,
        /// Remove parts where the picture stands still for --min-silence or more (a screen recording): with sound, only
        /// where it is silent too.
        #[arg(long, conflicts_with = "cut_silence")]
        cut_still: bool,
        /// Shortest pause to cut, in seconds.
        #[arg(long, default_value_t = 0.6)]
        min_silence: f64,
        /// Air kept at each cut, in seconds.
        #[arg(long, default_value_t = 0.15)]
        pad: f64,
        /// Sound below this level counts as silence (dB).
        #[arg(long, default_value_t = -35.0, allow_hyphen_values = true)]
        noise_db: f64,
        /// Crop to 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9 around the centre.
        #[arg(long)]
        reframe: Option<String>,
        /// With --reframe: `blur` keeps all of the picture over a blurred copy, and nothing is cut. Default: `crop`.
        #[arg(long, value_name = "FIT")]
        fit: Option<String>,
        /// With --reframe: follow the subject instead of the centre. Needs a vision service (`SCRIPTVOX_VISION_URL`).
        #[arg(long)]
        track: bool,
        /// With --reframe on a ScriptVox MP4: crop anyway. Without it the edit refuses, because the crop cuts the
        /// burned captions and cards; make the video in the new frame instead (`resume` with "aspect").
        #[arg(long)]
        crop_text: bool,
        /// With --captions: a JSON file of words speech to text must get right (see `analyze --terms`).
        #[arg(long, value_name = "FILE")]
        terms: Option<PathBuf>,
        /// Cut "um", "uh", "erm" and Arabic "اممم", "ااا" (and "يعني" between pauses) with the pause around each,
        /// from the video's own speech (needs speech to text: `scriptvox setup speech-local`, or a speech row). Give --lang ar for Arabic speech. Not with --keep
        /// or --cut-silence.
        #[arg(long)]
        cut_fillers: bool,
        /// With --track: what to follow, for example "the camel".
        #[arg(long)]
        subject: Option<String>,
        /// Keep only these parts, in time order (a later part cannot come first): "12.3-30.5,41-58.2" (seconds). Cut a talk by its transcript:
        /// pick sentences from `analyze --transcript`. Not with --from/--to or --cut-silence.
        #[arg(long)]
        keep: Option<String>,
        /// Cut by words: copy the `transcript.file` of `analyze --transcript`, delete words from its `words` list, and
        /// give it here. The rest is kept. Each removed word is listed. Not with --keep, --from/--to, --cut-silence or
        /// --cut-fillers.
        #[arg(long, value_name = "FILE")]
        keep_words: Option<PathBuf>,
        /// Hide spoken words, with commas between them: "word,two words". Each time the speech says one, that stretch
        /// of sound (the word, 0.05 s before it and 0.2 s after it) is replaced, and the length stays. A phrase is words said in a row.
        /// Case and punctuation do not count; another form ("words") is another entry. With --captions the word is
        /// `****`. The result lists each match, and warns for an entry that was not heard; when none was heard nothing
        /// is written (exit 3). Needs speech to text, or --words. Not with a cut, --speed, --duration, --from/--to or
        /// --lipsync: use two calls. Listen to the result before a release.
        #[arg(long, value_name = "WORDS")]
        bleep: Option<String>,
        /// With --bleep: `tone` (default, 1 kHz, a little under the voice) or `silence`. The word is not mixed under it.
        #[arg(long, value_name = "SOUND")]
        bleep_with: Option<String>,
        /// With --bleep: seconds of air replaced on each side of a word, 0 to 1. Default 0.05 before and 0.2 after: a
        /// speech row's word edges are not exact. Raise it when a piece of the word is still heard; lower it when the
        /// words next to it suffer.
        #[arg(long, value_name = "SECONDS")]
        bleep_pad: Option<f64>,
        /// With --bleep: the heard words with their times, instead of speech to text: the `transcript.file` of
        /// `analyze --transcript`, or JSON with `words` (text, start, end). --captions then spell these words too.
        #[arg(long, value_name = "FILE")]
        words: Option<PathBuf>,
        /// Captions from a file on its own times: SRT, or JSON with `words` (text, start, end). No speech to text, so no
        /// sound leaves. Implies --captions karaoke. Not with a cut, --speed, --text or --terms; with --from/--to, the words of that part.
        #[arg(long, value_name = "FILE")]
        captions_file: Option<PathBuf>,
        /// With --captions: a text file with the words of the speech. The captions spell these, on the heard times. Gate
        /// `words` says how much was heard. After a cut, give only the words that stay.
        #[arg(long, value_name = "FILE")]
        text: Option<PathBuf>,
        /// A clip or picture over the talk, repeat for more: "road.mp4@12.0:4" (the file, seconds of the result, how long).
        /// It fills the frame. The talk's sound stays, and new captions stay on top. Not with a cut or --speed: cut first.
        #[arg(long, value_name = "SPEC")]
        broll: Vec<String>,
        /// A slow push in with ease, repeat for more: "12-15@1.2" or "12-15@1.2,0.3,0.4" (seconds of the result, zoom 1.05 to
        /// 2, centre as shares of the frame). Captions stay still on top. Not with a cut or --speed: cut first.
        #[arg(long, value_name = "SPEC")]
        zoom: Vec<String>,
        /// A small push in (1.12x) on each --emphasis word. Needs --emphasis and --captions.
        #[arg(long)]
        zoom_on_emphasis: bool,
        /// Lower the noise under a voice: light, medium or strong. Reports `noise_floor_db_before` and `_after`. Start
        /// with light: a strong level can thin a voice, and a measure cannot hear that.
        /// Every level also finds mains hum (50 or 60 Hz and harmonics) and low rumble (under 46 Hz), removes only
        /// what it found (a notch per harmonic, a 60 Hz high-pass), and reports it: `clean_voice.hum` and `.rumble`
        /// with dB before and after.
        #[arg(long, value_name = "LEVEL")]
        clean_voice: Option<String>,
        /// Cut a sentence said again close by: the last whole take stays. `--plan` lists each removal and renders
        /// nothing. A warning names the words where two takes differ. Not with --keep, --keep-words, --from/--to,
        /// --cut-silence or --cut-fillers.
        #[arg(long)]
        cut_retakes: bool,
        /// Play faster or slower: 0.5 to 2 (1.25 is a quarter faster). The voice keeps its pitch. --captions are heard
        /// after it. Or only a part: "12-15@0.5" (seconds of the video after any cut, then the speed; commas for more,
        /// at most 8) with an ease in and out of 0.4 s; caption files beside the input (.srt, .vtt) are written again
        /// beside the result on the new times.
        #[arg(long, value_name = "SPEED|FROM-TO@SPEED")]
        speed: Option<String>,
        /// Blur a fixed box, repeat for more: "x,y,w,h@12-15", pixels and seconds of the source (faces, plates, a
        /// screen). The result has `blur_check`, a frame from each box. A blur is not proof of redaction.
        #[arg(long, value_name = "BOX@FROM-TO")]
        blur: Vec<String>,
        /// The length of the result in seconds: scriptvox sets the speed (kept length over this), 0.5 to 2. Same rules
        /// as --speed; after any cut.
        #[arg(long, conflicts_with = "speed")]
        duration: Option<f64>,
        /// Keep only from this second (a clip out of a long video).
        #[arg(long)]
        from: Option<f64>,
        /// Keep only up to this second.
        #[arg(long)]
        to: Option<f64>,
        /// Burn word-timed captions from the video's own speech (needs speech to text: `scriptvox setup speech-local`, or a speech row): `karaoke` (default, the same as in a brief), `pop` or `reveal`.
        #[arg(long, num_args = 0..=1, default_missing_value = "karaoke")]
        captions: Option<String>,
        /// Language of the speech for --captions (ar, en); default: detected.
        #[arg(long)]
        lang: Option<String>,
        /// A headline shown big for the first 1.6 s: the hook of a clip cut from a long video.
        #[arg(long)]
        hook: Option<String>,
        /// A small line in the top left corner for the whole video, for example "Made with AI". The words are yours: scriptvox makes no claim that a rule is met.
        #[arg(long)]
        label: Option<String>,
        /// Words of the speech to mark in --captions, comma separated: the accent colour and a size up.
        #[arg(long, requires = "captions")]
        emphasis: Option<String>,
        /// A small second caption line under the captions, for example a translation: a text file, one line per
        /// sentence of the speech, in order. Needs --captions (or --captions-file).
        #[arg(long, value_name = "FILE")]
        captions_second: Option<PathBuf>,
        /// Add a music bed under the sound; the speech ducks it: `calm`, `pulse`, `library:<name>`, a music file inside this project, or
        /// an https address (needs `scriptvox setup fetch`; one download, kept for a rerun; its source goes to
        /// `<out>.credits.txt`). Any song can be used: the choice and the responsibility are yours.
        #[arg(long)]
        music: Option<String>,
        /// Start of the cut of music from an address (seconds or 0:12).
        #[arg(long, requires = "music")]
        music_from: Option<String>,
        /// End of that cut.
        #[arg(long, requires = "music")]
        music_to: Option<String>,
        #[command(flatten)]
        caption_style: crate::caption_style::Style,
        /// How loud the bed is against the speech: `quiet`, `normal` (default) or `loud`.
        #[arg(long, requires = "music")]
        music_level: Option<String>,
        /// A sound effect at a set time, repeat for more (at most 32): "whoosh@12.0". NAME is `whoosh`, `pop`, `tick`
        /// or a sound file inside this project; the time is seconds of the result. A whoosh peaks at its time (it
        /// starts 0.4 s before); the others start at it. The length stays; alone, the picture is copied.
        #[arg(long, value_name = "NAME@SECONDS")]
        sfx: Vec<String>,
        /// How loud each effect is: its peak is 18 (`quiet`), 12 (`normal`, default) or 6 (`loud`) dB under the peak
        /// of the video's own sound.
        #[arg(long, requires = "sfx")]
        sfx_level: Option<String>,
        /// Crop the black bars that are found on the picture (letterbox or pillarbox), measured on 2 frames a second.
        /// The answer's `bars` gives the crop. No bars: a usage error, and nothing is written. This flag and the next
        /// five work with each other only (order: deinterlace, bars, tilt, hold, loop); other options need their own call.
        #[arg(long)]
        remove_bars: bool,
        /// Hold a frame, repeat for more (at most 16): "4.0@2" keeps the frame at 4.0 s of the source for 2 s more
        /// (at most 60), with silence under it. The video gets longer.
        #[arg(long, value_name = "AT@SECONDS")]
        hold: Vec<String>,
        /// Play the video this many times, 2 to 100 (the result is at most 600 s).
        #[arg(long, value_name = "TIMES", conflicts_with = "loop_to")]
        r#loop: Option<u32>,
        /// Repeat the video up to this length in seconds; the last play is cut there.
        #[arg(long, value_name = "SECONDS")]
        loop_to: Option<f64>,
        /// Make an interlaced video progressive (bwdif; the frame rate and the length stay). A video that is not
        /// interlaced by measurement is refused. The answer's `deinterlace` gives the field counts before and after.
        #[arg(long)]
        deinterlace: bool,
        /// Turn the picture by this many degrees, -15 to 15 (a minus turns it to the left), for a horizon that is not
        /// level. The picture is zoomed in just enough that no corner is empty; the frame size stays.
        #[arg(long, value_name = "DEGREES", allow_hyphen_values = true)]
        tilt: Option<f64>,
        /// A new voice file: a lip sync row moves the mouth in the video to it, before the other steps. Sends the video
        /// and the voice to that row.
        #[arg(long)]
        lipsync: Option<PathBuf>,
        /// A logo (PNG, transparent background) in the top right corner, 16 percent of the width.
        #[arg(long)]
        logo: Option<PathBuf>,
        /// Brand colour for the spoken-word highlight of --captions, as #RRGGBB.
        #[arg(long)]
        accent: Option<String>,
        /// A clean background behind the subject: `#RRGGBB` (a plain colour), `blur` (the same picture, blurred), or a
        /// picture or a clip inside this project (a clip repeats to the length). Sends a small copy of the picture,
        /// with no sound, to the matte row for a mask of the subject; a paid row needs `--plan`, then `--approve`. At
        /// most 60 s. Not with a cut, --from/--to, --speed, --reframe or --lipsync: use two calls.
        #[arg(long, value_name = "COLOUR|blur|FILE")]
        background: Option<String>,
        /// The subject on a transparent background, for an editor: a ProRes 4444 `.mov` (with the sound) and a WebM
        /// with alpha (no sound) beside it. By the matte row's mask, as --background. Alone: no other edit option.
        #[arg(long)]
        alpha: bool,
        /// A small or soft clip made larger and sharper by the upscale row: the same clip at the row's size (the short
        /// side to 1080 on the built-in row), the sound and the length kept. Sends the picture, with no sound, to that
        /// row; `--plan` shows the cost, the size and the time, and a paid row needs `--approve`. At most 10 s. Alone:
        /// no other edit option.
        #[arg(long)]
        enhance: bool,
        /// Where to write the MP4 (default: scriptvox-out/<name>-edit.mp4).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Dub a video on its own times. `--plan` alone prints a sheet of timed lines. The agent writes the new text, then
    /// `--lines` runs voice, fit, captions and gates as one `make`, at one plan and one cost.
    Dub {
        /// The video to dub.
        input: PathBuf,
        /// The new lines: the sheet with its `text` changed, or a list of `{start, end, text}`.
        #[arg(long)]
        lines: Option<PathBuf>,
        #[command(flatten)]
        caption_style: crate::caption_style::Style,
        /// Language of the new lines (ar, en). With `--plan` and no `--lines`: the language of the source's speech.
        #[arg(long)]
        lang: Option<String>,
        /// TTS engine (edge, elevenlabs, kokoro, kokoro-rust, selfhost).
        #[arg(long)]
        tts: Option<String>,
        /// Voice id from `scriptvox voices`.
        #[arg(long)]
        voice: Option<String>,
        /// The source's sound under the voice: none, keep or background. Default: background with a free stem row, else none.
        #[arg(long)]
        base_sound: Option<String>,
        /// Move the mouths to the new voice, on a free lip sync row.
        #[arg(long)]
        lipsync: bool,
        /// With `--plan` and no `--lines`: each line of the sheet says who speaks (`S1`, `S2`), from a row that tells
        /// speakers apart.
        #[arg(long)]
        speakers: bool,
        /// A voice per speaker of the lines, `S1=af_heart,S2=am_michael`: a line with `speaker` gets that voice. Same
        /// engine for all.
        #[arg(long, value_name = "MAP")]
        voices: Option<String>,
        /// A small second caption line under the captions, for example the source's words: a text file, one line per
        /// sentence of the new lines, in order (brief `captions_second`).
        #[arg(long, value_name = "FILE", requires = "lines")]
        captions_second: Option<PathBuf>,
        /// Output file.
        #[arg(long)]
        out: Option<PathBuf>,
        /// A JSON file of terms: a hint for the sheet, listed there as `keep_terms`; with --lines, a warning for each term
        /// not in the new lines (see `analyze --terms`).
        #[arg(long, value_name = "FILE")]
        terms: Option<PathBuf>,
    },
    /// A long talk into ranked shorts (needs speech to text and a vision service): picks the parts that stand alone, writes a hook for each, then cuts, crops and captions them
    Clips {
        /// The long video.
        input: PathBuf,
        /// How many shorts.
        #[arg(long, default_value_t = 3)]
        count: usize,
        /// Shortest short, in seconds.
        #[arg(long, default_value_t = 20.0)]
        min: f64,
        /// Longest short, in seconds.
        #[arg(long, default_value_t = 60.0)]
        max: f64,
        /// Frame shape of the shorts: 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9.
        #[arg(long, default_value = "9:16")]
        aspect: String,
        /// Keep the subject in the crop (sends frames to your vision service, slower).
        #[arg(long)]
        track: bool,
        /// Caption style: karaoke, pop or reveal.
        #[arg(long, default_value = "pop")]
        captions: String,
        #[command(flatten)]
        caption_style: crate::caption_style::Style,
        /// Language of the speech (ar, en); default: detected.
        #[arg(long)]
        lang: Option<String>,
        /// List the parts only: nothing is cut.
        #[arg(long)]
        plan: bool,
        /// A JSON file of words speech to text must get right (see `analyze --terms`).
        #[arg(long, value_name = "FILE")]
        terms: Option<PathBuf>,
    },
    /// Collect facts about a topic with their sources, before the script: writes `research.json`. Opt-in: needs a
    /// text row (`scriptvox connect <name> --url <base> --text --model <id>`); with none it stops with exit 10.
    ///
    /// The text row first gives search terms in one small call (with none, the code takes the key words of the topic).
    /// It asks the search rows (Wikipedia, NASA, Europe PMC, DOAJ and Wikidata: free, no key), reads the pages (main text only; a private
    /// address is refused), skips a page that is not about the topic before any call, and a text row gives facts.
    /// When the search finds no right page: name one with `--url`, or give the topic as a name ("Rayleigh
    /// scattering"), not as a question. The code then checks each fact: one with no exact sentence in
    /// the page text is dropped, and a number or date is `confirmed` only when two publishers give it (`single`: one
    /// publisher; `conflict`: publishers disagree, both values are kept). Each fact has its address, page title,
    /// publisher, page date, date read and sentence. Then: `scriptvox write --from research.json`. An agent with its
    /// own web search can write the same file format by itself (`docs/specs/research.md`).
    Research {
        /// The topic in a few words. For example "why the sky is blue".
        topic: String,
        /// en (default) or ar.
        #[arg(long)]
        lang: Option<String>,
        /// Pages read: quick (3), normal (6) or deep (12).
        #[arg(long, default_value = "normal")]
        depth: String,
        /// The new file (default research.json). It must not exist.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Also read this public https page. Repeat for more, at most 10. It is read with no check for the topic. A
        /// page of another publisher is what lets a fact become `confirmed`.
        #[arg(long = "url", value_name = "ADDRESS")]
        urls: Vec<String>,
        /// The text row (default: the first).
        #[arg(long, value_name = "NAME")]
        text_row: Option<String>,
        /// Search and read only, no facts. Needs no text row and calls no model.
        #[arg(long)]
        pages_only: bool,
    },
    /// Write a brief (a script in scenes) with a text row, from the facts of a research file or from a topic alone.
    /// Opt-in: needs a text row (`scriptvox connect <name> --url <base> --text --model <id>`).
    ///
    /// `--from research.json`: the script uses only the facts of the file, and the brief lists them in `sources`.
    /// The code checks the answer (the brief schema, each fact id, each number against the facts) and asks one more
    /// time when it is not valid. The code owns the length: the word target is the duration times the speaking rate
    /// of the voice, and an answer more than 15 percent off is asked for one more time with its count (at most 3
    /// calls). The model can name a card for a scene (title, stat, quote, bars) by fact id; the code fills it from the
    /// facts and drops one that does not hold. When the facts have a number, the script must say one and a stat or
    /// bars card must show one: the same one more question asks for it, and the finding `cards` says when it is missing. `--topic`: the model writes from what it knows; the brief has no
    /// `sources` and the result says that its facts are not checked. Then: `scriptvox plan --brief brief.json`.
    Write {
        /// A research file. From `scriptvox research`, or one an agent wrote in that format.
        #[arg(long, value_name = "FILE", conflicts_with = "topic")]
        from: Option<PathBuf>,
        /// A topic, with no research. The facts of the script are then not checked.
        #[arg(long)]
        topic: Option<String>,
        /// Seconds, 10 to 600 (default 45).
        #[arg(long)]
        duration: Option<f64>,
        /// en or ar. Default: the language of the research file, else en.
        #[arg(long)]
        lang: Option<String>,
        /// The new brief (default brief.json). It must not exist.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// The text row (default: the first).
        #[arg(long, value_name = "NAME")]
        text_row: Option<String>,
        /// The voice engine that will read the script (edge, elevenlabs, kokoro, kokoro-rust, selfhost, none). Its
        /// speaking rate sets the word target, and the brief gets `tts`. Default: the engine `make` picks here.
        #[arg(long, value_name = "ENGINE")]
        tts: Option<String>,
        /// Put `sources_card` in the brief: a last card with the names of the publishers of the facts.
        #[arg(long)]
        sources_card: bool,
    },
    /// Upload an export folder to your own YouTube channel or Hugging Face repo. Opt-in, private by default, and each
    /// upload stops with exit 4 until the person approves it (`--approve <publish_id>`).
    ///
    /// Order: `scriptvox export <run_id>`, then `scriptvox publish youtube|hf <folder>`. The first call sends nothing
    /// and shows what would go where. An agent must not approve an upload by its own decision: ask the person.
    Publish {
        #[command(subcommand)]
        action: crate::publish::PublishCmd,
    },
    /// Get a video or audio file from an https address: a media file or an HLS playlist. Off until you run `scriptvox setup fetch` one time. You can get a file from any address: the choice and the responsibility are yours. fetch never signs in to a site. When a site refuses the download, save the file yourself and give the local file to `scriptvox analyze <file>`, `scriptvox make --music <file>` or `--media <folder>`.
    #[cfg_attr(feature = "fetch-youtube", command(after_help = crate::fetch_helper::HELP))]
    Fetch {
        /// The https address of a media file or an HLS playlist (.m3u8).
        url: String,
        /// Keep the sound only (copied, not encoded).
        #[arg(long)]
        audio: bool,
        /// Start of a cut: seconds (90.5) or a clock (1:30). The cut is a copy, so it starts at the picture key frame at or before this time.
        #[arg(long, value_name = "T")]
        from: Option<String>,
        /// End of a cut: seconds or a clock.
        #[arg(long, value_name = "T")]
        to: Option<String>,
        /// The file to write. It must not exist. Default: a name from the title, in the current folder.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        #[cfg(feature = "fetch-youtube")]
        #[arg(long, value_name = "HEIGHT", help = crate::fetch_helper::QUALITY)]
        quality: Option<u32>,
    },
    /// Compare 2 videos frame by frame: how alike they look and the times that differ. Picture only, free and local.
    Compare {
        /// The first video. The second is scaled to its size and rate.
        a: PathBuf,
        /// The second video.
        b: PathBuf,
        /// A frame differs above this, 0 to 1 (1 minus the frame's SSIM). Measured: a re-encode stays under the default, a full-width bar is over it, a small tag needs 0.005.
        #[arg(long, default_value_t = 0.02)]
        threshold: f64,
    },
    /// Join clips of any size and rate into one video, locally and free, with a transition and mastered sound
    Join {
        /// The clips, in order (2 or more).
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        /// The transition between clips: fade (default), slide, cut, wipe, push, cover, dissolve, blur, circle, dip,
        /// flash, pixel, radial, wind, zoom, reveal, iris, split, slice or rise.
        #[arg(long)]
        transition: Option<String>,
        /// Frame shape: 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9. Default: the first clip's size.
        #[arg(long)]
        aspect: Option<String>,
        /// A clip of another shape: `pad` (bars, keeps all of the picture; default), `blur` (a blurred copy in place of the
        /// bars) or `crop`.
        #[arg(long, default_value = "pad")]
        fit: String,
        /// Where to write the MP4 (default: scriptvox-out/<first clip>-join.mp4, never over an earlier one).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Make one AI video clip with a generator from `scriptvox providers` (paid: plan, then --approve).
    Generate {
        /// A generator name (kling-2.6-pro), or `free`, `cheapest` or `best`: scriptvox picks the row and says why.
        /// `best` means the highest price among rows that can run; scriptvox has no quality score.
        #[arg(long, default_value = "cheapest")]
        model: String,
        /// With a word as --model: video, image, music, sfx, tts or stt (default video).
        #[arg(long)]
        kind: Option<String>,
        /// What to make: the shot (video, image), the sound (music, sfx), or the text to speak (tts).
        #[arg(long)]
        prompt: Option<String>,
        /// Audio inside this project (wav, mp3, m4a): the input of a speech to text row, or a voice sample to clone.
        #[arg(long)]
        audio: Option<PathBuf>,
        /// A video inside this project (mp4): the input of an upscale, interpolate or matte row.
        #[arg(long)]
        video: Option<PathBuf>,
        /// Voice name for a text to speech row.
        #[arg(long)]
        voice: Option<String>,
        /// Language code (ar, en) for speech rows.
        #[arg(long)]
        lang: Option<String>,
        /// A start image inside this project (png, jpg, webp): image to video. On an image row that takes one, a reference picture.
        #[arg(long)]
        image: Option<PathBuf>,
        /// Clip length in seconds; the nearest length the model allows is used.
        #[arg(long)]
        seconds: Option<f64>,
        /// Frame shape: 9:16 (default), 16:9 or 1:1.
        #[arg(long, default_value = "9:16")]
        aspect: String,
        /// Where to write the result (default: scriptvox-out/).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Seconds to wait for the provider; after that, run the same command again to resume the same job.
        #[arg(long, default_value_t = 900)]
        wait_s: u64,
        /// A seed: the same seed and input give the same picture on a row that takes one (`takes_seed` in
        /// `scriptvox providers`). The plan says when the row does not.
        #[arg(long)]
        seed: Option<u64>,
        /// The picture the clip ends on, inside this project (png, jpg, webp). Needs --image and a row with
        /// `takes_last_frame` in `scriptvox providers`.
        #[arg(long)]
        last_image: Option<PathBuf>,
        /// Return at once with the run id; the job continues in the background. Then `scriptvox wait <run_id>`.
        #[arg(long)]
        detach: bool,
        /// Internal: the detached supervisor attaches to this run.
        #[arg(long, hide = true)]
        run_id: Option<String>,
    },
    /// Render a short test video: no keys, no Python, no Node.
    Demo {
        /// Where to write the MP4 (default: scriptvox-demo.mp4 in this folder).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Small, short render for golden tests.
        #[arg(long, hide = true)]
        golden: bool,
    },
    /// Start the Python worker, check the protocol and list its tools.
    #[command(hide = true)]
    Worker,
}

#[derive(Debug, Subcommand)]
pub enum ModelsCmd {
    /// Look in a folder for model files that a setup target would download. A match is by the size and the sha256 of the
    /// pin; scriptvox records the path and only reads the file, it never copies, moves or deletes it.
    Scan {
        /// The folder (default: your Hugging Face cache).
        folder: Option<PathBuf>,
    },
    /// Add a model as your own row: a Hugging Face id, a local file or a fal endpoint. The revision is pinned, and size,
    /// sha256 and licence are read where the host publishes them; the licence stays on the row. A file format that can
    /// run code gets a warning. A local file needs one file with a whole SD 1.x, SD 2.x or SDXL model.
    Add {
        /// `org/model`, `./model.safetensors` or `fal-ai/some-model`.
        source: String,
        /// A short name for the row (default: from the source).
        #[arg(long)]
        name: Option<String>,
        /// Which file of a Hugging Face repo, when it has more than one.
        #[arg(long)]
        file: Option<String>,
        /// sd1, sd2 or sdxl, when the file does not say (.ckpt, .gguf).
        #[arg(long)]
        family: Option<String>,
        /// What a fal endpoint makes, when its name does not say: video.gen, image.gen, tts ...
        #[arg(long)]
        kind: Option<String>,
        /// Show what would be downloaded and written, and do nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum MediaCmd {
    /// Check a folder (inside this project) and index its clips and stills.
    Add { dir: PathBuf },
    /// Search open images and sound with no key (Openverse). Each result keeps its own licence. `--get` saves one, with its terms beside it.
    Search {
        /// What to look for.
        query: String,
        /// image or sound.
        #[arg(long, default_value = "image")]
        kind: String,
        /// How many results, 1 to 20.
        #[arg(long, default_value_t = 10)]
        limit: u32,
        /// Save this result (1 is the first) in scriptvox-out/open-media/.
        #[arg(long)]
        get: Option<usize>,
    },
    /// The music library: songs with a traced source and a free licence (CC0). Give one to make as
    /// "music": "library:<name>"; it downloads once and is kept for a rerun.
    Music {
        /// Only this mood: calm, happy, chill, energetic, tense, sad or epic.
        #[arg(long)]
        mood: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ProvidersCmd {
    /// The smallest call this row can do, to prove your key and the row work: shortest length, short prompt, small test
    /// file where it needs one. A paid row goes through plan and --approve. The row's terms are in the result.
    Test {
        /// A row from `scriptvox providers`.
        row: String,
        /// Give up waiting after this many seconds. A rerun picks up the same job.
        #[arg(long, default_value_t = 900)]
        wait_s: u64,
    },
    /// Print a block for your own row in generators.toml: fal, selfhost or openai-speech.
    Template { gateway: String },
    /// Add your own row to generators.toml. A price you do not know stays unknown, and `generate` then needs --max-spend.
    Add {
        /// A short name of a-z, 0-9, '.', '_' or '-'.
        name: String,
        #[arg(long)]
        gateway: String,
        /// video.gen, image.gen, music.gen, sfx.gen, tts, stt ...
        #[arg(long)]
        kind: String,
        /// Text to video endpoint or model name.
        #[arg(long)]
        t2v: Option<String>,
        /// Image to video endpoint.
        #[arg(long)]
        i2v: Option<String>,
        /// The host of a self-hosted or OpenAI speech service (https).
        #[arg(long)]
        base: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum VoicesCmd {
    /// Record that a person agreed to the use of their voice: a hash of the sample and of your proof, with an id. A row that
    /// clones a voice refuses a sample with no record. The tool keeps the hashes, not the files, and does not judge the proof.
    /// Your own voice: `--own` records your statement that the sample is your voice, with no proof file.
    #[command(group(clap::ArgGroup::new("who").required(true).args(["consent", "own"])))]
    Add {
        /// The voice sample.
        #[arg(long)]
        sample: PathBuf,
        /// Your proof of consent for another person's voice: a signed form, a recording, any file.
        #[arg(long)]
        consent: Option<PathBuf>,
        /// The sample is your own voice: records your statement by hash, no proof file.
        #[arg(long)]
        own: bool,
        /// Whose voice this is, for your own list.
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeysCmd {
    /// Store a key in keys.toml (user-only). The value never goes in argv.
    Set {
        /// elevenlabs, pexels, pixabay, fal, vision (your vision service), selfhost (your own GPU service) or hf (a
        /// Hugging Face token, for a model behind an Agree button).
        provider: String,
        /// Read the value from stdin (pipe it), for scripts and agents.
        #[arg(long)]
        stdin: bool,
    },
    /// Show each provider and where its key comes from.
    List,
}

#[derive(Debug, Subcommand)]
pub enum RecipesCmd {
    /// One recipe: `brief` (save it as brief.json), `shape` (script, cards, look) and `adapt` (what to change).
    Show {
        /// A name from `scriptvox recipes`, for example `explainer` or `news-brief`.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillsCmd {
    /// Write the skills to ~/.claude/skills. `--agents`: to ~/.agents/skills, which Codex, Cursor, Gemini CLI, opencode and Pi read, and to the Hermes skills folder when Hermes is installed.
    Install {
        /// Write for the other agents: ~/.agents/skills, and ~/.hermes/skills (or `HERMES_HOME`) when that home is there.
        #[arg(long)]
        agents: bool,
    },
    /// Print the skill (the file in the repo is generated from this).
    Render,
}

/// The parser. Flags about money, plans and engines stay accepted everywhere but only show in the help of the commands
/// that use them, so `check --help` lists what `check` does.
/// The clap tree of `Cli`. In a debug build the derived builder has one very large stack frame, and under a deep call
/// chain on a test thread (2 MB) it overflowed when the command list grew (13 Nov 2026, `mcp` test of the skills). So
/// it is built on a thread with its own stack.
pub fn tree() -> clap::Command {
    use clap::CommandFactory;
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(Cli::command)
        .ok()
        .and_then(|t| t.join().ok())
        .unwrap_or_else(Cli::command)
}

pub fn command() -> clap::Command {
    const SPENDS: [&str; 9] = [
        "make", "generate", "remake", "resume", "batch", "dub", "export", "research", "write",
    ];
    const HIDE: [&str; 6] = [
        "approve",
        "max_spend",
        "idempotency_key",
        "no_embed",
        "plan",
        "local_only",
    ];
    let mut cmd = tree();
    // Hidden flags drop out of the normal list, so these commands get them back as text after it.
    let list: String = cmd
        .get_arguments()
        .filter(|a| HIDE.contains(&a.get_id().as_str()))
        .map(|a| {
            let name = a.get_long().unwrap_or_default();
            let help = a.get_help().map(|h| h.to_string()).unwrap_or_default();
            format!("      --{name}\n          {help}\n")
        })
        .collect();
    let text = format!("Money and engine options:\n{list}");
    for flag in HIDE {
        cmd = cmd.mut_arg(flag, |a| a.hide(true));
    }
    cmd = cmd.after_help(text.clone());
    for name in SPENDS {
        let text = text.clone();
        cmd = cmd.mut_subcommand(name, |s| s.after_help(text));
    }
    cmd
}
