# fetch: get a video or audio file from a URL

State: built on 13 Nov 2026 (direct file, HLS). Not built: DASH. The code differs from section 8 in small points (an `Http` struct in place of `get_range`); the code is the truth. Date of the facts: 13 Nov 2026.


```
scriptvox fetch <url> [--audio] [--from T --to T] [--out FILE]
scriptvox setup fetch [--dry-run]
```

All of it is our own Rust code in the binary (sections 3 and 4): no Python, no JavaScript engine, no other program. FFmpeg is the one that the binary already finds with `Ffmpeg::locate`.

No downloader code is read or ported into this tree. The other rules of `docs/plan/MILESTONE-FETCH.md` stay (off until `setup fetch`, one notice, the envelope fields). One field of a brief downloads: `music` with an address (section 10, owner decision of 13 Nov 2026).

## 1. What works and what does not

| Source | State | Result |
|---|---|---|
| Direct media URL (https) | Designed in full | The file as it is |
| HLS playlist (`.m3u8`), not live, not encrypted | Designed in full | One MP4 (or audio file) |
| DASH manifest (`.mpd`) | Not built | Refused with a `fix` line, see 4.3 |
| A YouTube address | Not read | Refused with a `fix` line and with no request (3.1) |
| Any other site (a web page) | Not read | Refused with a `fix` line |

## 2. The opt-in

`fetch` is off until the user runs `scriptvox setup fetch` one time.

- `setup fetch` prints this notice and writes a marker file. It downloads nothing.

  > fetch gets a file from any address that you give. The choice and the responsibility are yours: ScriptVox does not check the rights to a file.

  (Owner decision of 14 Nov 2026, `user-music.md`: one short line, shown one time. The first text was longer and spoke about the terms of a site.)

- Marker: `keys::config_dir()/fetch-consent.json` with `{"schema":"scriptvox.fetch-consent/v1","notice":1,"accepted_at":"<UTC time>"}`. `fetch` reads `notice`; a later change of the notice text raises the number and asks again.
- `setup fetch --dry-run` prints the notice and writes nothing.
- Through MCP, `setup fetch` is refused: "Run `scriptvox setup fetch` in a terminal yourself." The consent must come from the person, and an agent can add `--yes` by itself. An agent with a shell can still run the command; we cannot stop that, and we say so here.
- `fetch` is not an MCP tool in v1. It goes in the left-out list in the header of `mcp.rs` and in `docs/specs/mcp.md`. The tool count stays 32.
- `make` calls `fetch` for one thing only: `music` with an address (section 10). A URL in a script stays text. A test proves that `make` sends no request for it.
- `uninstall --purge` does not need a change: the downloads are in the data folder. The marker is in the config folder and stays until the user removes it (same as keys).

## 3. Flow of the command

1. No marker: stop with `SVX_FETCH_SETUP`.
2. Check the flags: `--from` and `--to` can be given alone or together, and `--from` must be less than `--to`. The `--out` file must not exist.
3. Find the kind of the source (table below).
4. Download to `provision::data_dir()/downloads/fetch/`.
5. `media_gate::downloaded(source, file)`: a file that is not plain media is removed.
6. If `--audio`, a cut, or a join is necessary: one FFmpeg run with stream copy (no encode).
7. Move the result to `--out` and return the envelope.

### 3.1 Kind of the source

The host test costs no request, so it runs first. A YouTube page address is never a media file, so no probe is sent to that site.

| Step | Test | Kind |
|---|---|---|
| 1 | Scheme is not `https` (only `http://127.0.0.1` is allowed, and only with the test variable of section 8) | Refusal `SVX_USAGE` |
| 2 | Host is `youtube.com`, `www.`, `m.` or `music.youtube.com`, `youtube-nocookie.com`, `www.youtube-nocookie.com`, or `youtu.be` | Refusal `SVX_FETCH_UNSUPPORTED`, no request |
| 3 | `http_rows::local_host(url)` is true | Refusal: private address |
| 4 | Probe: `GET` with `Range: bytes=0-1023`, no automatic redirect | by the first bytes: |
| 4a | `media_gate::sniff` accepts them | Direct file (4.1) |
| 4b | They start with `#EXTM3U` | HLS (4.2) |
| 4c | They start with `<?xml` or `<MPD` | DASH refusal (4.3) |
| 4d | Other (for example an HTML page) | Refusal `SVX_FETCH_UNSUPPORTED` |

Redirects on the probe are followed by hand, at most 5. Each new address must be `https` and must pass `local_host`. The last address is the one that is downloaded.

### 3.2 Times, names, flags

- `T` is seconds (`90`, `90.5`) or a clock (`1:30`, `0:01:30.5`). One small function `parse_time` in `fetch.rs`; the crate has no such parser today.
- A cut is a stream copy: `-ss FROM -to TO -c copy`. It starts at the key frame at or before `FROM`. The envelope gives the real duration from `Ffmpeg::readback`. We do not encode, so the cut is fast and not frame exact. HLS skips whole segments outside the window before the download.
- `--audio`: when the file is a sound already, it is kept. When it is a video, the audio track is copied out: AAC to `.m4a`, Opus or Vorbis to `.ogg`, MP3 to `.mp3`, other to `.mka`.
- Default `--out`: the current folder, `<title in safe letters, at most 60>-<id or 8 hash letters>.<ext>`.
- Limits: 4 GB for one file (the limit of `http_rows::download`), 20,000 HLS segments.
- Progress goes to stderr with the prefix `scriptvox: ` as bytes done and total. A URL that came from a server is never printed.

## 4. Kind 1: direct file and manifest

### 4.1 Direct file

Use `http_rows::download_no_redirect(final_url, dir, dest, None)`. It already resumes through a `.part` file, waits on 429 and retries connect faults. Then `media_gate::downloaded`.

- title: the `filename` of the `Content-Disposition` header of the probe, else the last path part without the extension.
- uploader: null. licence: null. duration: from `readback`.

### 4.2 HLS

The media gate does not let FFmpeg open a URL or a playlist. So our code reads the playlist and gets each part with HTTP, and FFmpeg sees local files only.

How it works, in our words: a master playlist is a text file that lists variants (the same content in different sizes) with `#EXT-X-STREAM-INF` lines; the line after each one is the address of a media playlist. A media playlist lists the parts in play order, each with an `#EXTINF` duration line. `#EXT-X-ENDLIST` says that the list is complete. `#EXT-X-MAP` names a start part for fragmented MP4. `#EXT-X-KEY` says that the parts are encrypted. `#EXT-X-BYTERANGE` says that a part is a byte span of a larger file.

| Step | Action |
|---|---|
| 1 | Get the playlist text (at most 2 MB). |
| 2 | Master playlist: take the variant with the largest `BANDWIDTH` whose `RESOLUTION` height is at most 1080. With `--audio`, take the `#EXT-X-MEDIA:TYPE=AUDIO` entry that has a `URI`, when there is one. |
| 3 | If the variant names an `AUDIO` group whose entry has a `URI`, that is a second media playlist (sound is separate). |
| 4 | Media playlist: no `#EXT-X-ENDLIST` means live: refuse. `#EXT-X-KEY` with a method other than `NONE`: refuse. |
| 5 | Resolve each part address against the playlist address (own function `join_url`: absolute, `//host`, `/path`, relative). Each one must be `https` and pass `local_host`. |
| 6 | With `--from`/`--to`: keep only the parts whose time span touches the window (sum of `#EXTINF`). |
| 7 | Get the start part (if any) and each part in order, with a `Range` header when `#EXT-X-BYTERANGE` is given, and append the bytes to one local file (`.ts`, or `.mp4` when there is a start part). 3 tries for each part. |
| 8 | `media_gate::downloaded` on each local file. |
| 9 | One FFmpeg run: `-i video [-i audio] -c copy` to MP4 (or the audio file for `--audio`), with the cut when a window was given. The `-ss` and `-to` values are the window times minus the start time of the first part that was kept. |

- title: the last path part of the playlist address without the extension. uploader, licence: null.

### 4.3 DASH (not built)

A DASH manifest is XML. The lock file has no XML parser, and the manifest has several ways to list parts (one file with an index, a number template, a time line). The smallest honest v1 refuses it:

- message: "A DASH manifest (.mpd) is not supported yet."
- fix: "Give the address of the media file itself, or of the HLS playlist (.m3u8) if the site has one."

When it is needed: add `roxmltree` (MIT OR Apache-2.0, no dependencies) and support the one-file form first (`BaseURL` per representation), which is a plain file download for the video and one for the sound, then the join of 4.2 step 9.

## 5. Other sources

None. An address that is not a media file and not an HLS playlist is refused (section 7).


## 6. The envelope

```json
{
  "schema": "scriptvox.result/v1", "ok": true, "command": "fetch",
  "artifacts": { "video": "/abs/path/clip-1a2b3c4d.mp4" },
  "metrics": { "duration_s": 634.5, "bytes": 28523658, "width": 640, "height": 360 },
  "source_kind": "direct",
  "source_url": "https://media.example.org/v/clip.mp4",
  "title": "clip", "uploader": null, "licence": null,
  "layer": "native",
  "findings": []
}
```

- `artifacts.audio` in place of `artifacts.video` with `--audio`.
- `source_kind`: `direct` or `hls`.
- `layer`: `native`, our own code gave the file.
- `source_url` is the address without the query, the fragment and the user part, because a query can hold a signed value.
- `uploader` and `licence` are null: a file and a playlist do not say them. The notice of `setup fetch` already says that the user must check the rights.

## 7. Failures

All use an existing `Exit` value with `Envelope::fail_code`. No new `Exit` value.

| Cause | Exit | Code | Message | Fix |
|---|---|---|---|---|
| `setup fetch` was not run | 2 Usage | `SVX_FETCH_SETUP` | fetch is off. | Run `scriptvox setup fetch` one time and read the notice. |
| Bad flag, `--from` not less than `--to`, `--out` exists, not `https` | 2 Usage | `SVX_USAGE` | Names the flag | Names the correct form |
| Private or local address (first or after a redirect) | 2 Usage | `SVX_FETCH_UNSUPPORTED` | fetch does not read from a private address. | Give a public https address. |
| A web page of another site, or any YouTube address (that one with no request) | 2 Usage | `SVX_FETCH_UNSUPPORTED` | fetch cannot read this site. | Give the address of the media file itself (it often ends in .mp4, .m4a or .m3u8). |
| DASH manifest | 2 Usage | `SVX_FETCH_UNSUPPORTED` | See 4.3 | See 4.3 |
| HLS with no end mark (live) | 2 Usage | `SVX_FETCH_UNSUPPORTED` | This is a live stream. fetch reads complete recordings only. | Fetch it after the stream has ended. |
| HLS with `#EXT-X-KEY` | 2 Usage | `SVX_FETCH_UNSUPPORTED` | This stream is protected. fetch does not read protected streams. | Use a source that is not protected. |
| The file is not plain media | 2 Usage | `SVX_FETCH_NOT_MEDIA` | The text of `media_gate` | Give the address of the media file itself. |
| 429 after the waits, 5xx, connect fault, read fault | 5 ProviderRetryable | `SVX_PROVIDER_RETRYABLE` | The download stopped: `<cause with no address>` | Run the same command again. A download of one file continues from the part that is on disk. |
| Direct file or HLS part answers 403 or 404 | 1 Error | `SVX_ERROR` | The server refused the file (HTTP `<code>`). | Check the address in a browser. A signed address can be too old. |
| File above 4 GB, playlist above the limits | 2 Usage | `SVX_USAGE` | Names the limit | Use `--from` and `--to` for an HLS source. |
| FFmpeg not found | as `Ffmpeg::locate_failure` | | | |

Error text rule: a message can hold the host of the address that the user gave. It never holds an address that came from a server answer, and never the query of any address. `ureq` error text is not passed through as it is; the code maps it to a short cause (`connect`, `timeout`, `read`).

## 8. Module layout

Flat files, like the other commands. No new crate, no new workspace member.

| File | Holds |
|---|---|
| `crates/scriptvox/src/fetch.rs` | The command, the consent marker, the kind test, the direct file path, cut and audio copy, the envelope |
| `crates/scriptvox/src/fetch_hls.rs` | Playlist parse and part download |
| `crates/scriptvox/tests/fetch.rs` | All tests of section 9 |

```rust
// fetch.rs
pub struct Args { pub url: String, pub audio: bool, pub from: Option<String>,
                  pub to: Option<String>, pub out: Option<PathBuf>,
                  pub use_system: bool }
pub fn run(a: Args) -> Envelope;
pub fn setup(dry_run: bool) -> Envelope;          // prints the notice, writes the marker
pub fn ready() -> bool;                           // the marker exists with the current notice number
pub const NOTICE: &str;

pub enum Kind { Direct, Hls }
pub struct Fetched { pub files: Vec<PathBuf>,     // 1 file, or video then audio
                     pub kind: &'static str, pub source_url: String,
                     pub title: String, pub uploader: Option<String>,
                     pub duration_s: Option<f64>, pub licence: Option<String>,
                     pub findings: Vec<String> }
pub struct Fail { pub exit: Exit, pub code: &'static str, pub message: String, pub fix: Option<String> }

pub(crate) fn youtube_host(url: &str) -> bool;    // the refusal with no request
pub(crate) fn parse_time(s: &str) -> Option<f64>;
pub(crate) fn join_url(base: &str, reference: &str) -> Option<String>;
pub(crate) fn public_url(url: &str) -> String;    // no query, fragment or user part
pub(crate) fn get_range(agent: &ureq::Agent, url: &str, user_agent: &str,
                        start: u64, end: u64, sink: &mut dyn Write) -> Result<Ranged, HttpFault>;
pub(crate) struct Ranged { pub status: u16, pub total: Option<u64>, pub written: u64,
                           pub location: Option<String> }

// fetch_hls.rs
pub struct Variant { pub uri: String, pub bandwidth: u64, pub height: Option<u32>, pub audio_group: Option<String> }
pub struct Part { pub uri: String, pub duration_s: f64, pub range: Option<(u64, u64)> }
pub struct Media { pub init: Option<Part>, pub parts: Vec<Part>, pub ended: bool, pub encrypted: bool }
pub(crate) enum Playlist { Master { variants: Vec<Variant>, audio: Vec<(String, String)> }, Media(Media) }
pub(crate) fn parse(text: &str) -> Result<Playlist, String>;
pub fn fetch(url: &str, first_bytes: &[u8], audio: bool, window: Option<(f64, f64)>, dir: &Path) -> Result<Fetched, Fail>;
```

Reused from the crate: `http_rows::{agent_ca, download_no_redirect, url_allowed, local_host}`, `media_gate::{sniff, downloaded}`, `Ffmpeg::{locate, locate_failure, run, readback}`, `provision::data_dir`, `keys::config_dir`, `Envelope::{ok, fail_code, with}`.

Test variable (it accepts only a value that starts with `http://127.0.0.1`, as `open_search::test_base` does):

| Variable | Effect |
|---|---|
| `SCRIPTVOX_FETCH_BASE` | A user URL, a redirect or an HLS part that starts with this value skips the `https` and `local_host` tests |

Other files to change:

| File | Change |
|---|---|
| `crates/scriptvox/src/cli.rs` | Variant `Fetch` in `enum Cmd` with its help text; `fetch` in the `what` help of `Cmd::Setup` |
| `crates/scriptvox/src/lib.rs` | `pub mod fetch; pub mod fetch_hls;` |
| `crates/scriptvox/src/main.rs` | Match arm for `Cmd::Fetch`; match arm `setup fetch`; `fetch` in the "unknown setup target" fix line |
| `crates/scriptvox/src/mcp.rs` | `fetch` in the left-out list of the header; the `setup` tool refuses target `fetch` |
| `docs/specs/mcp.md` | `fetch` in the left-out list, with the reason |
| `docs/command-status.md` | One row for `fetch`, state Unverified until the live probe passes |
| `plugin/skills/scriptvox/SKILL.md` | Made again by `just docs` |

## 9. Tests

All tests in `tests/fetch.rs` use a hand-written server on `127.0.0.1:0` (the pattern of `serve` in `tests/provision.rs`). No real site in CI. Test data uses short made-up values; no real session id and no real signed address, because the pre-commit hook refuses them. Tests that need FFmpeg use `common::ready()` and make their media with FFmpeg.

| # | Test | Proves |
|---|---|---|
| 1 | `fetch` with no marker | Exit 2, `SVX_FETCH_SETUP`, the fix names `setup fetch`, the server saw no request |
| 2 | `setup fetch --dry-run`, then `setup fetch` | Dry run writes nothing; the real run writes the marker and prints the notice |
| 3 | Direct MP4 | File at `--out`, envelope has `artifacts.video`, `source_kind`, `title`, `duration_s`; `source_url` has no query |
| 4 | Direct file, server closes at half, second run | The second run sends `Range` and the file is whole |
| 5 | Direct URL that returns HTML | `SVX_FETCH_UNSUPPORTED`, nothing left in the downloads folder |
| 6 | Redirect to a private address | Refused, the second host got no request |
| 7 | `--audio` on a video; `--from 1 --to 2` | Audio file with no video track; duration about 1 s |
| 8 | HLS: master with two variants, media playlist with 3 `.ts` parts | The larger variant is taken, one MP4 with the full duration |
| 9 | HLS with a start part and byte ranges | `Range` headers are correct, the MP4 plays |
| 10 | HLS with no end mark; HLS with `#EXT-X-KEY:METHOD=AES-128` | Each refused with its message, no part was requested |
| 11 | A `.mpd` body | The DASH refusal with its fix |
| 20 | Every run of every test | stdout and stderr hold no query text of the address |
| 21 | Plain `http`, a private address, YouTube addresses (watch, short, embed, playlist) | Each refusal, with no request |
| 22 | `make` from a script that holds a loopback URL | The server saw no request |
| 24 | `make --music <address>` (section 10) | Fetch off: `SVX_FETCH_SETUP`. The plan has the `fetch` stage with the address and sends nothing. `--local-only`, a private address and plain `http` are refused. The cut is in the run folder and in the sound, the picture is the same as with no music, the credits, the receipt and the export hold the source, `commercial_ok` is false, the recipe holds the address, a second run sends no request |
| 23 | Unit: `parse_time`, `join_url`, `public_url`, `fetch_hls::parse` | Table tests with good and bad input |

`tests/agent_mode.rs` runs each command in agent mode; it calls `fetch` with no argument, so the usage envelope must come as for the other commands that need an argument. The `findings` entries are objects with `gate`, `status` and `message`, as in `compare.rs`. Custom codes need no list entry: `SVX_CHECKSUM` is only in the code that uses it.

Before a merge: `just docs`, then `just ci`. No new crate, so `just notices` and `deny.toml` do not change.

## 10. Open risks

- `local_host` does not catch a public name that changes its answer between the check and the download.
- A cut with stream copy is not frame exact.

## 10. Music from an address in `make`

Owner decision of 13 Nov 2026: one step for the user. It replaces work item 4 of the milestone ("a brief holds a local file only").

```
scriptvox make ... --music <https address> [--music-from T --music-to T]
```

Brief fields: `music` (the address), `music_from`, `music_to`. The times are as for `fetch`: seconds or a clock. They are refused when `music` is not an address.

- `make` calls the code of `fetch` (`fetch::music_check`, `fetch::music`) with `--audio`. There is no second downloader. The same rules hold: https only, no private address, the same kinds of source, the media gate.
- Fetch must be on. When it is off, `make` and `plan` stop before any other work with `SVX_FETCH_SETUP` and the fix `scriptvox setup fetch`. The notice is shown one time, by `setup fetch`, as for the command.
- The plan has a stage `music` with tool `fetch`, `network: true`, and the address as the user gave it in `explain`. A plan downloads nothing.
- `--local-only` refuses with `needs_input` (field `music`): get the file with `scriptvox fetch <address> --audio` first, or run without `--local-only`.
- The download happens one time for an address and a cut. The file goes to `<run>/fetch/` and to the store (`store::Cas`, key from the address and the two times), with `pick.json` beside it. A rerun, a `resume` and a `replay` take it from the store and send no request. A `replay` with no stored file stops with `SVX_CACHE_MISS`.
- Credits: one line in `credits.txt`, `Music: "<title>" by <uploader>, <source_url> (<licence note>; downloaded by scriptvox fetch, layer <layer>)`. The receipt holds the same facts as one `stock` item with `provider: "fetch"`, and `export` copies it to `parts` in `metadata.json`.
- `commercial_ok`: the item has `commercial: true` only when `http_rows::file_terms` reads a free licence from the `licence` field (the rule of the stock files). No licence, or a text that it cannot read, gives `false`. A file and a playlist say no licence, so they give `false`.
- Nothing is drawn on the video for this music, and no line about its terms is printed. The data is in `credits.txt` and `metadata.json`.
- The recipe in the MP4 keeps the address and the two times, not the file.
- An address that gives a picture is refused.

## 11. Music from an address in `edit` (13 Nov 2026)

```
scriptvox edit <video> --music <https address> [--music-from T --music-to T] [--music-level quiet|normal|loud]
```

The same code path as section 10 (`fetch::music_check`, `fetch::music`) and the same rules:

- Fetch must be on. When it is off, `edit` stops before any other work with `SVX_FETCH_SETUP` and the fix `scriptvox setup fetch`.
- https only, no private address, the same kinds of source, the media gate. An address that gives a picture is refused.
- `--music-from` and `--music-to` are refused when `--music` is not an address.
- `--local-only` refuses with `needs_input` (field `music`).
- One download for an address and a cut. The file goes to the store with the same key as in `make`, so `make` and `edit` share it, and a second `edit` sends no request.
- The download comes before the encode. A refused address leaves no video.
- Credits: the same line as in `make`, in `<out>.credits.txt` beside the video (`artifacts.credits`). The answer has `music`: the address, the cut, and `source` with the same facts as the `stock` item of a `make` receipt. `commercial` is `true` only when a free licence is read from the `licence` field.
- Nothing is drawn on the video for this music, and no line about its terms is printed.

Not the same as `make`: `edit` has no plan and no run folder, so there is no plan stage. The address is not kept in the MP4, and `export` of the result does not copy the source to `metadata.json`: read `<out>.credits.txt`.

Test: `edit_gets_music_from_an_address_one_time_and_credits_it` (`tests/fetch.rs`, a local server, no network). Real run (13 Nov, a local file server): a 5 s clip, music cut 1 s to 5 s; the picture MD5 is the same; the band of the music went from -77.8 dB to -32.6 dB; `check` gives -14.0 LUFS and 8 ms drift; an address that answers 404 gives an error and writes no video.
