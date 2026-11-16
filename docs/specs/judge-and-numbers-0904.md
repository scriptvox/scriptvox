# Judge clarity, plain background, held captions, numbers in the words gate (4 Nov 2026, day)

From the open items of the night (`docs/daily/2026-11-04.md`, items 1 to 3) and the hand eval rushed-read.

## Judge findings name their frame
`findings` holds only frames under the bar. An eval agent read "3 of 6 frames" as "the judge listed 3 of 6". Each
finding now has `frame` ("frame 2 of 6") and `score`; the message says all frames are in `frames`; the new metric
`frames_scored` gives the count. The judge always scores every frame (`parse_reply` refuses any other count).

## Plain background is a valid choice
The rubric says: when the brief asks for no footage, score the background frames on the text and cards and do not
ask for stock. Rerun on r_9f0fec.1.1: frames that scored 2 now score 3 and 4. The model still suggests visuals in
`fix`; the review skill says such a fix is not a must-fix when the brief chose no footage.

## A caption page holds through a short pause
The one frame that scored 1 ("empty frame") fell in a 0.4 s pause between two sentences: no caption, only the
gradient. A page now stays on screen until the next page starts when the pause is 0.8 s or less (`HOLD_S`); a longer
pause still clears the screen. `Page.said_end` keeps the time the last word is said, and the `captions` gate measures
that, so the hold never makes a page look out of step.

## Numbers in the words gate
A number of 21 or more is compared as its words on both sides: "two thousand five" in the script matches "2005" heard,
"342" matches "three hundred forty-two", and "1990" matches "nineteen ninety" (a year has a second reading; the one
that matches the other side is taken). Hyphenated number words count as two words.

## Frames off card entrances (5 Nov 2026)

`judge` with a run moves a frame that falls in a card's first second to the time the card has landed (`cards.json`). Before, a bars card 0.05 s after its start showed only its title and scored 1 as "chart missing". The rubric also knows a plot's edge numbers are its scale. `next` now fills after analyze, doctor, setup, demo, voices, tools, providers, templates and explain.

`next` names `judge` only when a vision service is set (audit contract-2, 9 Nov 2026).
