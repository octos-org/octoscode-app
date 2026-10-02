#!/usr/bin/env python3
"""#35c — prove sidebar_toggle_hit at 384x788 (the phone-width shell).

The entry requires the toggle at a phone size: below 760 px the sidebar is
hidden and the toggle is the only way back, so p0-map-f saw it dead on the 6T.
Run as a script (not an inline heredoc) so the mount can outlast a tool timeout.
"""
import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.request
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

ROOT = pathlib.Path(__file__).resolve().parents[2]
SD = ROOT / "tmp" / "walk" / "35c" / "phone"
BIN = sys.argv[1] if len(sys.argv) > 1 else "tmp/octosense-35c"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8380
SCEN = int(os.environ.get("WALK_SCENARIO_PORT", "8387"))


def get(path):
    return urllib.request.urlopen(f"http://127.0.0.1:{PORT}{path}", timeout=25).read().decode()


def main():
    SD.mkdir(parents=True, exist_ok=True)
    serve = subprocess.Popen(
        [str(ROOT / "target/debug/examples/replay_serve"), str(SCEN),
         "--scenario", "conversation"],
        stdout=open(SD / "serve.log", "w"), stderr=subprocess.STDOUT)
    time.sleep(2)
    if "AddrInUse" in (SD / "serve.log").read_text():
        print(f"replay_serve could NOT bind {SCEN} (AddrInUse) — INVALID RUN, "
              "the module cannot mount without a live conversation")
        serve.terminate()
        return 1
    env = os.environ.copy()
    env.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SCEN}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "OCTOSENSE_WINDOW_SIZE": "384x788",
        # The app is launched with `--module octoscode`; without it the shell
        # starts but never launches the module and the snap shows shell chrome
        # only ("wm: launch octoscode failed: binary not found: octoscode"),
        # which reads as "every control missing" rather than a real failure.
        "HEADLESS_ARGS": "--module octoscode",
        "HEADLESS_STATE": str(SD / "state"),
    })
    subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", BIN, str(PORT)],
                   env=env, capture_output=True, text=True, timeout=240)
    try:
        snap = None
        for _ in range(240):          # >100 s on this shared host
            try:
                snap = json.loads(get("/snap?all=1"))
                if any(str(w.get("t", "")) == "OctosCode" for w in snap.get("s", [])):
                    break
            except Exception:
                pass
            time.sleep(2)
        if snap is None:
            print("NEVER MOUNTED")
            return 1
        time.sleep(2)
        snap = json.loads(get("/snap?all=1"))
        (SD / "snap.json").write_text(json.dumps(snap))
        (SD / "d.txt").write_text(get("/d"))
        idx = {str(w.get("i")): w for w in snap.get("s", [])}
        print("widgets:", len(idx))
        for name in ("sidebar_toggle_hit", "sidebar_toggle_wrap", "threads_column"):
            w = idx.get(name)
            print(f"  {name:22} {w.get('r') if w else 'ABSENT'}")
        tog = idx.get("sidebar_toggle_hit")
        if not tog or not tog.get("r") or tog["r"][2] <= 0:
            print("sidebar_toggle_hit NOT laid out at 384x788")
            return 1
        r = tog["r"]
        # CONTROL: read the state twice with NO click, so a late-settling
        # layout cannot be mistaken for the toggle responding. The sidebar is
        # hidden at 384 px, so threads_column must stay 0x0 here.
        def col():
            s = json.loads(get("/snap?all=1"))
            return next((w.get("r") for w in s.get("s", [])
                         if str(w.get("i", "")) == "threads_column"), None)
        col_before1 = col()
        time.sleep(2.0)
        col_before2 = col()
        print(f"no-click control: threads_column {col_before1} then {col_before2}")
        n0 = len(json.loads(get("/log?n=400")).get("l", []))
        cx, cy = int(r[0] + r[2] / 2), int(r[1] + r[3] / 2)
        get(f"/click?x={cx}&y={cy}&wait=2")
        time.sleep(1.5)
        new = json.loads(get("/log?n=400")).get("l", [])[n0:]
        col_after = col()
        print(f"clicked ({cx},{cy}); threads_column after = {col_after}")
        # The toggle is UI-local (lib.rs: it flips sidebar_open + sync_chrome) and
        # logs nothing, so the PROOF is this state delta — and it only counts
        # because the two no-click reads above stayed 0x0.
        stable_hidden = all(c == [0, 0, 0, 0] for c in (col_before1, col_before2))
        opened = col_after is not None and col_after != [0, 0, 0, 0]
        print("VERDICT toggle responds:", stable_hidden and opened,
              f"(stable_hidden={stable_hidden} opened={opened})")
        print("new [octoscode] lines:",
              [l.split("[octoscode]")[-1][:70] for l in new if "[octoscode]" in l] or "(none - the toggle is UI-local and logs nothing)")
        return 0 if (stable_hidden and opened) else 1
    finally:
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(PORT)],
                       capture_output=True, timeout=60)
        serve.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
