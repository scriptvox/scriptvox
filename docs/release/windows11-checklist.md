# Windows 11 release checklist (release blocking)

Run on native Windows 11 (not WSL), PowerShell, a fresh user folder.

- [ ] `install.ps1 -From scriptvox-<v>-x86_64-pc-windows-gnullvm.zip`: installs to `%LOCALAPPDATA%\scriptvox\bin`, no admin, PATH untouched.
- [ ] `scriptvox doctor --json`: exit 0 after `scriptvox setup` (FFmpeg win64 pin).
- [ ] `scriptvox demo --json`: exit 0.
- [ ] Arabic burn-in renders with the embedded fonts (paths with spaces and an apostrophe in the user name).
- [ ] `make --detach` runs in the foreground with a warning (flip rule d), then `wait` exit 0.
- [ ] `keys set pexels --stdin`: `keys.toml` gets a user-only ACL.
- [ ] Own-media run in under 5 minutes from install.

Run 5 Nov 2026, Windows 11 evaluation VM, llvm-mingw cross-build of the newest tree, a folder with a space and an apostrophe, the SYSTEM account through the VM's script runner, FFmpeg by `SCRIPTVOX_FFMPEG` (the VM has no internet, so `setup` and `install.ps1` from a zip were not run):

| Step | Result |
|---|---|
| `--version`, `doctor`, `demo` | exit 0 |
| Arabic burn-in in that folder | exit 0, 3.7 s, right to left and shaped, seen by eye |
| `make --detach` | runs in the foreground, exit 0, 4.1 s, with `transition: zoom`. The warning text was not read |
| `keys set pexels --stdin` | **Fault found and fixed:** the ACL step used `USERNAME`, failed with no message under SYSTEM, and left "Authenticated Users: modify". Now `whoami` with the exit status checked; a failure removes the file and stops. After the fix: `NT AUTHORITY\SYSTEM:(F)` only. Not run as an ordinary user |
| Own media | exit 0, 4 s |
| `export --format webm,hls --overlay`, `check`, `snapshot`, `join --transition zoom` | exit 0 |

Not run: `install.ps1` and PATH, `doctor` after `setup`, an interactive user account.

Run 9 Nov 2026, the same VM (build 26200), llvm-mingw cross-build of commit 114d9f8, the pinned win64 FFmpeg by `SCRIPTVOX_FFMPEG`, a folder with a space and an apostrophe, the SYSTEM account through the VM's script runner, a fake key value, no internet.

| Step | Result |
|---|---|
| `--version`, `doctor`, `demo` | exit 0 |
| `export` onto its own input, same path and other letter case | exit 0, input unchanged by hash |
| `edit --out` onto its input, through `..` and in other letter case | exit 2, input unchanged by hash |
| `snapshot --out` onto its input through `..` | exit 2, input unchanged by hash |
| `keys set` with a damaged `keys.toml` | exit 1, file unchanged by hash |
| `keys set` new file | exit 0, ACL is the owner only |
| `check` on random bytes, `export` on a text file | exit 2, no video stream |
| Arabic burn-in | exit 0, shaped and right to left, seen by eye |
| Own media | exit 0, gates pass, `text_bounds` pass |
| `cancel` of a running `make` | exit 8, status cancelled, no `scriptvox.exe` or `ffmpeg.exe` left, a second `cancel` is exit 2 |

Not run: the Python worker on a Windows code page (robustness-1, needs internet for uv and Python), `install.ps1` and PATH, `doctor` after `setup`, an interactive user account. Test files removed from the VM.

Run 13 Nov 2026, the real Windows 11 host of the build machine (Windows PowerShell 5.1.26100), an ordinary user account with internet, started from WSL with `powershell.exe -NoProfile`. The zip is the llvm-mingw cross-build of the public tree (`scripts/public-tree.sh`, then `just package-windows`), served from a loopback address. Everything was in one new folder under `%TEMP%`: the install target (`-Dest`), `SCRIPTVOX_CONFIG`, `SCRIPTVOX_DATA`, `TEMP` and `TMP`. Full report: `stranger-test-20261113.md`.

| Step | Result |
|---|---|
| `install.ps1 -BaseUrl http://127.0.0.1:<port> -Dest <folder>\bin` (script and zip from the loopback address) | exit 0, 4 s, zip 9.8 MB, "sha256 ok", exe 22 MB. No admin, PATH not changed |
| `scriptvox.exe --version` | exit 0, 0.1.0 |
| `scriptvox.exe doctor` (fresh) | exit 1, FFmpeg not found, fix line `scriptvox doctor --fix` |
| `scriptvox.exe demo` | exit 0, 18 s with the FFmpeg download (194 MB), 240 frames |
| `scriptvox.exe check scriptvox-demo.mp4` | exit 0, 6 gates pass, loudness is a warning (-27.9 LUFS) |
| `scriptvox.exe doctor` (after the demo) | exit 0, `mode_ready` local and hybrid |
| `scriptvox.exe make --script script.txt --music calm` (the first example of the README, free Edge voice, no uv on PATH) | exit 0, 42 s with the uv download (15 MB) and the Python worker, 6.9 s video, 10 gates pass |
| `scriptvox.exe check` on that video | exit 0 |
| From the start of the install to the first narrated video | 96 s |
| Data folder afterwards | 591 MB (tools 518 MB, Python 67 MB) |
| `install.ps1 -From <zip> -Dest <folder>\bin` (build of the final tree) | With the `.sha256` file: "sha256 ok", exit 0. With a wrong `.sha256` file: "checksum mismatch, nothing installed", exit 1, no exe |
| Outside the folder | The listing of `%LOCALAPPDATA%`, `%APPDATA%` and `%USERPROFILE%` (first level) and the user PATH are the same before and after; no newer file in the user's own `scriptvox`, `uv`, `.local` and `.cache` folders; no entry of the test folder under `HKCU\Software\Python`. No process of the folder was left. The folder was deleted |

Not run on 13 Nov: the default target `%LOCALAPPDATA%\scriptvox\bin`, a zip downloaded with a browser (it gets the mark of the web, and the exe is not signed, so SmartScreen can ask), Arabic burn-in, `make --detach`, `keys set`, a folder with a space, an interactive console window.
