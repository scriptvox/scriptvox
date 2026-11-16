# edit --captions and --from/--to: caption any talking video, cut a clip

Gap: scriptvox could re-voice a video (translate) but not caption one that keeps its own sound, the most common edit
in CapCut, OpusClip and the MoneyPrinterTurbo audience; and it could not cut a clip out of a long video (the first
step of the clip factory, OpenMontage backlog item 24).

- `scriptvox edit <video> --from 12.5 --to 48 [--reframe 9:16 [--track]] [--cut-silence] [--captions [karaoke|pop|reveal]]
  [--lang ar]`. Any mix; a clip under 0.5 s, an unknown style, or `--captions` without a speech to text key (exit 10,
  before any encode) are refused.
- Clip: read with `-ss/-to` on the input in every pass (frame-exact on a re-encode; filters see clip time from 0).
  Only `--track` cuts the part first into `.scriptvox/edit/<name>/clip-<from>-<to>.mp4` (x264 CRF 16), because tracking
  samples frames from a file.
- Captions: a sound-only pass with the same clip and silence cuts gives the speech (16 kHz mono WAV) for the free
  selfhost Whisper row; the words become pages with the same sizing, layout and karaoke style as `make`
  (`make::caption_pages`, shared); the one video pass burns `captions.ass` (over the logo) with the bundled fonts. The transcript is cached by a hash of the
  speech plus an engine version. Envelope: `captions.{words, pages, lang, style}`.
- Whisper engine (`rawivox-gen-audio:20261103a`): a short punctuated prompt per language (`initial_prompt`). Before
  it, a clip that started mid-sentence came back in lower case with no punctuation; after it: "An old wooden dow
  drifts across the calm water of the gulf." Word alignment in `make` ignores punctuation, so the voice path is
  unchanged.
- Measured 3 Nov: an 8 s English clip cut from 8.7 s, 25 words, 3 pages, 9 s end to end; an Arabic 11.8 s video with
  `--captions pop` and no `--lang`: Arabic detected, 18 words, 13 pages, the full stop on the left. Both checked by
  eye on sheets (`night-output/editcap/`).
- One video encode for clip plus captions plus logo (was three: clip, edit, burn; 3 Nov). Measured: 6.7 s clip of a
  1080x1962 video with captions and logo in 1.2 s with the transcript cached; captions over the logo and in step,
  and the 1:1 pop path without a logo, checked by eye (`night-output/editcap/one-sheet.png`, `sq-sheet.png`).
- Known ceiling:   Whisper misspells rare words ("dow" for dhow); `--lang` and a cleaner sound help.
- Tests: clip length, refused short clip, refused style, missing key (integration); a real English and Arabic run.

`--hook` (5 Nov 2026): the same headline as a brief's `hook`, in the same encode, with no speech step when there are no captions. `--emphasis` needs `--captions`, as brief `emphasis`. Clip factories put a headline on every short (OM #24).
