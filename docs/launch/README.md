# Launch assets

Every asset is made with ScriptVox and has a row in `PROVENANCE.tsv`. No asset uses the Edge voice (its terms are personal or non-commercial). `just release-check` fails while one is missing.

| Asset | Status | Blocked on |
|---|---|---|
| `hero-en.mp4`: own-media reel, English, 24 s | done 10 Nov (`hero-en.json`, ElevenLabs voice, $0 after the clips) | |
| `hero-ar.mp4`: own-media reel, Arabic, 23 s | done 10 Nov (`hero-ar.json`, the same clips and voice; gate `words` warns at 38 of 43 heard on the small local model) | |
| `kill-and-resume.gif`: kill a render, rerun, it continues | done (scripts/kill-resume-demo.sh, from a real session log) | |
| `session-s4.gif`: the §4 session (plan exit 3, a shorter script, plan, make --detach, wait, resume --patch for a hook, wait, snapshot) | done 10 Nov | `scripts/session-s4-demo.sh`, real commands, 48 KB |

Media for the hero videos (owner decision, 10 Nov 2026: paid quality): 6 clips of 5 s, 9:16, made with `scriptvox generate --model kling-3.0-pro`, $0.56 each, $3.36 in all, then used as a `clips` folder, so the hero still shows "your clips are the shot list". The clips are not in the repository (74 MB); the owner keeps them. The two videos are under vendor terms, not Apache-2.0: `LICENSES/LicenseRef-VendorTerms.txt`. Read Kling's terms on commercial use before the public release.

Prompts, each ending "No text, no letters, no logos." (a prompt without it gave a clip with nonsense words on 9 Nov): a developer at a dark desk types one line and leans back; a hand writes in a notebook by a desk lamp; glowing sound waves on black glass; a strip of blank glowing film frames; a hand presses one glowing round button; dawn over a sea of clouds.

The ElevenLabs voice reads faster than the free voice the scripts were first timed for (about 23 s for the 30 s target), so `duration` is 24 and 23.
