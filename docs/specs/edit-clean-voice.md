# `edit --clean-voice` (G025, 7 Nov 2026)

Noisy phone talks are common. Public docs of Descript and ffmpeg-skill describe a noise cleaning step; no code read.

- `--clean-voice light|medium|strong`: `afftdn` from the pinned FFmpeg, at the front of the sound path, so it learns the noise over the whole recording, before any cut.
- `clean_voice` in the envelope: `level`, `noise_floor_db_before`, `noise_floor_db_after`. `check` reports `noise_floor_db` in `metrics` for any file. The figure comes from `astats`: None when the sound has no noise to measure.
- Alone, it copies the picture. With a cut or captions, the picture encodes as usual.

| Level | Filter |
|---|---|
| light | `afftdn=nr=8:nf=-45:tn=1` |
| medium | `afftdn=nr=14:nf=-50:tn=1` |
| strong | `afftdn=nr=24:nf=-60:tn=1` |

`anlmdn` was tried: it moved the floor 0.4 dB here, so it is not used. `arnndn` needs a model file that we do not ship.

## Measured, 7 Nov 2026: Edge voice with pink noise added (`a=0.02`), so the noise is synthetic

| Clip, level | Floor before | Floor after | Words heard |
|---|---|---|---|
| English, light | -44.0 dB | -49.4 dB | 69 of 69, same text |
| English, medium | -44.0 dB | -50.5 dB | 69 of 69, same text |
| English, strong | -44.0 dB | -52.1 dB | 69 of 69, same text |
| Arabic, medium | -43.6 dB | -51.2 dB | 26 of 26, one word spelled differently |
| English, light, with `--cut-silence` | -44.0 dB | -49.4 dB | 3 cuts, exit 0; text not read |

All gates pass. Speech to text heard the noisy clip well too, so this does not show a gain in understanding.

Hum and rumble: see `edit-hum-rumble.md` (13 Nov 2026). Not done: a real room or wind. No person listened, so "does the voice sound thin" has no answer, and the levels come from one kind of noise. The skill says to start with light.
