# Captions: an initial or a short form does not end a page (13 Nov 2026)

Seen in agent trial T3: "W. H. Auden" gave a caption page that held "H." alone. A caption page never runs past a sentence end, and every word that ends with a period was a sentence end.

## The rule

`captions::ends_sentence` is the one place that says where a sentence ends for the caption pages, for the count of `captions_second` (`captions::sentences`) and, since this change, for the scenes that `media_match` cuts from a script. A word with a period is not an end when it is:

- one capital letter: `W.`, `H.` (not `I.`, which is more often the end of a sentence);
- single letters with periods: `U.S.`, `e.g.`, `i.e.`, `a.m.`;
- a title from a closed list: `Dr.`, `Mr.`, `Mrs.`, `Ms.`, `Prof.`, `Sr.`, `Jr.`, `St.`, `Mt.`, `vs.`

Only ASCII letters count. Arabic text does not change: `المطر.`, `المطر؟` and a single Arabic letter with a period end a sentence as before.

A brief with `captions_second` counts sentences by the same rule, so a script with "Dr. Lee" now has one sentence fewer than before, and its list needs one entry fewer.

## Checked

Test `captions::tests::an_initial_or_a_short_form_does_not_end_a_page`: "The poet W. H. Auden wrote it. Dr. Lee read it in the U.S. Senate, e.g. twice." gives 2 pages, one per sentence; a list of words that end a sentence and a list that do not; an Arabic text gives the same 3 pages as before.

## Open

- A sentence that ends in one capital letter or in letters with periods ("vitamin C.", "in the U.S.") runs on into the next sentence's page. The break by width still cuts the page. The fix is to look at the next word; not built.
- Three other places split text at a period for their own use and did not change: the repeat check in `gates.rs`, the first sentence in `export.rs`, and the sentences of a transcript in `analyze.rs`.
- `etc.` and `No.` end a sentence, because both often do.
