# Opt-in local speech to text (5 Nov 2026)

Gap: `edit --captions`, `--cut-fillers`, `check --hear`, `analyze --transcript` and the `words` gate of `make` worked only with a self-hosted service. A stranger had none.

- `scriptvox setup speech-local [--dry-run]`: downloads Systran/faster-whisper-small (486 MB, pinned at one revision by sha256 in `toolchain.lock`) and prepares `plugins/whisper/` with its own environment. It is opt-in for its size. All parts are MIT, so there is no notice to accept. Kokoro keeps its GPL notice.
- It runs as one process per transcript (`python -m scriptvox_whisper`) on the CPU, int8. Same JSON as the self-hosted row, so every caller stays as it was: the row `whisper-small-local` comes from `opentts::rows("stt")`, and `genjob` runs it as a process. It is not in the registry, and `providers` shows it under `speech_local`.
- A self-hosted row with a key goes first: larger model on a GPU. Nothing else leaves the machine for this step.
- `--local-only` still skips listening, and `clips` still needs a vision service to pick its parts. Local speech to text frees the transcript only.

Measured 5 Nov, `light` (2 cores), a real install and a real run, empty config:

| Check | Result |
|---|---|
| `setup speech-local` | 4 min 24 s on this network, about 1 GB with Python and packages |
| `analyze --transcript`, English, 16.5 s of Chatterbox voice | 8.4 s, all sentences right but "Rawevox" for "ScriptVox" |
| `check --hear` on that video | gate `words`: 71 of 73 |
| `analyze --transcript --lang ar`, same length | 8.2 s, usable with a few wrong words ("إقاع", "الجمة" for "ترجمة", "المنف" for "الملف") |
| `edit --captions --cut-fillers`, English | 27.8 s, av_drift pass |
| `edit --captions --lang ar` | 22.7 s, av_drift pass |
| `make --tts edge` | gate `words`: 10 of 10 |

Arabic on `small` is weaker than `large-v3` on a GPU: the captions need a read before they ship.

Fault of the first run, kept in `plugins/whisper/pyproject.toml`: `av` 16 and later refuse an argument that faster-whisper 1.2.1 passes, so `av` is pinned.

Tests: dry run with no download, the message with nothing installed, and a stub plugin through `analyze --transcript`. CI does not run the model. Not run on Windows.

## Empty alignment (G004, 5 Nov 2026)

`analyze --transcript` stopped with exit 5 on every retry on one 30.0 s clip. It is tied to about 30.0 s but not to every such file: the hand evals saw it on 4 of 5 clips of exactly 30.0 s, and never at 15, 20, 25, 29, 29.5, 29.9, 45 or 100 s. Here 29.99 s of the same sound, another 30.0 s sound and a 60 s file pass. A retry gives the same job id and the same fault, so "run it again" never helps. Cause: faster-whisper 1.2.1 with large-v3 decodes a false repeat past the end of the audio (30 to 60 s), that window aligns to nothing, and `find_alignment` raises `IndexError`. It fails the same way on CPU with int8 on this machine, so the cluster is not the cause. The `small` model of the local row did not fail on this clip.

Fix, in `plugins/whisper` and `services/gen/engines_audio.py`: `guard_alignment` turns the crash into a window with no word times, and a segment with no words is dropped. Result on the failing clip with large-v3 on CPU: 71 words, 4 segments, the last ends at 29.18 s. Test: `python/tests/test_whisper_guard.py`. Live on `scriptvox-audio` since 6 Nov (image 20261106a): 6 of 6 clips at exactly 30.0 s pass.

Limit: the guard works per 30 s window. If a window with real speech ever aligned to nothing, its words would go with no error, where before the run stopped with exit 5. An empty alignment means no aligned tokens, so this is unlikely, and it was not seen.

## Larger tier (G039, 7 Nov 2026)

`scriptvox setup speech-large [--dry-run]`: deepdml/faster-whisper-large-v3-turbo-ct2, MIT, 1.62 GB, pinned at one revision, in `model-large` beside `model`. Row `whisper-large-v3-turbo-local`. It runs when 3 GB of memory is free, or when it is the only model. Else the small row runs. A transcript cache names its row, so one model is never read as the other.

Measured on 2 cores, int8, Edge voice, the same model pair on the same audio:

| Audio | small | large-v3-turbo |
|---|---|---|
| Arabic, 29.2 s | 6.6 s, 45 of 49 words | 23.4 s, 48 of 49 words |
| English, 16.0 s | 5.4 s, 48 of 49 words | 10.3 s, 49 of 49 words |

Peak memory 2.05 GB (one Arabic run). The script here is one voice, one script per language, and a synthetic voice: this shows the direction, and no real speaker was tried. Both rows wrote "أنطار" for "أمطار". `analyze --transcript --lang ar` ran through scriptvox on the large row. No GPU.

The download failed 2 times on DNS and network faults: a rerun resumed it, as for the other pins.
