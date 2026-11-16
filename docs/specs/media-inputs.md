# Media inputs: a file is opened only as what its first bytes say (11 Nov 2026)

Why: FFmpeg guesses a file's format from its content. A file named `x.png` or `x.mp4` that is really a list or a script (ffconcat, an HLS `.m3u8`, DASH, an SVG) makes FFmpeg open the other local paths or URLs that the list names. The file can come from the user (a brief, a flag, a media folder) or from a service (a row's result, a stock clip). The first fix was for `edit --background` and the mask (`edit-background.md`). A review found the same fault at every other input. This is the one rule for all of them. Code: `crates/scriptvox/src/media_gate.rs`.

## What is accepted

ScriptVox reads the first 4096 bytes itself. The name's ending decides nothing.

- A picture: PNG, JPEG, WebP, BMP.
- A video: the mov family (MP4, MOV, M4V, 3GP), Matroska and WebM, AVI, MPEG-TS.
- A sound: WAV, MP3 (an ID3 tag or a frame start), FLAC, OGG and Opus, AAC in ADTS frames. M4A is the mov family.

FFmpeg and ffprobe then get the file with `-protocol_whitelist file` and the format named: `-f mov`, `matroska`, `avi`, `mpegts`, `wav`, `mp3`, `flac`, `ogg`, `aac`. A picture gets `-f image2 -pattern_type none` and its decoder (`-c:v png`, `mjpeg`, `webp`, `bmp`). So no format is guessed, no number pattern is read, and no network, `concat:` or `subfile:` protocol can be reached.

## What is refused

Everything else: an ffconcat list, an HLS playlist, an XML file (DASH, SVG), plain text, a URL, a `concat:` value, a name with a number pattern, and any format not in the list (GIF, TIFF, AIFF). No feature takes a GIF as an input today (`export --gif` only writes one), so GIF is not in the list.

- A file from the user: exit 2 before FFmpeg, ffprobe, a plan or a row is given anything. The message names the file, what it is ("an ffconcat list", "an HLS playlist"), and the accepted kinds.
- A file from a service: exit 1, the file is removed, and the message names the service and what the file is. A paid job stays on record, so the same command fetches the same result and buys nothing. A free row's job is forgotten, so the same command asks the row again (the rule of the mask in `edit-background.md`).

## Where the check runs

1. At the start of a command (exit 2, `media_gate::refuse`):
   - `make` and `plan`: the scene `media`, `overlay`, `inset`, a `generate` scene's `image` and `last_frame`, `logo`, `character` and `characters` files, `narration`, `base_video`, a `music` file, each clip or photo in the `media` folder, and the cutaway files and their base video. The files that a recipe from an MP4 names go the same way.
   - `edit`: the video, `--music`, `--logo`, `--lipsync`, each `--broll` file. `--background` keeps its own narrower check (a picture or a clip, inside the project).
   - `check`, `snapshot`, `judge`, `analyze`, `compare`, `join`, `clips`, `dub`, `export <file>`. In `judge` the check comes after the check for a vision service (exit 10 stays first).
2. After a download (exit 1, `media_gate::downloaded`): the result of a queue row (`genjob`), a stock clip (`make`), a free picture or sound (`media search --get`). The results of the connected rows (ComfyUI, OpenAI-compatible pictures, speech) come as bytes and keep `genjob`'s own older, narrower checks: an MP4, a PNG, JPEG or WebP, a WAV.
3. In the FFmpeg wrapper, as the last line of defence (`media_gate::guard` in `Ffmpeg::run` and `run_watched`): every `-i` with no `-f` of its own is checked and gets the arguments above. A refused file stops the call. Every ffprobe helper (`duration`, `readback`, `has_stream`, `shown_size`, `hdr_kind` and the rest) does the same, and so do the direct FFmpeg calls of the gates, `analyze`, `beats`, `compare` and `edit`. So a path that step 1 or 2 missed still cannot reach FFmpeg's own guess; it fails with exit 1 or the command's own message, not with exit 2. Known paths that have only this step: the MP4 whose recipe `explain`, `remake` and `make --from` read (exit 1), `analyze --like`, the sample of `voices`, `generate --audio` and `--video` where a length is measured, the video of a dub brief (`dub::brief`), and a free picture that `make` fetches for a scene's `open_image`.

## What is trusted

- An input for which ScriptVox names the format itself (`-f lavfi`, `-f concat` for the chunk list it writes in the run folder, `-f s16le` and other raw streams, `-f ffmetadata` for the chapters it writes). The wrapper leaves these as they are. The chunks in that list are files this run rendered.
- Files that ScriptVox made (rendered scenes, mixes, masters, frames) are plain media, so the same check passes them and adds the same arguments. It costs one 4 KB read. A file from an earlier run, from the cache or from the store is checked like any other at the point of use.
- ASS caption files go to the `ass` filter (libass), not to an FFmpeg input.
- The subtitles of `export --soft-subs` are text from the user. ScriptVox decodes them and writes its own copy, and FFmpeg now reads that copy with `-f srt` and nothing else.

## What changed for valid files

- Nothing for the formats in the list, by test: PNG, JPEG, MP4, WAV and MP3 through `make`, `edit`, `check` and `snapshot`.
- A file whose name and content differ (a PNG named `.jpg`) is opened as what it is.
- An input in a format outside the list was opened before and is refused now: GIF, TIFF, AIFF, WMA, FLV, MPEG-PS, a WAV in RF64 or W64 form, an MP3 that starts with junk before the first frame, a sound file with an ID3 tag that is not MP3. The `media` folder took only the endings mp4, mov, mkv, webm, m4v, avi, jpg, jpeg, png, webp, bmp, which are all in the list.
- `media search --get` and a stock clip: a service that answers with a format outside the list (Commons can give a TIFF, a GIF or an SVG) is now exit 1 with the file removed. Before, the file was kept, and FFmpeg failed or guessed later.
- `export --soft-subs lang=file`: the file must be SRT (the option's own words). A WebVTT or ASS file there was guessed before and is refused now.

## Tests

`tests/media_gate.rs` and two tests in `tests/generate.rs`, with an FFmpeg and ffprobe that write every argument list to a file (`common::record`). All six failed first on the code before the fix: `make` gave the list to FFmpeg, which opened it as ffconcat and was stopped only by that demuxer's own "Unsafe file name" rule for an absolute path; `edit`, `check` and the others gave it to ffprobe; `generate` kept the list as the clip and failed gate `clip_decode` (exit 3).

- `edit_refuses_a_list_as_the_video_the_music_or_the_logo`: an `.m3u8` text named `.mp4` as the video, an ffconcat named `.mp3` as `--music`, named `.png` as `--logo`, named `.mp4` as `--broll`. Each: exit 2, the message names the file and the kind, and neither the list nor the outside file is in any FFmpeg or ffprobe argument list.
- `check_snapshot_analyze_and_compare_refuse_a_list`: the same for `check`, `snapshot`, `analyze`, `compare`, `join`, `clips`.
- `make_refuses_a_scene_picture_or_a_logo_that_is_a_list`: a scene `media` that is an ffconcat named `.png` and an `.m3u8` named `.mp4`, the same under `--plan`, a `logo`, a `music` file and a `narration`.
- `a_result_that_is_a_list_is_refused_and_never_reaches_ffmpeg` (`tests/generate.rs`): a queue row answers with an ffconcat and with an `.m3u8`: exit 1, nothing written, no FFmpeg input, 1 submit for both runs.
- `a_scene_image_that_is_a_list_is_refused_before_any_row_is_called` (`tests/generate.rs`): a generated scene's `image` (its first frame) that is an ffconcat named `.png`: exit 2 at `--plan` and at the run, 0 submits. Before the fix the plan priced the scene ($0.23) with the list as its first frame.
- `real_pictures_clips_and_sounds_still_work`: `edit` with a WAV and an MP3 as `--music`, a PNG and a PNG named `.jpg` as `--logo`; `check` and `snapshot` on an MP4; `make` with a PNG, a JPEG and an MP4 as scene media, a logo and an MP3 as music. Every `-i` in every recorded call has a format named before it.
- Unit tests in `media_gate.rs`: each format by its first bytes, the lists by name, and the wrapper's rewrite of an argument list.

## Not done

- The check reads the first bytes, and FFmpeg opens the file a moment later. A file swapped in between is still opened only as the named format from the file protocol, so a list cannot be followed; it only fails to decode.
- A mov file can name outside data in a `dref` box. FFmpeg does not follow these by default (`enable_drefs` is off), and ScriptVox does not turn it on.
- `generate --image`, `--audio` and `--video` are sent to the row as bytes, with no check of their kind. Only the paths that measure a length use ffprobe, and those go through the wrapper (exit 2 with the old message "cannot measure ...").
- `make` checks only files inside the project. A file outside it is refused by the older rule (`SVX_PATH_OUTSIDE_ROOT`) and is not read.
- `export --format hls` reads back the playlist it wrote a moment before, as HLS by name. That is the one list ScriptVox opens besides its own chunk list.
- The voice files that a TTS service or the Python worker writes are checked by the wrapper when FFmpeg converts them (exit 1), not at the download.
- The GPU services in `services/gen` run their own FFmpeg on what a client sends. That is another program and is not changed here.
- Not run: WebP, BMP, AVI, MPEG-TS, FLAC, OGG, Opus and ADTS files through a real command (their first bytes are unit tested); Windows and macOS.
