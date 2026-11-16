# Brand file: the look of a series in one place

Gap: MoneyPrinterTurbo 16, Manim 21, OpenMontage 21 (saved style or playbook). Before, every brief repeated the
accent, logo, look, voice and outro.

- Brief field `brand`: a JSON file, path relative to the brief. It may hold only the look and sound of a series:
  `lang`, `tts`, `voice`, `aspect`, `fps`, `captions`, `transition`, `look`, `sfx`, `music`, `music_prompt`, `logo`,
  `accent`, `outro` (`brief::BRAND_KEYS`). Any other field is refused with the list, so a brand never carries one
  video's script or scenes.
- A field the brief sets wins; the brand fills only what the brief leaves out (or sets to null).
- A logo path in the brand file is relative to the brand file, so one `kit/` folder serves every brief.
- The merge happens when the brief loads, so the run's request (and `resume`, `replay`) hold the full values.

Measured 4 Nov 2026: run r_5ed689, a brief with only `brand` and one scene: the video took 4:5, the logo in the left
corner, the light-blue accent, the warm look and the outro text from the brand file; 7 gates pass; sheet by eye.
Seen on the way and fixed: about 0.4 s before the outro showed only the background (the last card ended at the last
word plus the tail, the outro started at the audio end plus the tail). Now one `speech_end` (audio plus tail, or the
asked duration) ends the last scene card and the last caption page and starts the outro. Rerun r_65743b: card, caption
and logo stay until 4.18 s, the outro starts there; 7 gates pass.
