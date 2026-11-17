# The user's own music (14 Nov 2026)

Owner decision of 14 Nov 2026: the user can load any song, from a file or from an address. The choice and the responsibility are the user's. The tool does not refuse a song and does not check the rights to it. One short line of information, one time, is right. Most users make a video for fun, so this use must work well and be tested.

## What the tool says

- One line, one time: the notice of `scriptvox setup fetch`: "fetch gets a file from any address that you give. The choice and the responsibility are yours: ScriptVox does not check the rights to a file." A JSON run has it in the field `notice`, a person at a terminal gets the line.
- A music file of the user's own (`--music song.mp3`) shows no note.
- For real music with clear terms, the music library (`library:<name>`, `music-library.md`) has CC0 songs with a traced source.
- No refusal, no question and no repeated warning in `plan`, `make`, `wait` and `check`. Nothing about rights is drawn or said in the video.
- The `plan` line of the music says what the mix does: the source, the cut, the loop, the fade and the level.
- Licence data stays in `metadata.json`, `credits.txt` and the listing commands. For a file of the user's own, the entry in `metadata.json` says "supplied by the user". For music from an address with no licence stated, the note is "no licence stated; the user gave this address".
- The help of `make --music`, of brief `music`, of `fetch` and of `edit --music`, and the README, say it in a positive form: any song or media file can be used; the choice and the responsibility are yours.

## What the mix does with a music file

| Case | Behaviour |
|---|---|
| Any file with sound | mp3, m4a, wav, flac, ogg, mono or stereo, 44.1 or 48 kHz. A video file gives its sound. The mix is 48 kHz stereo |
| `music_from`, `music_to` | Cut the file first: seconds as a number (`12.5`), as text, or a clock (`"0:12"`). New for a file of the user's own; before it was for an address only. The cut is `music-cut.wav` in the run folder. The beat grid, the loop and the fade work on the cut |
| The music is longer than the video | It stops at the end of the video, with a fade out over the last second |
| The music is shorter than the video | It starts again from its start, as many times as needed. There is no crossfade at the join. For no join, give a longer file or a shorter video |
| No words (`no-words.md`) | The music is the only sound, from the first frame to the last, 1 s fade in and 1 s fade out, and the video is set to -14 LUFS |
| With a voice or a `narration` | The music is 10 dB under the voice (`music_level`: quiet 14, normal 10, loud 7) and goes lower while the voice speaks. The voice is not changed |
| `cut_on: "beat"` | The beats come from the file (or from its cut). Music from an address is fetched first (`reel-gaps.md`) |

Errors, exit 2, at `plan`:

| Case | Message and fix line |
|---|---|
| No such file | `no music <name>`. Use calm, pulse, or a music file inside this project |
| The file is not media (a text file with the name of a song) | `<name> is not a media file ScriptVox opens ...`, with the list of the formats |
| A media file with no sound (a picture, a silent video) | `music <file> has no sound that can be read`. Give a sound file or a video with sound; `scriptvox analyze <file>` shows what a file holds |
| `music_to` before `music_from`, or not a time | Says the two forms, with an example |
| `music_from` past the end of the file | Stops at `make` before the voice, names the file |
| `music_from` with a built-in bed or no music | Give a file or an address, or drop the fields |

Limits: `edit --music` still takes a cut for an address only. A file outside the project folder is refused as every other input file is (copy it into the project).

## Checked

`tests/user_music.rs`: each case is a real `make` with the gates, with a song made by FFmpeg, no network. The address path is `tests/fetch.rs` (`make_gets_music_from_an_address_one_time_and_credits_it`, `make_cuts_on_the_beat_of_music_from_an_address`).
