# Notes: scriptvox-review

Written 2 Nov 2026 before the skill, from a full read of (OpenMontage, AGPL, ideas only): `skills/meta/reviewer.md`,
`.claude/skills/video-understand/SKILL.md`. No text is carried over.

## Ideas worth keeping (our words)

1. A finding is only useful when it points at a place (a frame time, a field) and carries the exact change. "The
   hook is weak" helps nobody. "At 0.6 s the hook wraps to three lines: cut it to five words" does.
2. When one fault appears, look for the same fault elsewhere before fixing. Fixing one bad stock shot and missing the
   two others costs a whole extra round.
3. Grade by consequence: some faults must be fixed before the video goes to the user, others are worth a fix only if
   a round is left, and taste comments without a rule are not findings.
4. Stop after a fixed number of rounds and report what is left. Endless polishing wastes time and money.
5. Review the real output, not the plan: look at frames of the rendered file.
6. Never claim what you did not check. If the file was not inspected, say so.
7. Sample frames across the whole video, and look closer around problem times.

## Weak points of the sources

- The reviewer is a 350-line rule book about the source's own artifacts (decision logs, playbooks, runtime choice,
  "atelier" doctrine). Most of it checks paperwork, not the video a viewer sees.
- Many checks are mandatory records with no tool behind them. The agent must fill forms that nothing reads.
- No link between a frame and the line of narration it belongs to. The agent guesses from timing.
- The frame tool needs a separate Python script and an optional Whisper install. It reads frames but gives no
  captions and no fit data.
- Audio cannot be judged by a model that cannot listen, yet the sources ask for an "audio spot check" without saying
  how.

## What our skill does better

- Code does the measurable checks first (`check` gates: black, frozen, silence, loudness, drift) and returns a `fix`
  line. The agent judges only what a camera eye can judge.
- `snapshot` now returns each frame's time and the caption on screen at that time (added 2 Nov 2026 for this skill),
  so every frame maps to a line and a scene of the brief with no guessing.
- The result now carries `voice_rate_pct` and `voice_tempo` (added 2 Nov 2026), so the agent sees a dragged or rushed
  read as a number and fixes the word count, without listening.
- Three grades with a short fixed list each, two rounds, then an honest report. The fix is always an edit to
  `brief.json` and `scriptvox resume`, so every fix is recorded and repeatable.
- Arabic-specific checks (letter joining, right-to-left order, punctuation side), which no source has.
