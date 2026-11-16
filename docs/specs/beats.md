# `analyze --beats`: the beat grid of a file's sound (G123, first part, 10 Nov 2026)

Why: the 10 Nov comparison pass lists a beat grid as missing; the sound skill said "not supported".

- `scriptvox analyze <file> --beats` adds `beats` to the result: `bpm`, `confidence` (0 to 1), `steady`, `count`, `interval_s`, `times` (seconds) and a `note`. `metrics.bpm` has the tempo.
- No row and no network. FFmpeg decodes the first audio stream to mono at 11025 Hz, at most 10 minutes; the rest is `src/beats.rs`.
- Method: the power is smoothed over about 30 ms; a rise is the gain in log energy over 50 ms, above 0.08 and above its mean over 1 s. The tempo is the lag with the most autocorrelation of that curve between 60 and 200 BPM, with a mild pull to 120 so a tempo wins over its half and its double. The phase is the offset whose grid collects the most rise. The times are moved 35 ms earlier, the measured lateness of the curve.
- One steady tempo is assumed. `confidence` is the mean autocorrelation at the lag and its two neighbours. `steady` is true from 0.25.
- Under 4 s of sound, or silence: `bpm` null, no times. A file with no audio stream: exit 2.

Measured on 10 Nov 2026 at home, on real files through the command:

| File | Truth | `bpm` | `confidence` | `steady` |
|---|---|---|---|---|
| Click every 0.5 s | 120 | 120.2 | 0.85 | true |
| Built-in `pulse` bed, no voice | 100, a soft swell | 100.2 | 0.56 | true |
| Built-in `calm` bed, no voice | no beat | 71.7 | 0.01 | false |
| Edge voice alone | no beat | 72.4 | 0.06 | false |
| Voice over the `calm` bed, English and Arabic | no beat | 162.5, 81.3 | 0.15, 0.10 | false |
| Promo with voice over `pulse` | 100 under a voice | 113.4 | 0.18 | false |

So a voice over a bed hides the bed's beat: analyze the music file, not the finished video. Click times were within 40 ms of the clicks over 10 s; the tempo is known to about 0.3 BPM, so a grid drifts on a long track.

Faults found on the way, each now a unit test: a pad of three sines read as 113 BPM at 0.91 (windows too short); the 100 BPM swell read as 97 (one reading per hop let a 110 Hz ripple in); one false rise at the start (the smoother began at zero); a lone spike at one lag on the pad (0.58, with 0.05 and 0.02 beside it).

Tests: 5 unit tests in `src/beats.rs` (clicks at 90, 120 and 150; the swell; the pad; a tone; silence and a short sound) and `beats_gives_the_tempo_of_a_click_track_and_none_for_a_tone` in `tests/analyze.rs`.

Scene cuts on the grid: brief `cut_on: "beat"`, see `cut-on-beat.md` (13 Nov 2026). Not run: real recorded music with drums, a track that changes tempo, a file longer than 10 minutes.
