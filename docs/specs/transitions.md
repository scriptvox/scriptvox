# One transition per video (daily, 3 Nov 2026)

Gap: HyperFrames ships a transition catalogue and Remotion a transitions package; scriptvox had one 0.25 s
crossfade. Idea from the HyperFrames motion notes (few transitions per film, one direction of travel); our own
design, FFmpeg `xfade` only.

- Brief: `"transition": "fade" | "slide" | "cut"` for the whole video. Default `fade` (0.25 s, as before). `slide`
  is a 0.4 s soft-edged push that moves with the reading direction: `smoothleft` in English, `smoothright` in Arabic.
  `cut` is a one-frame fade, so every option keeps the same shot timing, length and captions. Part of the plan id;
  an unknown name is exit 2.
- Each shot except the last renders longer by the transition length, as with the crossfade.
- Checked by eye on 3 Nov with a still and two clips (9:16): the English slide enters from the right, the Arabic one
  from the left; all gates pass. `zoom` (`zoomin`) was tried and dropped: its middle frame washes out to a pale
  blur.
- Tests: `transitions_follow_the_reading_direction` (names, direction, cut length, refusal, the graph string).

## More names and a transition per scene (5 Nov 2026)

HyperFrames ships 14 shader transitions and Remotion 21 presentations; both need a browser. New names, each an FFmpeg
`xfade` kind with a length chosen by eye (0.3 to 0.5 s): wipe, push, cover, dissolve, blur, circle, dip (through black),
flash (through white), pixel, radial, wind.

- `scenes[i].transition` overrides the video's name for the cut into that scene. On the first scene it stops at `plan`
  (exit 2). The timeline does not change: the shot before grows by the transition's length.
- Sideways moves (slide, wipe, push, cover, wind) follow the reading direction.
- FFmpeg's `zoomin` stays out (see "A real `zoom`" below for ours): tested again on detailed pictures, frame by frame, its middle frame is one flat colour (the 3 Nov
  finding holds). A real zoom needs our own filter.
- Checked by eye: 10 transitions in one film at each scene's first word (r_c40bf7), and frames across 6 of them on
  test pictures. Unit test: all names exist; a scene's own name and length reach the graph.

Own `zoom` by an `xfade` custom expression (5 Nov 2026): rejected. 12 frames at 1080x1920 took 130 CPU seconds, and `st()` registers clash across FFmpeg's slice threads (garbage frames). A later zoom needs its own scale step on the shots, not per-pixel expressions.

## A real `zoom` (5 Nov 2026, rerun)

`transition: zoom` (brief, scene, `join --transition`), 0.4 s: a fade over two shots that move. The last 0.4 s of a shot pushes in to 145 percent and gathers speed, the first 0.4 s of the next settles from 120 percent to 100. One `zoompan` step on each shot it touches, after any camera move. The name never reaches `xfade`, because FFmpeg's `zoomin` washes out its middle frame.

Measured, 18.2 s, 3 detailed stills, 1080x1920: fade render 12.2 s, zoom 21.0 s, because `zoompan` runs on all frames of the shot. Same on 16:9 (20.8 s). Length same, every gate passes. Frame by frame across a cut, by eye (r_48e374, r_862269): detail stays in the middle frame, both pictures visible, no flat colour. Motion is seen only on frames: no person watched it at speed.

## Five more names (P1.3, 13 Nov 2026)

Five plain FFmpeg `xfade` kinds that the brief could not name before. No new filter and no per-pixel expression.

| Name | xfade kind | Seconds | What is seen |
|---|---|---|---|
| `reveal` | `revealleft`, in Arabic `revealright` | 0.35 | the old shot slides away and the new one is under it |
| `iris` | `circleclose` | 0.45 | a soft circle closes on the old shot |
| `split` | `vertopen` | 0.4 | the new shot opens from a line down the middle |
| `slice` | `hlslice`, in Arabic `hrslice` | 0.45 | the new shot comes in thin upright strips |
| `rise` | `slideup` | 0.35 | the new shot pushes the old one up and out |

- They work where every name works: the brief's `transition`, a scene's `transition`, `join --transition`.
- `reveal` and `slice` follow the reading direction. The other three have no side.
- An unknown name is refused as before, and the message lists all 20 names. FFmpeg's `zoomin` stays out.
- Names of the brief that do not use a new name do not change, so old plan ids do not change.

**A transition must never lose frames.** `tests/transitions.rs` renders the shot graph of `make` for every one of the 20 names, at 24 fps and at 30 fps (the second in the Arabic direction), over four stills with the scene times of the brief that lost its last scene on 13 Nov 2026, and counts the frames FFmpeg wrote: 149 at 24 fps and 186 at 30 fps for each name. Limit of this test, measured: with the two-frame tail of `media::xfade_tail` taken out, it still passes on this machine, and so did 150 random sets of scene times. So the test proves the count for each name, and it does not show the fault of 13 Nov again. The unit test `a_shot_runs_two_frames_past_its_cut_so_the_xfade_reaches_the_next_shot` stays the guard for the tail itself.

Run on 13 Nov 2026, `tts: none`, 24 fps, 9:16, three stills, one video for each new name in English and one in Arabic (10 videos): 168 frames of 168 (7.0 s) in English and 184 of 184 (7.67 s) in Arabic, all 8 gates pass on each. By eye on a sheet of 8 frames across the first cut of each video: the motion of each name is as the table says, `reveal` and `slice` turn in Arabic, and the last frame of each row is the whole new shot with nothing left of the old one.

Not done: the other xfade kinds (diagonal wipes, squeeze, the up and down forms of wipe, cover and smooth); a length set by the brief. Not run: these five in `join`; at 60 fps; on clips that move (stills only).
