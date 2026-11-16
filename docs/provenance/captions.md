# Captions: ideas and what we did better

- MoneyPrinterTurbo `app/services/subtitle.py` `correct()` (MIT): lines Whisper output up with the script by Levenshtein similarity. Ours (`captions::align`): the TTS reads our own script, so an in-order match with an 8-event lookahead is enough. It restores punctuation (edge-tts drops it) and merges split words ("2026" read as two words). No code copied.
- HyperFrames captions docs (Apache-2.0): grouping words into pages. Ours measures each line with harfrust at libass scale (OS/2 win ascent plus descent), so a break is a measured fact, not a character-count guess. Checked against a real libass render within 0.5%.
- Remotion (source-available): behaviour idea only (word highlight). Our schema and code are our own.
