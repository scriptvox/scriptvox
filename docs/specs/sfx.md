# Built-in sound effects (daily, 3 Nov 2026)

Gap: Remotion ships a set of hosted sound effects that need the network; OpenMontage generates them with a paid
API. ScriptVox had none. Same rule as the beds (Q10): synthesised by FFmpeg, so no third-party audio, no licence
question, and they work with `--local-only`.

- Brief: `"sfx": true` (off by default). Placed on scenes and cards only:
  - whoosh: pink noise, band 300 to 2,500 Hz, swells for 0.35 s and dies in 0.15 s, starting 0.4 s before each
    scene cut, so it peaks at the cut;
  - pop: a 0.15 s blip falling from 1,200 to 500 Hz, when a title, stat without a number, lower third or each list
    item appears;
  - tick: a 30 ms click at 1,800 Hz on each of the 12 steps of a counting stat.
- Mix: one `[sfx]` track (`asplit` per sound, `adelay` per cue, `amix`, volume 0.6) added to the voice and bed
  before the two-pass loudness, not ducked (each sound is shorter than half a second).
- Part of the plan id, so turning it on makes a new plan.
- Measured 3 Nov on the 4-scene cards brief: the master minus the same video without effects peaks at -17 dBFS
  at the cuts (voice peak -1.5 dBFS, voice RMS -16.8 dB), and the master stays -14.0 LUFS; all gates pass.
  Not heard by a person: the level is from numbers only.
- Tests: the graph places each cue (`audio.rs`), the cues follow the card kind (`cards.rs`).
