# Make one like this: `analyze --like` (5 Nov 2026)

Gap: OpenMontage #22, its runner-up star feature, removes the blank-prompt problem with a reference video. Ideas only, own design and code. No download: the user gives a local file (platform terms stay with the user).

`scriptvox analyze ref.mp4 --like new.json` adds to `analyze` one output, a valid starter brief:

- Measured: `aspect` (nearest shape), `fps` (nearest allowed), `duration`, `transition` (`cut` when the mean scene is under 3 s, else `fade`).
- Scenes: shots merged so each holds a sentence that starts inside it, at most 12 (the shortest neighbours merge first). scriptvox cuts on scenes, so a fast-cut reel becomes one scene per sentence.
- Words from the free STT row (a guide), with a `like` warning. No speech or no key: a placeholder with a word count (2.5 words a second).
- One request to the free vision row: `look`, `captions`, one shot prompt per scene (15 to 30 words, no real person, brand, place or quoted text). When it fails, a `like` warning and the measured brief still comes.
- An existing file is refused. `next` is `plan`.

Measured: 55 s video to 5 scenes in 9 s; `plan` passes. Real run r_4db84d: 2 scenes, new words, `fastwan22-self`, 12.2 s, all gates pass, boat and tower shots with the warm look, checked by eye.
Skipped: scene cards in the reference (a card kind from frames) and music style. Add when an eval asks for them. Steps for agents: `scriptvox-edit` section 4b.

G156 (6 Nov 2026): a sentence goes to the shot that holds its middle, not its start. Speech can lead the picture, and each scene then got the words of the next. The prompt now says to describe only what the frame shows.
