# Evals: scriptvox-motion (run by hand: they cost model calls)

| Case | Pass when |
|---|---|
| static-explainer | the brief has one card at most per scene, at most one `stat` and one `title` per 20 s, list items reuse the narration's words in order, and the agent ran `snapshot --at` on each card scene and `judge` before reporting |
| no-charts | for the bar chart the agent writes a `bars` card with `items` as years and `values` as numbers and the unit in `label`; asked for a line chart instead, it writes a `line` card (one series, 3 to 8 points); asked for a pie chart, it says pie charts do not exist and offers `bars`; it writes no raw FFmpeg |
| camera-move | the agent says Wan barely follows camera words (measured), adds a scene `camera` (`pan_left` or `pan_right`) instead of rewording the prompt, resumes, and looks at two frames of that scene |
| formula-card | the brief has a `formula` card with `text` in the TeX subset (`A = \pi r^2`), not an image and no LaTeX install; a `label` says what the letters mean; the agent opened a `snapshot --at` after the write-on (1.3 s or more into the scene) before saying the formula shows |
| code-card | the brief has a `code` card with the two lines, the indentation kept and a file name `label`; the narration talks about those lines; the agent opened a `snapshot --at` after the last line typed in |
| plot-card | the brief has a `plot` card with the function in `text` and an x range in `values` that shows both points where it crosses zero; the agent opened a `snapshot --cards` or `--at` after the curve finished drawing |

All: no claim that a card moved or landed without a frame the agent looked at.
