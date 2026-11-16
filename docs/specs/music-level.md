# `music_level` (5 Nov 2026)

Gap: MoneyPrinterTurbo users set the music volume; ScriptVox had one fixed gain. Brief and brand `music_level`: `quiet`, `normal` (default), `loud`; values below, in the fix section. A bad name stops when the brief loads. Also: `plan`'s `next` names `--approve` only when the plan costs money (test 3).

## Fix, 5 Nov 2026 (bed could not be heard)

Measured with `edit --music`: the duck (threshold 0.02, ratio 10) took the bed 31 dB under the speech, and `loud` against `quiet` changed nothing a listener could hear. The bed also depended on the voice engine and on the music file's own level.

Now every voice enters the mix at -22 LUFS, every bed is measured and placed `music_level` dB under it in the pauses (quiet 14, normal 10, loud 7), and the duck lowers it about 7 dB more (threshold 0.04, ratio 3). Also for a generated bed and a base video's sound, which kept their own level before. Effects and ambience stay as they were for Edge voices, and are the same for other engines now.

Measured, RMS under Edge speech at -17.5 dB: calm 38.3, 34.6, 32.1 dB (21, 17, 15 under the voice); pulse 33.8, 29.2, 26.4 dB (16, 12, 9 under; its energy is low bass). Words heard with `calm` and `pulse` at `loud`: 20 of 20. ACE-Step bed at `normal`: 27 of 27, -14.1 LUFS. No listener test: the numbers are measured, not heard.
A track is raised by at most 14 dB on its way in, so a file with only room tone stays under the bed. The mix is float, so a peak survives to the loudness pass.
