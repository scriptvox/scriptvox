# RVX-002 spike: answers (2 Nov 2026)

FFmpeg under test: BtbN `autobuild-2026-10-31-12-59`, `n8.1.3-27-g8dca1586f4` linux64 gpl static, sha256 `41e9c565…3380`, in `~/.local/opt/ffmpeg/n8.1.3-btbn-20261031`. Windows check: Gyan FFmpeg 7.1 full build on the host.

| # | Question | Answer | Evidence |
|---|---|---|---|
| 1 | libass draws Arabic with `fontsdir`, `FONTCONFIG_FILE`, `WrapStyle: 2`? | **Yes.** Letters join, RTL order is correct, the Latin run and the Arabic comma sit in the right place. The debug log shows both fonts come only from `assets/fonts`. `WrapStyle: 2` never wraps on its own, so the line count is exactly our `\N` breaks; the harfrust comparison moves to RVX-007 where the breaker is written. | `fixtures/spike/ar_line.png`, `ar_line_windows.png`, `ar_line.ass`, `fonts.conf` |
| 2 | Windows filtergraph path escaping? | **Yes, two levels.** One escape level fails (`No option name`). Level 1 escapes `\ ' :` in the option value, level 2 escapes `\ ' [ ] , ;` for the graph. Better: run FFmpeg with the run folder as cwd and use fixed relative names (`ass=captions.ass:fontsdir=fonts`): no escaping at all. Both rendered on Windows from a path with a drive colon, a space and an apostrophe. | spike log below |
| 3 | `-ss/-t` chunk seams after stream copy? | **No seams** when chunks are video only, same encoder settings, closed GOP and a fixed `-video_track_timescale`, and the audio is encoded once and muxed after the concat. 300 of 300 frames, constant 33.3 ms step, PSNR 46.0 dB against a single-pass render. | spike log below |
| 4 | uv restores the worker offline? | **Yes.** `UV_OFFLINE=1 uv sync --frozen --managed-python` from a warm cache: exit 0, import ok. Cache 27 MB plus managed Python 105 MB. | spike log below |
| 5 | edge-tts word events with `boundary="WordBoundary"`? | **Yes**, on 7.3.1 (and 7.2.3), English `en-US-AndrewNeural` and Arabic `ar-AE-HamdanNeural`. Offsets and durations in 100 ns units. | spike log below |
| 6 | `setsid` and Windows breakaway for detach? | **Linux yes** (`setsid -f` child outlived its parent). **Windows not tested** (no Windows Rust build yet). | flip rule d applies |
| 7 | Kokoro int8 size? | **92.4 MB** model plus 28.2 MB voices (`model-files-v1.0`). fp32 is 325.5 MB. | GitHub release API |
| 8 | Piper `ar_JO` licence? | **Not clear.** The model card says "License: See URL", and the dataset repo has no licence. Treat it as not redistributable. | HF model card, GitHub API |
| 9 | macOS FFmpeg `configuration:` shows nonfree? | **Open.** evermeet.cx 9.0.2 does not publish its configure line, and no Mac is here. `doctor` parses `ffmpeg -version` at run time and refuses `--enable-nonfree`, so the risk stays covered. | evermeet info API |

## Flip rules

- (a) libass Arabic fails: **not triggered.** Arabic captions stay in v0.1.
- (b) chunk seams appear: **not triggered**, under the conditions in row 3. RVX-017 must keep those conditions.
- (c) RPC round trip costs more than its work: **not measured yet.** RVX-009 records the round-trip time in its tests.
- (d) Windows breakaway denied: **applied by default** because it is untested. Windows runs in the foreground in v0.1 (preview).

## Plan changes

1. **Fonts ship as static instances (Regular and Bold), not variable fonts.** On Windows the variable font lost its bold weight. RVX-006 converts or fetches static TTFs.
2. **`fontsdir` loads every file in the folder.** Font folders hold only font files. Provenance lives in `PROVENANCE.tsv`, and the licence lives in `REUSE.toml`.
3. **RVX-006: relative paths first.** The run folder is the FFmpeg cwd. Escaping (two levels) is only for paths that the user supplies.
4. **RVX-017:** chunks are video only. Audio is encoded once and muxed after the stitch.
5. **Q9 (local Arabic voice): no Piper `ar_JO` in v0.1** (dataset licence unclear).
6. **FFmpeg pin for `toolchain.lock`:** the BtbN `autobuild-2026-10-31-12-59` n8.1.3 gpl build (sha256 above).
