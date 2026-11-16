---
name: scriptvox-known-errors
description: When a scriptvox command fails or a gate does not pass - read the exit code and the envelope, take the one safe next step, and know which failures must never be retried because money may be spent.
---

# scriptvox-known-errors

Every command with `--json` prints one envelope. Read `exit`, `error.code`, `error.fix`, `findings` and `next`. The `fix` line and the first `next` entry are written for this exact failure. Use them before this table.

## Exit codes

| Exit | Code | Meaning | Do this |
|---|---|---|---|
| 0 | `SVX_OK` | Done. Also a detached start. | With `--detach`, run `scriptvox wait <run_id> --json`. |
| 1 | `SVX_ERROR` | An error you can fix. | Read `error.fix`. Do it. Run again. |
| 2 | `SVX_USAGE` | A wrong flag, a missing file or a bad brief. | Correct the command. Nothing ran. Do not retry it unchanged. |
| 3 | `SVX_GATE_FAILED` | A quality gate failed. The video stays. | Read the failed finding below. Fix the cause through the brief, then `scriptvox resume`. |
| 4 | `SVX_APPROVAL_NEEDED` | This plan needs the user's yes. | Show the plan and its cost. Wait for the user. Never approve for the user. |
| 5 | `SVX_PROVIDER_RETRYABLE` | A provider or network fault. Nothing was charged. | Retry once or twice. Then pick another row with `scriptvox providers --json`. |
| 6 | `SVX_UNCONFIRMED_CHARGE` | A paid call may have run. The answer is unknown. | **Stop. Do not retry.** Tell the user. A retry can pay twice. `error.fix` says what to check. |
| 7 | `SVX_WATCHDOG_KILL` | A render step stalled and was stopped. | Run `scriptvox resume <run_id> --json`. Finished chunks are kept. |
| 8 | `SVX_CANCELLED` | The run was cancelled. | Ask before you start it again. |
| 9 | `SVX_STILL_RUNNING` | `wait` timed out. The run goes on. This is not a failure. | Run the command in `error.fix` (`scriptvox wait <run_id> --timeout <n>`). `scriptvox status <run_id>` says `alive`. Do not start a second run. |
| 10 | `SVX_NEEDS_INPUT` | A key, a setup step or a choice is missing. | Read `human_action` and `next`. A key is the user's step: `scriptvox keys set <provider>`. Never ask for a key in chat. |
| 70 | `SVX_INTERNAL` | A bug in scriptvox. | Do not work around it. Report the command and the output of `scriptvox doctor --redact --json`. |

## Gates

A `fail` stops with exit 3. A `warn` lets the run finish, and you must say it to the user. Each finding has `detail` and `fix`. Take the `fix`, then look at the frames: `scriptvox snapshot <run_id> --sheet 9 --json`.

| Gate | It measures | Usual cause | First step |
|---|---|---|---|
| `black` | A black stretch of the picture | A black shot or a bad clip | Replace that media file or stock clip. |
| `first_frame` (warn) | The video starts on a black frame | A fade from black, or black frames at the start of the first clip | Start on a picture: a feed shows the first frame. Cut the black frames with `scriptvox edit --keep`. |
| `flashes` (warn) | 3 or more hard changes of the picture inside 1 s | A strobe, a flash, or very fast cuts | Slow the cuts at the time the finding gives, or remove the flash. More than 3 flashes a second can harm a viewer. |
| `slideshow` | How much of the video does not move | Still pictures only | Use video clips, or scenes with camera motion. |
| `silence` | A long stretch with no sound | A pause tag, or no music bed | Check the pause tags, or give the brief `music`. |
| `loudness` | The level of the master | A video that scriptvox did not master | Render it with scriptvox. The master goes to -14 LUFS. |
| `av_drift` | Sound against picture length | Lengths differ by more than one frame | Render again. Report it if it repeats. |
| `words` | Script words heard in order | A dropped line, or a loud bed | Say which part failed. Remake the voice, or use a calmer bed. |
| `captions` | Time on screen for each caption page | Numbers or symbols the voice reads longer | Open a snapshot at the time it names. Write numbers as the voice says them. |
| `text_bounds` | Every text box against the frame | A long word in a title or a label | Shorten the text. On a warning, look at that scene. |
| `contrast` (warn) | The letters of up to 8 text boxes against the picture around them, as a contrast ratio | Captions with `caption_box` 0 on a light picture | Give the box back, or a `caption_outline` of 4 or more, or a darker picture. Then look at the time the finding gives. |
| `loop_seam` (warn) | The last frame against the first, only with brief `loop` | The video ends on another picture, or on an outro | End on the picture it starts on, or drop `loop`. |
| `fit` | The voice rate needed to fill the length | Too many or too few words | Take the word count in the finding. |
| `base_text` | Text already burned into a source video | Footage with its own captions | Use footage with no text, or accept it. |
| `media_match` | Pictures against the words | Stock that does not fit the scene | Give other search terms, or your own media. |
| `clip_black`, `clip_decode`, `clip_length`, `clip_motion`, `clip_size` | A generated or stock clip that came back | A bad clip from the row | The clip may have cost money. Do not resubmit by yourself. Show the finding, and let the user choose a new prompt or row. |
| `platform` | Shape, length and size against one platform's limits | The wrong export | Use `scriptvox edit --reframe` or `scriptvox export`, then `scriptvox check --platform`. |

These are warnings only. Say each one to the user.

| Warning | It means | First step |
|---|---|---|
| `lang` | The script looks like another language than the voice. | Set `--lang`, or pick a voice from `scriptvox voices --json`. |
| `picture_shape` | A start picture has another shape than the video, so part of it is cut. | Give a picture in the video's shape, or look at the first frame. |
| `emphasis` | A word in `emphasis` is not in the script. | Use the word as the script says it. |
| `behind` | The subject hides a large part of a title placed behind it. | Move the title, or look at that scene. |

## Rules

1. Never retry exit 6 by yourself. This rule has no exception.
2. A finding on the `not_checked` list was not looked at. Do not report it as passed.
3. `coverage` and `limitations` say what a pass cannot show. Say the limit with the pass.
4. Exit 2 and exit 4 mean that nothing ran and nothing was charged. A wrong flag needs a new command, not a retry.
5. Exit 2 with "no picture, video or sound format", or with a list or XML file named, means an input file is not plain media. Its first bytes decide, not its name. Give a real picture, clip or sound file.
6. Do not change a gate's result by editing `result.json`. Fix the video.
