# `resume <run> --patch p.json` (G066, 8 Nov 2026)

Changing one thing should not need the whole brief again (HeyGen's public docs describe edits of one field; no code read). `resume --brief` drops look fields that the new brief leaves out, so a short brief there would lose them.

- `--patch` takes a JSON merge patch: only the fields that change. `{"hook": "Three colours", "music": null}` sets the hook and drops the music; every other field keeps the parent's. `null` takes a field back to its default (none, empty list, false).
- Only brief fields that a run keeps under the same name. `script_file` and `brand` are refused (give `script` or the fields), so is any field outside the brief (`run_id`, `approve` and the rest). `stock` may be a list, as in a brief. Types are checked: `{"fps": "fast"}` is exit 2.
- Paths are relative to the project folder (the run's own paths are too).
- New `scenes` or `lines` set the script, as `--brief` does; a new `script` with `media_match` drops the old picks.
- Not with `--brief`. The result lists `patched`. The child is a normal child run: `.1`, its own output file, the parent kept.
- MCP: `scriptvox_resume` gets `patch` from the clap tree.

## Measured, 8 Nov 2026, 2 cores

| Check | Result |
|---|---|
| `{"hook": "Three colours", "music": null}` on the 3-shot run of G053, `--wait` | 14.2 s, 7 gates pass, `patched` [hook, music]; the child's request differs from the parent's only in hook, music, out and parent_run; no `bed.wav` in the child run |
| Frame at 0.8 s, parent and child, read by eye | the child has "Three colours" over the red shot, the parent has none; captions the same |
| `{"bogus": 1}`, `{"script_file": ...}`, `{"duration": "long"}`, `[]` | each exit 2 with the reason |
| Unit test on a saved request | 4 fields change and nothing else; new scenes set the script; 4 bad patches refused |

## Not done

A JSON Patch (RFC 6902) list. A patch of one scene inside `scenes` (the whole list is given).

13 Nov 2026: a patch that changes the words of some scenes voices only those scenes, and the others keep their voice and pace (`docs/specs/resume-scene-voice.md`).

13 Nov 2026: `--patch` takes `sources_card` (true, false or null) and `sources` on a run that had none (`docs/specs/sources-card.md`).
