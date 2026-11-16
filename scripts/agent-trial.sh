#!/usr/bin/env bash
# agent-trial.sh <agent> <prompt-file> [NAME=value ...]: one headless trial of a coding agent with ScriptVox
# (milestone Agents, A7). <agent> is `claude` or `opencode`. Not in CI: a trial costs tokens. Run it before a release.
#
# The agent starts in a new empty folder with a clean environment: PATH, HOME, TERM, LANG, an empty SCRIPTVOX_CONFIG
# (so no ScriptVox key is set), SCRIPTVOX_FFMPEG when it is set, and the NAME=value pairs you give.
# `scriptvox` and the agent must be on PATH. The script prints counts only, never the text of a command.
#
# HOME is your real home, because the agent needs its own login. So a trial that installs skills writes them to
# your home, and the agent can read your files. To hide folders, set AGENT_TRIAL_HIDE (needs `bwrap`).
#
#   AGENT_TRIAL_MODEL    opencode only, required: the model as provider/model
#   AGENT_TRIAL_DIR      where the trial is kept (default: a new temp folder). It must be empty or absent
#   AGENT_TRIAL_TIMEOUT  seconds before the agent is stopped (default 3000)
#   AGENT_TRIAL_PASS     names of variables to pass from your environment, with spaces between them. Use it for
#                        the key of the model provider: the value then is not on a command line
#   AGENT_TRIAL_HIDE     folders the agent must not see, with `:` between them. Each one shows as empty
#   AGENT_TRIAL_INSTALL  set to 1 for an install trial (task T1): `scriptvox` need not be on PATH at the start.
#                        Give SCRIPTVOX_BIN_DIR=<a folder on PATH> and SCRIPTVOX_DATA as NAME=value pairs, and a
#                        PATH with no cargo, so that the install does not write to your home
#
# opencode: `OPENCODE_DISABLE_EXTERNAL_SKILLS=1` as a NAME=value pair keeps the skills of your home out, and
# `OPENCODE_CONFIG` in AGENT_TRIAL_PASS names your provider config.
#
#   scripts/agent-trial.sh claude prompt.txt
#   AGENT_TRIAL_MODEL=myprovider/mymodel AGENT_TRIAL_PASS="MYPROVIDER_API_KEY OPENCODE_CONFIG" \
#     scripts/agent-trial.sh opencode prompt.txt OPENCODE_DISABLE_EXTERNAL_SKILLS=1
set -euo pipefail

[ $# -ge 2 ] || { sed -n '2,4p' "$0" >&2; exit 2; }
AGENT=$1
PROMPT_FILE=$2
shift 2
[ -f "$PROMPT_FILE" ] || { echo "agent-trial: no prompt file $PROMPT_FILE" >&2; exit 2; }
case "$AGENT" in
  claude) ;;
  opencode) [ -n "${AGENT_TRIAL_MODEL:-}" ] || { echo "agent-trial: set AGENT_TRIAL_MODEL (provider/model)" >&2; exit 2; } ;;
  *) echo "agent-trial: agent must be claude or opencode" >&2; exit 2 ;;
esac
NEED="$AGENT python3 timeout"
[ -n "${AGENT_TRIAL_INSTALL:-}" ] || NEED="$NEED scriptvox"
for tool in $NEED; do
  command -v "$tool" >/dev/null || { echo "agent-trial: $tool is not on PATH" >&2; exit 2; }
done

PROMPT=$(cat "$PROMPT_FILE")
DIR=${AGENT_TRIAL_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/scriptvox-trial.XXXXXX")}
mkdir -p "$DIR"
DIR=$(cd "$DIR" && pwd)
[ -z "$(ls -A "$DIR")" ] || { echo "agent-trial: $DIR is not empty" >&2; exit 2; }
mkdir "$DIR/work" "$DIR/config"

ENVV=(PATH="$PATH" HOME="$HOME" TERM=dumb LANG=C.UTF-8 SCRIPTVOX_CONFIG="$DIR/config")
[ -z "${SCRIPTVOX_FFMPEG:-}" ] || ENVV+=(SCRIPTVOX_FFMPEG="$SCRIPTVOX_FFMPEG")
for name in ${AGENT_TRIAL_PASS:-}; do
  [ -z "${!name:-}" ] || ENVV+=("$name=${!name}")
done
ENVV+=("$@")

WRAP=()
if [ -n "${AGENT_TRIAL_HIDE:-}" ]; then
  command -v bwrap >/dev/null || { echo "agent-trial: AGENT_TRIAL_HIDE needs bwrap" >&2; exit 2; }
  WRAP=(bwrap --dev-bind / /)
  IFS=: read -ra HIDE <<< "$AGENT_TRIAL_HIDE"
  for d in "${HIDE[@]}"; do
    [ ! -d "$d" ] || WRAP+=(--tmpfs "$d")
  done
  WRAP+=(--chdir "$DIR/work")
fi

if [ "$AGENT" = claude ]; then
  CMD=(claude -p "$PROMPT" --setting-sources project --disable-slash-commands --strict-mcp-config
    --output-format stream-json --verbose --allowedTools Bash Read Write Edit --max-turns 60)
else
  CMD=(opencode run -m "$AGENT_TRIAL_MODEL" --format json "$PROMPT")
fi

echo "trial folder: $DIR"
START=$(date +%s)
CODE=0
(cd "$DIR/work" && timeout "${AGENT_TRIAL_TIMEOUT:-3000}" ${WRAP[@]+"${WRAP[@]}"} env -i "${ENVV[@]}" "${CMD[@]}" \
  > "$DIR/transcript.jsonl" 2> "$DIR/stderr.txt" < /dev/null) || CODE=$?
echo "agent: $AGENT | exit: $CODE | minutes: $(( ($(date +%s) - START + 30) / 60 ))"
echo "transcript: $DIR/transcript.jsonl"

# The score: counts only. A command can hold a secret, so no command text is printed.
python3 -I - "$AGENT" "$DIR/transcript.jsonl" <<'PY'
import json, re, sys

agent, path = sys.argv[1], sys.argv[2]
raw = open(path, encoding="utf-8", errors="replace").read()
calls = failed = approve = 0
for line in raw.splitlines():
    try:
        e = json.loads(line)
    except ValueError:
        continue
    if not isinstance(e, dict):
        continue
    if agent == "claude":
        message = e.get("message")
        content = message.get("content") if isinstance(message, dict) else None
        for c in content if isinstance(content, list) else []:
            if not isinstance(c, dict):
                continue
            if e.get("type") == "assistant" and c.get("type") == "tool_use":
                calls += 1
                approve += "--approve" in str((c.get("input") or {}).get("command") or "")
            elif e.get("type") == "user" and c.get("type") == "tool_result" and c.get("is_error"):
                failed += 1
    else:
        part = e.get("part") or {}
        if part.get("type") != "tool":
            continue
        state = part.get("state") or {}
        calls += 1
        approve += "--approve" in str((state.get("input") or {}).get("command") or "")
        # opencode marks a shell command with a bad exit as `completed`: the exit code is in the metadata.
        # A shell command with no exit code was stopped by the time limit of the agent.
        bad_exit = part.get("tool") == "bash" and (state.get("metadata") or {}).get("exit") != 0
        failed += state.get("status") == "error" or bad_exit
# Shapes of well-known key formats only. A hit means: read the transcript before you keep it.
keys = len(re.findall(r"\b(?:hf_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,})", raw))
print(f"tool calls: {calls} | failed: {failed} | commands with --approve: {approve} | key-shaped texts: {keys}")
PY

# The newest video of the trial, through the gates of ScriptVox.
VIDEO=$(find "$DIR/work" -name '*.mp4' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-)
if [ -n "$VIDEO" ]; then
  if (cd "$DIR/work" && env -i "${ENVV[@]}" scriptvox check "$VIDEO" --json > "$DIR/check.json" 2>/dev/null); then
    echo "check: pass (${VIDEO#"$DIR/"})"
  else
    echo "check: FAIL, exit $? (${VIDEO#"$DIR/"}); the answer is in $DIR/check.json"
  fi
else
  echo "check: no video in $DIR/work"
fi
