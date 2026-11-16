# WSL release checklist

Run on WSL (Ubuntu or Arch), from the release tarball, in a fresh folder.

- [ ] `sh install.sh --from dist/scriptvox-<v>-x86_64-unknown-linux-musl.tar.gz`: installs to `~/.local/bin`, prints the path, no profile edited.
- [ ] `scriptvox doctor --json`: exit 0 (or exit 1 with `--fix` that works), `mode_ready` lists local and hybrid.
- [ ] `scriptvox demo --json`: exit 0, 240 frames.
- [ ] Arabic burn-in: `scriptvox make --script ar.txt --json`: exit 0, captions read right to left (check the sheet).
- [ ] `scriptvox make ... --detach --json`, then `scriptvox wait <id> --json`: exit 0.
- [ ] `echo $KEY | scriptvox keys set pexels --stdin`: `keys.toml` is mode 0600, the key is never printed.
- [ ] Own media: `scriptvox make --script s.txt --media clips --json` finishes in under 5 minutes from install.
- [ ] A project on `/mnt/c`: `state.db` goes to the data folder (doctor says so).

## Result, 2026-11-02 (scripts/clean-check.sh, fresh HOME and data folder, minimal PATH)

All items passed in 40 s from install to the own-media video: install from the tarball, doctor exit 1 with a safe fix, `doctor --fix` (FFmpeg 152 MB with one retry after a connection reset, uv 18 MB) to `mode_ready: local, hybrid`, demo 240 frames, Arabic make, detached own-media make plus wait, `keys set --stdin` mode 600. HOME afterwards holds only `.config`. Not checked here: a project on `/mnt/c` (covered by tests/run_store.rs).
