# IPA in `say`, delivery cues, and SSML in a script (G189, G190, P4.5, 13 Nov 2026)

Ideas from public product pages only (a term with a phonetic form, a cue word in brackets). Own design; no code read.

## IPA in `say`

A `say` value can carry an IPA form between slashes, and a respelled form after it:

```json
{"say": {"Doha": "/ˈdoʊhə/ doh-ha", "Nguyen": "/ŋwiən/", "ACME": "A C M E"}}
```

- A voice row with `takes_ipa = true` says the IPA form. Today that is the local voice `kokoro-rust` (both rows). `scriptvox voices --json` shows the field.
- Any other row reads the respelled form, or the written word when the value has no respelled form, with one warning (gate `say`) that names each term and what is read.
- Captions keep the written word, as with every `say` value.
- `kokoro-rust` has the sounds of American English only. An IPA form with another sound (`x`, `ʁ`, `y`) is exit 2 before any work; the message names the sound and lists the sounds the voice has. A few near sounds are mapped (`r` to `ɹ`, `ɒ` and `a` to `ɑ`, `e` to `ɛ`, `o` to `ɔ`, the flap to `d`; the length mark and syllable dots go).
- No brief field is new. A value that does not start with a slash, or has one slash only (`km/h`), is a respelled form as before: old briefs keep their plan id and their voice file. An IPA form enters the voice key of a `takes_ipa` row.

## Delivery cues

A cue is one of these words in square brackets: `whisper`, `whispers`, `excited`, `happy`, `sad`, `angry`, `calm`, `serious`, `cheerful`, `curious`, `sarcastic`, `shouting`, `shouts`, `laughs`, `laughing`, `chuckles`, `sighs`, `gasps`, `crying`, `surprised`. The list is closed, so `[sic]`, `[1]` and `[paused]` stay text.

- A voice row states the cues it takes: `takes_tags = ["whisper", "excited"]`. The engine then gets the cue in the text.
- A row that does not take a cue: the cue leaves the script before the plan, with one warning (gate `cues`) that lists the cues. For a brief with `script`, the plan id is the id of the same script with no cue (and a break tag gives the id of the pause tag written by hand). A brief with `scenes` keeps the scenes as written in its plan id.
- Captions, the SRT and VTT files, word counts, the `words` gate and the `claims` gate never see a cue (`pauses::strip`).
- No row in the registry takes a cue today. A row gets `takes_tags` only after a real run with its engine proves it, and no such run was possible here with no key and no paid call. So today every cue is dropped with the warning. The path that keeps a cue has a unit test only.

## SSML

Fault before: `<break time="4s"/>` in a script was shown in the captions and read by the voice, with no warning.

Choice: a break tag is changed to a pause tag, with a warning (gate `ssml`). Any other SSML tag stops the run with exit 2.

Why: a break has one meaning and the script language has the same thing (`[pause 4]`), so the result is what the writer wanted, and the warning teaches the own form. The other tags (`<emphasis>`, `<prosody>`, `<phoneme>`, `<speak>`, `<say-as>`, `<sub>`, `<voice>`, `<lang>`, `<mark>`, `<audio>`, `<p>`, `<s>`) have no equal here: to drop them silently would change the read, and to keep them would show them on screen. The error names the tag and the fix line names the own form for each need (`[pause 2]`, `voice_rate`, `say`, `voice`).

- `time="4s"` and `time="500ms"` are read; a break with no time is 1 second. The pause limits (0.1 to 10 s) apply.
- Text with `<` that is not one of these tags (`a < b`, `<3`, `<b>`) stays text.

## Errors

| Case | Result |
|---|---|
| IPA with a sound the voice does not have | exit 2: `say "Khalid": the IPA form /xɑlid/ has x that the local voice cannot say ...`. Fix: write the term with the sounds the voice has, or give a respelled form, or choose another voice |
| SSML tag other than break | exit 2: `the script has the SSML tag <emphasis>, and no voice here takes SSML ...`. Fix: take the tag out; `[pause 2]`, `voice_rate`, `say`, `voice` |
| Cue the row does not take | warning `cues`, the run goes on |
| IPA on a row with no `takes_ipa` | warning `say`, the run goes on |
| Break tag | warning `ssml`, the run goes on |

## Real runs, 13 Nov 2026 (no key, no paid call)

| Run | Result |
|---|---|
| `edge`, script with `[whisper]`, `<break time="2s"/>`, `[excited]`, `say` with `/ŋwiən/ win` and `A C M E` | 8.6 s, three warnings (`cues`, `say`, `ssml`), one each in `plan` and in `make`. SRT: "Come closer and listen." ends at 1.34 s, the next page starts at 3.53 s (the 2 s pause). No bracket and no tag in the SRT |
| `kokoro-rust`, "I like tomato soup ...", `say` `{"tomato": "/pətˈeɪtoʊ/"}` | 6.7 s. Captions show "tomato". Local speech to text wrote "I like potato soup on a cold day. A good potato makes a good soup for all of us." So the voice says the IPA form. The `words` gate warns 17 of 19, which is right: the voice does not say the written word |
| `kokoro-rust`, `say` `{"Khalid": "/xɑlid/"}` | exit 2 at `plan`, the message above |
| `edge`, script with `<emphasis>` | exit 2 at `plan`, the message above |

## Limits and not done

- No row takes cues, so no real run of a kept cue.
- The IPA map for the local voice is small and not checked by a person who listens.
- A cue inside a `captions_second` line or a card text is not handled: those are not spoken.
- A script with a cue loses its line breaks when the cue is taken out (the text is joined with single spaces).

Tests: `tests/delivery_cues.rs` (4, stub voice; one with `scenes`), `cues::tests` (5), `say::tests::a_say_value_carries_ipa_a_respelled_form_or_both`, `kokoro_rust::tests::ipa_becomes_the_sounds_of_the_voice_or_names_what_it_cannot_say`.
