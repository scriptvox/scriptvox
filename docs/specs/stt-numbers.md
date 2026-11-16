# A number stays whole in speech to text

Gap G153. Whisper can split a number in its word list: "3" then ",000", "1" then ".4", "90" then "%". The segment text is right. The word list reaches the screen, so `clips` and `edit --captions` drew "3 ,000".

`analyze::stt_words` puts the tail back on the number before it: a word that starts with `,` `.` (or the Arabic separators) and a digit, or with a percent sign, after a word that ends in a digit. The new word keeps the first start and the last end. Every reader of the reply goes through it: `analyze --transcript`, `timed_words`, `opentts::transcribe`.

The fix is in the client, so the running pods and the local plugin need no new image.
