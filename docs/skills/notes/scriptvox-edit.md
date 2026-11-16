# Notes: scriptvox-edit

Written 2 Nov 2026 before the skill. Read in full (OpenMontage, AGPL, ideas only, no text carried over):
`.agents/skills/video-edit` (SKILL.md, references/operations.md), `.agents/skills/video-translate/SKILL.md`,
`.agents/skills/video-understand` (SKILL.md, output-format.md, script skimmed for method), `skills/creative/video-editing.md`,
`skills/creative/video-understand-usage.md`, `skills/core/subtitle-sync.md`, `tools/video/silence_cutter.py` and
`tools/video/auto_reframe.py` (method only). Remotion (`packages/skills/skills/remotion-captions`, all four files).
Ours: `scriptvox --help`, `edit`, `analyze`, `make`, `schema brief`, P2-022, P3-003, P3-002, `make.rs` (base_video), `edit.rs`.

## 1. What the sources teach (our words)

1. Look before you cut. Probe length, size and frame rate first; sample frames; get the words with time stamps.
2. Shot detection compares frames for change; a fixed level decides a cut. If nothing is found, fall back to
   frames at even spacing so the agent still sees the whole video.
3. Pause removal: find quiet stretches below a dB level and longer than a minimum, keep a little air on each side
   so word starts and ends are not clipped, then join what is left. A softer option plays the quiet parts fast.
4. Keep natural breaths (well under a second). Remove only long dead air, false starts and repeated takes.
   Short videos take harder cutting than long ones.
5. Cut on word edges, never inside a word. Check for clicks at each join.
6. Vertical reframe: crop a 9:16 window from a wide frame. Best case follows the face, smoothed so the window
   does not jitter; fallback is the centre. Pixel aspect (SAR) must be handled or the crop is wrong.
7. Translation services do the whole job in one call (new voice, optional lip sync, glossary of terms kept as is,
   custom subtitle file). They warn that one clear speaker at a steady pace works best, and that jobs are slow.
8. Captions: keep each line short on a phone (few words per cue), show a cue at least half a second, keep it off
   the face, start it with the word. Captions belong to the clip they came from, so a trim moves both together.
9. Transcribe each source file on its own; keep its times relative to that file.

## 2. Weak points of the sources

- The video-edit skill is a list of raw FFmpeg lines. Stream-copy trims land on keyframes, so cuts drift by up
  to a second and nothing checks it. The 9:16 "resize" pads with black bars, which is not a reframe.
- The editing skill names J and L cuts but gives no tool that can make them. Its 1.5 s dead-air rule and the
  silence tool defaults (0.5 s, 0.08 s air) disagree with each other; neither says which to use.
- Pause removal runs on the mixed track. Under a music bed there is no silence, so it silently does nothing.
- The scene mode uses one fixed level on every frame. A crossfade changes a little per frame and is missed
  (we measured this: our crossfades score about 0.09 per frame). No sampling rate is stated.
- The understand skill keeps only 20 frames and drops the middle when there are more; short shots vanish.
  Keyframe mode has no times at all.
- Translation is one paid vendor with a 30-minute poll. The agent never sees the translated words before the
  money is spent, cannot fix a term, and cannot check timing. No Arabic right-to-left note at all.
- Face-tracked reframe needs extra Python packages; without them it quietly becomes a centre crop and still
  reports success.
- Quality checks (blur, brightness) are numbers from a model the agent may not have; none checks the edit itself
  (length before and after, cut count, sound at the joins).

## 3. What scriptvox-edit must do better (rules)

Commands that exist: `analyze`, `analyze --transcript [--lang]`, `edit --cut-silence [--min-silence --pad
--noise-db]`, `edit --reframe 9:16|16:9|1:1`, `make --brief` with `base_video`, `lang`, `duration`, `aspect`,
`voice`, `tts`, `music`, `hook`, `script` (with `[pause N]` tags), `check`, `snapshot`.

Analyze first
1. Every task starts with `scriptvox analyze <video>`. Read `shot_list`, cuts per minute, LUFS, silence share and the
   `shots_image` strip before deciding anything. Add `--transcript` when the task touches words.
2. If the shot count looks too low for what the strip shows (one shot, but the picture clearly changes), run again
   with `--threshold 0.15`. Crossfaded videos need the lower level. Report which level found the cuts.
3. Use `scriptvox snapshot --sheet N` to see the middle of long shots; the strip shows one frame per shot only.

Cut silence
4. Use `edit --cut-silence` only on speech without music. If `analyze` shows a silence share near zero on a
   talking video, assume a bed is present: say so and do not run the cut (it would remove nothing).
5. Start from the defaults (0.6 s pause, 0.15 s air, -35 dB). For a noisy room raise `--noise-db` toward -30.
   For a calm talk raise `--min-silence` to 1.0 so breaths stay. Never set `--pad` below 0.1.
6. After the cut, read `seconds_before`, `seconds_after` and `cuts`. More than one cut every 3 s on a talk means
   the level is too high: rerun with a lower `--noise-db`.
7. Then `scriptvox check` the output and `snapshot` two join points. Not yet possible: cutting filler words or
   false starts by transcript (needs a word-level cut list in `edit`, which does not exist).

Reframe
8. `edit --reframe 9:16` is a centre crop, scale to cover. Before using it, look at the strip: if the subject
   sits off centre in most shots, tell the user the subject will be cut, and offer 1:1 as a softer crop.
   Face or subject tracking does not exist yet (waits for a detector).
9. Do not crop twice. If the next step is `make` with `base_video`, skip `edit --reframe` and set `aspect`
   in the brief; the base video is scaled to cover the frame there.
10. Order when both are wanted: `edit --cut-silence --reframe 9:16` in one call (one encode).

Re-voice and translate (base_video)
11. Steps: `analyze --transcript --lang <src>`; the agent writes the new script from the transcript; a brief with
    `base_video`, `lang`, `voice` (or `tts selfhost` for the open voice), `music none` or a bed; `make --plan`, then run.
12. The new video length is the new voice length. If the voice is longer than the base video, the last frame
    freezes; if shorter, the video is cut early. So set `duration` to the base video length from `analyze`.
13. Keep sentence timing close to the source: where the transcript has a gap longer than 0.8 s, put a
    `[pause N]` tag of that length in the new script. This keeps speech near the matching picture.
14. Arabic usually runs longer than English for the same meaning. Write the Arabic tight: aim for the source
    word count or fewer, then check the voice length in the plan.
15. Keep names, product terms and numbers exactly as in the source transcript. The agent makes the glossary.
16. The source sound is dropped. If the base video has on-screen speakers, warn that lips will not match;
    lip sync does not exist and is not planned.
17. Keeping the original sound low under the new voice is not possible today (needs a brief field such as
    `base_audio: duck`, which does not exist).
18. Captions always show the new script, right to left for Arabic. A separate SRT export does not exist.

After every edit
19. `scriptvox check <out.mp4>` and one `snapshot --sheet 6`. Report the path, length before and after, cuts,
    LUFS, and anything the gates flagged. Never claim a fix that was not looked at.

## 4. Claims to test

- A crossfaded scriptvox video gives 2 shots at `--threshold 0.25` (4 fps). Does a slow 1 s dissolve in outside
  footage need 0.15? Find the level that misses no cut and adds no false cut on a moving shot.
- `--cut-silence` on a talk with a ducked music bed cuts nothing. Measure; if true, the skill rule 4 stands.
- Default 0.6 s / 0.15 s keeps breaths and leaves no clicks on Arabic speech. Listen at 5 joins.
- `--cut-silence` and `--reframe` together give one encode and the same result as two calls.
- `edit` output loudness: is it left as the source (no loudnorm in `edit.rs`)? Measure with `check`.
- base_video longer than the voice is cut at the voice end; shorter one freezes on the last frame (tpad clone).
- `duration` set to the base length fits a translated Arabic voice without audible speed change.
- `[pause N]` tags from transcript gaps keep each new sentence within 0.5 s of its source sentence.
- Whisper large-v3 transcript of the source keeps product names right in ar and en (count errors on 3 videos).
