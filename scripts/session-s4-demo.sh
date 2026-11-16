#!/bin/bash
# Records the plan's §4 agent session from real commands (free Arabic voice, the fixture clips) and renders it as
# docs/launch/session-s4.gif. Every line in the GIF is captured from the real commands.
set -u
R="$(cd "$(dirname "$0")/.." && pwd)"
W="$(mktemp -d)"
cd "$W"
cp -r "$R/fixtures/media" media
# Too long for 6 s on purpose: `plan` stops with exit 3 and the words to cut; the shorter brief fits.
cat > brief.json <<'B'
{"lang": "ar", "aspect": "9:16", "media": "media", "duration": 6,
 "script": "التمر غذاء قديم في الخليج، يحمله الناس في السفر الطويل لأنه لا يفسد ويعطي طاقة كبيرة كل يوم."}
B
cat > brief-fit.json <<'B'
{"lang": "ar", "aspect": "9:16", "media": "media", "duration": 6,
 "script": "التمر غذاء قديم في الخليج، يعطي طاقة كبيرة ولا يفسد بسرعة."}
B
cat > hook.json <<'B'
{"hook": "تمر الخليج"}
B
LOG="$W/session.txt"
say() { printf '%s\n' "$*" >> "$LOG"; }
cat > fmt.py <<'P'
import json, sys
d = json.loads(sys.stdin.read().strip().splitlines()[-1])
m, e = d.get("metrics", {}), d.get("error") or {}
a = d.get("artifacts", {})
if not d["ok"]:
    print(f"  exit {sys.argv[1]}  {e.get('code')}: {e.get('message')}")
    if e.get("fix"):
        print(f"  fix: {e['fix']}")
elif d["command"] == "plan":
    print(f"  ok  cost ${m.get('cost_usd', 0):.2f}  " + ", ".join(s["tool"] for s in d.get("stages", [])))
elif "image" in a:
    print("  ok  " + a["image"].split("/")[-1] + f"  ({len(d.get('frames', []))} frames)")
elif "video" not in a:
    print(f"  {d['status']}  run {d.get('run_id')}")
else:
    print(f"  ok  {m.get('duration_s')} s  gates: " + ", ".join(f["gate"] + " " + f["status"] for f in d.get("findings", [])))
P
env_line() { python3 -I fmt.py "$1"; }
run() { say "\$ scriptvox $*"; out=$(scriptvox "$@" --json 2>/dev/null); code=$?; printf '%s' "$out" | env_line "$code" >> "$LOG"; printf '%s' "$out"; }
# Since 9 Nov 2026 the plan itself says the script is too long, before any voice is made, so the session is:
# plan stops, the agent cuts the script, plan passes, make in the background, wait, a new hook through resume, a sheet.
run plan --brief brief.json > /dev/null
say "  (the agent cuts the script)"
run plan --brief brief-fit.json > /dev/null
id=$(run make --brief brief-fit.json --detach | python3 -I -c 'import json,sys; print(json.loads(sys.stdin.read().splitlines()[-1])["run_id"])')
run wait "$id" > /dev/null
say "  (the owner asks for a headline: only that field changes, the voice is kept)"
child=$(run resume "$id" --patch hook.json | python3 -I -c 'import json,sys; print(json.loads(sys.stdin.read().splitlines()[-1])["run_id"])')
run wait "$child" > /dev/null
run snapshot "$child" --sheet 9 > /dev/null
cat "$LOG"
"$R/scripts/log-to-gif.sh" "$LOG" "$R/docs/launch/session-s4.gif" 500
cp "$LOG" "$R/docs/launch/session-s4.log.txt"
rm -rf "$W"
