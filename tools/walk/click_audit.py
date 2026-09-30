#!/usr/bin/env python3
"""#35a — click audit: which controls on the docked screens actually respond?

Mounts each docked surface in the REAL app (the same env gates the walk uses),
lists its clickable widgets from `/snap?all=1`, clicks each centre, and records
the observed effect (a new `[octoscode]` log line over HTTP /log, a /snap state
change, or nothing). Re-runnable: once the #32h dispatcher fix lands, the dead
rows must flip to respond.

NO app code here — inventory + harness only (the entry forbids app edits).

```sh
python3 tools/walk/click_audit.py \
  --app-bin tmp/32b2-fork-target/debug/octosense \
  --out docs/walk/click-audit.csv
```
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import pathlib
import subprocess
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HEADLESS = ROOT / "harness" / "headless.sh"
REPLAY = ROOT / "target" / "debug" / "examples" / "replay_serve"
WORK = ROOT / "tmp" / "walk" / "click-audit"

CLICKABLE = {"KitButton", "Button", "TextInput", "Input"}
SCENARIO_PORT = 8387  # this lane's documented block (8380–8389)
DEAD_URL = "http://127.0.0.1:8399"  # nothing listens — the app stays first-run

# expected action per control, from each screen's ACTIONS table (screens/*.rs).
# Keyed by lowercase substring of (instance id + text). "" = unmapped.
EXPECTED: dict[str, list[tuple[str, str]]] = {
    "connect-first-run": [
        ("retry", "connect.retry"),
        ("solo", "connect.use_local_solo"),
        ("deepseek", "onboarding.provider.deepseek"),
        ("kimi", "onboarding.provider.kimi"),
        ("glm", "onboarding.provider.glm"),
        ("server", "input.server"),
        ("token", "input.token"),
        ("profile", "input.profile"),
        ("api key", "input.apikey"),
        ("apikey", "input.apikey"),
        ("connect", "connect"),
        ("create", "create_profile"),
    ],
    "chrome-settings": [
        ("refresh", "ws.refresh"),
        ("browse", "ws.browse"),
        ("create", "ws.create_folder"),
        ("copy", "set.diagnostics.copy"),
        ("close", "settings.close"),
    ],
    "chrome-review": [
        ("last turn", "diff.scope"),
        ("review", "review.start"),
        ("start", "review.start"),
    ],
    "dock-palette": [("palette", "palette.query.set")],
}


class Http:
    def __init__(self, port: int):
        self.base = f"http://127.0.0.1:{port}"

    def get(self, path: str, tries: int = 3) -> str:
        last = None
        for _ in range(tries):
            try:
                with urllib.request.urlopen(self.base + path, timeout=20) as r:
                    return r.read().decode()
            except Exception as e:  # noqa: BLE001
                last = e
                time.sleep(0.2)
        raise AssertionError(f"{path} failed: {last}")

    def snap(self) -> dict:
        return json.loads(self.get("/snap?all=1"))

    def click(self, x: int, y: int) -> None:
        self.get(f"/click?x={x}&y={y}&wait=1")

    def log(self) -> str:
        return self.get("/log?n=400")


def wait_mount(app: Http, marker_text: str, timeout: float = 100.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            s = app.snap()
        except Exception:  # noqa: BLE001 — the bridge comes up before the module
            time.sleep(1.0)
            continue
        if any(str(w.get("t", "")) == marker_text for w in s.get("s", [])):
            return
        time.sleep(1.0)
    raise AssertionError(f"the module never mounted (no {marker_text!r} widget)")


def app_view(snap: dict) -> tuple[int, int, int, int] | None:
    for w in snap.get("s", []):
        if str(w.get("ty", "")) == "OctoscodeView":
            return tuple(w["r"])  # type: ignore[return-value]
    return None


def clickables(snap: dict):
    view = app_view(snap)
    out, seen_centers = [], set()
    for w in snap.get("s", []):
        r = w.get("r") or [0, 0, 0, 0]
        if len(r) != 4 or r[2] <= 0 or r[3] <= 0:
            continue
        if view and not (r[0] >= view[0] - 1 and r[1] >= view[1] - 1
                         and r[0] + r[2] <= view[0] + view[2] + 1
                         and r[1] + r[3] <= view[1] + view[3] + 1):
            continue
        if str(w.get("ty", "")) not in CLICKABLE:
            continue
        c = (int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
        if c in seen_centers:  # component + its inner hit layer share a centre
            continue
        seen_centers.add(c)
        out.append((str(w.get("i", "")), str(w.get("ty", "")), r, str(w.get("t", ""))))
    return out


def expected_for(screen: str, ident: str, text: str) -> str:
    hay = f"{ident} {text}".lower().strip()
    for needle, action in EXPECTED.get(screen, []):
        if needle in hay:
            return action
    return ""


def snapshot_fingerprint(snap: dict) -> set:
    return {(str(w.get("i", "")), str(w.get("t", "")), tuple(w.get("r") or []))
            for w in snap.get("s", [])}


def run_screen(cfg: dict, bin_path: pathlib.Path, app_port: int, rows: list) -> None:
    name = cfg["name"]
    sdir = WORK / name
    sdir.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update({
        "OCTOS_BASE_URL": DEAD_URL if cfg["serve"] is None
        else f"http://127.0.0.1:{SCENARIO_PORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_ARGS": "--module octoscode",
        "HEADLESS_STATE": str(sdir / "state"),
    })
    env.update(cfg.get("env", {}))
    serve = None
    app = Http(app_port)
    try:
        if cfg["serve"] is not None:
            log = open(sdir / "serve.log", "w")
            serve = subprocess.Popen(
                [str(REPLAY), str(SCENARIO_PORT), "--scenario", cfg["serve"]],
                stdout=log, stderr=log)
            for _ in range(75):
                if "listening" in (sdir / "serve.log").read_text():
                    break
                time.sleep(0.2)
        r = subprocess.run(
            ["bash", str(HEADLESS), "start", str(bin_path), str(app_port)],
            env=env, cwd=str(ROOT), capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise AssertionError(f"app start failed: {(r.stdout + r.stderr)[-400:]}")
        marker = "Connect" if cfg["serve"] is None else "OctosCode"
        wait_mount(app, marker)
        time.sleep(1.5)  # let the first full layout settle

        before_snap = app.snap()
        before_log = app.log()
        before_fp = snapshot_fingerprint(before_snap)
        for ident, ty, rect, text in clickables(before_snap):
            cx, cy = int(rect[0] + rect[2] / 2), int(rect[1] + rect[3] / 2)
            try:
                app.click(cx, cy)
            except AssertionError as e:
                rows.append([name, ident, ty, json.dumps(rect), text,
                             expected_for(name, ident, text), f"click-failed: {e}"[:120]])
                continue
            time.sleep(0.6)
            after_log = app.log()
            new_log = after_log[len(before_log):] if after_log.startswith(before_log[:200]) \
                and len(after_log) > len(before_log) else ""
            # /log is a ring buffer; diff by lines not present before
            new_lines = [l for l in after_log.splitlines()
                         if l not in before_log.splitlines() and "[octoscode]" in l]
            action_line = next((l.split("[octoscode]", 1)[1].strip()[:100]
                                for l in reversed(new_lines)), "")
            after_snap = app.snap()
            changed = len(snapshot_fingerprint(after_snap) ^ before_fp)
            before_fp = snapshot_fingerprint(after_snap)
            before_log = after_log
            if action_line:
                observed = f"log: {action_line}"
            elif changed:
                observed = f"state: {changed} widget deltas"
            else:
                observed = "nothing"
            rows.append([name, ident, ty, json.dumps(rect), text,
                         expected_for(name, ident, text), observed])
    finally:
        subprocess.run(["bash", str(HEADLESS), "stop", str(app_port)],
                       cwd=str(ROOT), capture_output=True, timeout=60)
        if serve is not None:
            serve.terminate()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--app-bin", required=True)
    ap.add_argument("--out", default="docs/walk/click-audit.csv")
    ap.add_argument("--port", type=int, default=8376)
    ap.add_argument("--only", default="", help="comma-separated screen names")
    args = ap.parse_args()

    SCREENS = [
        {"name": "connect-first-run", "env": {}, "serve": None},
        {"name": "dock-palette", "env": {"OCTOSCODE_SCREEN": "palette"},
         "serve": "conversation"},
        {"name": "dock-error", "env": {"OCTOSCODE_SCREEN": "error"},
         "serve": "conversation"},
        {"name": "dock-loading", "env": {"OCTOSCODE_SCREEN": "loading"},
         "serve": "conversation"},
        {"name": "chrome-review", "env": {"OCTOSCODE_CHROME": "review"},
         "serve": "conversation"},
        {"name": "chrome-settings", "env": {"OCTOSCODE_CHROME": "settings"},
         "serve": "conversation"},
    ]
    only = {s.strip() for s in args.only.split(",") if s.strip()}
    screens = [s for s in SCREENS if not only or s["name"] in only]

    WORK.mkdir(parents=True, exist_ok=True)
    rows: list[list[str]] = []
    out = ROOT / args.out
    for cfg in screens:
        try:
            run_screen(cfg, pathlib.Path(args.app_bin), args.port, rows)
        except Exception as e:  # noqa: BLE001 — record and continue
            print(f"[click-audit] {cfg['name']}: {e}", flush=True)
        # append as we go (LESSONS: work that exists only in the head is lost)
        with open(out, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["screen", "instance", "widget", "rect", "text",
                        "expected_action", "observed"])
            w.writerows(rows)
        print(f"[click-audit] {cfg['name']}: {sum(1 for r in rows if r[0]==cfg['name'])} controls",
              flush=True)

    total = len(rows)
    expected_rows = [r for r in rows if r[5]]
    respond = sum(1 for r in expected_rows if not r[6].startswith(("nothing", "click-failed")))
    dead = sum(1 for r in expected_rows if r[6] == "nothing")
    print(f"SURFACE: {total} controls, {len(expected_rows)} with an expected action, "
          f"{respond} respond, {dead} dead "
          f"(+{total - len(expected_rows)} unmapped)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
