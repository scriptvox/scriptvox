# Gates `contrast` and `loop_seam` (P6.6, G188, 13 Nov 2026)

The last two parts of G188. Both read frames of the made video with FFmpeg. No model, no network, no new download. Both are warnings: exit 0.

## `contrast`: the letters against the picture around them

Before: `text_bounds` measured where a text box is, and no gate looked at the picture under it. With `caption_box` 0 (`caption-style.md`), white captions on a light picture passed every gate.

- **When.** `make`, `resume`, `remake` and `dub` run it after the render, on the run's `captions.ass`. `check <run>` shows the run's finding (run on 13 Nov on a warned run and on a `loop` run: both findings are there). `check` on a plain file cannot run it: there is no record of the text.
- **What is measured.** For one text event at the middle of its time:
  1. libass draws the letters of that event alone (no outline, no box, no shadow) on a black canvas and on a white canvas. A pixel that is the same on both is solid letter. This is the mask and the fill colour. Nothing is guessed from the picture, so the mask is right also when the letters cannot be seen in the video.
  2. The ring is each pixel 2 to 4 px from a solid letter pixel (4 px on a 1080 px short side, at least 3). The pixel next to a letter is its soft edge and is left out.
  3. The ring is read from the made video's frame at that time. Each ring pixel gives a WCAG 2 contrast ratio against the fill. Karaoke has two fills (white, and the accent for the spoken words): the fill that reads worst counts.
  4. The event's value is the ratio that 80 percent of its ring reaches or passes. So a fifth of the letters' surroundings on a bad patch is enough to lower it.
- **Limit.** Under **3.0 to 1** the gate warns. This is the WCAG 2 AA limit for large text; captions and card texts are large text. The detail gives the words, the time and the ratio. The fix names `caption_box`, `caption_outline` and a darker picture.
- **An outline counts.** The ring is what is around the letters in the video, so a black outline as wide as the ring gives white letters a full ratio on any picture. The default outline of a caption with no box is a tenth of the size (8 px), wider than the ring. A 1 px outline leaves most of the ring on the picture, and warns on a light picture.
- **When in the event.** The middle, or 1.5 s in for a long event. The mask is drawn at the same moment of the event's own time.
- **Which events.** At most 8, to keep the cost near a second: first the hook, the card texts and the second line (at most 4, spread over the video), then the captions (spread over the video). Drawings, events shorter than 0.8 s (the steps of a stat count) and events with no solid letter at their middle are left out. If none could be measured the gate warns "not measured".
- **Cost.** One FFmpeg call per event (two canvases and one frame, stacked, as raw RGB). Measured: 0.1 to 0.2 s per event on 1920x1080.

Measured 13 Nov 2026, `tts: none`, 16:9, 1920x1080, background `#f4f4f4,#d8e8ff` (light) or `#101020,#202840` (dark):

| Captions | Picture | Result |
|---|---|---|
| default box | light | pass, 9.5 |
| `caption_box` 0 (outline 8 px) | light | pass, 12.6 |
| `caption_box` 0, `caption_outline` 1 | light | warn, 1.0 |
| `caption_box` 0, `caption_outline` 0 | light | warn, 1.0 |
| `caption_box` 0, `caption_outline` 2, then 3 | light | warn, 1.0 and 1.1 |
| `caption_box` 0, `caption_outline` 4 | light | pass, 12.1 |
| `captions` `reveal`, `caption_outline` 3, 9:16 | light | warn, 1.1 |
| `caption_box` 0, `caption_outline` 0 | dark | pass, 9.9 |
| `caption_box` 0.2, `caption_outline` 0 | light | warn, 1.2 |
| Arabic, `caption_box` 0, `caption_outline` 0 | light | warn, 1.0 |
| Arabic, default box | light | pass, 9.4 |
| `captions` `reveal` | light | pass, 12.6 |
| default box and a hook | light | pass, 9.5 (4 boxes) |

Frames looked at through `scriptvox snapshot`: the three warned videos show white letters that cannot be read on the light picture; the passed ones read.

Limits, also in the `limitations` text of the result:
- Up to 8 events, one frame each. Other frames and other text are not seen. A moving picture can be light under a caption on a frame that was not read.
- A title behind the subject (`behind.ass`) is not measured.
- The ratio is for the fill against the ring. It does not say that a font, a size or a busy picture is easy to read.
- `edit` does not run it.

## `loop_seam`: the last frame against the first

- **Brief field `loop`: true** says the video is meant to play in a loop. The field only asks for this gate. It changes nothing in the video and does not enter the plan id.
- The gate takes the first and the last frame as 64 by 36 grey pictures and gives the mean difference as a share of the grey range. Over **8 percent** it warns: the loop will jump. With no `loop` there is no finding.
- Fix line: end on the picture the video starts on, with no outro, or drop `loop`.

Measured 13 Nov 2026:

| Video | Difference | Result |
|---|---|---|
| moving background, captions differ | 3.4 and 4.0 percent | pass |
| first and last scene on the same still (slow zoom), other captions | 2.8 and 6.0 percent | pass |
| last scene on another still | 20 and 37 percent | warn |

Limits: the captions are part of both frames, so the limit is not 0. Two pictures that are dark in the same places can pass. The sound at the seam and the motion across it are not checked. An outro or a fade at the end warns, which is right for a loop.

Tests: `contrast::tests` (the ratio on made pictures: white on white, on black, on mid grey 4.5 to 1, an outline, two fills, nothing solid; the choice of events; the frame difference) and `tests/contrast.rs` (seven real renders for `contrast`, English and Arabic; three for `loop_seam`).
