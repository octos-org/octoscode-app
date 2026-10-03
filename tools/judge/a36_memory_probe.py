#!/usr/bin/env python3
"""A36b: does a6ea8505 accept `profile_id` on memory/* (the proposal's
parameter, ignored by a server without it)? A PRIVATE serve (a copy of
live-gate data, a fresh random mode-600 token, never :50190/9401/9402),
no model turn. The token is never printed or saved; every written file is
scanned for it (whole, first 8, last 8); the serve's dir is deleted.

    python3 tools/judge/a36_memory_probe.py <serve-port> <out.jsonl>
"""
import asyncio
import json
import os
import pathlib
import secrets
import shutil
import subprocess
import sys
import time
import urllib.request

import websockets

ROOT = pathlib.Path(__file__).resolve().parents[2]
PORT = int(sys.argv[1])
OUT = pathlib.Path(sys.argv[2])
NOINDEX = pathlib.Path.home() / "home" / "oa.noindex"
OCTOS = NOINDEX / "p0-build/tmp/octos-target/release/octos"
SRC = NOINDEX / "live-gate/data"
FEATURES = ["approval.typed.v1", "pane.snapshots.v1", "session.workspace_cwd.v1", "auxiliary.rest_to_ws.v1",
            "state.session_hydrate.v1"]
T0 = time.time()
LINES = []


def rec(direction, method, body):
    LINES.append({"at_ms": int((time.time() - T0) * 1000), "body": body, "dir": direction, "method": method,
                  "wall_ms": int(time.time() * 1000)})


async def run(token, session):
    q = "&".join(f"ui_feature={f}" for f in FEATURES)
    url = f"ws://127.0.0.1:{PORT}/api/ui-protocol/ws?{q}"
    headers = {"Authorization": f"Bearer {token}", "x-profile-id": "dsflash"}
    async with websockets.connect(url, additional_headers=headers, max_size=8 * 1024 * 1024) as ws:
        n = [0]

        async def call(method, params, timeout=60):
            n[0] += 1
            rid = f"p{n[0]}"
            rec("out", method, params)
            await ws.send(json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}))
            end = time.time() + timeout
            while time.time() < end:
                raw = await asyncio.wait_for(ws.recv(), timeout=max(0.1, end - time.time()))
                msg = json.loads(raw)
                if msg.get("id") == rid:
                    body = {"error": msg["error"]} if "error" in msg else msg.get("result")
                    rec("in", method, body)
                    print(f"{method}: {json.dumps(body)[:300]}")
                    return body
            print(f"{method}: TIMEOUT")

        await call("session/open", {"profile_id": "dsflash", "session_id": session})
        LINES.pop()  # the open's large reply is not needed here
        p = {"profile_id": "dsflash"}
        await call("memory/overview", dict(p))
        await call("memory/entity", dict(p, name="a36b-missing-page"))
        await call("memory/search", dict(p, query="steer queue", limit=20))
        await call("memory/load", dict(p, id="doc:octoscode:0123456789abcdef"))
        now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        await call("memory/ingest", dict(p, records=[{
            "id": "doc:octoscode:0123456789abcdef", "kind": "document", "source": "octoscode", "timestamp": now,
            "title": "Backoff for redelivery",
            "abstract": "Start at 250 ms, double up to 8 s, reset after a successful send."}]))


def main():
    live = ROOT / "tmp" / f"live-a36b-{PORT}"
    if live.exists():
        shutil.rmtree(live)
    (live / "inst").mkdir(parents=True)
    (live / "ws").mkdir()
    shutil.copytree(SRC, live / "data")
    token = secrets.token_hex(24)
    tok = live / "token"
    tok.write_text(token)
    os.chmod(tok, 0o600)
    serve = subprocess.Popen(
        [str(OCTOS), "serve", "--port", str(PORT), "--host", "127.0.0.1", "--solo", "--data-dir", str(live / "data"),
         "--instance-data-dir", str(live / "inst"), "--cwd", str(live / "ws")],
        env=dict(os.environ, OCTOS_AUTH_TOKEN=token), stdout=open(live / "serve.log", "w"), stderr=subprocess.STDOUT)
    try:
        for _ in range(120):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{PORT}/api/version", timeout=2).read()
                break
            except Exception:
                if serve.poll() is not None:
                    raise SystemExit("serve exited")
                time.sleep(0.5)
        asyncio.run(run(token, "dsflash:a36bprobe"))
    finally:
        serve.terminate()
        try:
            serve.wait(10)
        except Exception:
            serve.kill()
    text = "".join(json.dumps(line, ensure_ascii=False, sort_keys=True) + "\n" for line in LINES)
    text = text.replace(str(live), "<TMP>").replace(str(ROOT), "<WORKSPACE>").replace(str(pathlib.Path.home()), "<HOME>")
    for s in (token, token[:8], token[-8:]):
        assert s not in text, "token in the recording"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text)
    shutil.rmtree(live)
    print(f"recorded {len(LINES)} frames -> {OUT}; serve dir and token removed")


if __name__ == "__main__":
    main()
