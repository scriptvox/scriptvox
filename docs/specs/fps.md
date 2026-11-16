# Brief `fps`: 24, 25, 30 or 60

Source: OpenMontage cherry #19 (output profiles and true fps; their fps was fixed at 30). ScriptVox was fixed at 30 too.

- Brief `fps` (default 30): the render, the scene timing, the gradient, Ken Burns, transitions and camera moves all
  use it; it is part of the run request, so a change makes a new render. Other values exit 2 before any work.
- At 24 a generated 24 fps clip plays frame for frame (no blend); 60 suits fast screen content.

Measured 3 Nov 2026, the two-Wan-clip brief: 24 fps 145 frames in 9 s, 25 fps 151 frames in 11 s, 60 fps 363 frames
in 13 s, all gates pass, the stream rate matches. Motion in the Wan scene at 24: variation 0.23, no repeated frames
(the clip's own motion); at 30 with the blend 0.19.
