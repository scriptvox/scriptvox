# `edit --cut-still` (G070, 8 Nov 2026)

A screen recording often has long parts where nothing moves (auto-editor's docs describe cutting them; no code read).

- `--cut-still`: FFmpeg `freezedetect` (noise -60 dB) finds parts where the picture stands still for `--min-silence` seconds or more (default 0.8). With a sound track, only the parts that are also silent (same `silencedetect` as `--cut-silence`) are cut: a still slide under a voice stays. With no sound track, every still part goes. `--pad` keeps a little of each side, as for silence.
- The result has `cut_still`: how many parts and their seconds.
- It counts as a cut for every rule: not with `--keep`, `--keep-words`, `--cut-fillers`, `--cut-retakes`, `--zoom`, `--broll`, `--blur` or `--captions-file`; and not with `--cut-silence` (one way to choose the cuts). Those messages name `--cut-silence`.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| 10 s recording: moving 0 to 3 s, still 3 to 7 s, moving 7 to 10 s, silent sound | 6.3 s left, 1 cut, `cut_still` 4.03 s, `av_drift` pass |
| Time code read by eye at 1.0, 3.0, 3.2 and 5.5 s of the result | 1.000, 3.000, 3.000 (the kept pad of the still), 9.167: the still part went, the rest is in order |
| The same with a tone under the whole video | nothing cut, 10.0 s: the still part is not silent |
| Unit test | the freeze log parses (also a freeze to the end); still and silent parts meet |

## Not done

A cursor that blinks over a still screen breaks a freeze (it is movement); a higher noise level would be needed. Not run on a real screen recording.
