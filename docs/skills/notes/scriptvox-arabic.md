# Notes: scriptvox-arabic

Written 2 Nov 2026. The studied projects have almost nothing on Arabic: one language row in OpenMontage's translate
skill (`ar-SA`, Modern Standard Arabic), a right-to-left export test mentioned in a HyperFrames release note, and the
OpenMontage localization script director (`skills/pipelines/localization-dub/script-director.md`, AGPL, ideas only).
So this skill rests on our own measurements. No text is carried over.

## Ideas kept from the localization source (our words)

1. Names, brand terms and product names must stay the same in every language. Keep a short list of protected terms.
2. Numbers are a common error. Check every number in the script before any voice is made.
3. A translation changes length. Never assume the source timing holds.
4. Review the text before audio exists. A fix in text costs nothing. A fix after synthesis costs a rerun.

## What we measured (2 Nov 2026, scriptvox renders, Edge voices)

1. Speech rate: Arabic voices read 105 to 110 words a minute (Hamdan 105, Fatima 106, Hamed 110) against 190 to 212
   for English. The same 30 seconds hold about 48 Arabic words.
2. A number written in digits is one caption word but many spoken words. "3999 درهمًا." stayed on screen 3.6 s with
   two words. "عام 2026" took about 2 s. Long exact numbers make slow, nearly empty caption pages.
3. A number written out in words ("ألفين وستة وعشرين") can break across two caption pages.
4. A rounded number ("أربعة آلاف درهم تقريبًا") reads cleanly and fits the page.
5. Before 2 Nov 2026, any Arabic caption line that started with a digit or a Latin word showed in reversed order
   (libass forced left to right). Fixed in scriptvox the same day (Encoding -1 and a right-to-left mark per line),
   checked by render: "2026 بسعر", "3999 درهمًا." and "Apple أطلقت" all read correctly now.
6. Western digits (2026) and Arabic-Indic digits (٢٠٢٦) both render and are both read aloud by the Edge voices.
7. Arabic pause tags work: `[توقف ١٫٥ ثانية]` gives the same silence as `[pause 1.5]`.

## Not measured (the skill must not claim these)

- Whether short vowel marks (tashkeel) improve Edge pronunciation. We cannot listen in the test loop. The skill says
  "may help" for one misread word and nothing more.
- Dialect quality of each voice. The voices are labelled by country, not by dialect.

## What our skill does better

- Every number in it comes from a render we can repeat.
- It covers the parts no source covers: budget, numbers, mixed scripts, punctuation, pause tags, voice choice, and a
  check list that matches what scriptvox-review looks for in Arabic frames.
