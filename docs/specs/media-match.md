# Brief `media_match`: the right file for each line (5 Nov 2026)

Gap: MoneyPrinterTurbo #1183 (its most-reacted open issue) and #1137, OpenMontage #23: the shots do not fit the words. ScriptVox played the user's folder in order.

- Brief `media` plus `media_match: true`. A script becomes one scene per sentence (a sentence under 3 words joins its neighbour). In a brief with `scenes`, only the scenes with no picture of their own are filled.
- The vision service set by the user (`docs/specs/self-hosted.md`) gives each file one short sentence from its middle frame, kept by the file's hash. One more call picks a file per line, each file once before any repeat. Replies are cached, so the second run makes no call.
- **A plan sends nothing.** `plan` is a dry run, so frames leave only at `make` (before 5 Nov `plan` sent them with no notice). `plan` gives a stage `vision` that names the host, as for `base_video`. With no service set it warns that nothing was sent and the files play in folder order.
- `make` returns `media_match`: scene, file, what it shows, and `unused` files. A later `plan` shows the kept picks and no stage, because nothing more goes. To see picks early, run `make --draft`. To change a pick, the agent writes `scenes`.
- The picks do not enter `plan_id`: they do not change the cost, so an approval from the plan holds at `make`. `make` checks approval first (G001): a paid plan with no valid `--approve` exits 4 and no frame leaves.
- A failed call is a warning (`media_match`): the files play in folder order. A bad pick in a reply falls back to the least used file. `--local-only` refuses it.
- A new script through `resume --brief` is matched again; a new look is not.
- A clip over 12 s is looked at in parts: its longest shots, up to 4, or three even parts when it has no cuts. The pick sets scene `media_from` and `media_to`, which a brief can also set by hand. A line longer than its part slows the part or holds its last frame, and never runs on into the next subject. A start past the end of the picture stops at `plan`; both fields without `media`, or on a photo, are refused.

Measured: 6 pictures from the free image row, script in other words than the pictures ("saffron and cardamom" for a market, "herders" for camels): 6 of 6 right in English, 5 of 5 in Arabic with the sixth reported in `unused`. First plan 3.9 s, second 0.01 s, 0 calls. Draft r_1d6c3b checked by eye.

Parts: an 18 s clip of camels, market and rain with hard cuts, plus a harbour photo, and a script in another order: rain from 12 s, camels from 0 s, harbour, market from 6 s, all right, seen on the sheet. Plan 4.1 s. A frozen test clip fails the `slideshow` gate, as it should.
