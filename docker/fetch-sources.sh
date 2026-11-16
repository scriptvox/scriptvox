#!/bin/sh
# Downloads the corresponding sources for the GPL FFmpeg build in the image (GPLv3 §6).
set -eu
mkdir -p sources && cd sources
curl -fsSL -o ffmpeg-n8.1.3.tar.gz https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n8.1.3.tar.gz
curl -fsSL -o btbn-builds-autobuild-2026-10-31-12-59.tar.gz https://github.com/BtbN/FFmpeg-Builds/archive/refs/tags/autobuild-2026-10-31-12-59.tar.gz
echo "Library sources: run the BtbN scripts with --download-only, as listed in their README."
