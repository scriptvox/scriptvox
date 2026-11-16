# `edit --background` and `--alpha`: a clean background behind the subject (G056, 11 Nov 2026)

Why: a person filmed in a messy room wants a plain or soft background, or the subject alone for an editor. The idea is from the one-line fact that avatar and video-call tools replace a background; no other tool's code or docs were read. The mask is the one the title behind the subject already uses (`title-behind.md`).

## The rule

- `scriptvox edit talk.mp4 --background <value>`. The subject stays, the rest of the picture is replaced.
  - `#RRGGBB`: a plain colour.
  - `blur`: the same picture, blurred, behind the sharp subject.
  - A picture or a clip inside the project. A picture is scaled to cover the frame. A clip is scaled to cover, has no sound, and repeats to the length.
- `scriptvox edit talk.mp4 --alpha`: the subject on a transparent background. Two files: a ProRes 4444 `.mov` (the source's sound, copied) and a WebM with alpha beside it (VP9, no sound: a WebM cannot hold AAC). The encoder settings are those of `export --overlay`. Default name `scriptvox-out/<name>-alpha.mov`; `--out` must end in `.mov`.
- The length, the size and the frame rate stay. The sound is the source's own packets, copied, when `--background` or `--alpha` is the only option. With another option in the same call the usual `edit` pass encodes the sound and sets the loudness, as it always does.
- At most 60 s in one call. A longer video is exit 2 with the cut command in the fix line.

## The mask

- From the first `matte` row with a key: a free row first, else a paid row. The call goes through `generate` (the same code as `generate --model <row> --video <file>`): one plan, one approval, one job on record, the result download held to the row's hosts.
- What is sent: a copy of the picture with no sound, the long side at most 1280 px (the matte model works at 1024 px). The mask is scaled back to the source size. The sound never leaves the machine.
- The copy is named by a hash of the source's bytes and is made once. The plan id is a hash of the copy, so `--plan`, `--approve` and every later background on the same clip are the same plan and the same job: three backgrounds on one clip are one call.
- `edit ... --plan` shows the row, the cost, the size sent and the length, and calls nothing. A free plan runs with the same command without `--plan`. A paid row with no approval is exit 4 and nothing is sent; `--approve <plan_id>` runs it; `--max-spend` under the price stops it.
- A mask with another frame count than the copy (more than one frame apart) is exit 1 and nothing is written.
- No matte row with a key: exit 10.

## Checks before any call (exit 2)

- A value that starts with `#` must be exactly `#RRGGBB` (`captions::ass_colour`). Only the six checked hex digits reach the filter text.
- A file must exist, be inside the project folder (real paths compared), and be a picture or hold a video stream. It goes to FFmpeg as its own `-i` argument, never into filter text.
- `--background` and `--alpha` in one call; `--alpha` with any other edit option.
- With a cut (`--cut-silence`, `--cut-still`, `--cut-fillers`, `--cut-retakes`, `--keep`, `--keep-words`), `--from/--to`, `--speed`, `--duration`, `--reframe`, `--fit`, `--track` or `--lipsync`: "use two calls". A cut would send frames to the row that the result drops; a crop or a new speed changes what the mask must fit. Cut, crop or lip sync first, then the background on the result.
- Allowed with `--background`: `--captions`, `--captions-file`, `--text`, `--hook`, `--label`, `--logo`, `--music`, `--clean-voice`, `--bleep`, `--blur`, `--zoom`, `--broll`. The background is made first; they work on the new picture.

## How

- One FFmpeg pass over plain inputs at one frame rate: the source, the mask file, and the background (a `color` source, a looped picture, or a file). The mask is stretched from video levels to 0..255, `alphamerge` puts it on the source, `overlay` lays the subject over the background. `blur` splits the source inside the graph (one source, one speed): small, `gblur`, large again.
- A background clip is first made into its own file at the source's rate and size, then read as a plain input. This is the rule of `audiogram.md` ("Fault found"): no second stream of another speed inside the graph.
- A variable-rate source is made constant with the same `fps` step in the copy and in the result, so each mask frame meets its own picture.
- The gates of `edit` run on the result: a picture stream, `av_drift`, the length. `not_checked` names `matte`: no gate judges the mask, so look at the frames.
- MCP: the `edit` row now says it can cost money (a paid matte row).

## Run on 11 Nov 2026, FFmpeg held to 8 GB

Tests failed first ("unexpected argument '--background'" and "'--alpha'", 7 of 7), then passed. `tests/edit_background.rs`, with a fake queue (`tests/common/mod.rs`, the protocol of the fake in `tests/generate.rs`) whose mask is an H.264 MP4 with the left half white:
- `#00ff00`: the right half is green with no ghost of the source, the left half is the source; the sound packets are the source's bytes; the length is the same; `av_drift` passes; a second colour on the same clip is still 1 submit.
- `blur`: the subject half keeps its sharpness, the other half has under half of it. A picture and a 1 s clip (repeated to 3 s) stand behind. One submit for all three.
- `--alpha`: the `.mov` is `yuva444p`, the WebM has `alpha_mode=1`; alpha is 255 on the subject and 0 on the rest.
- Exit 2 with 0 submits: `#00ff0`, `#00ff00,drawtext=text=x`, a file that does not exist, a file outside the project, a file that is no picture and no video, `--reframe`, `--speed`, `--from`, `--alpha` with `--background` or `--hook`, `--alpha --out x.mp4`.
- A paid matte row (a smallest charge of $0.06, and a price of $0.02 by the second of the source): exit 4 and 0 submits with no approval; `--plan` shows $0.06; `--max-spend 0` with the approval is exit 4 and 0 submits; with `--approve` 1 submit; another background with the same approval is still 1.
- A free plan calls nothing; `--label` combines and the result keeps the name `<name>-edit.mp4`.

Real runs on the owner's GPU service, row `birefnet-self`, cost $0 in every result. The clip: one person at a desk in a dark room, 1920x1080, 24 fps, 5.04 s, no sound. Sheets of 4 frames and two crops read by eye:
- `--background "#1d2b64"`: 14 s for the whole command with the row's call. The person is whole in all 4 frames, with glasses, hood cords and both arms; the room, the screen and the lamp are gone. On a crop at full size the hair edge is clean, with a thin teal line in the loose hair: the old curtain's colour. In the last third the back of the chair is kept as a part of the subject.
- `--background blur`: the same mask (no new call). The person is sharp, the room is soft, no hard line around the person.
- `--alpha`: no new call. `.mov` ProRes 4444 `yuva444p12le`, 39 MB; `.webm` VP9 with `alpha_mode=1`, 1 MB. Laid over magenta, the `.mov` shows the person and the chair back and nothing of the room.
- Three results, 1 call: one run folder holds the job.
- A 10 s copy of the clip with a tone as sound (1920x1080): `--background "#1d2b64"` gave 10.00 s before and after and `av_drift` pass; the row's call took 3 min 46 s (the 5 s clip took about 12 s: the shared GPU was busy or loading). `--background blur --label "Made with ScriptVox"` on it: no new call, `av_drift` and `text_bounds` pass, the label is over the new picture, loudness -14.0 LUFS (the usual pass sets it).
- After the two review fixes below, on the final code: `--background wall.png` (a 1280x720 gradient picture) on the 5 s clip: the person over the picture in all 4 frames, $0, peak 879 MB.
- Calls to the matte row in all: 3 (the 5 s clip, the 10 s clip, and the 5 s clip again after the copy's name changed).
- Peak FFmpeg memory (resident), each run measured: 5 s colour 814 MB, 5 s blur 827 MB, 5 s `.mov` 522 MB, 5 s WebM 927 MB, 10 s colour 827 MB, 10 s blur 825 MB, the copy for the row 401 to 425 MB. It does not grow with the length.

## A background file is opened only as what it is (11 Nov 2026, found by a review before the merge)

Before the fix the file was checked to be inside the project, and FFmpeg then guessed its format from the content. A file inside the project that is a list (HLS `.m3u8`, ffconcat, DASH) under a clip's name would make FFmpeg open the files or URLs the list names, outside the project. Now:
- ScriptVox reads the first bytes itself. Only these pass: PNG, JPEG, WebP, BMP (one picture); the mov family (mp4, mov, m4a, 3gp), Matroska and WebM, AVI, MPEG-TS (a clip). Everything else is exit 2 before FFmpeg or ffprobe is given the file, and the message names what it is ("an ffconcat list", "an HLS playlist"). The name's ending decides nothing.
- FFmpeg and ffprobe open the file with `-protocol_whitelist file` and the demuxer named (`-f mov`, `matroska`, `avi`, `mpegts`; a picture with `-f image2 -pattern_type none` and its decoder), so no format is guessed, no number pattern is read and no network or `concat:` protocol can be reached.
- FFmpeg gets the real path, with links followed, that the project check passed. A link inside the project to a file outside it is "outside this project folder".
- Test `a_list_or_a_link_under_a_clips_name_is_refused_before_ffmpeg_opens_it`, with an FFmpeg that records every argument: an ffconcat and an `.m3u8` list named `.mp4`, a text file named `.png` and a link to a clip outside are exit 2 with 0 submits and never reach FFmpeg; a PNG under another name, a JPEG, an MP4 and an MKV still work. It failed first, on the message of the old check, which gave the file to ffprobe.
- Not changed: the video to edit itself, and the files of the older options (`--broll`, `--logo`, `--music`, `--lipsync`), are still opened with FFmpeg's own format guess.
- Later the same day: the check moved to one shared place and now covers those files too, and every other media input of every command. See `media-inputs.md`.

## The mask is not trusted either (11 Nov 2026, second review point)

The mask is a file from a service. Before the fix it went to ffprobe and to the compose pass with FFmpeg's own format guess, so a service that answers with a list (HLS, ffconcat) could make FFmpeg open other local files or URLs. Now the downloaded file passes the same first-bytes check as a background file, and only a plain clip container goes on (the mov family, Matroska, AVI, MPEG-TS); anything else is exit 1, the file is removed, and no FFmpeg pass is given it. ffprobe and the compose pass open the mask with `-protocol_whitelist file` and the demuxer named. The same holds for the clip that `edit --enhance` gets back. `genjob`'s own `is_mp4` check (`ftyp` at byte 4) is private to the connected rows and narrower; the check here is the one of the background file. Tests `a_mask_that_is_a_list_is_refused_before_ffmpeg_opens_it` (an ffconcat and an `.m3u8` answer: exit 1, the mask and the file it names never in FFmpeg's arguments; a real mask is opened with `-protocol_whitelist file -f mov`) and `a_result_that_is_a_list_is_refused` in `tests/edit_enhance.rs`. Both failed first. A free row's bad answer (a list, or a frame count that does not fit) is forgotten, so the same command asks the row again (tested: a list, then a good mask, 2 submits); a paid job stays on record and the fix line names its folder. Not changed: the same mask in `make` (a title behind the subject) and the results of `generate` itself. Both are covered now: `media-inputs.md`.

## Not done, not run

- No edge work: no spill removal, no feather, no choke. The old background's colour stays in fine hair.
- The mask is the row's choice of subject. There is no way to name the subject or to drop a part (a chair).
- `blur` blurs the whole picture, so the subject's own colour spreads a little into the background around it.
- No frame from the middle of the result is saved for a check; `snapshot --sheet` does that.
- With another option in the same call the picture is encoded twice (the background, then the usual pass), and a wrong value of that other option stops the run after the mask was made. The mask stays on record, so the next run does not call the row again.
- A background clip always repeats; it cannot hold on its last frame.
- Not run: a WebP or BMP picture and an AVI or MPEG-TS clip as the background (PNG, JPEG, MP4 and MKV are in the tests); a real paid matte row (none exists in the registry), a clip longer than 10 s, a vertical clip, a variable-rate phone clip, two people, a moving camera, HDR, Windows and macOS, the MCP tool call itself.
