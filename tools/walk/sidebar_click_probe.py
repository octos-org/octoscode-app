#!/usr/bin/env python3
"""#D2a — prove a board-2 sidebar card's OWN control routes to screens::sidebar.

The audit's centre-dedupe picks the base chrome (new_chat_hit/row_hit share a
centre with the card), so it never clicked a `ctl_*` control. This drives the
card's own widgets by id, clicks each one's laid-out centre, and asserts the
app's OWN effect line ("[octoscode] sidebar action: <event>") — the LESSONS-135
rule: wiring is proven by a CLICK, never by calling the action id.

Run:
  OCTOSCODE_CARDS_DIR=design/stage-b/phase4-new2/cards \
    python3 tools/walk/sidebar_click_probe.py tmp/octosense-d2a2 sidebar_grouped
"""
import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCEN_PORT = int(os.environ.get("WALK_SCENARIO_PORT", "8382"))
PORT = int(os.environ.get("WALK_APP_PORT", "8385"))


def get(path):
    return urllib.request.urlopen(f"http://127.0.0.1:{PORT}{path}", timeout=25).read().decode()


def wait_window(app_port: int, tries: int = 120) -> bool:
    """Wait for the SHELL's window before trusting any /snap.

    The outer loop's 2026-10-01 lesson: "wait until /s lists main_window before
    /snap (the shell takes several seconds to open its window)". A /snap taken
    earlier returns the shell's pre-window widget set, which reads as "nothing
    mounted" and sent me chasing a layout bug that was never there.
    """
    # /s ground truth (makepad remote.rs route_status): {"app":...,"pid":...,
    # "w":[{"i":<numeric id>,"t":"<title>","sz":[...],...}]} — the id is a
    # NUMBER, so an earlier draft's `str(w["i"])` substring check could never
    # match anything. The window exists iff `w` is non-empty.
    for _ in range(tries):
        try:
            with urllib.request.urlopen(
                    f"http://127.0.0.1:{app_port}/s", timeout=10) as r:
                if (json.loads(r.read().decode()).get("w") or []):
                    return True
        except Exception:
            pass
        time.sleep(1.0)
    return False


def snap():
    return json.loads(get("/snap?all=1"))


def log_lines():
    try:
        return json.loads(get("/log?n=400")).get("l", [])
    except Exception:
        return []


def main():
    binary = sys.argv[1] if len(sys.argv) > 1 else "tmp/octosense-d2a2"
    screen = sys.argv[2] if len(sys.argv) > 2 else "sidebar_grouped"
    work = ROOT / "tmp" / "walk" / "sidebar-probe"
    work.mkdir(parents=True, exist_ok=True)

    serve = subprocess.Popen(
        [str(ROOT / "target/debug/examples/replay_serve"), str(SCEN_PORT),
         "--scenario", "conversation"],
        stdout=open(work / "serve.log", "w"), stderr=subprocess.STDOUT)
    time.sleep(2)

    env = os.environ.copy()
    env.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SCEN_PORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "MAKEPAD_WM_TEST_APP": "octoscode",
        # REQUIRED (click_audit.py:240 sets it too): without this the shell
        # starts but the module never launches.
        "HEADLESS_ARGS": "--module octoscode",
        "OCTOSCODE_SCREEN": screen,
        "OCTOSCODE_CARDS_DIR": str(ROOT / "design/stage-b/phase4-new2/cards"),
        # First witness for "the page never enters the widget tree": the lowered
        # DSL itself, dumped by sidebar::lower when this env var is set.
        "OCTOSCODE_DUMP_LOWER": str(work / f"lowered-{screen}.dsl"),
        "HEADLESS_STATE": str(work / "state"),
    })
    subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", binary, str(PORT)],
                   env=env, capture_output=True, text=True, timeout=240)
    try:
        # GATE FIRST: the shell's window, not the module's text (outer loop's
        # 2026-10-01 lesson — "/s lists main_window before /snap, the shell
        # takes several seconds to open its window").
        if not wait_window(PORT):
            print("NO main_window in /s — the shell never opened its window")
            return 1
        print("window open (/s lists main_window)")
        # The 360x780 drawer check needs PROOF the window really shrank (the
        # card is a fixed 406x776 canvas, so identical control rects prove
        # nothing). /s carries sz per window (remote.rs route_status).
        try:
            w0 = json.loads(get("/s"))["w"][0]
            print(f"window size: {w0.get('sz')} px")
        except Exception as e:
            print(f"window size: unavailable ({e})")
        # >100 s mount on this shared host.
        s = None
        for _ in range(240):
            try:
                s = snap()
                if any(str(w.get("t", "")) == "OctosCode" for w in s.get("s", [])):
                    break
            except Exception:
                pass
            time.sleep(2)
        if s is None:
            print("NEVER MOUNTED")
            return 1
        # A mounted card's text lands a FRAME BEFORE its KitButtons get layout
        # (click_audit.py records this: 'Connect' present, every clickable still
        # 0x0). The card DOES lower and wire — the log already says
        # "card events: N tap(s) wired" — so wait for a NON-ZERO ctl_ rect
        # rather than trusting a single snap.
        ctl = []
        for _ in range(60):
            time.sleep(2.0)
            s = snap()
            ctl = [(str(w.get("i")), w.get("r"), w.get("t", ""))
                   for w in s.get("s", [])
                   if "ctl_" in str(w.get("i", ""))
                   and (w.get("r") or [0, 0, 0, 0])[2] > 0]
            if ctl:
                break
        print(f"[{screen}] mounted controls: {len(ctl)}")
        for i, r, t in ctl:
            print(f"   {i:34} {r} {str(t)[:24]!r}")
        if not ctl:
            # Dump the tree BEFORE dying: is the card's page in the tree at
            # all, and what ancestor chain do the ctl_ widgets hang from?
            # (The partial-ACK evidence was a /d taken by hand; keep the dump
            # beside this probe's own logs so the verdict is reproducible.)
            dump = work / f"dump-{screen}.txt"
            try:
                dump.write_text(get("/d"))
            except Exception as e:
                dump.write_text(f"/d failed: {e}")
            (work / f"snap-{screen}.json").write_text(json.dumps(snap()))
            n_page = dump.read_text().count("phase4n2-01")
            print(f"NO ctl_ CONTROLS LAY OUT — tree dumped to {dump} "
                  f"(phase4n2-01 mentions: {n_page})")
            return 1

        # The card's OWN event for each control id, from its service-actions.
        card = {"sidebar_grouped": "phase4n2-01", "sidebar_statuses": "phase4n2-02",
                "sidebar_search": "phase4n2-03", "sidebar_collapsed": "phase4n2-04",
                "sidebar_drawer": "phase4n2-05"}[screen]
        sa = json.loads((ROOT / "design/stage-b/phase4-new2/cards" / card /
                         "service-actions.json").read_text())
        ev_of = {f"{n}_control": c.get("event")
                 for n, c in (sa.get("controls") or {}).items()}

        responded, dead = [], []
        for ident, r, _t in ctl:
            want = ev_of.get(ident, "")
            before = len(log_lines())
            get(f"/click?x={int(r[0] + r[2] / 2)}&y={int(r[1] + r[3] / 2)}&wait=2")
            time.sleep(1.2)
            lines = log_lines()[before:]
            new = [l for l in lines if "sidebar action:" in l]
            if new:
                line = new[-1].split("sidebar action:")[-1].strip()[:40]
                ok = (not want) or want in line
                (responded if ok else dead).append((ident, want, line))
            elif want == "workspace.new_chat_here" and any(
                    "sidebar new chat in workspace" in l for l in lines):
                # The dispatcher logs THIS effect on a different line by
                # design (lib.rs NewChatInWorkspace: an info! line naming the
                # workspace, then it routes the router's own new-chat) —
                # recorded as forwarded, not as a bare "sidebar action:".
                line = [l for l in lines
                        if "sidebar new chat in workspace" in l][-1]
                line = line.split("sidebar new chat in workspace")[-1]
                responded.append((ident, want,
                                  "(forwarded)" + line.strip()[:52]))
            else:
                dead.append((ident, want, "(no 'sidebar action:' line)"))
            time.sleep(0.3)

        print()
        print(f"RESPOND {len(responded)} / DEAD {len(dead)}")
        for ident, want, line in responded:
            print(f"   ok  {ident:34} -> {line}   (card declares {want})")
        for ident, want, line in dead:
            print(f"   --  {ident:34} -> {line}   (card declares {want})")
        return 0 if responded and not dead else 1
    finally:
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(PORT)],
                       capture_output=True, timeout=60)
        serve.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
