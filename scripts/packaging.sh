#!/bin/sh
# packaging.sh <dist-dir> [out-dir]: package-manager files for one release, made from the release archives.
# It reads the version from Cargo.toml and each sha256 from <dist-dir>/*.sha256 (`just package`, `just
# package-windows`, the macOS workflow). A channel whose archive is not there is skipped, with a line that says so.
# It publishes nothing. Default out-dir: <dist-dir>/packaging.
#
#   aur/PKGBUILD, aur/.SRCINFO   Arch Linux: the files of the AUR repository `scriptvox-bin`
#   deb/scriptvox_<v>_amd64.deb  Debian and Ubuntu: a release asset, `sudo apt install ./scriptvox_<v>_amd64.deb`
#   homebrew/scriptvox.rb        Homebrew: `Formula/scriptvox.rb` of a tap repository. macOS (Apple silicon and Intel) when
#                                <dist-dir> has the archives of the macOS workflow, and Linux x86_64. On macOS the formula
#                                brings FFmpeg (`ffmpeg-full`, it has libass) and uv from Homebrew: one command installs all
#   scoop/scriptvox.json         Windows: `bucket/scriptvox.json` of a Scoop bucket repository
#   winget/                      Windows: the three manifest files of `ScriptVox.ScriptVox`, for a pull request to
#                                microsoft/winget-pkgs under manifests/s/ScriptVox/ScriptVox/<version>/
#   chocolatey/                  Windows: the source of a Chocolatey package. `choco pack` makes the .nupkg from it, and
#                                `choco push` sends it to community.chocolatey.org (an account and its API key)
#
# Variables:
#   SCRIPTVOX_MAINTAINER  required, "Name <address>": the public contact in the PKGBUILD and the .deb. No default.
#   SCRIPTVOX_BASE_URL    the folder address that holds the archives. Default: the GitHub release of this version.
set -eu
[ $# -ge 1 ] || { echo "usage: packaging.sh <dist-dir> [out-dir]" >&2; exit 2; }
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DIST=$(cd "$1" && pwd)
OUT=${2:-$DIST/packaging}
: "${SCRIPTVOX_MAINTAINER:?set SCRIPTVOX_MAINTAINER to the public contact, as \"Name <address>\"}"
V=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
REPO=$(grep -m1 '^repository' "$ROOT/Cargo.toml" | cut -d'"' -f2)
BASE=${SCRIPTVOX_BASE_URL:-$REPO/releases/download/v$V}
BASE=${BASE%/}
DESC="Command-line tool that makes a narrated, captioned video from a script"
LIN="scriptvox-$V-x86_64-unknown-linux-musl"
WIN="scriptvox-$V-x86_64-pc-windows-gnullvm"
MARM="scriptvox-$V-aarch64-apple-darwin"
MINT="scriptvox-$V-x86_64-apple-darwin"
# The sha256 of an archive, checked against the file. Empty when the archive is not in <dist-dir>.
sha() {
  [ -f "$DIST/$1" ] && [ -f "$DIST/$1.sha256" ] || return 0
  want=$(cut -d' ' -f1 "$DIST/$1.sha256")
  if command -v sha256sum >/dev/null; then got=$(sha256sum "$DIST/$1" | cut -d' ' -f1); else got=$(shasum -a 256 "$DIST/$1" | cut -d' ' -f1); fi
  [ "$want" = "$got" ] || { echo "packaging: $1 does not match its .sha256 file" >&2; exit 1; }
  echo "$want"
}
LSHA=$(sha "$LIN.tar.gz")
WSHA=$(sha "$WIN.zip")
ASHA=$(sha "$MARM.tar.gz")
ISHA=$(sha "$MINT.tar.gz")
[ -n "$LSHA$WSHA$ASHA$ISHA" ] || { echo "packaging: no release archive with a .sha256 file in $DIST" >&2; exit 1; }
rm -rf "$OUT" && mkdir -p "$OUT"
skip() { echo "skipped: $1 (no $2 archive in $DIST)"; }

if [ -n "$LSHA" ]; then
# Arch Linux. The binary is static (musl): no depends. `options=(!strip !debug)` keeps the file equal to the release file.
mkdir -p "$OUT/aur"
cat > "$OUT/aur/PKGBUILD" <<EOF
# Maintainer: $SCRIPTVOX_MAINTAINER
pkgname=scriptvox-bin
pkgver=$V
pkgrel=1
pkgdesc="$DESC"
arch=('x86_64')
url="$REPO"
license=('Apache-2.0' 'OFL-1.1')
provides=('scriptvox')
conflicts=('scriptvox')
options=('!strip' '!debug')
source=("$BASE/scriptvox-\$pkgver-x86_64-unknown-linux-musl.tar.gz")
sha256sums=('$LSHA')

package() {
  cd "scriptvox-\$pkgver-x86_64-unknown-linux-musl"
  install -Dm755 scriptvox "\$pkgdir/usr/bin/scriptvox"
  install -Dm644 LICENSE OFL-1.1.txt THIRD_PARTY.md CREDITS.md -t "\$pkgdir/usr/share/licenses/\$pkgname/"
  install -Dm644 README.md -t "\$pkgdir/usr/share/doc/\$pkgname/"
}
EOF
if command -v makepkg >/dev/null; then (cd "$OUT/aur" && makepkg --printsrcinfo > .SRCINFO); fi
else skip "aur, deb" Linux; fi

# Homebrew. On macOS the binary goes to libexec and `bin/scriptvox` is a small script that names the FFmpeg and the uv
# of Homebrew: ScriptVox has no pinned FFmpeg or uv for macOS. On Linux the binary downloads its own pinned tools.
if [ -n "$LSHA$ASHA$ISHA" ]; then
mkdir -p "$OUT/homebrew"
{
cat <<EOF
class Scriptvox < Formula
  desc "$DESC"
  homepage "$REPO"
  version "$V"
  license "Apache-2.0"

EOF
if [ -n "$ASHA$ISHA" ]; then
  echo "  on_macos do"
  [ -z "$ASHA" ] || printf '    on_arm do\n      url "%s"\n      sha256 "%s"\n    end\n' "$BASE/$MARM.tar.gz" "$ASHA"
  [ -z "$ISHA" ] || printf '    on_intel do\n      url "%s"\n      sha256 "%s"\n    end\n' "$BASE/$MINT.tar.gz" "$ISHA"
  printf '    depends_on "ffmpeg-full"\n    depends_on "uv"\n  end\n\n'
fi
if [ -n "$LSHA" ]; then
  printf '  on_linux do\n    on_intel do\n      url "%s"\n      sha256 "%s"\n    end\n  end\n\n' "$BASE/$LIN.tar.gz" "$LSHA"
fi
cat <<'EOF'
  def install
    doc.install "README.md", "CREDITS.md", "THIRD_PARTY.md", "OFL-1.1.txt"
    if OS.mac?
      libexec.install "scriptvox"
      (bin/"scriptvox").write_env_script libexec/"scriptvox",
        SCRIPTVOX_FFMPEG: Formula["ffmpeg-full"].opt_bin/"ffmpeg",
        SCRIPTVOX_UV:     Formula["uv"].opt_bin/"uv"
    else
      bin.install "scriptvox"
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/scriptvox --version")
  end
end
EOF
} > "$OUT/homebrew/scriptvox.rb"
else skip homebrew "Linux or macOS"; fi

if [ -n "$WSHA" ]; then
# Scoop (Windows).
mkdir -p "$OUT/scoop"
cat > "$OUT/scoop/scriptvox.json" <<EOF
{
  "version": "$V",
  "description": "$DESC",
  "homepage": "$REPO",
  "license": "Apache-2.0",
  "architecture": {
    "64bit": {
      "url": "$BASE/$WIN.zip",
      "hash": "$WSHA",
      "extract_dir": "$WIN"
    }
  },
  "bin": "scriptvox.exe"
}
EOF
else skip scoop Windows; fi

# Chocolatey (Windows). The package holds no binary: its install script downloads the release zip and checks the sha256.
# Chocolatey makes the `scriptvox` command itself, from the .exe that it finds in the unpacked folder.
if [ -n "$WSHA" ]; then
mkdir -p "$OUT/chocolatey/tools"
cat > "$OUT/chocolatey/scriptvox.nuspec" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd">
  <metadata>
    <id>scriptvox</id>
    <version>$V</version>
    <title>ScriptVox</title>
    <authors>The ScriptVox contributors</authors>
    <projectUrl>$REPO</projectUrl>
    <licenseUrl>$REPO/blob/main/LICENSE</licenseUrl>
    <requireLicenseAcceptance>false</requireLicenseAcceptance>
    <projectSourceUrl>$REPO</projectSourceUrl>
    <packageSourceUrl>$REPO</packageSourceUrl>
    <bugTrackerUrl>$REPO/issues</bugTrackerUrl>
    <tags>scriptvox video captions narration cli</tags>
    <summary>$DESC</summary>
    <description>ScriptVox is a command-line tool that makes a narrated and captioned MP4 from a script. It is one binary. At first use it downloads a pinned FFmpeg build into the data folder of the user and checks its sha256. \`scriptvox uninstall --purge\` removes that folder.</description>
  </metadata>
  <files>
    <file src="tools\\**" target="tools" />
  </files>
</package>
EOF
cat > "$OUT/chocolatey/tools/chocolateyInstall.ps1" <<EOF
\$ErrorActionPreference = 'Stop'
\$toolsDir = Split-Path -Parent \$MyInvocation.MyCommand.Definition
Install-ChocolateyZipPackage -PackageName 'scriptvox' -Url64bit '$BASE/$WIN.zip' -Checksum64 '$WSHA' -ChecksumType64 'sha256' -UnzipLocation \$toolsDir
EOF
else skip chocolatey Windows; fi

# winget (Windows). The zip holds one portable .exe; winget makes the `scriptvox` command from it.
if [ -n "$WSHA" ]; then
mkdir -p "$OUT/winget"
ID=ScriptVox.ScriptVox
MV=1.6.0
cat > "$OUT/winget/$ID.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.$MV.schema.json
PackageIdentifier: $ID
PackageVersion: $V
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $MV
EOF
cat > "$OUT/winget/$ID.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.$MV.schema.json
PackageIdentifier: $ID
PackageVersion: $V
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
- RelativeFilePath: $WIN\\scriptvox.exe
  PortableCommandAlias: scriptvox
Installers:
- Architecture: x64
  InstallerUrl: $BASE/$WIN.zip
  InstallerSha256: $(echo "$WSHA" | tr 'a-f' 'A-F')
ManifestType: installer
ManifestVersion: $MV
EOF
cat > "$OUT/winget/$ID.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.$MV.schema.json
PackageIdentifier: $ID
PackageVersion: $V
PackageLocale: en-US
Publisher: ScriptVox
PublisherUrl: $REPO
PublisherSupportUrl: $REPO/issues
PackageName: ScriptVox
PackageUrl: $REPO
License: Apache-2.0
LicenseUrl: $REPO/blob/main/LICENSE
ShortDescription: $DESC
Moniker: scriptvox
Tags:
- video
- captions
- narration
- cli
ManifestType: defaultLocale
ManifestVersion: $MV
EOF
else skip winget Windows; fi

# Debian and Ubuntu: an `ar` archive of debian-binary, control.tar.gz and data.tar.gz, in this order, all root-owned.
if [ -n "$LSHA" ]; then
mkdir -p "$OUT/deb"
W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT
tar -xzf "$DIST/$LIN.tar.gz" -C "$W"
mkdir -p "$W/data/usr/bin" "$W/data/usr/share/doc/scriptvox" "$W/control"
install -m 755 "$W/$LIN/scriptvox" "$W/data/usr/bin/scriptvox"
for f in README.md CREDITS.md THIRD_PARTY.md OFL-1.1.txt; do install -m 644 "$W/$LIN/$f" "$W/data/usr/share/doc/scriptvox/$f"; done
{
  printf 'Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nUpstream-Name: scriptvox\nSource: %s\n\n' "$REPO"
  printf 'Files: *\nCopyright: the ScriptVox contributors\nLicense: Apache-2.0\n On Debian systems the full text is in /usr/share/common-licenses/Apache-2.0.\n'
  printf ' The licences of the parts inside the binary are in THIRD_PARTY.md, and the font licence in OFL-1.1.txt, in this folder.\n'
} > "$W/data/usr/share/doc/scriptvox/copyright"
chmod 644 "$W/data/usr/share/doc/scriptvox/copyright"
cat > "$W/control/control" <<EOF
Package: scriptvox
Version: $V
Architecture: amd64
Maintainer: $SCRIPTVOX_MAINTAINER
Installed-Size: $(du -sk "$W/data" | cut -f1)
Section: video
Priority: optional
Homepage: $REPO
Description: $DESC
 One static binary. At first use it downloads a pinned FFmpeg build into the
 data folder of the user. \`scriptvox uninstall --purge\` removes that folder.
EOF
(cd "$W/data" && find . -type f ! -path './DEBIAN/*' -exec md5sum {} + | sed 's# \./# #' > "$W/control/md5sums")
T="--owner=0 --group=0 --numeric-owner --sort=name --mtime=@$(stat -c %Y "$DIST/$LIN.tar.gz")"
# shellcheck disable=SC2086
tar $T -czf "$W/control.tar.gz" -C "$W/control" .
# shellcheck disable=SC2086
tar $T -czf "$W/data.tar.gz" -C "$W/data" .
printf '2.0\n' > "$W/debian-binary"
DEB="$OUT/deb/scriptvox_${V}_amd64.deb"
(cd "$W" && ar rcD "$DEB" debian-binary control.tar.gz data.tar.gz)
(cd "$OUT/deb" && sha256sum "$(basename "$DEB")" > "$(basename "$DEB").sha256")
fi
find "$OUT" -type f | sort
