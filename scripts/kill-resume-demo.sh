#!/bin/bash
# Records a real kill-and-resume session and renders it as docs/launch/kill-and-resume.gif.
# Every line in the GIF is a line this script captured from the real commands.
set -u
R="$(cd "$(dirname "$0")/.." && pwd)"
W="$(mktemp -d)"
# --render-only LOG: skip the session, only render the GIF from a saved log.
RENDER_ONLY="${1:-}"
cd "$W"
cp -r "$R/fixtures/media" clips
cat > s.txt <<'S'
This is a thirty second render, and in the middle of it the machine dies. Not a crash in ScriptVox: the process is killed with signal nine, the way a laptop lid or an out of memory killer would end it. Then the same command runs again. ScriptVox finds the finished chunks, keeps them, and renders only what is missing. The video that comes out is the same video.
S
LOG="$W/session.txt"
if [ "$RENDER_ONLY" = "--render-only" ]; then cp "$2" "$LOG"; else
say() { printf '%s\n' "$*" >> "$LOG"; }
export SCRIPTVOX_CHUNK_S=3
CMD="scriptvox make --script s.txt --media clips --duration 30"
say "\$ $CMD"
( scriptvox make --script s.txt --media clips --duration 30 --json > run1.json 2> run1.err & echo $! > pid; wait ) &
# Kill it once some chunks exist.
for i in $(seq 1 300); do
  n=$(ls .scriptvox/chunks/*/chunk_*.mp4 2>/dev/null | wc -l)
  [ "$n" -ge 4 ] && break
  sleep 0.2
done
grep '^scriptvox:' run1.err | sed 's/^/  /' >> "$LOG"
kill -9 "$(cat pid)" 2>/dev/null
pkill -9 -f "ffmpeg.*chunk_%04d" 2>/dev/null
sleep 1
say "  ... killed with SIGKILL while chunk $(ls .scriptvox/chunks/*/chunk_*.mp4 | wc -l) was being written"
say ""
say "\$ $CMD"
scriptvox make --script s.txt --media clips --duration 30 --json > run2.json 2> run2.err
grep '^scriptvox:' run2.err | grep -E 'kept|rendering|checking' | sed 's/^/  /' >> "$LOG"
python3 - "$LOG" <<'P' 
import json, sys
d = json.load(open("run2.json"))
m = d["metrics"]
with open(sys.argv[1], "a") as f:
    f.write(f'  ok={str(d["ok"]).lower()}  chunks={m.get("chunks")}  resumed_from_chunk={m.get("resumed_from_chunk")}  frames={m.get("frames")}\n')
    f.write(f'  gates: ' + ", ".join(x["gate"] + " " + x["status"] for x in d["findings"]) + "\n")
P
fi
cat "$LOG"
"$R/scripts/log-to-gif.sh" "$LOG" "$R/docs/launch/kill-and-resume.gif"
cp "$LOG" "$R/docs/launch/kill-and-resume.log.txt"
rm -rf "$W"
