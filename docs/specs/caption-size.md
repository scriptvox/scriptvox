# Caption size (10 Nov 2026)

Before: the frame alone set the caption size (84 px on a 1080 px short side). A phone short could not have bigger captions and a dense explainer could not have smaller ones.

- Brief field `caption_size`: `small`, `normal` (default) or `large`. Also a brand field; the brief wins. No `make` flag: `make` has no flag for any caption field.
- Factors: small is 4/5, large is 5/4, as whole fractions, so `normal` is exactly the old size. On 1080 px: 67, 84 and 105 px. Seen on a 9:16 frame with a 34 word script: a line holds about 7 words at small, 5 or 6 at normal and 4 or 5 at large, and the script takes 4, 4 and 5 pages. A larger step than 5/4 would leave too few words on a line.
- The factor goes on top of the style's own size: karaoke 84, `pop` 113, `reveal` 100 become 105, 141, 125 at large. `Measurer` is built at the final size and the pages are made again, so every line with two words or more fits the same line width as before (83 percent of the width, 62 percent for `pop`).
- `pop` takes the full factor, with no cap. Measured at large `pop` (141 px): a 13 letter word is 684 px and a 20 letter word 926 px, inside a 1080 px frame. A word wider than the line stands alone on its line, as at any size; the size does not shrink for it. If it leaves the frame, `text_bounds` fails the run (exit 3).
- Arabic: the same factor after the Arabic font's own scale (120 becomes 150 on 16:9). Right to left is unchanged.
- `captions_second`, the hook and the cards keep their size: they come from the frame's base size, not from the caption size. The main captions still move up by the right amount for the second line.
- Gates: `text_bounds` reads the size from `captions.ass` and measures with the same shaper, so it sees the new size. `captions` (page times) is unchanged. `text_bounds` checks a box against the frame only: it does not check a caption against a card or the hook.
- An unknown value is exit 2 and names the three values, in a brief, a brand file and a `resume --patch`.
- No `caption_size`, or `normal`, gives the same plan id and the same `captions.ass`, byte for byte. Only `small` and `large` enter the plan id.
- `resume <run> --patch {"caption_size": "large"}` makes a child run with the voice reused.

Run on 10 Nov 2026, worktree build:
- Old briefs, `tts: none`, before and after the change: English karaoke 9:16, English `pop` with a hook 9:16, Arabic `reveal` with a second line 16:9. Three plan ids the same, three `captions.ass` files the same by SHA-256.
- Free Edge voice, English, 34 words, 9:16, with a hook: `small` (67 px, 4 pages), then `resume --patch --wait` to `normal` (84 px, 4 pages) and `large` (105 px, 5 pages). Both children: `voice_reused` true, cost 0. All 8 gates pass on each.
- Free Edge voice, Arabic, 16:9, `large` (150 px, 5 pages): all 8 gates pass.
- Contact sheets of the four videos by eye: the three sizes differ clearly, no line is cut off, the lines are right to left in Arabic, and the captions stay under the hook.
- Tests: `tests/caption_size.rs` (the size in the ASS file, the widest line measured, Arabic, the plan id, the brand file, exit 2) and `make.rs` `caption_size_scales_the_style_and_normal_is_the_old_size` (the old sizes as numbers, the three styles, the lone wide word).

Done 13 Nov 2026 (`caption-style.md`): `edit --captions`, `dub` and `clips` take `--caption-size`. Not run: `pop` and `reveal` at another size with a real voice (unit test only); a large caption together with a scene card or `caption_at` `middle` or `top`; a word that leaves the frame at large (the gate's own tests cover a box outside the frame, not this field).
