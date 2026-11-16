#!/bin/bash
# Renders a session log as a terminal GIF: each line appears in turn in a dark frame (FFmpeg drawtext, palette).
# Usage: scripts/log-to-gif.sh LOG OUT.gif [height]
set -eu
R="$(cd "$(dirname "$0")/.." && pwd)"
LOG="$1"; OUT="$2"; H="${3:-520}"
FF="${SCRIPTVOX_FFMPEG:-$(ls -d "$HOME"/.local/share/scriptvox/tools/ffmpeg/*/bin/ffmpeg | head -1)}"
W="$(mktemp -d)"
N=$(grep -c '' "$LOG")
for i in $(seq 0 "$N"); do
  head -n "$i" "$LOG" > "$W/t_$i.txt"
  "$FF" -v error -y -f lavfi -i "color=c=0x0d1117:s=1200x$H" -frames:v 1 \
    -vf "drawtext=fontfile=$R/assets/fonts/NotoSans-Regular.ttf:expansion=none:textfile=$W/t_$i.txt:fontcolor=0xe6edf3:fontsize=20:line_spacing=6:x=24:y=24" "$W/f_$(printf %03d "$i").png"
done
"$FF" -v error -y -framerate 1.4 -i "$W/f_%03d.png" -vf "fps=10,split[a][b];[a]palettegen[p];[b][p]paletteuse" -loop 0 "$OUT"
rm -rf "$W"
echo "gif: $OUT"
