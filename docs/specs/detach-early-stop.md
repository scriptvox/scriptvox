# A detached run that stops early still has a result and a request

Found 4 Nov 2026 while recording `session-s4.gif`: `make --detach` with a script far too long for its duration
(2.41x, stopped by the estimate before TTS) made `wait` report SVX_SUPERVISOR_DIED, and `resume` said "no saved
request". The §4 test missed it: its stub voice fails after TTS, when the run folder is already in use.

- `main`: a supervisor (`make --run-id X`, `_run X`) whose run has no `result.json` when it ends records its
  envelope with `Run::finish`, so `wait` replays the real error (SVX_NEEDS_SHORTER_SCRIPT, "Cut about 16 words").
  One place for both supervisor kinds and every early return (brief errors too).
- `make::run` saves `request.json` first when it runs as a supervisor, so `resume <run> --brief` works after an
  early stop.
- Test: `session_s4::detached_early_stop_is_replayed_and_resumable` (wait exit 3 with the real code, then resume).
- Checked by hand: early stop, wait (real error), resume with a shorter brief, child 6.0 s, all gates pass.

Also: `scripts/log-to-gif.sh` holds the GIF renderer the kill-and-resume and session scripts share (same output
size as before, 99,086 bytes).
