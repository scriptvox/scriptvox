# Music library (MILESTONE-PARITY 12.5, 17 Nov 2026)

Songs with a traced source and a free licence, for a user who wants real music and no question about rights.

## Use

- Brief: `"music": "library:picnic"`. CLI: `--music library:picnic` on `make` and `edit`.
- List: `scriptvox media music [--mood calm|happy|chill|energetic|tense|sad|epic]`. Each row gives the name, title, composer, mood, length, size, licence, source page and whether it is downloaded.
- `music_from`, `music_to`, `cut_on: "beat"`, `music_level` and the loop and fade work as for a file of the user's own (`user-music.md`).

## What the tool does

| Step | Behaviour |
|---|---|
| Name | A wrong name stops at `plan` with exit 2 and lists the right names |
| `plan` | One stage `music.library` with the title, composer, licence and source. When the track is not downloaded: the size, "downloads one time", and `network: true`. A plan sends no request |
| Download | From the pinned address in `registry/music.toml`, into the content store, and copied for the mix to `music/` in the ScriptVox data folder (never a fixed path in the project, where a planted link could redirect the write). The sha256 must match the pinned one. A file that does not match is deleted and the run stops: "changed at its source" |
| Rerun | From the store, with no request. `--local-only` works when the track is in the store. When it is not: `needs_input`, with the built-in bed as the offline choice |
| Credits | `Music: "<title>" by <composer>, <source> (CC0 1.0, from the ScriptVox music library)` in `credits.txt` |
| Receipt and export | One `stock` item, `provider: "library"`, with its terms. CC0 gives `commercial: true` and no credit duty, so `commercial_ok` stays true |

## The tracks

11 tracks, all CC0 1.0, from Komiku and Loyalty Freak Music. The trace of each is in `docs/provenance/music-library.md`. ScriptVox ships no audio: the files stay on Wikimedia Commons.

## Not done

- A track is heard only by a person. The owner should listen to each one before the library is shown on the site.
- More tracks, and tracks made by our own music model, need a host that we own (a publication step that needs the owner's yes).

## Tests

`tests/user_music.rs`: a track from the store is the sound of the video, the credits and `commercial_ok`; a wrong name; `--local-only` with no store copy; a file with the wrong sha256 from a loopback server is refused and not kept. `music_lib` unit tests: every row has a Commons file, an FMA source, a sha256, a free licence, and a size under 8 MB.
