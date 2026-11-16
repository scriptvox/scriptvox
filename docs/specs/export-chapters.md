# Export: chapters and text per platform

Gap: Manim 20 (chapter and hold markers) and MoneyPrinterTurbo 15 (metadata per platform). Upload stays out (owner
decision); export only writes what an uploader or a person needs.

- Chapters come from the scenes: each scene's first word time (the first chapter at 0) and a short name: the card
  ("400+ km of coastline", text and label) or the scene's first clause, cut at a comma or stop, at most 40
  characters. One-scene videos have none.
- `video.mp4` gets the chapter marks inside (ffmetadata, stream copy, no re-encode); `metadata.json` lists them and
  says `chapters_in_mp4`.
- `metadata.json` `platforms`: YouTube title (100) and description (with the chapter list only when YouTube accepts
  it: first at 0:00, 3 or more, each 10 s or longer; at most 15 hashtags), TikTok caption (2,200), Instagram caption
  (2,200 and 5 hashtags), X text (280). Limits as of late 2025; check before relying on one.
- `export --tags abudhabi,islands`: the hashtags, without '#'. Each platform takes as many as it allows.

Measured 4 Nov 2026: export of r_419e05 (4:5, two scenes, 9.5 s): ffprobe shows two chapters, "400+ km of
coastline" from 0 and "It counts over two hundred islands" from 3.7 s; YouTube chapter list left out (too short),
hashtags on every platform text.

## Fresh-agent test 3 fixes (5 Nov 2026)

Chapters read the voice's `words.json`, whose times come before a fit, so they ran late by the voice tempo (1.07 s at 19 s). `make` now writes the final word times there, also for a dialogue and for no voice. Measured: chapter 6.6 s for a cut at 6.56 s with tempo 0.955. Title: `export --title`, else the hook, else the first clause that fits, never a cut in mid sentence. `cover.jpg` waits for a card on the first scene to land.
