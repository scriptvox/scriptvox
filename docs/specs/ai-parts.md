# `ai_parts` and a visible label (G046)

Owner rule: all rows are allowed and the user decides. This gives the user data and a tool. It is not a legal view. A legal reading of the EU rule was not made, and scriptvox claims no compliance.

- `export` adds `ai_parts` to `metadata.json`: the voice, and each generator row with its kind. Stock, the user's own media and built-in music are not in it. `parts` keeps the terms.
- `edit --label "Made with AI"`: a small line in the top left corner for the whole video, over the picture and burned in. The words are the user's. With `--hook` and `--captions` in one encode.
- Stock clips with terms per file (G038) now count in `commercial_ok`: a file with `commercial = false` makes it false. A Pexels or Pixabay pick has no such field and counts as before.
- Not done: a label in `make`, and a signed C2PA credential. `make` users run `edit --label` on the result. Add to `make` when someone asks for it in one step.
