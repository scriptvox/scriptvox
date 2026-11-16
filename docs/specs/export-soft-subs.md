# `export --soft-subs en,ar=ar.srt` (G072, part, 8 Nov 2026)

A viewer picks the caption language in the player; the picture stays clean. Public MP4 and player docs; no code read.

- `--soft-subs`, comma separated: a bare code (`en`) is this run's own `captions.srt`; `ar=ar.srt` adds an SRT in that language (a path in the project). Codes: ar, en, fr, es, de, ur, hi, or any three lowercase letters (ISO 639-2), written as the track's language. The first track is the default.
- Writes `video-subs.mp4`: picture and sound copied as they are (no re-encode), one `mov_text` track per entry. The burned captions of `video.mp4` stay in the picture; a soft track is for the second language, or for a clean picture made elsewhere.
- A video file with no run has no captions of its own: a bare code is then refused; give `code=file.srt`.

## Measured, 8 Nov 2026

| Check | Result |
|---|---|
| `export <run> --soft-subs en,ar=ar.srt` on the 3-shot video | 0.23 s; streams: h264, aac, mov_text `eng` (default), mov_text `ara` |
| Picture packets of `video.mp4` and `video-subs.mp4` | the same md5 of all packet sizes: not re-encoded |
| Tracks read back by FFmpeg | the English cues of the run, the Arabic cues of the file; the Arabic track drawn by FFmpeg `subtitles` read by eye: letters joined, right to left |
| `--soft-subs xx` | exit 2, "xx is not a language code" |

## Not done

Brief `captions_second` (a small second line under the main captions). A soft track in WebM.
