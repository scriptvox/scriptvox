# Variable-rate clips (5 Nov 2026)

Radar: ComfyUI #17517 (a reported frame rate is the time base, not the real rate) and HyperFrames #7115 (FFmpeg errors say why it died).

Measured: a variable-rate clip, nominal 60 fps, real 26.4 fps, 8 s. `edit --cut-silence` reported 6.81 s and wrote 2.33 s: `setpts=N/FRAME_RATE` squeezed
the picture and `-shortest` cut the sound with it. `av_drift` passed because both were short. Real phones and screen recorders make such clips.

- `edit`: a clip whose nominal and average rate differ by more than 1 percent goes through `fps=<rounded average>` first (15 to 60). Result 6.77 s, 176 frames, av_drift 7 ms.
- New finding `length` (warn): the file differs from what the edit kept by more than 0.5 s, for any reason.
- `analyze`: `fps` is the real average; `variable_rate: true` when it differs.
- `make --media` was already right (`framerate` works on time); checked with the same clip, 7 gates pass.
- FFmpeg errors name the exit code or signal: `FFmpeg failed (signal: 9 (SIGKILL)): ...`, so an empty log still says something.
