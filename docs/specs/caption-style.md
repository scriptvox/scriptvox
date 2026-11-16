# Caption style: outline, box and a free size (P4.6, 13 Nov 2026)

Before: the main captions had one look, white letters on a 65 percent black box, and three sizes. A brand that wants plain outlined letters, a lighter box, or a size between the three could not have it.

Three brief fields. They are also brand fields; the brief wins. `resume --patch` takes them. No `make` flag, as for every caption field.

- **`caption_box`**: the opacity of the black box behind the captions, 0 to 1. 0 is no box, 1 is solid black. Default 0.65.
- **`caption_outline`**: a black outline around the letters, 0 to 12, in px on a 1080 px short side. It follows the frame (6 on 1080 is 4 on 720). Default: none inside the box. With `caption_box` 0 and no `caption_outline`, the outline is a tenth of the caption size (the look of `reveal`), so the letters are not bare.
- **`caption_size`**: also a percent of normal as text, `"50%"` to `"200%"`, next to `small` (80%), `normal` and `large` (125%). `"115%"` gives 96 px where normal gives 84 px. The pages are made again at the new size, as before (`caption-size.md`).

How it is drawn. Measured with libass on 13 Nov 2026 with six small files: with BorderStyle 4 the outline goes around the letters in the outline colour, and the box is padded by Outline plus Shadow. So one style can have a box and a real outline. `caption_style::apply` changes only the `Style: Cap,` line that `captions::to_ass` wrote:

| Brief | BorderStyle | Outline | Shadow | Outline colour | Box colour |
|---|---|---|---|---|---|
| neither field | not touched | | | | |
| `caption_box` 0.3 | 4 | size/6 | 0 | 30 percent black | 30 percent black |
| `caption_outline` 4 | 4 | 4 | size/6 less 4 | black | 65 percent black |
| `caption_box` 0 | 1 | size/10 | 0 | black | not drawn |
| `caption_box` 0, `caption_outline` 5 | 1 | 5 | 0 | black | not drawn |

The box keeps its padding of a sixth of the size in every row with a box.

Errors, exit 2, each with a fix line, from `plan`, `make` and a `resume --patch`:
- `caption_outline` outside 0 to 12; `caption_box` outside 0 to 1.
- `caption_box` above 0 with `captions` `reveal`: that style has no box by design. `caption_outline` works with `reveal`.
- `caption_size` that is not one of the three names or a whole percent from 50 to 200.

Old briefs: with no new field `captions.ass` is the same byte for byte and the plan id is the same. Only a field that is set enters the plan id.

Limits:
- The outline and the box are black. The fill colours do not change (white, and the accent for the spoken word).
- The hook, the cards and `captions_second` keep their look.
- A brief with `cutaways` (a base video with cutaways) does not take these fields yet.
- A caption with no box on a light picture can be hard to read. The gate `contrast` measures it (`gate-contrast-loop.md`).
- In a brief a bare number is read as a percent (`115` is `"115%"`). A `resume --patch` takes the text form only.

Tests: `caption_style::tests` (the style line for each row of the table; the refusals) and `tests/caption_style.rs` (a real `make` with `tts: none` in English and Arabic, `resume --patch`, exit 2 for five bad values, the plan id, the brand file).

## The same fields in `edit`, `dub` and `clips` (13 Nov 2026)

```
scriptvox edit <video> --captions karaoke|pop|reveal [--caption-size SIZE] [--caption-outline PX] [--caption-box 0-1]
scriptvox dub <video> --lines <file> [the same three flags]
scriptvox clips <video> [the same three flags]
```

- One type holds the three values for every command: `caption_style::Style`. Its `check` is the one check (the size, the outline, the box, `reveal` with a box), `scale` gives the size to `make::caption_pages`, and `apply` writes the style line. `make`, `edit`, `dub` and `clips` call these; no command has its own copy.
- `edit`: the flags need `--captions` (or `--captions-file`). The pages are measured at the size shown, as in `make`.
- `dub`: the flags become the three brief fields of the `make` that it runs.
- `clips`: the flags go to the `edit` of each clip. A bad value stops before the talk is heard.
- Errors, exit 2, each with a fix line that names the flag: a size that is not a name or a percent from 50% to 200%; an outline outside 0 to 12; a box outside 0 to 1; a box with `reveal`; a flag with no captions in `edit`.
- With no flag the caption file of `edit` is the same as before.

Tests: `edit_takes_the_caption_style_flags_and_writes_the_style_of_make` (`tests/caption_style.rs`: the same values give the same `Cap` style as the `make` test), `dub_runs_lines_as_one_make_and_keeps_the_source_size` (the brief of the dub holds the fields) and `clips_refuses_bad_ranges_before_any_call` (`tests/edit.rs`).

Real run (13 Nov, 1280x720, captions from an SRT file): no flag gives 56 px with a box; `--caption-box 0 --caption-outline 6 --caption-size 150%` gives 84 px, no box, a 4 px outline. Both frames were looked at. Not run: `dub` and `clips` with a real voice or a real speech row.
