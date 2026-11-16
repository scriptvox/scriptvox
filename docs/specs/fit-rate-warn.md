# Fit: warn when the voice is slowed or sped up too far

Found by the hand eval of 4 Nov 2026 (`plot-card`): 27 words for a 15 s video. The fitter re-read the Edge voice at
-40 percent speed. It fit, all gates passed, and nothing said the read would drag.

- `fit::RATE_WARN` is 15 percent, the same line the review skill already used for `voice_rate_pct`.
- `plan` (with `duration`): for a voice that takes a rate, an estimate more than 15 percent off the target gives a
  `fit` warn: "the voice would read at about -42 percent speed to fit; it may sound dragged", with the words to add
  or cut. The open voice keeps its own band (16 percent, it takes no rate).
- `make`: when the fitter changed the rate by more than 15 percent, a `fit` warn with the words to add or cut so the
  voice reads near its own speed (`fit::rate_warn`).
- Brief skill: says both warnings exist and to take the word count they give.

Measured 4 Nov 2026 on the agent's brief: `plan` warns "-42 percent", add about 21 words; `make` (run r_6807f3) warns
"-40 percent", add about 18 words; the video is still made (warn, not fail). A brief with no `duration` gets no
`fit` finding. Tests: `fit::tests::the_open_voice_reads_faster_and_takes_no_rate`.

Numbers in the estimate (4 Nov 2026): a 54-word script with three years ("2005", "2015", "2025") for 13 s was
estimated at 1.26x and read at 1.48x (`voice_rate_pct` +48, run r_6d3155). `fit::read_words` counts a number as
the words it is read as (1 to 2 digits one word, 3 or 4 digits three, longer two per group of three, a decimal
adds "point" and its digits, a percent sign one). It is used for `precheck`, the `plan` fit warning and the music
length. Same brief now: 1.40x. Test `fit::tests::numbers_count_as_the_words_they_are_read_as`.

Arabic (G151, 6 Nov 2026): three plans warned on runs that read at rate 0 with tempo 0.93 to 0.97 (12 written words "add 11 words, -41 percent", 47 "add 8", 38 "-30 percent"). 6 real Edge runs read 1.37 to 1.62 written words a second, but 6.8 to 7.5 letters a second, so Arabic counts letters (4.67 to a word, the short vowel marks left out) at 1.5 words a second, and a whole number counts double (two 4-digit prices took 7.7 of 14.28 s). The same three scripts now estimate within 7 percent and give no warning; the plans were run again. The open voice keeps 2.9 a second, not measured again. English stays 3.3; these runs read about 3.05, inside the band. Test `fit::tests::arabic_fits_the_real_runs_of_the_6_nov_eval`.
