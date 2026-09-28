#!/bin/bash
# live-gate.sh : Phase-1 live gate. Real octos serve (a6ea8505, dsflash) on :50190, native app hidden on :8490.
# open workspace -> prompt -> streamed answer -> interrupt a second turn mid-stream. Evidence under $G/evidence.
set -u
G=/Users/yuechen/home/oa.noindex/live-gate; E=$G/evidence; P=8490; U=http://127.0.0.1:$P
BIN=/Users/yuechen/home/oa.noindex/p0-build/tmp/octosense-target/debug/octosense
SHELL_CWD=/Users/yuechen/home/oa.noindex/p0-build/tmp/octosense/desktop
mkdir -p $E
lsof -iTCP:50190 -sTCP:LISTEN -P >/dev/null || { echo "serve not listening on 50190"; exit 1; }
cd $SHELL_CWD
OCTOS_BASE_URL=http://127.0.0.1:50190 OCTOS_BEARER="$(cat $G/.token)" OCTOS_PROFILE_ID=dsflash OCTOS_WORKSPACE_CWD=$G/ws OCTOSCODE_TRACE_FILE=$E/trace.jsonl \
  RUST_LOG=info MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_WM_TEST_APP=octoscode \
  $BIN --module octoscode --remote $P > $E/app.log 2>&1 &
echo $! > $G/app.pid
for i in $(seq 1 90); do grep -q 'makepad-remote\] listening' $E/app.log && break; sleep 1; done
grep 'makepad-remote\] listening' $E/app.log | head -1
sleep 4
snap(){ curl -s "$U/snap?all=1" > $E/$1.json; python3 - "$E/$1.json" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
for w in d.get('s',[]):
    t=(w.get('t') or '').replace('\n',' | ')
    if t and w.get('ty') in ('Label','Button','TextInput'): print(f"  {w.get('ty'):9} {w.get('i','')[:28]:28} {t[:150]}")
PY
}
rect(){ python3 - "$E/$1.json" "$2" <<'PY'
import json,sys
d=json.load(open(sys.argv[1])); want=sys.argv[2]
for w in d.get('s',[]):
    if str(w.get('i',''))==want or (w.get('ty')==want):
        x,y,ww,hh=w['r']; print(int(x+ww/2),int(y+hh/2)); break
PY
}
echo "== 1. workspace open"; snap s1-open
read ix iy < <(rect s1-open draft); read sx sy < <(rect s1-open send)
echo "input at $ix,$iy send at $sx,$sy"
echo "== 2. prompt"
curl -s "$U/click?x=$ix&y=$iy&wait=1" >/dev/null
curl -s --get "$U/t" --data-urlencode 't=In one short paragraph: what does main.rs in this workspace print, and why?' --data wait=1 >/dev/null
curl -s "$U/click?x=$sx&y=$sy&wait=1" >/dev/null
T0=$(date +%s)
sleep 3; snap s2-streaming; curl -s "$U/g?raw=1" -o $E/g2-streaming.png
for i in $(seq 1 90); do sleep 2; curl -s "$U/snap?all=1" | grep -q 'Worked for' && break; done
echo "turn 1 finished after $(( $(date +%s) - T0 ))s"
snap s3-completed; curl -s "$U/g?raw=1" -o $E/g3-completed.png
echo "== 3. second turn, interrupt mid-stream"
read ix iy < <(rect s3-completed draft); read sx sy < <(rect s3-completed send); read tx ty < <(rect s3-completed stop)
curl -s "$U/click?x=$ix&y=$iy&wait=1" >/dev/null
curl -s --get "$U/t" --data-urlencode 't=Write a detailed 600-word explanation of integer overflow in Rust, with examples.' --data wait=1 >/dev/null
curl -s "$U/click?x=$sx&y=$sy&wait=1" >/dev/null
BASE=$(curl -s "$U/snap?all=1" | python3 -c "import sys,json;d=json.load(sys.stdin);print(sum(len(w.get('t') or '') for w in d['s'] if 'assistant' in (w.get('t') or '')))")
for i in $(seq 1 60); do sleep 0.5; curl -s "$U/snap?all=1" | python3 -c "import sys,json;d=json.load(sys.stdin);print(sum(len(w.get('t') or '') for w in d['s'] if 'assistant' in (w.get('t') or '')))" > $E/.len; [ $(( $(cat $E/.len) - BASE )) -gt 300 ] && break; done
echo "turn 2 streamed $(( $(cat $E/.len) - BASE )) new chars before stop"
curl -s "$U/click?x=$tx&y=$ty&wait=1" >/dev/null; echo "stop clicked at $(cat $E/.len) chars of assistant text"
sleep 4; snap s4-interrupted; curl -s "$U/g?raw=1" -o $E/g4-interrupted.png
curl -s "$U/log?n=80" > $E/app-log.json
echo "== 4. cleanup"; curl -s "$U/gq" > $E/gq.json; cat $E/gq.json; echo
sleep 3; kill -0 $(cat $G/app.pid) 2>/dev/null && echo "app still running" || echo "app exited"
