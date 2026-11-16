#!/bin/bash
# WSL checklist in a clean environment, from the release tarball.
R="$(cd "$(dirname "$0")/.." && pwd)"
C=/tmp/claude-1000/clean
rm -rf $C && mkdir -p $C/proj $C/home
cp -r $R/fixtures/media $C/proj/clips
printf 'هذه تجربة نظيفة لسكريبت فوكس. الترجمة العربية تقرأ من اليمين إلى اليسار.\n' > $C/proj/ar.txt
printf 'A clean install test. My own clips, my voice, captions that fit.\n' > $C/proj/s.txt
cd $C/proj
export HOME=$C/home SCRIPTVOX_DATA=$C/data SCRIPTVOX_BIN_DIR=$C/bin PATH=$C/bin:/usr/bin:/bin
unset SCRIPTVOX_FFMPEG SCRIPTVOX_UV SCRIPTVOX_WORKER_SRC
j() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
T0=$(date +%s)
sh $R/install.sh --from $R/dist/scriptvox-0.1.0-x86_64-unknown-linux-musl.tar.gz | head -1
scriptvox doctor --offline --json | j '"doctor:", d["status"], d["error"]["fix"] if d["error"] else ""'
scriptvox doctor --fix --offline --json | j '"doctor --fix:", d["status"], d["mode_ready"]'
scriptvox demo --json | j '"demo:", d["ok"], d["metrics"]["frames"]'
echo "t=$(( $(date +%s)-T0 ))s after demo"
scriptvox make --script ar.txt --json 2>/dev/null | j '"arabic make:", d["ok"], d["metrics"].get("words"), d["error"]'
ID=$(scriptvox make --script s.txt --media clips --music calm --detach --json | j 'd["run_id"]')
scriptvox wait $ID --json | j '"detach+wait own media:", d["ok"], d["metrics"].get("shots"), d["artifacts"].get("video")'
echo "pexels-clean-test-key-123" | scriptvox keys set pexels --stdin --json | j '"keys set:", d["ok"]'
stat -c '%a %n' $HOME/.config/scriptvox/keys.toml
ls -a $HOME
echo "TOTAL t=$(( $(date +%s)-T0 ))s"
