# No voice: `tts: none` (5 Nov 2026)

Gap: every narrated tool needs a voice; text-on-screen reels with music are common, and HyperFrames or Remotion make them only with hand-written timing. `tts: none`: one registry row, never chosen as a fallback. Word times at reading pace (2.5 words a second, Arabic 1.8; longer words a little longer), pause tags hold, `duration` changes the pace through the fitter. Audio is silence, with the music bed if any. Plain silence skips loudnorm (it has no level: FFmpeg refused -inf). Gates: `silence` and `words` skipped, `loudness` skipped only with no music.

Measured: r_8927ca reveal + calm bed 9 s -13.4 LUFS; r_acafac cards on 16:9 with no sound 6.6 s; r_fe61e1 Arabic to 12.0 s. Checked by eye.

Metrics: `voice_rate_pct` and `voice_tempo` are left out when there is no voice (they showed 0 and 1.0).
