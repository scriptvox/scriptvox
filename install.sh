#!/bin/sh
# ScriptVox installer (Linux and macOS). It uses no sudo and makes no change to a shell profile.
# On macOS the Homebrew package is the easy way, because it also brings FFmpeg and uv.
#   sh install.sh                            # the release at SCRIPTVOX_BASE_URL (when published)
#   sh install.sh --from scriptvox-*.tar.gz  # a local tarball
# Variables: SCRIPTVOX_VERSION, SCRIPTVOX_BIN_DIR (default ~/.local/bin), and SCRIPTVOX_BASE_URL: the folder
# address that holds the tarball and its .sha256 file (a release mirror, or a local test server).
set -eu
VERSION="${SCRIPTVOX_VERSION:-0.1.0}"
BASE="${SCRIPTVOX_BASE_URL:-https://github.com/scriptvox/scriptvox/releases/download/v$VERSION}"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) TARGET="x86_64-unknown-linux-musl" ;;
  Darwin-arm64) TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  *) echo "scriptvox: no build for $(uname -s) $(uname -m) (Linux x86_64, macOS and Windows x64 only). Build from source: cargo build --release" >&2; exit 1 ;;
esac
NAME="scriptvox-$VERSION-$TARGET"
# macOS has `shasum`, not always `sha256sum`.
if command -v sha256sum >/dev/null; then SHACHECK="sha256sum -c"; else SHACHECK="shasum -a 256 -c"; fi
DEST="${SCRIPTVOX_BIN_DIR:-$HOME/.local/bin}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ "${1:-}" = "--from" ]; then
  [ -f "${2:-}" ] || { echo "scriptvox: no file '${2:-}'. Use: sh install.sh --from <tarball>" >&2; exit 1; }
  cp "$2" "$TMP/$NAME.tar.gz"
  [ -f "$2.sha256" ] && cp "$2.sha256" "$TMP/$NAME.tar.gz.sha256"
else
  URL="${BASE%/}/$NAME.tar.gz"
  command -v curl >/dev/null || { echo "scriptvox: curl is not installed. Install curl, or download $URL yourself and run: sh install.sh --from <tarball>" >&2; exit 1; }
  echo "scriptvox: downloading $URL"
  curl -fsSL "$URL" -o "$TMP/$NAME.tar.gz" || { echo "scriptvox: download failed: $URL. Check the address (SCRIPTVOX_BASE_URL, SCRIPTVOX_VERSION) and the network, nothing installed" >&2; exit 1; }
  # A release always has the checksum file: without it nothing is installed.
  curl -fsSL "$URL.sha256" -o "$TMP/$NAME.tar.gz.sha256" || { echo "scriptvox: no checksum file at $URL.sha256, nothing installed" >&2; exit 1; }
fi
if [ -f "$TMP/$NAME.tar.gz.sha256" ]; then
  (cd "$TMP" && $SHACHECK "$NAME.tar.gz.sha256" >/dev/null) || { echo "scriptvox: checksum mismatch, nothing installed" >&2; exit 1; }
  echo "scriptvox: sha256 ok"
fi
tar -xzf "$TMP/$NAME.tar.gz" -C "$TMP"
mkdir -p "$DEST"
install -m 755 "$TMP/$NAME/scriptvox" "$DEST/scriptvox"
echo "scriptvox installed: $DEST/scriptvox"
case ":$PATH:" in
  *":$DEST:"*) ;;
  *) echo "Add it to PATH yourself (this installer never edits profiles): export PATH=\"$DEST:\$PATH\"" ;;
esac
if [ "$(uname -s)" = Darwin ]; then
  echo "macOS: ScriptVox needs an FFmpeg that has libass, and uv. With Homebrew: brew install ffmpeg-full uv"
  echo "Then: export SCRIPTVOX_FFMPEG=\"\$(brew --prefix ffmpeg-full)/bin/ffmpeg\""
fi
"$DEST/scriptvox" --version --json
