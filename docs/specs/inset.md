# Picture in picture (5 Nov 2026)

Gap: Manim #22 (own footage inside a scene), Remotion's video layers. `scenes[i].inset = {file, corner, width}`: a clip or photo over that scene for the whole scene, under captions and logo, with a thin white edge. A clip starts at its first frame and holds its last frame if short; its sound is not used. The size is `width` of the frame (default 0.4), with a height cap (50 percent on tall, 64 percent on wide), so a tall clip fits a wide frame. Bottom corners stay above a two-line caption. Bad file, corner or width stop when the brief loads.

Checked by eye: clip and still on 9:16 (r_faa233) and 16:9, all gates pass. The first 16:9 test ran off the frame: the height cap fixed it. Skipped: rounded corners, a moving inset.
