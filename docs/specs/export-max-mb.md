# `export --max-mb 16` (G071, 8 Nov 2026)

Chat apps and mail have a file size limit. ffmpeg-skill and others describe a two-pass size target; no code read.

- `--max-mb N`: one more copy, `video-<N>mb.mp4`, of at most N MB, where 1 MB is 1,000,000 bytes (the smaller reading, so it fits either way). It is listed with the other copies and in `artifacts` as `video_max_<N>mb`, with its bytes and video bitrate.
- Bitrate: N MB over the true length, 3 percent kept for the container, 128 kbit/s for the sound, the rest for the picture. Two libx264 passes at that rate, capped at it (`-maxrate`, `-bufsize` twice the rate), because a short clip overshoots the average. Then the file is measured; if it is still over, once more at 85 percent; if still over, exit 2 and no file. The pass logs are removed.
- Under 150 kbit/s for the picture is refused at once (exit 2): "9.5 s does not fit in 0.1 MB".
- Needs libx264 in the FFmpeg build (the one `scriptvox setup` installs has it).

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| The 9.47 s, 3-shot video to `--max-mb 0.5`, first version (no cap) | 515,669 bytes after two tries, over: exit 2. The picture came out at 297 kbit/s for 254 asked |
| With the cap | 7.4 s, 493,314 bytes on the first try, 281 kbit/s; sheet of 4 frames read by eye: captions sharp, colours right |
| `--max-mb 0.1` | exit 2 at once, "9.5 s does not fit in 0.1 MB" |
| Unit test | 16 MB over 60 s is 1941 kbit/s for the picture; too small and negative refused |

## Not done

A smaller frame when the bitrate is low (the size stays). Sound under 128 kbit/s for very long videos.
