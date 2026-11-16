# Speakers: `analyze --transcript --speakers`, `dub --voices` (G020, 8 Nov 2026)

A talk with two people needs two voices in its dub. Studied by behaviour and public docs: pyVideoTrans, ElevenLabs, WhisperX. No code read.

- `analyze <video> --transcript --speakers` and `dub <video> --plan --speakers`: each word and sentence (each sheet line) gets `speaker`, `S1`, `S2`, in the order they first speak. A new speaker starts a new sentence.
- Only a row that tells speakers apart can do it: a `stt` row whose `extra` has `diarize`, turned on for this request. Today that is `scribe-fal` only, a paid row, so it goes through the money rule of `paid-speech-rows.md` (`--approve` of its plan). No row: exit 10, with the row named. A free local route (a WhisperX plugin) is not built.
- A speakers request has its own job record, so it never reuses a transcript made without speakers.
- `dub --lines f --voices S1=af_heart,S2=am_michael`: each line with `speaker` gets that voice. A line can also name `voice` itself. No match: the dub's `--voice`.
- Brief `lines[].voice` (new): the voice of that line. Same rules as a scene `voice`: the same engine as the brief's voice, it speaks the language, and `selfhost` has one speaker. A wrong one is exit 2 before any voice is made.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| `dub base.mp4 --plan --speakers` with a real fal key and no approval | exit 4, "scribe-fal is a paid speech to text row and would hear 18 s for $0.01", nothing sent |
| Fake fal queue: `analyze --transcript --speakers`, approved | exit 0, its own submit, sentence 1 `speaker` S1 |
| Real dub of the 17.5 s talk, 6 lines marked S1 and S2 by hand, Kokoro `af_heart` and `am_michael` | first try exit 3: line 1 needed 1.47x (the budget works); with "Morning." for "Good morning.": 44.8 s, $0, all 9 gates pass |
| Median pitch of the first 2 s of each line, from the result | S1 lines 140 (a 0.6 s line), 186 and 205 Hz; S2 lines 127, 116 and 125 Hz: two voices |
| Sheet of 6 frames read by eye | captions with the new words on each line's time |

## Not done

No real speaker ids from a row: the only row is paid, and a paid call needs the owner's approval. The speaker marks of the real run were written by hand. A free local route (WhisperX) is not built. A second speaker's words are not in a second caption colour, as scene voices are. No person listened.
