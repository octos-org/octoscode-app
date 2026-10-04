#!/usr/bin/env python3
"""A36b — the live proof: Settings > Capabilities (Skills, MCP servers,
Memory) and Memory against a REAL octos serve — a PRIVATE one this script
starts and stops (a copy of live-gate data, a fresh random mode-600 token,
never :50190 / 9401 / 9402). No model turn.

What a6ea8505 does (the scope finding, docs/proposals/memory-profile-scope.md):
`memory/overview` answers the signed-in account's (`admin`) memory and names
no profile; `memory/search` is refused `runtime_unavailable`. So the app,
built for the Session's profile (D2 A), shows its honest refusal at open and
for a search, with D1's bounded problem + next step and no raw error; the
Skills dialog and the MCP view read the server's real (empty) lists.

    python3 tools/walk/a36_live.py <desktop|phone> <outdir> [serve-port]

The token reaches the serve as OCTOS_AUTH_TOKEN and the app as OCTOS_BEARER
(environment only); it is never printed or saved. Every file the run wrote
is scanned for it (whole, first 8, last 8); the serve's directory (its data
copy and the token file) is deleted.
"""
import os
import pathlib
import secrets
import shutil
import subprocess
import sys
import time
import urllib.request

from a10_lib import Walk, checks_line, dialog_checks, run_session, scrub

WALK = {
    "name": "a36_live",
    "title": "A36b live: Settings > Capabilities (Skills / MCP / Memory) and Memory's honest refusal against a private "
             "octos serve (a6ea8505 answers memory for the signed-in account), no model turn",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}", "{fport}"], "env": {"A10_PORT": "{port}"}}],
    "needs": [],
    "timeout": 900,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a36/live/{MODE}")
SERVE_PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8477
PHONE = MODE == "phone"
NAV = "set_rail_capabilities" if PHONE else "set_nav_capabilities"
ROOT = pathlib.Path(__file__).resolve().parents[2]
NOINDEX = pathlib.Path.home() / "home" / "oa.noindex"
OCTOS = NOINDEX / "p0-build/tmp/octos-target/release/octos"
SRC = NOINDEX / "live-gate/data"


def texts(W: Walk):
    return [w.get("t") or "" for w in W.snap() if W.shown(w)]


def has(W: Walk, *needles, secs: float = 8.0) -> bool:
    return W.wait(lambda: all(any(n in t for t in texts(W)) for n in needles), secs)


def numeric(W: Walk, name: str):
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_mem_", "b3_close", "b3_title"), viewport="b3_scroll")
    W.check(f"{name}: numeric checks", c["ok"], checks_line(c))


def walk(W: Walk):
    W.check("live: connected to the private serve", W.wait(lambda: W.composer() is not None, 40))
    W.click("settings_open_hit")
    W.wait(lambda: bool(W.visible("settings_drawer")), 8)
    W.mark()
    W.click(NAV)
    W.check("live: Settings > Capabilities shows the three rows (the server advertises each method)",
            W.wait(lambda: all(W.visible(r) for r in ("set_cap_skills_row", "set_cap_mcp_row", "set_cap_memory_row")), 8))
    W.shot(f"01-capabilities-live-{MODE}")
    W.mark()
    W.click("set_cap_skills")
    W.check("live: Skills 'Open' -> the Skills dialog reads the profile's (empty) skills",
            W.wait(lambda: bool(W.visible("dlg_skills_t_title")), 10))
    time.sleep(1.5)
    W.shot(f"02-skills-live-{MODE}")
    W.click("dialog_close")
    W.wait(lambda: not W.visible("dlg_skills_t_title"), 6)
    W.click("set_cap_mcp")
    W.check("live: MCP servers 'Open' -> the server's real status: no servers, configured on the server",
            W.wait(lambda: bool(W.visible("b3_inv_mcp_note")), 10))
    W.shot(f"03-mcp-live-{MODE}")
    W.click("b3_close")
    W.wait(lambda: not W.visible("b3_inv_mcp_note"), 6)
    W.mark()
    W.click("set_cap_memory")
    W.check("live: Memory 'Open' -> the dialog", W.logged("b3.open.memory", 4) and W.wait(lambda: bool(W.visible("b3_mem_title")), 8))
    W.check("live: the honest refusal — a6ea8505 answered for the signed-in account, not dsflash (D2 A)",
            has(W, "Couldn't read memory.", "The server did not confirm the memory scope for this session in dsflash.",
                "Update octos to a version that reports session memory scope.", secs=12))
    joined = " ".join(texts(W))
    W.check("live: no raw server error, no admin memory shown as dsflash's, no Add note (D1)",
            not any(x in joined for x in ("ProfileRuntime", "admin", "No knowledge pages yet")) and not W.visible("b3_mem_add"))
    W.check("live: the scope line names the Session's profile in plain text (D1)", W.text("b3_mem_scope") == "Server Profile: dsflash")
    numeric(W, "live refused")
    W.shot(f"04-memory-refused-live-{MODE}")
    r = W.rect("b3_mem_query")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text("steer queue")
        W.mark()
        W.key("Return")
    W.check("live: a search is refused for the same reason, in bounded copy",
            has(W, "Couldn't search memory.", "not for dsflash.", secs=12) and "ProfileRuntime" not in " ".join(texts(W)))
    W.dismiss_keyboard("b3_mem_title")
    numeric(W, "live search refused")
    W.shot(f"05-memory-search-refused-live-{MODE}")
    W.click("b3_close")
    W.wait(lambda: not W.visible("b3_mem_title"), 6)
    W.click("set_back" if PHONE else "settings_close")


def main() -> int:
    live = ROOT / "tmp" / f"live-a36b-{SERVE_PORT}"
    if live.exists():
        shutil.rmtree(live)
    (live / "inst").mkdir(parents=True)
    (live / "ws").mkdir()
    shutil.copytree(SRC, live / "data")
    token = secrets.token_hex(24)
    tok = live / "token"
    tok.write_text(token)
    os.chmod(tok, 0o600)
    OUT.mkdir(parents=True, exist_ok=True)
    trace = OUT / "trace.jsonl"
    serve = subprocess.Popen(
        [str(OCTOS), "serve", "--port", str(SERVE_PORT), "--host", "127.0.0.1", "--solo", "--data-dir",
         str(live / "data"), "--instance-data-dir", str(live / "inst"), "--cwd", str(live / "ws")],
        env=dict(os.environ, OCTOS_AUTH_TOKEN=token), stdout=open(live / "serve.log", "w"), stderr=subprocess.STDOUT)
    rc = 1
    try:
        for _ in range(120):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{SERVE_PORT}/api/version", timeout=2).read()
                break
            except Exception:
                if serve.poll() is not None:
                    raise SystemExit("serve exited")
                time.sleep(0.5)
        env = {"OCTOS_BASE_URL": f"http://127.0.0.1:{SERVE_PORT}", "OCTOS_BEARER": token, "OCTOS_PROFILE_ID": "dsflash",
               "OCTOSCODE_TRACE_FILE": str(trace)}
        rc = run_session(walk, mode=MODE, outdir=str(OUT), replay_port=None, env=env)
    finally:
        serve.terminate()
        try:
            serve.wait(10)
        except Exception:
            serve.kill()
        # The evidence (and the app's own logs) never carry the token; the
        # evidence carries no machine path.
        leaks = []
        for f in list(OUT.rglob("*")) + list((ROOT / "tmp" / "hs").glob("*.log")):
            if f.is_file():
                data = f.read_bytes()
                for s in (token, token[:8], token[-8:]):
                    if s.encode() in data:
                        leaks.append(f.name)
                if f.parent.is_relative_to(OUT) and f.suffix in (".jsonl", ".json", ".log", ".txt"):
                    f.write_text(scrub(data.decode(errors="replace")))
        shutil.rmtree(live)
        print(f"token scan: {len(leaks)} file(s) carrying it {sorted(set(leaks))}; serve dir and token removed")
        if leaks:
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main())
