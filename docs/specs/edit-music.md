# `edit --music` (5 Nov 2026)

Gap: MoneyPrinterTurbo and OpenMontage add a bed in their pipelines; scriptvox could add one only in `make`.

- `scriptvox edit <video> --music calm|pulse|<file in the project> [--music-level quiet|normal|loud]`.
- Same mix as `make`: the file's own sound ducks the bed, fades 1 s at each end, master -14 LUFS. A file with no sound gets the bed alone.
- It mixes with every other edit (cuts first, then the bed, so the bed does not jump at a cut).
- Music alone copies the picture when the codec fits an MP4 (h264, hevc, av1, mpeg4), and says `picture_copied: true`. Others are encoded. Sound comes out at 48 kHz stereo.
- Same file rule as `make`: a music file lives inside the project. `export` of the result stays `commercial_ok: false`.

Measured (7.4 s Edge voice clip): picture MD5 unchanged, -14 LUFS, levels in `music-level.md`. No sound source: bed alone at -14.2 LUFS. With `--from --to --speed`: 3.2 s, av_drift 0.0 ms.

13 Nov 2026: `--music` also takes an https address, with `--music-from` and `--music-to`. See `fetch.md` section 11.
