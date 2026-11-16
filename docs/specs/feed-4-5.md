# 4:5 feed frame

Gap: MoneyPrinterTurbo 17, Remotion RM-18, ComfyUI A17 (any frame size). Instagram and Facebook feed posts use 4:5;
scriptvox had 9:16, 16:9 and 1:1.

- `aspect: "4:5"` (brief, `--aspect`, `edit --reframe`): 1080x1350. Captions and cards use the tall layout (the frame
  is taller than wide); stock search asks for portrait.
- No video or image model offers 4:5, so generator rows are asked for 9:16 (`make::gen_aspect`: same width) and the
  media graph crops the middle 70 percent of the height. The clip check measures the 9:16 clip it asked for.
- Not done: any WxH size. The aspect table is small on purpose: every frame in it is checked by eye.

Measured 4 Nov 2026: run r_419e05 (4:5, hook, stat card, list card, edge voice): 1080x1350, 6 gates pass, words warn
21 of 24 (the island names Saadiyat and Sir Bani Yas, not the frame). Contact sheet checked by eye: hook, stat, list
and captions inside the frame, no overlap.
