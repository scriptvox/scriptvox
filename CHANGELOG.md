# Changelog

## 0.1.0 (first public release)

Added after the private build of 10 Nov
- macOS preview for Apple silicon and Intel: the install script takes macOS, and FFmpeg and uv come from Homebrew.
- Local speech to text in Arabic: `scriptvox setup speech-local --arabic` downloads an Arabic Whisper model
  (Apache-2.0), used when the language is `ar`.
- Five more languages on the free voice: Spanish, French, German, Portuguese and Indonesian, with caption rules.
  IPA in `say`, delivery cues, and an SSML break as a pause.
- Reels: a video with no words, a pause at the end, beat cuts with music from an address, and any song as music
  with one short notice.
- `research` and `write`: facts from key-less sources, cards built from the facts, a Sources card, and Arabic.
- More cards (table, number line, diagram, steps, gauge, progress), several cards in one scene, `card_style` paper
  for light pages, and five more transitions.
- Twelve starter briefs (`scriptvox recipes`), two of them in Arabic.
- `edit`: remove bars, hold, loop, deinterlace and tilt. Brief sizes from 720 to 2160.
- Long videos render in groups of scenes, so memory does not grow with the number of scenes.
- Captions: one page on screen at a time, and the words of a scene end with its sound.


Renamed
- The project is now ScriptVox (it was Rawivox). The binary is `scriptvox`, variables are `SCRIPTVOX_*`, error codes
  start with `SVX_`, result schemas are `scriptvox.result/v1`, and the folders are `~/.config/scriptvox`,
  `~/.local/share/scriptvox`, `.scriptvox/` in a project and `scriptvox-out/`. Nothing was published under the old
  name, so there is no alias: move the two folders, and set the keys again if you do not.

Fixed in the last checks
- `--stock auto` with no key no longer fails the run when the free sources have nothing for a pair of words.
- `make --detach`, and so the MCP tool, stops at once on a paid plan with no approval.

Made and checked
- Scenes in the brief with their own visual (own clip, photo, stock or a generated clip), cards (title, stat,
  lower third, list, bar chart), hook, outro with logo, brand accent, transitions, `pop` captions.
- `look` colour grades, scene `camera` moves, smooth 24/25 fps clips, short clips slow down instead of freezing.
- Generated clips, images, music, speech, transcripts, upscale and cut-outs through one job runner with resume and
  no double submit; free open models on our own GPUs, paid rows only with approval.
- Sound: generated music beds, `sfx`, scene `ambience` (testing only), dubs that keep the original background
  (`base_sound`), -14 LUFS everywhere.
- Arabic first: right-to-left captions with numbers, Arabic-Indic digits in stats, an open Arabic voice for drafts.
- The same person across scenes (`character`, `characters`, `who`) from one reference picture, stills (`generate.still`), the right file for each line of a folder (`media_match`, `media_from`).
- Key words in the captions (`emphasis`), `reveal` captions, a colour for the second speaker, no-voice videos (`tts: none`), a scene `voice`, picture in picture (`inset`), plain `background`s, brand `font`, `.cube` LUTs in `look`, more cards (formula, plot, code, quote, compare, donut, timeline), 12 transitions, 8 frame shapes.
- `make --draft` and `resume --final`: a fast look first, with the same layout.

Existing video
- `analyze` (cuts, pace, loudness, transcript with sentences), `edit` (cut pauses, `--keep` by transcript,
  `--from/--to`, reframe with subject tracking, captions, logo, -14 LUFS), re-voice and translate over `base_video`.
- `clips` (a long talk into ranked shorts with hooks), `join` (clips of any size), `edit --music`, `--speed`, `--hook`, `--cut-fillers`, `analyze --like` (a starter brief in the style of a video).

Checks and hand-over
- Gates on every frame plus `words` (the voice is heard against the script), `judge` (a vision model scores frames
  against the brief), `export` (a publish folder with `commercial_ok`).

Agents
- Every command prints one JSON envelope; skills for brief, Arabic, prompts, providers, review, edit, sound and
  motion; `scriptvox mcp` for MCP clients.


## 0.0.1 (not released)

- skeleton with result envelope, exit table and push block (RVX-000)
- local licence and quality gates (RVX-001)
- spike: answer the 9 v0.1 risk questions (RVX-002)
- agent-mode guard and key redaction (RVX-003)
- filtergraph path escaping, fontconfig and static fonts (RVX-006)
- caption engine with measured line breaks (RVX-007)
- FFmpeg engine and zero-key demo video (RVX-004)
- RVX-RPC host and embedded Python worker (RVX-009)
- Edge TTS, voice policy and the first make --script (RVX-010)
- pause tags with exact silence, Arabic keywords and digits (RVX-011)
- duration fitter with TTS-rate retry and pre-TTS check (RVX-012)
- encoder probe, BT.709, atomic output, aspects and templates (RVX-005)
- own media as the shot list, media add, path-root check (RVX-013)
- run store with state.db placement, events and status (RVX-015)
- content-addressed cache with verify-on-read, TTS reuse (RVX-014)
- detach, wait, cancel and status for runs (RVX-016)
- chunked render with watchdog, resume and stream-copy stitch (RVX-017)
- pinned toolchain provisioner, setup and download limits (RVX-018)
- doctor with fix_safe checks, mode_ready, redact; uninstall (RVX-019)
- provider keys from env or a user-only keys.toml, keys set --stdin (RVX-020)
- stock video rows (Pexels, Pixabay) with fallback, cache and credits (RVX-021)
- providers and tools self-discovery with snapshots (RVX-022)
- synthesized music beds, ducking and two-pass loudness master (RVX-023)
- quality gates over every frame, check and snapshot (RVX-024)
- briefs, schema command, schema freshness and compat gates, 6 recipes (RVX-025)
- plans with plan_id, approval above zero cost, ledger spend cap, idempotency (RVX-026; its `--commercial` flag was removed on 5 Nov 2026)
- ElevenLabs paid TTS through ledger.reserve, keys only by env (RVX-028)
- resume --brief child runs, plan alias, exact exit replay; §4 session test (RVX-027)
- embedded recipe, receipt, explain, replay (cache only) and remake (RVX-030)
- generated Claude skill, plugin and marketplace, init --agents, README, AGENTS, CREDITS (RVX-032)
- plan-time text lint for font coverage and broken Arabic (RVX-008)
- scriptvox licenses with embedded cargo-about notices and components (RVX-031)
- bench command, release-check gate, launch scripts (RVX-033, partial)
