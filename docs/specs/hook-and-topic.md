# Hook title card and `make <topic>` (night of 2 Nov, polish)

- `make --hook "text"` (or `hook` in the brief): a big centred headline (1.5x the caption size) in the same 65% black box for the first 1.6 s, with a short pop-in and fade. It is measured with harfrust and wrapped greedily inside 86% of the frame width (libass never re-wraps), escaped like captions, and checked by the text lint.
- `scriptvox make "<topic>"` with no script or brief: exit 10 `kind:"script"`. The calling agent writes the narration (PLAN-R1 D3: no LLM call in v0.1), and `next` points to `scriptvox schema brief`.
- Tests: hook style and first line, escaping, the wrap width, and topic-only giving `kind:"script"`.
