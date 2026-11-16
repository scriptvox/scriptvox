# `scriptvox join` (5 Nov 2026)

Gap: OpenMontage #27 lists stitch among footage edits; ScriptVox could trim and cut but not join. Own code, FFmpeg filters.

`scriptvox join a.mp4 b.mp4 ... [--transition NAME] [--aspect A] [--fit pad|crop] [--out F]`. Each clip fitted to the frame (first clip's size unless `--aspect`), one rate (nearest allowed to the first clip), picture and sound locked to its length, silence for a clip with no sound. Transitions overlap, as in a brief; sound crossfades over the same time. Master -14 LUFS. Gates: av_drift, length.

Measured: 5.04 s tall silent clip, 25.67 s wide, 8 s variable-rate, dissolve: 37.92 s (expected 37.91), av_drift 2.7 ms, 10 s, checked by eye. Also an MCP tool.

Independent review, same day: the frame follows a clip's display rotation (a phone clip stored 1920x1080 and turned 90 degrees joins as 1080x1920; `analyze --like` too). The default output no longer overwrites an earlier join. Streams are found with ffprobe, not by a word in FFmpeg's banner (`analyze` too).

## Sound with a hole or a late start (12 Nov 2026)

Each clip's sound goes through `aresample=48000:async=1:first_pts=0`: a hole in the sound's times, or sound that starts
after time 0, is filled with silence. Before this, `atrim` counted the hole as sound and the join came out with less
sound than picture (video 24.000 s, sound 23.837 s, gate `av_drift` fail). The holes came from `edit`: its loudness
step now sets new times (`asetpts`) before it trims, and `edit` cuts the sound on frames of 64 samples, so each cut is
exact to 1.3 ms against the picture. Tests: `join_fills_a_hole_in_the_sound_of_a_clip`,
`a_cut_of_a_quiet_clip_keeps_all_of_its_sound` in `tests/edit.rs`.
