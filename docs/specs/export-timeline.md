# `export --timeline fcpxml,otio --stems` (G053, 8 Nov 2026)

An editor wants the video as a timeline, not one flat file. From the public FCPXML and OpenTimelineIO specifications; no code of OpenChatCut, Venturi, OpusClip or auto-editor read.

- `--stems` (a run id only): `narration.wav` (the voice) and `music.wav` (the bed as it went into the mix, before its level and ducking), 48 kHz stereo, each the video's length.
- `--timeline fcpxml`, `otio` or both: `timeline.fcpxml` (version 1.10: Final Cut Pro, DaVinci Resolve) and `timeline.otio` (OpenTimelineIO JSON: Kdenlive, Resolve and others) next to `video.mp4`. The picture is one clip per shot of `video.mp4`, cut at its shot changes: found with the 4 fps scene test of `analyze` (threshold 0.25), then placed on the exact frame by the same test on every frame of the second around it. With `--stems` the sound is one track per stem (FCPXML: connected clips on lanes -1 and -2, and the picture's own sound at -96 dB); without, the video's own sound.
- Files are named by absolute `file://` URLs, so move the folder and the editor asks to relink.
- The picture still has the captions and cards burned in; `--overlay` writes them alone, but a clean picture without them is not made here.

## Measured, 8 Nov 2026, 2 cores

A 9.47 s `make` run: 3 stills with hard cuts, Kokoro voice, the `calm` bed.

| Check | Result |
|---|---|
| `export --timeline fcpxml,otio --stems` | 0.8 s; 2 WAVs, 2 timelines |
| Parse back with OpenTimelineIO 0.18.1 (PyPI, through uvx) | V1 3 clips, A1 narration.wav, A2 music.wav, each 9.467 s |
| Parse back with Python's XML parser | FCPXML 1.10, 3 spine clips at 0/30s, 100/30s, 181/30s, lanes -1 and -2 |
| Frames 99 and 100, 180 and 181, read by eye | red then green, green then blue: each clip starts on the first frame of its shot |
| The first version, at 4 fps only | cut 1 was 2 frames early (3.27 s for 3.33 s); fixed by the exact-frame step |
| Unit tests | shots cover the video once; OTIO durations add up; FCPXML spine adds up; a path with a space and `&` is escaped |

## Not done

A person opening the files in Resolve, Final Cut or Kdenlive (none runs here). A clean picture without captions. Sound effects and scene ambience as their own stems (they stay in the mix only). A dissolve between shots is found as one cut, or not at all.
