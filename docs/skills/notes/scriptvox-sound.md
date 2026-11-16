# Notes: scriptvox-sound

Written 2 Nov 2026 before the skill. Read in full (OpenMontage, AGPL, ideas only, no text carried over):
`.agents/skills/music` (SKILL.md, api_reference.md), `.agents/skills/sound-effects/SKILL.md`,
`.agents/skills/music-to-video` (SKILL.md, planning.md, montage.md, frame-skeleton.md; analyzer script skimmed for
method), `skills/creative/sound-design.md`, `skills/creative/music-gen-usage.md`, `tools/audio/audio_mixer.py`
(ducking and loudness method only). Remotion (`remotion-markup/audio.md`, `audio-visualization.md`).
Ours: `make --help`, `schema brief`, P3-007, P3-002, `audio.rs`, registry rows `acestep-xl-turbo-self`,
`chatterbox-mtl-self`, `whisper-large-v3-self`.

## 1. What the sources teach (our words)

1. Under a voice, music is a bed: instrumental only, even in energy, no big drops. Words in a song fight the
   narration.
2. A music prompt works best as a short list: style, tempo in BPM, mood or key, instruments, energy, and the word
   that says it sits under speech. Ask for a tempo outright instead of hoping the genre implies one.
3. Busy high sounds (bright hats, leads) sit where speech is understood (about 2 to 4 kHz). Ask for warm, low,
   sparse parts, or cut that band in the bed.
4. Ducking: the voice drives a compressor on the music (sidechain). Common targets put the bed well below the
   voice while someone speaks; quick attack, slower release so the bed swells back gently in gaps.
5. Loudness: short-form platforms normalise near -14 LUFS integrated, with a true-peak ceiling around -1 to -1.5
   dBTP. Measure, then normalise in two passes; do not let the mixer average the inputs down.
6. Ask for the bed at the video length rather than looping; if it must loop, cross the loop point.
7. One bed per video unless the story clearly changes.
8. Sound effects: short, placed a frame or two before the visual change they mark, quieter than the voice,
   kept in different pitch ranges when stacked.
9. Music-led edits: analyze the track once, then trust energy, onsets and silences everywhere, but trust the beat
   grid only on truly rhythmic music. On calm music, pace by phrases and hold shots long.
10. Words on screen in a music edit need at least one beat to be read; shorter flashes are texture.
11. Remotion's audio model: every layer has a start, a trim, a volume curve over time, and can be muted per span.

## 2. Weak points of the sources

- Level advice is copied from several guides and conflicts: "duck 6 to 12 dB", "20 dB below", "up to 22 dB",
  plus a -16 default in the mixer against a -14 table. The agent cannot tell which one to use.
- Ducking amounts are given in dB, but the tool takes a compressor ratio and threshold; nothing maps one to the
  other or measures the result.
- The mixer's own comments admit two past bugs: averaging inputs quietened the voice by many dB, and a fade
  was applied to the delay silence. No skill tells the agent to measure the voice level after mixing.
- Music skills target one paid vendor API; budget numbers are that vendor's. Nothing about a model whose piece
  ends early (ours faded after about 9.5 s of 15).
- Effects skill: generate a sound from text, but no rule for where effects go in a narrated video or how loud.
- Music-to-video is tied to an HTML template engine, a 500-line librosa analyzer, sub-agents per frame and
  a pip install. It mutes all clip sound; ducking under clip audio is "later".
- EQ and compression chains for AI voices are listed with exact numbers but no tool to apply them and no test.
- "Test on phone speakers" is advice an agent cannot follow.

## 3. What scriptvox-sound must do better (rules)

What exists: brief `music` (`calm`, `pulse`, a file in the project, `none`) or `music_prompt` (not both);
`make --music`; the master in `audio.rs` (bed at 0.35, 1 s fade in and out, sidechain on the voice, ratio 10,
15 ms attack, 450 ms release, then two-pass loudnorm to -14 LUFS, -1.5 dBTP, LRA 11); `analyze` (LUFS, silence
share); `check` (silence and loudness gates); `--local-only`, `make --plan`.

Choosing the bed
1. Default for a narrated explainer: `music calm`. For a fast promo: `music pulse`. No music for a serious or
   sad message, or when the user gives none. Built-in beds have no licence question.
2. A user file: it must sit inside the project. Ask for its source and licence; record it. It loops if short.
3. `music_prompt`: one line, in this order: style, tempo in BPM, mood, two or three instruments, "instrumental,
   steady, under a voice". Never name an artist or song. No lyrics: the row gives an instrumental when none are sent.
4. Avoid prompt words that bring bright or busy highs (lead synth, hats, vocals). Prefer pads, soft piano, low bass.
5. Always `make --plan` first: it shows the music stage and the row. Under `--local-only` or with no `selfhost` key
   the run stops (exit 10) and offers the built-in bed; take it or ask the user.
6. The row only makes 15, 30, 60 or 120 s, and scriptvox asks 10 s more than the video. After the run, use `analyze`
   on the MP4 and look for a silence share well above the voice's own pauses. A bed that ends early leaves a quiet tail; if seen, remake with a shorter `duration` or another prompt.

Ducking and loudness
7. Do not tune ducking in the brief: there is no field. The master always ducks a bed under the voice.
8. After every run, read the master loudness from `check` or `analyze`. It must be -14 LUFS within 1 LU. If not,
   report it as a fault, never hide it.
9. Do not normalise again after scriptvox. A second loudnorm by another tool changes the measured master.
10. If the user says the music is too loud, the only knob today is `music none` or a calmer bed. A bed level
    field (`music_level`) does not exist; ask for it as a feature, do not edit FFmpeg by hand.

Sound effects
11. There is no sound-effect command or brief field. Say so plainly. Do not add effects with raw FFmpeg in the
    skill. Needed later: a `sfx` list in the brief (time or scene, sound, level) mixed before loudnorm.

Music-led edits
12. ScriptVox has beat analysis since 10 Nov 2026 (`analyze --beats`, tempo and beat times, `steady` false for speech) and no music-first mode: `make` needs a script, and the voice sets the length.
    For a short with no narration, say this is not supported yet. Needed: `analyze` on an audio file returning
    onsets, energy and silences, and a brief mode where `scenes` follow those times.
13. Until then, the honest music fit is scene cuts on sentence ends (`scenes`) with a calm bed; never promise
    cuts on the beat.

Re-voice with music (base_video)
14. The base video's own sound is dropped, its music too. If the user wants music, set `music` or
    `music_prompt` in the same brief; the bed is ducked under the new voice.
15. Keeping the original track low under the new voice does not exist (see scriptvox-edit notes, rule 17).

Reporting
16. Report: bed used (name, file or prompt and row), master LUFS, true peak if shown, any gate finding.

## 4. Claims to test

- Duck depth: measure the bed level with voice against voice gaps in a master (ratio 10, threshold 0.02). Is it
  at least 15 dB below the voice while speaking? Is 450 ms release audible as pumping between words?
- The bed at 0.35 before ducking: is the voice clear on a calm and a pulse bed in Arabic and English?
- Master lands at -14.0 LUFS and below -1.5 dBTP on 5 runs (measured once: -14.0 on 8.6 s Arabic).
- ACE-Step fades early: on 30 s and 60 s requests, when does the piece end? Is the 10 s margin enough?
- A prompt with an explicit BPM gives that tempo (measure on 3 beds).
- A user music file shorter than the video loops with an audible seam (stream loop, no crossfade).
- `music` plus `music_prompt` exits 2; `music_prompt` with `--local-only` exits 10 with the built-in bed offered.
- `check` loudness gate flags a master off by more than 1 LU (feed it a quiet MP4).
- Generated bed speed: 15 s in 5.7 s; check 60 s and 120 s times before promising waits.
