# Scene `camera`: a sure camera move made by scriptvox

Gap: Wan 2.2 5B (our free video row) ignores camera words. Measured 3 Nov 2026 on one scene, camera motion from
`vidstabdetect` vectors at 352 px wide: no camera words 0.5 px/s, "slowly pans right" 0.0, "handheld tracking shot"
0.9, "fast drone shot flying forward" 3.4 (an aerial view, almost still); a Kling clip 11.5. Remotion, HyperFrames
and Manim users move the camera in code; scriptvox moved only stills.

- Scene field `camera`: `push_in`, `pull_out` (zoompan at 2x, 12 percent over the scene, half-pixel steps),
  `pan_left`, `pan_right` (per-frame crop of a 12 percent larger frame). `media::camera` builds the filter; it goes on
  video shots after the frame-rate step. Stills keep their own push-in.
- Unknown names exit 2 before TTS, with the other named options.

Measured: two static Wan clips as scenes with `push_in` and `pan_right`, 17 s for the make, all gates pass. The
frames show the push (boat larger at 3.2 s than at 0.1 s) and the pan (boat drifts left). Frame-to-frame change
variation 0.16 and 0.20, no repeated frames (smooth). `vidstabdetect` reads 0 on these synthetic moves under the
fixed caption boxes, so the check here is by eye and by frame change. Test `media::tests::camera_moves_are_known_or_refused`.

G150 (6 Nov 2026): the move stopped when the clip ended, so a 10 s scene with a 5 s clip froze from 5.5 s on (slideshow gate 44 to 47 percent, with or without `camera`). `tpad` now comes before the camera filters, so the move runs over the held last frame too. A pan also uses `crop` with `exact=1`: the default rounds to even pixels, 2 px steps every 4 frames on a slow pan, now 1 px steps every 2 frames. Real run with the same 5 s Wan clip in a 10 s scene: slideshow gate 0 percent frozen, frame change in 5.5 to 7 s no longer near zero, contact sheet shows the sacks still move past 5 s. Test `media::tests::a_camera_move_runs_over_a_held_last_frame`.
