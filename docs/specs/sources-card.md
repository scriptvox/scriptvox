# sources_card: the publishers on the last scene (13 Nov 2026)

Plan item P2.2 R8, second part, of `docs/plan/MILESTONE-PARITY.md`. The first part (sources in `metadata.json`, the credits and the recipe) is `docs/specs/brief-sources.md`.

Why: a viewer of a video with facts wants to see where the facts come from, without a long address on the screen.

```json
{"scenes": [...], "sources": [{"text": "...", "url": "https://...", "publisher": "Wikipedia"}], "sources_card": true}
```

## 1. What it does

- Off by default. A brief with no `sources_card`, or with `false`, is as before: no card, and the same plan id.
- With `true`, the code adds one card to the last scene when the brief is read. Nothing in it comes from a model.
- The card is a `lower_third`: the word "Sources" (Arabic: "المصادر"), and under it the names of the publishers, in the order of `sources`, each one time, with ", " (Arabic: "، ") between them.
- The name of a source is its `publisher` (a new optional field of a source), else its `title`, else the host of its `url` without `www.`. `write` fills `publisher` from the research file.
- The line does not wrap. It holds 30 characters on a frame 1080 wide (9:16, 1:1, 4:5), 42 on 4:3, 56 on 16:9. Names that do not fit are left out, and the line ends with " and more" (Arabic: " وغيرها"). No count is shown: a number on a card is a claim for gate `claims`.
- Arabic is used when `lang` is `ar`, or when there is no `lang` and a scene has Arabic letters.
- The last scene has a card already: the two become `cards`, and the Sources card is the last. With no times they share the scene (`scene-cards.md`). It has `cards`: the Sources card is added to the list.
- After this the card is a normal card: the checks of a card, `text_bounds`, `claims` and `resume` see it as any other. The field itself is cleared, so the request of a run has the card and not the field, and a second read adds no second card.
- Gate `claims` accepts a number that is in the name of a source ("France 24"): its `publisher`, else its `title`.
- `resume --patch` takes `sources_card` (13 Nov 2026). A run keeps the card, not the field, so the patch works on the card: an earlier Sources card (a `lower_third` on the last scene with the heading "Sources" or "المصادر") is taken off, and with `true` a new one is built from the `sources` and the scenes of the patched request. So the patch can be given two times with one card as the result, a patch of `sources` and `sources_card` together gives the new names, and `false` or `null` removes the card. Errors, each with what to do: a value that is not true or false; `true` with no `sources`; `true` with new words given as `script` or `lines` (the card needs `scenes`). `sources` itself can now be patched on a run that had none. Tests `a_patch_puts_the_card_one_time_and_takes_it_off` and `a_patch_changes_only_its_fields`.

`scriptvox write --from research.json --sources-card` puts the field in the brief.

## 2. Errors

All are exit 2 from `plan` and `make`, as each fault of a brief.

| Cause | Fix |
|---|---|
| `sources_card` and no `sources`, or no source has a name | Add `sources`, or drop `sources_card`. |
| `sources_card` and no `scenes` (a brief with `script` or `lines`) | Give the video as `scenes`, or drop `sources_card`. |
| The last scene has 4 cards | Remove one, or drop `sources_card`. |

## 3. Looked at

Real runs on 13 Nov 2026, `tts: none`, with captions, frames from `snapshot --cards`:

| Run | Card | Result |
|---|---|---|
| English, 16:9, 3 sources | "Sources" / "Wikipedia, NASA, France 24" | Left, above the caption box; no overlap; `claims` pass (24 is a name) |
| Arabic, 9:16, 2 sources | "المصادر" / "ويكيبيديا، ناسا" | Right, above the caption box; no overlap |
| English, 9:16, 8 sources | "Wikipedia, NASA and more" | Inside the frame; `text_bounds` pass (18 boxes) |
| Arabic, 16:9, 7 sources | "ويكيبيديا، ناسا، الجزيرة، وكالة الفضاء الأوروبية وغيرها" | Inside the frame; `text_bounds` pass (19 boxes) |

Before the width rule, the English 9:16 run with a line of 52 characters failed `text_bounds`: the line ran 543 px past the right edge.

## 4. Limits

- The room is counted in characters, not measured with the font. A line of wide letters can be longer than a line of narrow ones.
- The shape is the `aspect` of the brief (default 9:16). `--aspect` on the command line with a narrower shape than the brief can push a long line out of the frame; `text_bounds` then fails the run. Set the shape in the brief.
- The card shows during the last scene, or during its share of it. It has no time of its own after the narration.
- No source line under each scene, and no address on the screen.
- A source with a `publisher` and no `url` and no `title` is still refused by the form check of `sources`.
