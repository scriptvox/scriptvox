# Long scripts: Edge TTS in parallel sentence chunks

Source: radar MPT#1732 (MoneyPrinterTurbo: long scripts time out with Edge TTS). Measured 3 Nov 2026: our worker does
not time out on 960 words (292 s of audio), but Edge streams at about real time, so that one call took 269 s.

- `tts_edge._chunks`: a text of 80 words or more is split at sentence ends (. ! ? and the Arabic marks) into groups
  of up to 60 words. Shorter texts are one call, as before (same cache keys, same audio).
- Up to 4 parts run at once (`asyncio` semaphore). The MP3 parts are joined in order; Edge's MP3 is 48 kbit/s
  constant bitrate, so each part's length is its byte count and every word time moves by the parts before it.

Measured: the same 960 words in 41 s (was 269 s, 6.5 times faster), 20 parts, audio 293.8 s (1.4 s longer for 20
joins), word times in order; Whisper's last word ends at 293.26 s and the shifted Edge word at 293.38 s (no drift).
Test: `python/tests/test_tts_edge.py::test_long_text_goes_out_in_sentence_chunks_and_word_times_follow_the_audio`.
