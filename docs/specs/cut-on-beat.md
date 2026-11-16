# Brief `cut_on: "beat"`: scene cuts on the beat of the music (G123, second part, 13 Nov 2026)

Why: `analyze --beats` gives the grid of a music file (`beats.md`), and `make` did not use it. A cut that lands
on a beat looks planned. The idea is common in editors (the 10 Nov comparison names three); no code read.

- Brief: `"cut_on": "beat"`, with `scenes` and a music bed that `make` has before the voice is made: `music` as a
  built-in bed (`pulse`) or a music file inside the project.
- What moves: each scene boundary but the first, to the nearest beat of the bed, when that beat is at most
  **0.25 s** away. The picture cut, the scene's card and the whoosh of `sfx` move together. So do a scene's clip
  sound and `ambience`: they start at the cut (read in the code, not run).
  `scenes.json` holds the new times.
- What never moves: the voice. Its file, the word times and the captions are the same with and without the field.
  A cut that moves earlier shows the next picture before its first word; a cut that moves later holds the picture
  over the first word of the next scene, at most 0.25 s.
- The grid: the bed is read as `analyze --beats` reads a file (one steady tempo, 60 to 200 BPM). The bed starts
  with the video, so its beat times are the video's times. A file shorter than the video loops, and the grid
  starts again at each loop.

## When a boundary cannot move

It stays where it was, on the first word of its scene, and the result says why. Nothing fails.

| Case | `stayed` reason |
|---|---|
| The nearest beat is more than 0.25 s away (a slow track: at 60 BPM half of all places) | `no beat within 0.25 s` |
| The move would make the scene before or after it shorter than 1 s, or put two boundaries on one beat | `a scene would be shorter than 1 s` |
| The bed has no steady beat (`calm`; confidence under 0.25), or is shorter than 4 s | every boundary: `the music has no steady beat`, and a `cut_on` warning in `findings` |

Result: `cut_on` with `to`, `bpm`, `confidence`, `limit_s`, `moved`, `stayed` and `boundaries`: one entry per
boundary with `scene` (the scene that starts there, from 2), `from`, and `to` and `moved_s`, or `stayed`.

## Errors (exit 2, before any voice or render work)

| Message | Fix line |
|---|---|
| unknown `cut_on` | Use "cut_on": "beat", or drop the field. |
| `cut_on` needs `scenes` | Give the video as `scenes`, or drop cut_on. |
| `cut_on` needs a music bed | Set music to pulse or to a music file inside the project, or drop cut_on. |
| the music comes from a search or a music row | Get the file first and give it as `music`, or give `music` a built-in bed, a file or an https address, or drop cut_on. |

The field is part of the plan id.

## Measured, 13 Nov 2026, 2 cores

Real runs: 4 scenes, 37 words, the local voice (`kokoro-rust`, `--local-only`), `transition: cut`, `sfx: true`,
each scene a picture of another colour, 13.87 s, 416 frames at 30 fps. Without the field the boundaries are at
4.066, 6.846 and 10.500 s (the first word of scenes 2, 3 and 4).

| Bed | `bpm`, `confidence` | Boundaries with `cut_on` | Moved by | Picture changes at (read from the frames) |
|---|---|---|---|---|
| `pulse` (100 BPM swell) | 100.2, 0.95 | 4.086, 7.079, 10.671 | +0.020, +0.233, +0.171 s | 4.133, 7.100, 10.700 |
| A click file, 120 BPM, clicks at 0, 0.5, 1.0 ... | 120.2, 0.89 | 4.027, 7.022, 10.515 | -0.039, +0.176, +0.015 s | 4.067, 7.067, 10.533 |
| `calm` (no beat) | 176.9, 0.20, not steady | none moved, `cut_on` warning | 0 | 4.100, 6.867, 10.533 |
| No field, `pulse` or the click file | | | | 4.100, 6.867, 10.533 |

- The voice never moved: `voice` has the same MD5 (`0aac5772...`) in all 5 runs, `words.json` and `captions.ass`
  are the same byte for byte, and the `.srt` beside each video has the same MD5.
- Length and frames: 13.867 s and 416 frames in every run.
- The picture changes 1 to 1.4 frames (33 to 47 ms) after the boundary, with and without the field: the cut
  lands on the frame grid as before.
- Click file: the three new boundaries are 27, 22 and 15 ms after a click. That is the grid's own error
  (`beats.md`: within 40 ms).
- `pulse`: its swell is loudest at 0, 0.6, 1.2 ... s. The grid marks the rise of each swell, so the cuts are
  about 0.11 to 0.13 s before the loudest point (4.086 against 4.2; 7.079 against 7.2; 10.671 against 10.8).
- With `sfx`, the sound of the two click runs differs most in the 0.2 s around each boundary (3.95 to 4.05, 6.75
  to 7.0, 10.4 to 10.55 s): the whoosh moved with its cut.
- The default transition (fade), `pulse`, the same boundaries: the fade starts on the boundary and takes 0.23 s
  (frames 7.100 to 7.333 for the boundary at 7.079; without the field 6.867 to 7.100 for 6.846). So with a fade
  the beat is the start of the change, and its middle comes about 0.13 s later. All gates pass, 416 frames.
- A title card on scene 2 (test, no voice): its window in `cards.json` starts and ends on the two moved
  boundaries; without the field, on the two word times.
- Frames at 6.9, 7.05 and 7.15 s of the `pulse` run, read by eye: scene 2's picture at 6.9 and 7.05 with the
  caption of scene 3 already on it, scene 3's picture at 7.15.

Tests: `tests/cut_on_beat.rs` (no voice, a click file: each boundary within 0.25 s, on a click within 60 ms, the
picture change at that time to the frame, the `.srt` the same, another plan id; `calm`: nothing moves, a warning;
4 refusals at `plan`) and 3 unit tests in `src/beats.rs` (the limit, a short scene, no steady beat, a looped file).

Not run: real music with drums, a file that loops under the video (unit test only), a video longer than 14 s,
scenes with generated clips, scene `sound` or `ambience`, a slide transition, a card with `on` or `hold`. No
person watched.

## Limits

- One steady tempo. The tempo is known to about 0.3 BPM (`beats.md`), so on a long track the grid drifts from
  the music: about 0.1 s after 60 s at 100 BPM. The grid is not fitted to each beat.
- A voice does not know about the beat: only the picture moves, so a scene can start up to 0.25 s before or
  after its first word.
- Music from a search or a music row arrives after the cuts are planned, so it is refused. Music from an address is fetched before the voice since 14 Nov 2026 (`reel-gaps.md`).
- A boundary within 0.25 s of the place where a short file loops does not see the beats on the other side.
