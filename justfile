# Local quality gates. Nothing here pushes. A local `.env` (not committed) may set SCRIPTVOX_CARGO and SCRIPTVOX_FFMPEG.
set dotenv-load
export PATH := env("HOME") + "/.cargo/bin:" + env("PATH")
# FFmpeg for tests that call it: the pinned build `scriptvox setup` installs. Override with SCRIPTVOX_FFMPEG.
# Tests that need FFmpeg are skipped when the file is missing.
export SCRIPTVOX_FFMPEG := env("SCRIPTVOX_FFMPEG", `ls -d "${SCRIPTVOX_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/scriptvox}"/tools/ffmpeg/*/bin/ffmpeg 2>/dev/null | tail -1`)
# Tests share the user's uv cache (a per-test cache would download Python and packages each time).
export UV_CACHE_DIR := env("UV_CACHE_DIR", env("HOME") + "/.cache/uv")
export UV_PYTHON_INSTALL_DIR := env("UV_PYTHON_INSTALL_DIR", env("HOME") + "/.local/share/uv/python")
# Cargo command. Plain `cargo` by default; on a shared machine set SCRIPTVOX_CARGO, e.g. "light cargo".
cargo := env("SCRIPTVOX_CARGO", "cargo")
jobs := "-j2"

# Format and lint checks.
check:
    cargo fmt --all --check
    {{cargo}} clippy {{jobs}} ${SCRIPTVOX_FEATURES:-} --all-targets -- -D warnings
    uvx ruff check python
    # The Claude plugin and marketplace manifests (when the claude CLI is installed).
    if command -v claude >/dev/null; then claude plugin validate --strict . >/dev/null && claude plugin validate --strict plugin >/dev/null; fi
    just skills-check

# Every skill: frontmatter, size, real commands and flags, and no 8-word run shared with a studied project's skills.
skills-check:
    python3 scripts/skills_check.py --selftest
    {{cargo}} build {{jobs}} ${SCRIPTVOX_FEATURES:-} -q
    python3 scripts/skills_check.py

# Licence gates: crate licences and bans, REUSE metadata on every file.
# Python packages: licenses/python-packages.tsv, generated from the uv.lock files; a test fails when a row is missing.
# ponytail: cargo-about at the first release.
licenses:
    cargo deny check licenses bans
    uvx reuse lint
    @just provenance

# Every media file needs a source: a row in PROVENANCE.tsv or a <file>.license sidecar (PLAN-R1 L0).
provenance:
    #!/bin/sh
    bad=0
    for f in $(git ls-files --cached --others --exclude-standard | grep -Ei '\.(png|jpe?g|gif|webp|svg|mp4|mov|webm|mkv|mp3|wav|ogg|flac|m4a|ttf|otf|woff2?)$'); do
        [ -e "$f" ] || continue
        grep -qs '^Source: ' "$f.license" || grep -qs "^$f	" PROVENANCE.tsv || { echo "no provenance: $f (add a row to PROVENANCE.tsv or $f.license with a 'Source: ' line)"; bad=1; }
    done
    exit $bad

# All tests. Snapshots never auto-update here. Tests never see the developer's real keys or installed plugins: an empty config and data folder,
# and no provider key variables.
test:
    rm -rf target/test-config target/test-data target/tmp/test-config target/tmp/test-data && mkdir -p target/test-config target/test-data
    env -u HF_TOKEN -u ELEVENLABS_API_KEY -u PEXELS_API_KEY -u PIXABAY_API_KEY -u FAL_KEY -u REPLICATE_API_TOKEN SCRIPTVOX_CONFIG="$(pwd)/target/test-config" SCRIPTVOX_DATA="$(pwd)/target/test-data" INSTA_UPDATE=no {{cargo}} test {{jobs}} ${SCRIPTVOX_FEATURES:-}
    uv run --quiet --frozen --project python pytest -q python/tests

# The full local CI. Run on a clean checkout: git stash -u && just ci
ci: check licenses test
    @echo "just ci: green"

# Build and link the dev binary into ~/.local/bin.
install-dev:
    {{cargo}} build {{jobs}} ${SCRIPTVOX_FEATURES:-}
    ln -sf "$(pwd)/target/debug/scriptvox" ~/.local/bin/scriptvox

# Re-render a golden reference after a deliberate visual change. Writes a BLESS.md line.
golden-bless fixture="kinetic":
    {{cargo}} build {{jobs}} ${SCRIPTVOX_FEATURES:-}
    ./target/debug/scriptvox demo --use-system --golden --json --out fixtures/golden/{{fixture}}/ref.mp4 > /dev/null
    echo "$(date -I) {{fixture}}: $(git log -1 --format=%h) $(sha256sum fixtures/golden/{{fixture}}/ref.mp4 | cut -c1-16)" >> fixtures/golden/BLESS.md

# CI, then a signed local commit only when CI is green. Usage: just commit "feat: ..."
commit msg:
    cargo fmt --all
    just docs > /dev/null
    uvx ruff check --fix -q python
    just ci > /tmp/claude-1000/scriptvox-ci.log 2>&1 || { tail -20 /tmp/claude-1000/scriptvox-ci.log; exit 1; }
    git add -A
    # The session link comes from the caller (SCRIPTVOX_SESSION_URL); a fixed one gave every later session's commits
    # the first session's link.
    printf '%s\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n%b' "{{msg}}" "${SCRIPTVOX_SESSION_URL:+Claude-Session: $SCRIPTVOX_SESSION_URL\n}" | git commit -q -s -F -
    git log --oneline -1

# Regenerate schemas/*.json from the Rust types (CI fails when they are stale).
schemas:
    {{cargo}} build {{jobs}} ${SCRIPTVOX_FEATURES:-}
    for s in brief result event openapi; do ./target/debug/scriptvox schema $s --json | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin)["json_schema"], indent=2, sort_keys=True))' > schemas/$s.json; done

# Regenerate the generated docs (the Claude skill) from the binary.
docs:
    {{cargo}} build {{jobs}} ${SCRIPTVOX_FEATURES:-}
    ./target/debug/scriptvox skills render --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["skill"], end="")' > plugin/skills/scriptvox/SKILL.md
    mkdir -p plugin/skills/scriptvox-reference
    ./target/debug/scriptvox skills render --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["reference"], end="")' > plugin/skills/scriptvox-reference/SKILL.md

# Every pinned download still resolves. Network; run by hand or on a timer, not in `just ci`.
pins-check:
    python3 scripts/check_pins.py

# Regenerate the third-party notices embedded in the binary.
notices:
    {{cargo}} about generate -m crates/scriptvox/Cargo.toml licenses/about.hbs -o licenses/THIRD_PARTY.md

# Release gate: CI green, generated files fresh, every launch asset present (PLAN-R1 §7, step 19).
release-check version="0.1.0":
    just ci
    @missing=0; for f in docs/launch/hero-en.mp4 docs/launch/hero-ar.mp4 docs/launch/kill-and-resume.gif docs/launch/session-s4.gif docs/release/windows11-checklist.md docs/release/wsl-checklist.md CHANGELOG.md; do [ -e "$f" ] || { echo "missing: $f"; missing=1; }; done; [ "$missing" = 0 ] || { echo "release-check {{version}}: NOT ready"; exit 1; }
    @grep -q 'version = "{{version}}"' Cargo.toml || { echo "Cargo.toml is not at {{version}}"; exit 1; }
    @echo "release-check {{version}}: ready"

# Static Linux binary (musl), for the release tarball. Needs the musl cross gcc in ~/.local/opt.
musl := env("HOME") + "/.local/opt/x86_64-linux-musl-cross/bin/x86_64-linux-musl-gcc"
build-musl:
    CC_x86_64_unknown_linux_musl={{musl}} CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER={{musl}} {{cargo}} build --release {{jobs}} --target x86_64-unknown-linux-musl
    ls -la target/x86_64-unknown-linux-musl/release/scriptvox

# Windows exe, cross-built with llvm-mingw (no MSVC licence). The C runtime is linked statically.
mingw := env("HOME") + "/.local/opt/llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64/bin"
build-windows:
    CC_x86_64_pc_windows_gnullvm={{mingw}}/x86_64-w64-mingw32-clang AR_x86_64_pc_windows_gnullvm={{mingw}}/llvm-ar CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER={{mingw}}/x86_64-w64-mingw32-clang CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_RUSTFLAGS="-C target-feature=+crt-static" PATH={{mingw}}:$PATH {{cargo}} build --release {{jobs}} --target x86_64-pc-windows-gnullvm
    ls -la target/x86_64-pc-windows-gnullvm/release/scriptvox.exe

# Release tarball with the notices and a sha256 file (local; nothing is published).
package: build-musl
    #!/bin/sh
    set -eu
    v=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    n="scriptvox-$v-x86_64-unknown-linux-musl"
    rm -rf "dist/$n" && mkdir -p "dist/$n"
    cp target/x86_64-unknown-linux-musl/release/scriptvox LICENSE README.md CREDITS.md licenses/THIRD_PARTY.md LICENSES/OFL-1.1.txt install.sh "dist/$n/"
    tar -czf "dist/$n.tar.gz" -C dist "$n"
    (cd dist && sha256sum "$n.tar.gz" > "$n.tar.gz.sha256")
    cp install.sh dist/  # a release asset too: the install prompt fetches it first
    ls -la "dist/$n.tar.gz"

# Windows zip with the notices, the installer and a sha256 file (local; nothing is published). The name is the one
# install.ps1 asks for.
package-windows: build-windows
    #!/bin/sh
    set -eu
    v=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    n="scriptvox-$v-x86_64-pc-windows-gnullvm"
    rm -rf "dist/$n" "dist/$n.zip" && mkdir -p "dist/$n"
    cp target/x86_64-pc-windows-gnullvm/release/scriptvox.exe LICENSE README.md CREDITS.md licenses/THIRD_PARTY.md LICENSES/OFL-1.1.txt install.ps1 "dist/$n/"
    (cd dist && python3 -m zipfile -c "$n.zip" "$n" && sha256sum "$n.zip" > "$n.zip.sha256")
    cp install.ps1 dist/
    ls -la "dist/$n.zip"
