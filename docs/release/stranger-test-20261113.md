# Stranger test, 13 Nov 2026

Question: can a person with no knowledge of the project install the present product and use it? Answer: **yes on Linux x86_64 and on Windows 11 x64, from a release folder, in about 1.5 minutes to the first narrated video.** What is missing is the public release itself: there is no public download address yet. Nothing was published in this test.

## What was built

The release was built from the public tree, not from the work tree.

| Step | Result |
|---|---|
| `bash scripts/public-tree.sh <folder>` | 681 files, "clean: no internal names" |
| `just package` in that folder (static musl binary) | `scriptvox-0.1.0-x86_64-unknown-linux-musl.tar.gz`, 10.6 MB, with a `.sha256` file |
| `just package-windows` (llvm-mingw, target gnullvm) | `scriptvox-0.1.0-x86_64-pc-windows-gnullvm.zip`, 9.8 MB, with a `.sha256` file |
| Build time | 6 min 14 s for both, `-j2` |
| Toolchains | The musl gcc and the llvm-mingw folder that the justfile names were both there. Nothing was downloaded |

Which build ran: the Linux run, the Windows run and trial T1 used the build of the first commit of this work (the new installers). The fixes to the tool came after (the doctor line on a terminal, the size in the worker line). The release was then built again from the final public tree (682 files, clean), and that build got these runs only: the Linux install from the address and from a local tarball, `doctor` on a terminal, the message for a wrong address, and on Windows `install.ps1 -From` with `--version`. The new worker line was not seen in a run.

The justfile has no `dist` recipe. The recipes are `package` and `package-windows`, and they write the `dist` folder. They now also copy `install.sh` and `install.ps1` into `dist`, because the install prompt gets the script first.

## Linux

Sandbox: `bwrap` with the home folder, the Windows drives and `/tmp` hidden, a new empty HOME, `env -i` with `PATH=/usr/bin:/bin`, `HOME` and `TERM` only. No cargo, no checkout. The machine is Arch Linux on WSL 2, and the sandbox ran with 2 CPUs. The `dist` files were served by `python3 -m http.server` on a loopback address. The tool downloaded its own pinned FFmpeg (the FFmpeg in `/usr/bin` was not used, as designed).

| Step | Result | Time |
|---|---|---|
| `curl -fsSL <address>/install.sh -o install.sh`, then `SCRIPTVOX_BASE_URL=<address> sh install.sh` | "sha256 ok", installed to `~/.local/bin`, prints the PATH line | 1 s |
| `export PATH="$HOME/.local/bin:$PATH"` (the line that the installer prints) | | |
| `scriptvox --version` | exit 0, 0.1.0 | 0 s |
| `scriptvox doctor` | exit 1: FFmpeg not found, uv not found (warning). Fix line: `scriptvox doctor --fix` | 1 s |
| `scriptvox demo` | exit 0, 240 frames. It downloaded FFmpeg itself, with one line that says so | 28 s |
| `scriptvox check scriptvox-demo.mp4` | exit 0, 6 gates pass, loudness is a warning | 1 s |
| `scriptvox doctor` | exit 0, `mode_ready` local | 0 s |
| `scriptvox make --script script.txt --music calm` (first example of the README, free Edge voice) | exit 0, 6.9 s video, 17 words, 2 caption pages, 10 gates pass, -14.0 LUFS | 33 s |
| `scriptvox check` on that video, `snapshot --sheet 9` | exit 0 | 1 s |
| **From the start of the install to the first narrated video** | | **78 s** |

Downloads on Linux:

| What | Download | On disk |
|---|---|---|
| Release tarball | 10.6 MB | binary 24 MB |
| FFmpeg n8.1.3 (pinned, sha256 checked) | 152 MB | 516 MB |
| uv 0.13.1 (pinned) | 18 MB | 43 MB |
| Python 3.12 and the voice packages (by uv) | not measured | 107 MB, plus 13 MB of uv cache |
| Total in the data folder | | about 680 MB |

The empty home afterwards holds `.local/bin/scriptvox` and `.local/share/scriptvox` only. No profile file was made.

## Windows 11

The real Windows 11 host of the build machine, Windows PowerShell 5.1, an ordinary user account. All of it in one new folder under `%TEMP%`: install target, `SCRIPTVOX_CONFIG`, `SCRIPTVOX_DATA`, `TEMP`, `TMP`. The table is in `windows11-checklist.md` (run of 13 Nov). In short: `install.ps1` from the loopback address in 4 s, `--version`, `doctor` (exit 1 with a fix line, then exit 0), `demo` in 18 s with a 194 MB FFmpeg download, `check` exit 0, and the first narrated video in 42 s with a 15 MB uv download. **96 s from the start of the install to the first narrated video.** The data folder was 591 MB.

Outside the folder, these checks found no change: the first-level listing of `%LOCALAPPDATA%`, `%APPDATA%` and `%USERPROFILE%`; the length and text of the user PATH; no file newer than the start of the test in the `scriptvox`, `uv`, `.local` and `.cache` folders that the user already had; no entry with the test folder under `HKCU\Software\Python` (uv can register its Python there). Other registry keys and deeper folders were not checked. No process was left. The folder was deleted at the end.

Local archive: `install.ps1 -From <zip> -Dest <folder>` with the `.sha256` file next to the zip gave "sha256 ok" and exit 0. With a wrong `.sha256` file it gave "checksum mismatch, nothing installed", exit 1, and no exe. On Linux, `sh install.sh --from <tarball>` gave the same two results.

No security product stopped the exe. This does not show what SmartScreen does with a zip from a browser: the exe is not signed.

## Trial T1: a fresh agent with only the install prompt

One run, Claude Code headless through `scripts/agent-trial.sh claude <prompt>` with `AGENT_TRIAL_INSTALL=1`. The agent had an empty folder, a PATH with only itself, an empty install folder, `/usr/bin` and `/bin` (no cargo), its own `SCRIPTVOX_CONFIG` and `SCRIPTVOX_DATA`, and `SCRIPTVOX_BIN_DIR` set to the empty install folder (the agent needs the real home for its login, so the install must not go to `~/.local/bin` of that home). Hidden with `bwrap`: the secrets folder, the cloud drive folders, the Windows drives, the ScriptVox config and data of the home, every checkout and build folder, cargo and rustup, the skills folders of all agents, the session history of the agent, and the other scratch folders.

The prompt was the README block with step 1 as it can read at the release. Step 1 in this test:

```text
1. Get the binary.
   Linux x86_64: download the install script, then run it. It checks the sha256 of the download and prints where the binary is.
     curl -fsSL http://127.0.0.1:<port>/install.sh -o install.sh
     SCRIPTVOX_BASE_URL=http://127.0.0.1:<port> sh install.sh
   Windows x64 (PowerShell): get install.ps1 from the same address, then run: powershell -ExecutionPolicy Bypass -File install.ps1 -BaseUrl http://127.0.0.1:<port>
   If the folder of the binary is not on PATH, call the binary by its full path, or add the folder to PATH for this session only.
   There is no build for another system yet. Then stop and tell me.
```

Steps 2 to 5 and the money and key lines were the README text, not changed.

| Measure | Value |
|---|---|
| Result | **Pass.** The demo video exists and `scriptvox check` passes |
| Help from the person | 0 |
| Tool calls | 6 |
| Failed calls | 0 |
| Commands with `--approve` | 0 |
| Time | 35 s of agent time, 1 minute in all |
| Tokens | 0.29 USD (as the agent reported) |
| What it did | Read the install script before it ran it, installed, `doctor` (exit 1), `doctor --fix`, `demo`, looked at the file with ffprobe, installed the skills, then asked what video to make |
| Transcript scan (counts only) | key-shaped texts 0, internal host or organisation names 0. The transcript has the home path of the machine, so it stays out of the tree |

Limits of this trial: one agent, one run, Linux only. The install folder was already on PATH, so the "not on PATH" line of the prompt was not needed. The skills went to a hidden copy of the skills folder, so "the agent then uses the skill" was not tested.

## Stumbles

| # | Stumble | Fix |
|---|---|---|
| 1 | `install.sh` could only download from one fixed address. A mirror or a test could not use it | Fixed: `SCRIPTVOX_BASE_URL` |
| 2 | A failed download in `install.sh` gave only the curl error, and a wrong `--from` path gave only the `cp` error | Fixed: each failure has one line that says what to check, and that nothing was installed |
| 3 | `install.sh` on another system or CPU would install a binary that cannot run | Fixed: it stops with one line that names the builds that exist. Not run on such a machine |
| 4 | `install.ps1` had a fixed target folder, did not check the `.sha256` file next to `-From`, left its temp folder, had no base address, and read the checksum from the response body (a risk on PowerShell 5.1 when the server sends a binary type; the old code was not run, so this fault is not proven) | Fixed: `-Dest` (or `SCRIPTVOX_BIN_DIR`), `-BaseUrl` (or `SCRIPTVOX_BASE_URL`), the checksum is always checked from a file, the temp folder is removed, TLS 1.2 is set, and the progress bar is off. Run on PowerShell 5.1 |
| 5 | The `dist` folder had no install script at its top, so a prompt had nothing to fetch first | Fixed: both `package` recipes copy the script |
| 6 | On a terminal, `doctor` printed `fail: : FFmpeg was not found`, with no name of the check | Fixed in `envelope.rs` (a doctor row has `name`, a gate row has `gate`), with a test |
| 7 | "preparing the voice worker (Python 3.12 and packages, once)" gave no size, and the README gave no size for any first-run download | Fixed: the line says about 100 MB on disk, and the README gives the measured sizes |
| 8 | The README did not name the install scripts at all | Fixed: one paragraph in "Install" |
| 9 | `scripts/agent-trial.sh` could not run an install trial: it stopped when `scriptvox` was not on PATH | Fixed: `AGENT_TRIAL_INSTALL=1` |
| 10 | `check` on the demo video gives a loudness warning (-27.9 LUFS, target -14) with the fix "Re-render with scriptvox". The demo is the tool's own video, and a person cannot act on that line | Open. Make the demo sound at -14 LUFS, or leave this gate out for the demo |
| 11 | On Linux the binary goes to `~/.local/bin`, which is not on PATH of a minimal shell. The installer prints the export line, and the agent of T1 used it with no failed call | No change |
| 12 | A fresh `doctor` ends with exit 1 before anything is downloaded. The fix line is correct, and `demo` and `make` also download what they need without it | No change |
| 13 | The README install prompt still has the two placeholder lines, and "Install" still starts with cargo | Open until the release address exists |

No download failed and no path was wrong. The longest wait with no new line was about 20 s: the FFmpeg download, after its one line `downloading ffmpeg ... (152 MB)`.

## Still needed before a public release

1. The real download address: in the README prompt (both placeholder lines), in the "Install" section, in `llms.txt`, and a check that the default address in `install.sh` and `install.ps1` is the real one. Then one run of both installers against the real address with no variable set.
2. A private first release: the default address gives 404 with no login. Its users take `gh release download`, then `install.sh --from` or `install.ps1 -From` (both run here), or a mirror with `SCRIPTVOX_BASE_URL`. The README must say which.
3. The release assets as this test served them: the tarball, the zip, both `.sha256` files, `install.sh`, `install.ps1`.
4. A signature. Today there is a sha256 file from the same server as the archive: it finds a broken download, not a replaced one. The Windows exe is not signed, so SmartScreen can warn on a zip from a browser. Not tested.
5. The demo loudness warning (stumble 10).
6. T1 on the other agents of the matrix (opencode, Codex, Hermes, Pi), and T1 on Windows. Tasks T2 to T8 on the installed binary.
7. Windows steps not run on 13 Nov: the default install folder, Arabic burn-in from the installed exe, `make --detach`, `keys set`, a folder with a space.
8. Linux on a distribution with an old glibc is covered by the static binary, but it was run on one system only (Arch on WSL 2). One run on Ubuntu and one on a machine that is not WSL.
9. `just release-check` was not run here (it runs the full CI and needs the hero videos).
10. macOS: no build. The README says so.
