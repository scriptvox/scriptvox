# L3-C: sound that follows the picture (MMAudio, testing only)

> 5 Nov 2026: `--commercial` is deleted (`docs/specs/commercial-note.md`). Text below about it is history.

Gap: HyperFrames, OpenMontage and ComfyUI users add ambience and foley from a video model. ScriptVox had only the
synthesised `sfx` (whoosh, pop, tick), which knows the cuts and cards but not what is in the picture.

- Engine: MMAudio large 44k v2 in the audio image of `services/gen`, modes `mmaudio-large-44k/v2a` (video in)
  and `/t2a` (text only). Code MIT; **weights CC BY-NC 4.0, so testing only**: both rows are `commercial = false`,
  and `--commercial` refuses them. Inside: CLIP DFN5B (Apple) and BigVGAN v2 (NVIDIA, MIT).
- Weights relayed by `push_weights.sh` with the new `EXTRA` hook (GitHub release files, and a Hugging Face cache
  with symlinks resolved so oras keeps the files): `mirror/weights-mmaudio:v1`, 10 GB. The service reads
  it offline through `HF_HUB_CACHE`.
- Image `rawivox-gen-audio:20261103b`: MMAudio from a pinned commit tarball without its own dependency list (it
  pins numpy and gradio); the libraries it imports are pinned in the Dockerfile.
- Rows `mmaudio-v2a-self` (needs `--video`) and `mmaudio-t2a-self`, kind `sfx.gen`, 1 to 10 s (trained on 8 s).
- `generate` on an `sfx.gen` row with `--video` and no `--seconds` asks for the clip's own length (it took the row's
  first length, 1 s).

Measured 3 Nov 2026, all free:
- First call 65 s (model load), then a 5 s clip in 5.7 s. TTS on the same pod still works after the roll.
- Calm dhow clip: water texture with soft splashes, -26 LUFS. Stormy waves clip: a loud rising swell, -17 LUFS. The
  level follows the picture.
- Whisper on the dhow sound: 0 words (the default negative prompt "music, speech, voice" holds).
- Text only, "a door creaks open, then footsteps on a wooden floor", 6 s: quiet room tone, then a run of sharp hits
  from about 4 s. Judged from the spectrogram only: no one has listened yet.

Against the synthesised `sfx`: they do different jobs. `sfx` marks edits (cuts, cards, counts) and is always
allowed; MMAudio gives the scene its own sound. Not wired into `make` yet: a brief `"ambience": true` for generated
scenes, mixed about 12 dB under the voice, is the next step, and only for non-commercial runs until a commercial
model is found.

## Brief `ambience` (3 Nov 2026, evening)

- `"ambience": true`: for each generated scene, the clip goes to the first `sfx.gen` row whose endpoint is a `v2a`
  mode (MMAudio), with the scene's prompt and the clip's length (1 to 10 s). The sound is resumed from the run
  folder (`scene<N>_sound.wav`) on a rerun.
- `audio::master` places each sound at its scene start, at volume 0.25 (about 12 dB down), with 0.3 s fades at both
  ends, then mixes it with the voice, bed and `sfx` before the loudness pass.
- `--commercial` with `ambience` stops with exit 2 before anything is sent (unit test
  `ambience_is_refused_for_a_client_video`). No key: exit 10.
- Measured: 2 Wan scenes (stormy waves, calm dhow) with Edge voice: all gates pass, -14.0 LUFS. Scene sounds -17.5
  and -28.4 LUFS before the gain. The spectrogram shows a broadband sea under the voice in scene 1 and a much quieter
  bed in scene 2. Not yet listened to by a person.
- Stock and own-media video scenes get ambience too (prompt: the stock term, or none for own media). The row gets a
  480 p copy cut to the shorter of clip and scene, no sound (a long 4K stock clip would pass the 64 MB upload
  limit). Measured: 2 own-media scenes, uploads 32 KB and 108 KB, whole video in 33 s, all gates pass.
