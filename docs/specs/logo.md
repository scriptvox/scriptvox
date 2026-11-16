# Brand kit: logo, accent colour, end card

Gap: Remotion and HyperFrames templates carry a brand logo; MoneyPrinterTurbo and OpenMontage users ask for a
watermark. ScriptVox had none, and every self-hosted video needs the entity's mark.

- Brief: `"logo": {"file": "logo.png", "corner": "right" | "left", "width": 0.16, "opacity": 0.9}`. The file is
  relative to the brief and must exist; corner is a top corner (default right, not mirrored for Arabic: a brand keeps
  its place); width is a share of the frame width, 0.05 to 0.4; opacity 0.2 to 1. Checked when the brief loads.
- Graph: the PNG is one more FFmpeg input, `scale` to the width (even pixels), `colorchannelmixer` for the opacity,
  `overlay` in the corner: 4 percent of the width from the side, 5 percent of the height from the top (under phone
  status bars). It sits over the picture and the behind titles and under the captions and cards. The plan id holds
  the logo file's hash, so a new logo makes a new run.
- Measured 3 Nov: a 600x200 transparent PNG at width 0.3 on 9:16: 324 px wide, 45 px from the right edge, the
  transparent part clear and the colour at 90 percent, beside a behind title; all gates pass (checked by eye on a
  zoomed crop, `night-output/logo/`).
- Accent: `"accent": "#RRGGBB"` replaces the default yellow (`captions::ACCENT`) in the spoken-word highlight and the
  bars of a chart. Refused at load when its WCAG contrast on black (the caption box) is under 3: self-hosted-like blue
  #1F6FEB is 4.5, the default yellow 16.5, navy #000080 1.3 (refused). Measured 3 Nov: the blue highlight and blue
  bars beside the blue logo, readable on the box (`night-output/logo/`).
- `scriptvox edit <video> --logo logo.png [--accent #RRGGBB --captions]`: the same overlay (top right, 16 percent) in
  the edit pass, and the accent in the edit captions. Checked by eye on the English talk clip (`night-output/editcap/`).
- End card: `"outro": {"text": "scriptvox.dev", "seconds": 2.5}` (1 to 6 s; text at most 60 characters; text, a logo
  or both). The video gets those seconds after the narration (the music runs on). From there the picture blurs
  (`boxblur`) and darkens in 0.4 s, the logo shows at 40 percent of the width above the middle (a looped still for
  the card's length), the text is a title event at 64 percent of the height (50 without a logo), and the corner logo
  stops (`enable`), so it is not blurred into the card.
- Measured 3 Nov: two faults seen by eye and fixed: the big logo was missing (a single PNG frame faded in after its
  only frame stays transparent; now looped) and the corner logo was blurred into the card (now the card is drawn
  first and the corner logo ends at its start). Final render: 13.6 s, all gates pass (`night-output/logo/`).
- The text holds to the end of the card (`cards::until` lifts the 2.5 s title limit for the end card only).
  Measured 3 Nov: a 5 s card, text from 11.4 to 16.1 s, seen at 12, 14.5 and 15.9 s (`night-output/logo/outro-hold.png`).
- Tests: the graph for both corners and the clamps (unit); the brief checks (unit); a real render checked by eye.
