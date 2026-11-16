# Scenes in the brief (night of 2 Nov)

- `brief.scenes: [{script, media?, stock?}]`: the agent decides which visual goes with which line. A brief has exactly one of `script`, `script_file` and `scenes`. Scene media paths are relative to the brief.
- The narration is the scene scripts joined. Cuts fall at each scene's first word (from the TTS word times), not at caption pages.
- Each scene shows its file (a clip or a photo; inside the project, else `SVX_PATH_OUTSIDE_ROOT` before any voice call), or its stock term (fetched in one call in scene order, with credits), or else the media folder in turn, or else the built-in gradient (a new `Gradient` media kind, so gradient scenes mix with clips in one graph). Crossfades apply.
- `resume --brief` with scenes replaces the script and the scenes.
- `recipes/scenes-reel.json` is the 7th recipe.
- Tests: 3 scenes (clip, photo, none) give 3 shots; a scene file outside the project is refused; a brief with both `script` and `scenes` is refused.

## QA pass (Arabic and English, 9:16, 16:9, 1:1, with hook, scenes and music)

All 6 renders passed every gate, and the RTL order and punctuation were correct. One visual fix: Arabic captions looked much smaller than English at the same ASS size, because libass maps the size to the OS/2 win height and Noto Sans Arabic's win height is 2.17 em against 1.52 em for Noto Sans. The caption size is now scaled by each font's win-height-to-em factor (Arabic about 1.43x), so both scripts show the same em size. The measurer uses the same size, so line breaks stay exact.
