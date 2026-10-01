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
import re
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

ACTION_LOG = re.compile(
    r"\[octoscode\].*(clicked|perform|route|card tap|screen action|draft synced|->)")

# Host-chrome controls are the same on every docked screen; their actions come
# from lib.rs's host arms (cited), NOT from a screen ACTIONS table.
CHROME: list[tuple[str, str]] = [
    ("newchat", "session.new"),        # lib.rs:2129-2131 ACTION_NEW_CHAT
    ("new chat", "session.new"),
    ("threadrow", "thread.open"),      # lib.rs:2147-2150 row_hit
    ("row_hit", "thread.open"),
    ("send_hit", "composer.submit"),   # lib.rs:2124-2139 (turn.interrupt when live)
    ("composer_0", ""),                # the draft TextInput: focus only
    ("plus_hit", "(unwired)"),         # lib.rs:2140-2142: hit targets only, ids
    ("mic_hit", "(unwired)"),          # reserved "for a later card"
]

# Per-screen expectations, from each screen's ACTIONS table (screens/*.rs).
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
    "dock-error": [
        ("btn_reload", "error.reload"),           # palette.rs:295
        ("btn_diag", "error.copy_diagnostics"),   # palette.rs:282
    ],
    "chrome-review": [
        ("review_toggle_hit", "review.toggle"),   # lib.rs:2179-2182
        ("review_close", "review.toggle"),
    ],
    "chrome-settings": [
        ("settings_close", "settings.toggle"),    # lib.rs:2186-2188
        ("refresh", "session.refresh"),           # lib.rs:2117-2119
        ("browse", "ws.browse"),
        ("create", "ws.create_folder"),
        ("copy", "set.diagnostics.copy"),
    ],
    "dock-palette": [],
    "dock-loading": [],
}


SHADOWED_BY_DOCK = {
    "i0_newchat", "i0_threadrow", "i0_composer_0",
    "plus_hit", "mic_hit", "send_hit", "row_hit", "new_chat_hit",
}


def expected_for(screen: str, ident: str, text: str) -> str:
    hay = f"{ident} {text}".lower().strip()
    for needle, action in CHROME:
        if needle in hay:
            return action
    for needle, action in EXPECTED.get(screen, []):
        if needle in hay:
            return action
    return ""


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
        # First-run mounts land a frame LATER than their text: the module's
        # text widgets can report rects long before the KitButtons get their
        # layout (observed: 'Connect' present, every clickable still 0x0 —
        # the audit then sees 0 controls). Wait for the first NON-ZERO
        # clickable, not just for text.
        deadline = time.monotonic() + 40.0
        while time.monotonic() < deadline:
            if any((w.get("r") or [0, 0, 0, 0])[2] > 0
                   for w in app.snap().get("s", [])
                   if str(w.get("ty", "")) in CLICKABLE):
                break
            time.sleep(1.0)
        time.sleep(1.5)  # let the first full layout settle

        before_snap = app.snap()
        # forensics: keep the audited snapshot so a "0 controls" run can be
        # post-mortemed (ty/rect shapes, mount timing) without a rerun
        (sdir / "snap.json").write_text(json.dumps(before_snap, indent=1))
        before_log = app.log()
        before_fp = snapshot_fingerprint(before_snap)
        for ident, ty, rect, text in clickables(before_snap):
            # An earlier click may have dismissed the surface this control
            # lives on (review_close after review_toggle) — a stale-rect click
            # must not read as "dead".
            cur = app.snap()
            live_rect = next((w.get("r") for w in cur.get("s", [])
                              if str(w.get("i", "")) == ident), None)
            if not live_rect or tuple(live_rect) == (0, 0, 0, 0):
                rows.append([name, ident, ty, json.dumps(rect), text,
                             expected_for(name, ident, text),
                             "dismissed: by an earlier click (present while the surface was open)"])
                before_fp = snapshot_fingerprint(cur)
                continue
            rect = live_rect
            exp = expected_for(name, ident, text)
            cx, cy = int(rect[0] + rect[2] / 2), int(rect[1] + rect[3] / 2)
            try:
                app.click(cx, cy)
            except AssertionError as e:
                rows.append([name, ident, ty, json.dumps(rect), text,
                             exp, f"click-failed: {e}"[:120]])
                continue
            time.sleep(0.6)
            after_log = app.log()
            # Count-based evidence: only NAMED action lines count (a bare
            # redraw line like "design root: …" fires on any repaint), and a
            # line identical to one already on the log is a hit only if its
            # COUNT grew — two controls on one arm fire the same line twice
            # (review_toggle then review_close both -> review.toggle), which a
            # set-difference judge would miss.
            def entries(raw: str) -> list[str]:
                # /log answers a JSON object {n, pool, l: [lines…]} — the log
                # lines ride the "l" key (a list, on one physical line).
                # Fall back to physical lines if it ever changes shape.
                try:
                    v = json.loads(raw)
                except Exception:  # noqa: BLE001
                    return raw.splitlines()
                if isinstance(v, dict) and isinstance(v.get("l"), list):
                    return [str(x) for x in v["l"]]
                if isinstance(v, list):
                    return [str(x) for x in v]
                return raw.splitlines()

            def named_counts(raw: str) -> dict[str, int]:
                counts: dict[str, int] = {}
                for l in entries(raw):
                    if ACTION_LOG.search(l):
                        counts[l] = counts.get(l, 0) + 1
                return counts

            before_counts = named_counts(before_log)
            after_counts = named_counts(after_log)
            new_named = [l for l, n in after_counts.items()
                         if n > before_counts.get(l, 0)]
            action_line = next((l.split("[octoscode]", 1)[1].strip()[:100]
                                for l in reversed(new_named)), "")
            after_snap = app.snap()
            changed = len(snapshot_fingerprint(after_snap) ^ before_fp)
            before_fp = snapshot_fingerprint(after_snap)
            before_log = after_log
            dimmer = next((w.get("r") for w in cur.get("s", [])
                           if str(w.get("i", "")) == "dimmer"), None)
            if (ident in SHADOWED_BY_DOCK
                    and dimmer and len(dimmer) == 4 and dimmer[2] > 0 and dimmer[3] > 0
                    and rect[0] >= dimmer[0] and rect[1] >= dimmer[1]
                    and rect[0] + rect[2] <= dimmer[0] + dimmer[2]
                    and rect[1] + rect[3] <= dimmer[1] + dimmer[3]):
                observed = ("shadowed: the modal dimmer covers the chrome "
                            "(by design) — click lands on the dimmer")
            elif cfg.get("shadow_chrome") and ident in SHADOWED_BY_DOCK:
                observed = ("shadowed: the visible dock overlays the chrome "
                            "(by design) — click lands on the dock, not the control")
            elif action_line:
                observed = f"log: {action_line}"
            elif changed:
                observed = f"state: {changed} widget deltas"
            elif ty in ("TextInput", "Input") and exp.startswith("input."):
                observed = "nothing (focus-only: a live-text binding, not a dispatched action)"
            else:
                observed = "nothing"
            (sdir / "last-click-log.txt").write_text(
                after_log[-4000:])
            observed = observed.replace(os.path.expanduser("~"), "~")
            # The pattern is BUILT from pieces: repo_hermetic.rs fails the
            # build on the machine-path SUBSTRING in tracked source, so neither
            # the scrubber nor its comment may spell it literally.
            observed = re.sub("/" + "Users" + "/.*?(?=[\\s,\"]|$)", "~", observed)
            rows.append([name, ident, ty, json.dumps(rect), text, exp, observed])
    finally:
        subprocess.run(["bash", str(HEADLESS), "stop", str(app_port)],
                       cwd=str(ROOT), capture_output=True, timeout=60)
        if serve is not None:
            serve.terminate()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--app-bin", required=True)
    ap.add_argument("--out", default="docs/walk/click-audit.csv")
    # App port: THIS lane's block is 8380-8389 (docs/harness/GUIDE.md; 8387 is
    # the scenario server). 8376/8377 sit in p0-proto's 8370-8379 block — runs
    # there fail as "app start failed: already taken" whenever that lane is
    # live, which read as a mysterious 0-control screen.
    ap.add_argument("--port", type=int, default=8388)
    ap.add_argument("--only", default="", help="comma-separated screen names")
    args = ap.parse_args()

    SCREENS = [
        {"name": "connect-first-run", "env": {}, "serve": None},
        {"name": "dock-palette", "env": {"OCTOSCODE_SCREEN": "palette"},
         "serve": "conversation", "shadow_chrome": True},
        {"name": "dock-error", "env": {"OCTOSCODE_SCREEN": "error"},
         "serve": "conversation", "shadow_chrome": True},
        {"name": "dock-loading", "env": {"OCTOSCODE_SCREEN": "loading"},
         "serve": "conversation", "shadow_chrome": True},
        {"name": "chrome-review", "env": {}, "serve": "conversation"},
        {"name": "chrome-settings", "env": {}, "serve": "conversation"},
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
    real = [r for r in rows if r[5] and not r[5].startswith("(")]
    unwired = sum(1 for r in rows if r[5] == "(unwired)")
    respond = sum(1 for r in real if r[6].startswith(("log:", "state:")))
    dead = sum(1 for r in real if r[6] == "nothing")
    dismissed = sum(1 for r in real if r[6].startswith("dismissed:"))
    shadowed = sum(1 for r in real if r[6].startswith("shadowed:"))
    dimmed = sum(1 for r in real if r[6].startswith("shadowed: the modal"))
    focus = sum(1 for r in real if r[6].startswith("nothing (focus-only"))
    print(f"SURFACE: {total} controls, {len(real)} with an expected action: "
          f"{respond} respond, {dead} dead, {dismissed} dismissed-by-earlier-click, "
          f"{shadowed} shadowed-by-dock ({dimmed} by the modal dimmer), {focus} focus-only-bindings, "
          f"{unwired} documented-unwired (+{total - len(real) - unwired} unmapped)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
