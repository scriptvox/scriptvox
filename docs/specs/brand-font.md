# Brand font and caption place (5 Nov 2026)

Gap: MoneyPrinterTurbo lets a user pick a subtitle font and position; Remotion and HyperFrames can style text with any web font. ScriptVox had one font per script and one place.

- Brief and brand file `font`: a TTF or OTF inside the project (`make` refuses one outside, SVX_PATH_OUTSIDE_ROOT, as it does a logo, overlay, inset or narration file; audit security-5). Used for captions, cards and the hook, sized by its own metrics. Code and formula keep their fonts. `plan` refuses a bad file (exit 2) and a font without a letter of the script, naming the letters. Marks outside the font fall back to the built-in fonts. The font's hash is part of the plan id. The file is copied into the run's fonts folder. The user answers for the licence.
- `caption_at`: `bottom` (default), `middle` (page bottom at 62 percent of the height) or `top`. `middle` and `top` stop at `plan` when a hook or scene card uses that space.
- Not in `edit --captions` yet. Add when asked.
- Checked by eye: DejaVu Serif on captions, stat, list and hook (r_d946be); Noto Naskh Arabic joined letters at `middle` (r_47bc4e); `top` on 16:9 (r_d68fab). All gates pass.
