# Text bounds gate (G176)

Every text box `make` burns is in `captions.ass` or `behind.ass`: captions, hook, second line, scene cards, end card, titles behind the subject. `gates::text_bounds` reads those files after the render, with no model.

- Width comes from `captions::Measurer`, the shaper that wrapped the text, so Arabic is measured joined. Size, font, scale and position come from the override tags and the style. Where text rests counts: the end of a `\\move`, the last `\\fscx`.
- Box: width by the longest line, height by the number of lines times the font size (`Measurer` maps the size to the line height, as libass does). `\\an` and `\\pos` or the style margins place it.
- **Fail**: the box leaves the frame by more than 1 px. `make` exits 3 (`SVX_GATE_FAILED`), the message and finding name the scene, and the video is kept. Text cut off by the frame is a measured fault, as `black` and `av_drift` are.
- **Warn**: inside the frame but closer than 2 percent of the side to an edge. It is still readable and may be meant, as `loudness` and `silence` warn. 2 percent is under the closest place our own layouts use (the corner label, 40 px on 1920).
- A family that is not embedded or the brief's own, or text the shaper cannot read, is counted as "not measured" and never silently passes as measured. If nothing could be measured the finding warns.
- Scene number: the last scene started at the box's time. A video with no `scenes` is scene 1.
- A formula is checked for width only: the math font's win metrics make a glyph's box several lines tall.
- `make`, `resume`, `remake` and `dub` run this gate (`dub` renders through `make`). `edit` runs it on the captions, hook and label it burns, with no scene numbers, and exits 3 with the file kept. `check <run>` shows the run's own finding and exits 3 when it failed, as `wait` does. `check` on a plain file cannot run it: there is no record of the boxes.

Tests: `gates.rs` (English, Arabic shaped against letter by letter, `\\pos`, `\\move`, scale, drawings, unknown font), `cards.rs` (every labelled kind, English and Arabic, four frames), `tests/gates.rs` (`make`: exit 3 with the scene number, and a pass).

## A card's text over the captions (10 Nov 2026)

Two agents saw the same hole on 10 Nov: the gate checked each text box against the frame, so a card label lying on the captions passed. The gate now also compares every card text box (style `Card`) with every caption box (style `Cap`) that is on screen with it for 0.1 s or more. When the two text boxes share pixels, the gate is `warn`: "scene 2: "revenue growth" lies 45 px over the captions", one entry per card text and scene, with the fix (shorter text, `caption_at`, a smaller `caption_size`). A box that leaves the frame still fails first. The boxes are the text alone, without the padding of its background, so a label that only touches the caption's background is not reported. A formula has no measured height and takes no part.

A warning and not a failure for now: the box heights are the font size times the lines, a little taller than the ink, so a near miss can read as a small overlap. Unit test `a_card_text_over_the_captions_is_a_warning_with_its_scene` (clear, 45 px over, and the same place at another time). Real runs: a 5-scene card video, and an Arabic stat with `caption_size` large on 16:9, both pass with no warning; `caption_at` middle with a card is already refused at plan time.
