# Word times of a recorded narration, and aligning script words

G152, eval F6. A brief with `narration` and a speech row spread its words by characters: `narration_voice` read `words` from the `analyze::transcribe` result, but that field is a count, so it always fell back. Drift up to 0.47 s, `explain` said "estimated".

- `analyze::timed_words` reads the words from the result's `file`. All callers that want the words use it.
- `narration_voice` gives the heard words as events to `captions::align`, like a voice. `explain` and the receipt say "from speech to text". Without a speech row the even spread stays, and `explain` says "estimated".
- `captions::align` was greedy: speech to text writes "one point four" as "1 .4", and every later word shifted (2.9 s on this run once the real times arrived). It now fixes the longest common run of words, and the words between two fixed ones share the events between them: one to one when the counts agree, else by letters. Extra events stretch the word before them. This path also takes Edge and ElevenLabs events.

Measured on the eval recording `me.mp3`, 37 script words, 36 heard: 33 words match by text and all 33 have drift 0.00 s against the transcript. The 3 words of "one point four" share the 0.84 s of "1 .4". Tests `analyze::tests::timed_words_come_from_the_file_not_the_count`, `captions::tests::align_keeps_later_words_in_place_after_a_respelled_number`.
