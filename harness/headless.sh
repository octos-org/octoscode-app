#!/usr/bin/env bash
# headless.sh — shared hidden-window native test harness for the octoscode-app lanes (card #5).
#
# Runs a Makepad/OctoSense app with its window NEVER shown or focused, and drives
# it through the app's own HTTP instrument (`/snap`, `/click`, `/t`, `/g`, `/gq`).
# Applies to any Makepad binary that honours MAKEPAD_HIDE_WINDOWS / MAKEPAD_REMOTE:
# App Hub's `card-host` (a card/script-app bundle) and the OctoSense shells alike.
#
#   harness/headless.sh start <bundle|binary> <port>   # hidden, detached; refuses a taken port
#   harness/headless.sh snap  <port> [query]           # /snap (default: visible widgets)
#   harness/headless.sh click <port> <x> <y>           # /click with wait=1
#   harness/headless.sh type  <port> <text>            # /t   with wait=1
#   harness/headless.sh shot  <port> <out.png>         # /g?raw=1 → PNG
#   harness/headless.sh stop  <port>                   # /gq, wait for exit, report
#   harness/headless.sh status <port>                  # is it up, and whose pid
#   harness/headless.sh ports                          # every port this harness started
#
# Environment (all optional):
#   OCTOSENSE_NATIVE_ROOT  prepared native workspace   (default: <REPO>/../native)
#   OCTO_CARD_HOST         card-host binary            (default: <native>/OctoSense-App-Hub/target/release/card-host)
#   HEADLESS_STATE         run-state dir (pid/log)     (default: ${TMPDIR:-/tmp}/octos-headless)
#   HEADLESS_EVIDENCE      dir to write evidence into  (default: unset → stdout only)
#   HEADLESS_ARGS          extra args for a bare binary (space-separated; word-split)
#   HEADLESS_NO_STAMP=1    do not pass --stamp to card-host (for an immutable, pre-stamped bundle)
#   HEADLESS_TIMEOUT       seconds to wait for the listening line (default: 30)
#   HEADLESS_STOP_TIMEOUT  seconds to wait for exit after /gq   (default: 15)
#
# Rules (see docs/harness/GUIDE.md): only OS-free, app-owned captures (`/g`);
# never show/focus a window; never `pkill`; only `/quit`/`/gq` a port you started.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
NATIVE_ROOT="${OCTOSENSE_NATIVE_ROOT:-$ROOT_DIR/../native}"
CARD_HOST="${OCTO_CARD_HOST:-$NATIVE_ROOT/OctoSense-App-Hub/target/release/card-host}"
STATE="${HEADLESS_STATE:-${TMPDIR:-/tmp}/octos-headless}"
EVIDENCE="${HEADLESS_EVIDENCE:-}"
TIMEOUT="${HEADLESS_TIMEOUT:-30}"
STOP_TIMEOUT="${HEADLESS_STOP_TIMEOUT:-15}"
HOST=127.0.0.1
mkdir -p "$STATE"
[ -n "$EVIDENCE" ] && mkdir -p "$EVIDENCE"

die() { echo "headless: $*" >&2; exit 1; }

# A global counter so repeated snap/click/type in one run get distinct evidence files.
seq_next() { local f="$STATE/.seq"; local n=0; [ -f "$f" ] && n="$(cat "$f")"; n=$((n + 1)); echo "$n" > "$f"; echo "$n"; }

port_pid() { [ -f "$STATE/port-$1.pid" ] && cat "$STATE/port-$1.pid" || true; }

# 0 when 127.0.0.1:port answers something that looks like the Makepad remote bridge.
bridge_up() {
  local port="$1"
  curl -s --max-time 2 "http://$HOST:$port/s" 2>/dev/null | grep -q '"pid"'
}

# The pid holding a LISTEN socket on the port (best effort, for a clear refusal message).
listener_pid() {
  lsof -nP -iTCP:"$1" -sTCP:LISTEN -t 2>/dev/null | head -1 || true
}

wait_listening() {
  local port="$1" log="$2" pid="$3" deadline=$(( $(date +%s) + TIMEOUT ))
  while :; do
    if grep -q '^\[makepad-remote\] listening on ' "$log" 2>/dev/null; then
      grep -m1 '^\[makepad-remote\] listening on ' "$log"; return 0
    fi
    if grep -q '^\[makepad-remote\] bind ' "$log" 2>/dev/null; then
      echo "--- log ---"; cat "$log" >&2; die "remote bind failed on $port (see log above)"
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "--- log tail ---"; tail -20 "$log" >&2
      die "app exited before its remote came up (pid $pid); log $log"
    fi
    [ "$(date +%s)" -ge "$deadline" ] && { tail -20 "$log" >&2; die "timed out after ${TIMEOUT}s waiting for the listening line (log $log)"; }
    sleep 0.3
  done
}

cmd_start() {
  local target="${1:-}" port="${2:-}"
  [ -n "$target" ] && [ -n "$port" ] || die "usage: headless.sh start <bundle|binary> <port>"
  case "$port" in ''|*[!0-9]*) die "port must be a number, got '$port'";; esac

  if bridge_up "$port"; then
    die "127.0.0.1:$port is already taken by pid $(listener_pid "$port" || echo ?) — pick another port (see the port plan in docs/harness/GUIDE.md)"
  fi

  local log="$STATE/port-$port.log" cmd=()
  : > "$log"
  if [ -d "$target" ] && [ -f "$target/manifest.json" ]; then
    [ -x "$CARD_HOST" ] || die "card-host not found at $CARD_HOST (set OCTO_CARD_HOST or build it in $NATIVE_ROOT/OctoSense-App-Hub)"
    mkdir -p "$STATE/app-data-$port"
    cmd=("$CARD_HOST" --bundle "$target" --app-data "$STATE/app-data-$port" --allow-unsigned --remote "$port")
    [ "${HEADLESS_NO_STAMP:-0}" = "1" ] || cmd+=(--stamp)
  elif [ -x "$target" ]; then
    cmd=("$target" --remote "$port")
    # shellcheck disable=SC2206
    [ -n "${HEADLESS_ARGS:-}" ] && cmd+=($HEADLESS_ARGS)
  else
    die "'$target' is neither a bundle directory (needs manifest.json) nor an executable"
  fi

  echo "[headless] launch: MAKEPAD_HIDE_WINDOWS=1 ${cmd[*]}" | tee "$log"
  MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_REMOTE="$port" "${cmd[@]}" >>"$log" 2>&1 &
  local pid=$!
  echo "$pid" > "$STATE/port-$port.pid"
  printf '%s\t%s\t%s\n' "$port" "$pid" "$target" >> "$STATE/runs.tsv"

  local line; line="$(wait_listening "$port" "$log" "$pid")"
  echo "$line"
  echo "[headless] up: pid $pid  port $port  log $log"
  echo "[headless] drive it:  curl -s http://$HOST:$port/snap?q=Button   |   harness/headless.sh shot $port out.png"
  [ -n "$EVIDENCE" ] && { cp "$STATE/port-$port.log" "$EVIDENCE/start-$port.log" 2>/dev/null || true; }
  return 0
}

cmd_snap() {
  local port="${1:-}" q="${2:-}"
  [ -n "$port" ] || die "usage: headless.sh snap <port> [query]"
  local url="http://$HOST:$port/snap"; [ -n "$q" ] && url="$url?q=$(printf %s "$q" | python3 -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.stdin.read()))')"
  local out; out="$(curl -s --max-time 10 "$url")"
  echo "$out"
  if [ -n "$EVIDENCE" ]; then local n; n="$(seq_next)"; printf '%s\n' "$out" > "$EVIDENCE/snap-$port-$n.json"; fi
}

cmd_click() {
  local port="${1:-}" x="${2:-}" y="${3:-}"
  [ -n "$port" ] && [ -n "$x" ] && [ -n "$y" ] || die "usage: headless.sh click <port> <x> <y>"
  local out; out="$(curl -s --max-time 10 "http://$HOST:$port/click?x=$x&y=$y&wait=1")"
  echo "$out"
  if [ -n "$EVIDENCE" ]; then printf 'click x=%s y=%s -> %s\n' "$x" "$y" "$out" >> "$EVIDENCE/actions-$port.log"; fi
}

cmd_type() {
  local port="${1:-}" text="${2:-}"
  [ -n "$port" ] && [ -n "$text" ] || die "usage: headless.sh type <port> <text>"
  local out; out="$(curl -s --max-time 10 --get "http://$HOST:$port/t" --data-urlencode "t=$text" --data 'wait=1')"
  echo "$out"
  if [ -n "$EVIDENCE" ]; then printf 'type %q -> %s\n' "$text" "$out" >> "$EVIDENCE/actions-$port.log"; fi
}

cmd_shot() {
  local port="${1:-}" png="${2:-}"
  [ -n "$port" ] && [ -n "$png" ] || die "usage: headless.sh shot <port> <out.png>"
  mkdir -p "$(dirname "$png")"
  curl -s --max-time 20 -o "$png" "http://$HOST:$port/g?raw=1"
  # PNG magic, whitespace-independent (BSD od pads bytes with two spaces, GNU with one)
  [ "$(head -c 4 "$png" | od -An -tx1 | tr -d ' \n')" = "89504e47" ] || die "'$png' is not a PNG (is the app up on $port?)"
  local bytes; bytes="$(wc -c < "$png" | tr -d ' ')"
  echo "[headless] wrote $png ($bytes bytes)"
  [ -n "$EVIDENCE" ] && cp "$png" "$EVIDENCE/$(basename "$png")" 2>/dev/null || true
}

cmd_stop() {
  local port="${1:-}"
  [ -n "$port" ] || die "usage: headless.sh stop <port>"
  local gq; gq="$(curl -s --max-time 10 "http://$HOST:$port/gq")"
  echo "gq: $gq"
  local pid; pid="$(port_pid "$port")"
  [ -n "$pid" ] || pid="$(listener_pid "$port")"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    local deadline=$(( $(date +%s) + STOP_TIMEOUT ))
    while kill -0 "$pid" 2>/dev/null; do
      [ "$(date +%s)" -ge "$deadline" ] && break
      sleep 0.2
    done
    if kill -0 "$pid" 2>/dev/null; then
      die "pid $pid did not exit within ${STOP_TIMEOUT}s after /gq (do NOT pkill; check $STATE/port-$port.log)"
    fi
    echo "[headless] pid $pid exited after /gq"
  else
    echo "[headless] no owned pid for port $port (already gone?)"
  fi
  if bridge_up "$port"; then die "port $port still answers after /gq"; fi
  rm -f "$STATE/port-$port.pid"
  echo "[headless] port $port free"
  [ -n "$EVIDENCE" ] && printf 'gq: %s\npid %s exited\n' "$gq" "${pid:-?}" > "$EVIDENCE/stop-$port.txt"
}

cmd_status() {
  local port="${1:-}"; [ -n "$port" ] || die "usage: headless.sh status <port>"
  if bridge_up "$port"; then echo "port $port UP — $(curl -s --max-time 2 "http://$HOST:$port/s")"; else echo "port $port down"; fi
}

cmd_ports() {
  [ -f "$STATE/runs.tsv" ] || { echo "[headless] no runs recorded in $STATE"; return 0; }
  echo -e "port\tpid\ttarget\tstatus"
  while IFS=$'\t' read -r p pid tgt; do
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then st=up; else st=gone; fi
    echo -e "$p\t$pid\t$tgt\t$st"
  done < "$STATE/runs.tsv"
}

cmd="${1:-}"
shift || true
case "$cmd" in
  start)  cmd_start "$@";;
  snap)   cmd_snap "$@";;
  click)  cmd_click "$@";;
  type)   cmd_type "$@";;
  shot)   cmd_shot "$@";;
  stop)   cmd_stop "$@";;
  status) cmd_status "$@";;
  ports)  cmd_ports;;
  ""|-h|--help|help)
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//' | sed '/^$/q';;
  *) die "unknown command '$cmd' (start|snap|click|type|shot|stop|status|ports)";;
esac
