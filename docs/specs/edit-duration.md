# `edit --duration 60` (G069, 8 Nov 2026)

A platform has a length limit, and the user knows the length, not the factor (ffmpeg-skill describes this; no code read).

- `--duration S`: the speed is the kept length over S, after any cut or `--from/--to`, and it must be 0.5 to 2. Outside: exit 2 with the factor and the range of lengths that work ("17.50 s in 5.00 s needs 3.50x ... Give --duration from 8.8 to 35.0, or cut first"). Nothing is rendered then.
- All rules of `--speed` hold (it is set as a stand-in until the length is known): not with `--zoom`, `--broll` or `--captions-file`. Their messages name `--speed`. Not with `--speed` itself.
- The picture and the sound change together, the voice keeps its pitch (`atempo`), as with `--speed`.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| A 17.47 s `make` talk to `--duration 12` | 12.0 s, `av_drift` pass (11 ms); sheet of 4 frames: the whole talk in order |
| `--from 2 --to 12 --duration 8` on my test base | 8.01 s |
| `--duration 40` and `--duration 5` on 17.5 s | exit 2, 0.44x and 3.50x, with the range 8.8 to 35.0 |
| `--duration 12` on my test base, whose sound is 28 ms shorter than its picture | `av_drift` failed at 35 ms (one frame is 33 ms); `--speed 1.4583` by hand passed just under. The source's own drift, scaled; not a fault of `--duration` |
| Test | a 4 s clip to 3 s; 10 s refused at 0.40x; with `--speed` refused |

## Not done

A length target that cuts instead of speeding up.
