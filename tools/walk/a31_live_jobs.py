#!/usr/bin/env python3
"""A31 — row 15 against a REAL octos serve (a private instance this script
starts and stops; never the operator's). No model turn is made.

Proves what a replay cannot: the real server accepts the native client's
`skill.action_jobs.v1` (the handshake's 22nd feature), advertises it and
`skill/action/job/list` on `session/open`, and answers the dialog's scoped
list; the Skills dialog opened by the /skills palette row CLICK shows the
real reply (a fresh profile has no jobs: "No background jobs in this
Session.").

    OCTOSCODE_APP_BIN=<host octosense> python3 tools/walk/a31_live_jobs.py <app-port> <serve-port> <outdir> [desktop|phone]

The serve's data is a COPY of ~/home/oa.noindex/live-gate/data under the
worktree's tmp/ (deleted afterwards); a random token lives in a mode-600 file
and reaches the serve as OCTOS_AUTH_TOKEN and the app as OCTOS_BEARER only —
never printed, logged or saved; every written file is scanned for it (whole,
first 8, last 8) before the script returns.
"""
import json
import os
import pathlib
import secrets
import shutil
import subprocess
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)
from a10_lib import Walk, dialog_checks, checks_line, scrub  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
APP_PORT, SERVE_PORT, OUT = int(sys.argv[1]), int(sys.argv[2]), pathlib.Path(sys.argv[3])
MODE = sys.argv[4] if len(sys.argv) > 4 else "desktop"
OCTOS = pathlib.Path.home() / "home/oa.noindex/p0-build/tmp/octos-target/release/octos"
SRC_DATA = pathlib.Path.home() / "home/oa.noindex/live-gate/data"
P = "dlg_skills_"


def main() -> int:
    app_bin = os.environ.get("OCTOSCODE_APP_BIN")
    if not app_bin:
        raise SystemExit("set OCTOSCODE_APP_BIN")
    live = ROOT / "tmp" / f"live-a31-{SERVE_PORT}"
    if live.exists():
        shutil.rmtree(live)
    (live / "inst").mkdir(parents=True)
    (live / "ws").mkdir()
    shutil.copytree(SRC_DATA, live / "data")
    token = secrets.token_hex(24)
    tok_file = live / "token"
    tok_file.write_text(token)
    os.chmod(tok_file, 0o600)
    OUT.mkdir(parents=True, exist_ok=True)
    serve_env = dict(os.environ, OCTOS_AUTH_TOKEN=token)
    serve = subprocess.Popen(
        [str(OCTOS), "serve", "--port", str(SERVE_PORT), "--host", "127.0.0.1", "--solo",
         "--data-dir", str(live / "data"), "--instance-data-dir", str(live / "inst"), "--cwd", str(live / "ws")],
        env=serve_env, stdout=open(live / "serve.log", "w"), stderr=subprocess.STDOUT)
    state = live / "state"
    for d in ("cred", "downloads", "recents"):
        (state / d).mkdir(parents=True, exist_ok=True)
    trace = OUT / "trace.jsonl"
    if trace.exists():
        trace.unlink()
    app_env = dict(os.environ)
    app_env.update({
        "OCTOSCODE_DRAFTS_FILE": f"{state}/drafts.json", "OCTOSCODE_CREDENTIALS_DIR": f"{state}/cred",
        "OCTOSCODE_PREF_PATH": f"{state}/prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": f"{state}/notifications.json",
        "OCTOSCODE_SHOW_THINKING_FILE": f"{state}/show-thinking.json", "OCTOSCODE_DOWNLOAD_DIR": f"{state}/downloads",
        "OCTOSCODE_RECENTS_DIR": f"{state}/recents", "OCTOSCODE_DISPLAY_PREFS_PATH": f"{state}/display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": f"{state}/pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": f"{state}/driver-id",
        "OCTOSCODE_CONNECTION_FILE": f"{state}/connection-v1.json",
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SERVE_PORT}", "OCTOS_BEARER": token, "OCTOS_PROFILE_ID": "dsflash",
        "OCTOS_WORKSPACE_CWD": str(live / "ws"),
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"), "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": str(live / "hs"), "OCTOSCODE_TRACE_FILE": str(trace),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if MODE == "phone" else ""),
    })
    if MODE == "phone":
        app_env["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    rc = 1
    w = Walk(APP_PORT, str(OUT), MODE)
    try:
        end = time.time() + 60
        while time.time() < end:
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{SERVE_PORT}/health", timeout=2)
                break
            except Exception:  # noqa: BLE001
                if serve.poll() is not None:
                    raise SystemExit("octos serve exited (see its log under tmp/)")
                time.sleep(0.5)
        subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "start", app_bin, str(APP_PORT)], env=app_env, check=True,
                       stdout=subprocess.DEVNULL)
        w.wait(lambda: '"sz"' in w.get("/s", tolerant=True), 40, 1.0)
        time.sleep(4.0)
        if MODE == "phone":
            w.open_phone_app()
        w.check("live: connected (the composer is up)", w.wait(lambda: w.composer() is not None, 60))
        time.sleep(2.0)
        w.mark()
        w.check("live: the /skills palette row CLICK opens Skills with its Background jobs",
                w.palette_run("ski", "/skills") and w.wait_shown(f"{P}jobs_head", 15))
        w.check("live: the on-open job read answered (log)", w.logged("dialog load dialog.refresh.skill_jobs: 0 job(s) listed", 15))
        w.check("live: the section shows the real reply: no jobs in this Session",
                w.wait(lambda: w.text(f"{P}jobs_empty") == "No background jobs in this Session.", 10), repr(w.text(f"{P}jobs_empty")))
        w.check("live: no note (the real server seeds the section)", not w.visible(f"{P}jobs_note"))
        c = dialog_checks(w.snap(), "dialog_frame", (P,), viewport="dialog_scroll")
        w.check("live: numeric checks", c["ok"], checks_line(c))
        png = OUT / f"live-skills-{MODE}.png"
        w.shot(png.stem)
        # The protocol trace: the open reply advertised the feature and the
        # method, and the list went out scoped and came back.
        frames = [json.loads(l) for l in trace.read_text().splitlines() if l.strip()] if trace.exists() else []
        opens = [f for f in frames if f.get("method") == "session/open" and f.get("dir") == "in"]
        caps = [json.dumps(f.get("body") or {}) for f in opens]
        # octos lists the feature only for a connection that asked for it
        # (`skill_action_jobs_available`): the reply proves the handshake.
        w.check("live: session/open advertised skill.action_jobs.v1 (asked for) and skill/action/job/list",
                any("skill.action_jobs.v1" in c and "skill/action/job/list" in c for c in caps), f"{len(opens)} open reply(ies)")
        outs = [f for f in frames if f.get("method") == "skill/action/job/list" and f.get("dir") == "out"]
        body = (outs[0].get("body") or {}) if outs else {}
        w.check("live: skill/action/job/list went out with the dialog's Profile + Session",
                body.get("profile_id") == "dsflash" and bool(body.get("session_id")), json.dumps(body))
        rc = w.summary()
    finally:
        subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "stop", str(APP_PORT)], env=app_env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        serve.terminate()
        try:
            serve.wait(10)
        except Exception:  # noqa: BLE001
            serve.kill()
        # The token: in no file this run wrote (whole, first 8, last 8).
        leaks = []
        for p in OUT.rglob("*"):
            if p.is_file():
                data = p.read_bytes()
                if any(s.encode() in data for s in (token, token[:8], token[-8:])):
                    leaks.append(p.name)
        for p in leaks:
            (OUT / p).unlink()
        print(f"token scan: {len(leaks)} file(s) carried it{' (deleted)' if leaks else ''}")
        # No machine paths in what may be committed (the private serve's
        # folders appear in the trace and in the header's /snap text).
        for p in OUT.rglob("*"):
            if p.is_file() and p.suffix in (".json", ".jsonl", ".log", ".txt"):
                p.write_text(scrub(p.read_text(errors="replace")))
        shutil.rmtree(live, ignore_errors=True)
    return rc if not leaks else 1


if __name__ == "__main__":
    sys.exit(main())
