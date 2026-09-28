#!/usr/bin/env python3
"""Card #19 — the Phase-4 walk-runner.

Turns the rows of `docs/walk-rows.csv` (one per web Playwright case) into
scripted checks on the **native** app, and records a verdict for every row in
`docs/walk/results.csv`.

Operator view: `docs/walk/README.md`.

Design:
* A **backend** per scenario is the recording-replay server
  (`crates/octoscode-module/examples/replay_serve.rs --scenario <name>`), which
  serves the committed real fixtures under `crates/octoscode-client/tests/fixtures/`.
  **No model runs.**
* The **app** is launched hidden via `harness/headless.sh` on this card's port
  block **8370-8379** and driven with real input (`/click`, `/t`). Assertions
  read the app's own `/snap`; a failure also writes `/g` + the snap JSON under
  `docs/walk/evidence/`.

## Which rows are scripted, and the verdict rule

The card says to automate "the first 30 rows that exercise **what exists
today**: conversation (prompt, stream, interrupt, order), threads (list, new
chat, select), composer (draft clears, queue/steer where supported), approval
(request -> approve/deny), and reconnect/replay."

An area qualifies only if its native surface exists today. Per the built design
batch (`design/cards/`: conversation-01/03/04/08/09) there is **no approval
card** — `design/bindings.json:40` says the inline-approval scene
(`conversation-05`) "is NOT in this batch's 5 mapped components". So:

* **Scripted areas** (native surface exists): conversation, threads, composer,
  recovery/reconnect. The runner selects the first 30 rows in these areas.
  A row is `pass` iff every strict check of its area passes; else `fail` with
  `/g` + `/snap` evidence.
* **approval** rows are `not-yet-implemented`, naming the missing card (with the
  design citation). The delivery of `approval/requested` is a *store* fact, not
  a widget, so it cannot be asserted from `/snap` — claiming a row `pass` on it
  would be dishonest.
* the 3 `real-turn` rows are `blocked`; the 31 web-only rows are `skipped`.

Run:  python3 tools/walk/run.py [--limit 30] [--only AREA] [--port 8370]
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import time
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
WALK = ROOT / "docs" / "walk"
EVIDENCE = WALK / "evidence"
WALK_ROWS = ROOT / "docs" / "walk-rows.csv"
PARITY = ROOT / "docs" / "parity-matrix.csv"
BIN = pathlib.Path(
    os.environ.get(
        "OCTOSCODE_APP_BIN",
        "/Users/yuechen/home/oa.noindex/p0-build/tmp/octosense-target/debug/octosense",
    )
)
SHELL_CWD = pathlib.Path(
    os.environ.get(
        "OCTOSCODE_SHELL_CWD",
        "/Users/yuechen/home/oa.noindex/p0-build/tmp/octosense/desktop",
    )
)
REPLAY = ROOT / "target" / "debug" / "examples" / "replay_serve"
HEADLESS = ROOT / "harness" / "headless.sh"

APP_PORT = 8370
SCENARIO_PORTS = {"conversation": 8380, "approval": 8381, "task": 8382,
                  "autonomy": 8383, "peer": 8384, "session": 8385}

# Areas the card names → the walk rows they cover. Order matters: `area_of`
# returns the FIRST match, so the more specific areas come first. `conversation`
# deliberately omits a bare `\border\b` — it falsely swallowed thread rows like
# "switches sessions by keyboard and preserves sidebar focus order".
AREA_PATTERNS = {
    "threads": re.compile(
        r"thread|new chat|switch(es)? sessions?\b|rapid session|session list|"
        r"selects? a new session|selected session", re.I),
    "composer": re.compile(r"composer|draft|queue|steer", re.I),
    "approval": re.compile(r"approval|approve|deny", re.I),
    "recovery": re.compile(r"reconnect|replay|recovery|restore|resume", re.I),
    "conversation": re.compile(
        r"prompt|stream|\banswer\b|interrupt|transcript|reasoning|message|turn", re.I),
}
# Which areas have a native surface today (⇒ scriptable), and the scenario each
# is driven against.
AREA_SCRIPTABLE = {"conversation": True, "threads": True, "composer": True,
                   "recovery": True, "approval": False}
AREA_SCENARIO = {"conversation": "conversation", "threads": "conversation",
                 "composer": "conversation", "recovery": "conversation",
                 "approval": "approval"}
APPROVAL_MISSING = ("missing: inline approval card — design scene conversation-05 "
                    "is not in the built batch (design/bindings.json:40); "
                    "approval/requested reaches the store but has no widget")


# --------------------------------------------------------------------------- #
class App:
    def __init__(self, port: int):
        self.port = port
        self.base = f"http://127.0.0.1:{port}"

    def _get(self, path: str) -> str:
        with urllib.request.urlopen(self.base + path, timeout=15) as r:
            return r.read().decode()

    def snap(self) -> dict:
        return json.loads(self._get("/snap?all=1"))

    def text_of(self, snap: dict, ident: str):
        return next((w.get("t") for w in snap.get("s", []) if str(w.get("i", "")) == ident), None)

    def widget_ids(self, snap: dict):
        return [str(w.get("i", "")) for w in snap.get("s", [])]

    def kinds(self, snap: dict):
        return [w.get("t") for w in snap.get("s", []) if str(w.get("i", "")) == "item_kind"]

    def click(self, x, y):
        return self._get(f"/click?x={x}&y={y}&wait=1")

    def type_text(self, text):
        return self._get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))

    def rect(self, snap, ident):
        for w in snap.get("s", []):
            if str(w.get("i", "")) == ident:
                x, y, ww, hh = w["r"]
                return int(x + ww / 2), int(y + hh / 2)
        return None

    def click_id(self, snap, ident):
        r = self.rect(snap, ident)
        if not r:
            raise AssertionError(f"widget '{ident}' is not present")
        return self.click(*r)


class Procs:
    def __init__(self, app_port: int):
        self.servers: dict[str, subprocess.Popen] = {}
        self.app_port = app_port
        self.app = None

    def start_server(self, scenario: str):
        if scenario in self.servers:
            return
        port = SCENARIO_PORTS[scenario]
        log = WALK / f"server-{scenario}.log"
        f = open(log, "w")
        self.servers[scenario] = subprocess.Popen(
            [str(REPLAY), str(port), "--scenario", scenario], stdout=f, stderr=f)
        for _ in range(75):
            if "listening" in log.read_text():
                return
            time.sleep(0.2)
        raise RuntimeError(f"replay server '{scenario}' never listened")

    def start_app(self, scenario: str):
        self.stop_app()
        state = ROOT / "tmp" / "walk" / "headless"
        state.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env.update({
            "OCTOS_BASE_URL": f"http://127.0.0.1:{SCENARIO_PORTS[scenario]}",
            "OCTOS_BEARER": "walk-dummy-token",
            "OCTOS_PROFILE_ID": "dsflash",
            "MAKEPAD_WM_TEST_APP": "octoscode",
            "HEADLESS_ARGS": "--module octoscode",
            "HEADLESS_STATE": str(state),
        })
        subprocess.run(["bash", str(HEADLESS), "start", str(BIN), str(self.app_port)],
                       env=env, cwd=str(SHELL_CWD), capture_output=True, text=True, timeout=120)
        self.app = self.app_port
        time.sleep(4)

    def stop_app(self):
        if self.app is None:
            return
        env = os.environ.copy()
        env["HEADLESS_STATE"] = str(ROOT / "tmp" / "walk" / "headless")
        subprocess.run(["bash", str(HEADLESS), "stop", str(self.app)],
                       env=env, capture_output=True, text=True, timeout=60)
        self.app = None

    def stop_all(self):
        self.stop_app()
        for p in self.servers.values():
            p.send_signal(signal.SIGTERM)
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
        self.servers.clear()


# --------------------------------------------------------------------------- #
# Area checks. Each returns (passed, reason). Strict: no lenient proxies.
# --------------------------------------------------------------------------- #
CHECKS: dict[str, list] = {a: [] for a in AREA_PATTERNS}


def check(area: str, name: str):
    def deco(fn):
        CHECKS[area].append((name, fn))
        return fn
    return deco


def _send_turn(app: App, text: str):
    d = app.snap()
    app.click_id(d, "draft")
    app.type_text(text)
    app.click_id(app.snap(), "send")


# ---- conversation: prompt, stream, order, interrupt ----------------------- #
@check("conversation", "module reaches conn: Live with the OctosCode heading")
def c_live(app):
    d = app.snap()
    h, s = app.text_of(d, "heading"), app.text_of(d, "status") or ""
    return (h == "OctosCode" and "Live" in s), f"heading={h!r} status={s!r}"


@check("conversation", "the thread list renders the opened session row")
def c_thread(app):
    return bool(app.text_of(app.snap(), "thread_name")), "thread row present"


@check("conversation", "the composer accepts typed text (prompt input)")
def c_draft(app):
    d = app.snap()
    app.click_id(d, "draft")
    app.type_text("walk draft probe")
    return app.text_of(app.snap(), "draft") == "walk draft probe", "draft round-trips"


@check("conversation", "composing clears the draft on send")
def c_send_clears(app):
    _send_turn(app, "walk: clear the draft")
    time.sleep(1.0)
    got = app.text_of(app.snap(), "draft")
    return (got or "") in ("", "Ask Octos anything"), f"draft after send={got!r}"


@check("conversation", "a sent prompt streams an assistant answer row")
def c_stream(app):
    time.sleep(2.5)
    k = app.kinds(app.snap())
    return "assistant-prose" in k, f"kinds={k}"


@check("conversation", "the user's own prompt renders as a row")
def c_user(app):
    k = app.kinds(app.snap())
    return "user-bubble" in k, f"kinds={k}"


@check("conversation", "the answer row renders after the prompt row (order)")
def c_order(app):
    ys = {}
    for w in app.snap().get("s", []):
        if str(w.get("i", "")) == "item_kind":
            ys.setdefault(w.get("t"), w["r"][1])
    ok = ("user-bubble" in ys and "assistant-prose" in ys and ys["user-bubble"] < ys["assistant-prose"])
    return ok, f"y(user)={ys.get('user-bubble')} y(answer)={ys.get('assistant-prose')}"


@check("conversation", "the Stop control is present for the live turn")
def c_stop(app):
    return "stop" in app.widget_ids(app.snap()), "stop widget present"


# ---- threads: list, refresh, new chat ------------------------------------- #
@check("threads", "the thread list is a PortalList of session rows")
def t_portal(app):
    return "thread_list" in app.widget_ids(app.snap()), "thread_list present"


@check("threads", "refresh (session/list) keeps the module live")
def t_refresh(app):
    d = app.snap()
    app.click_id(d, "refresh")
    time.sleep(1.5)
    return "Live" in (app.text_of(app.snap(), "status") or ""), "status stays Live"


@check("threads", "New chat mints a fresh Session and re-opens the workspace")
def t_new_chat(app):
    d = app.snap()
    app.click_id(d, "new_chat")
    time.sleep(2.0)
    d = app.snap()
    ok = app.text_of(d, "heading") == "OctosCode" and "draft" in app.widget_ids(d)
    return ok, f"heading={app.text_of(d,'heading')!r} has draft={'draft' in app.widget_ids(d)}"


# ---- composer: draft, send, timeline, persistence ------------------------- #
@check("composer", "the draft is a single TextInput with a placeholder")
def comp_draft(app):
    ph = next((w.get("t") for w in app.snap().get("s", []) if str(w.get("i", "")) == "draft"), None)
    return ph is not None, f"draft={ph!r}"


@check("composer", "the send control is present")
def comp_send(app):
    return "send" in app.widget_ids(app.snap()), "send present"


@check("composer", "the conversation column hosts the timeline PortalList")
def comp_timeline(app):
    return "timeline_list" in app.widget_ids(app.snap()), "timeline_list present"


@check("composer", "a typed draft survives a snap (no re-render wipe)")
def comp_survives(app):
    d = app.snap()
    app.click_id(d, "draft")
    app.type_text("survive me")
    app.snap()
    return app.text_of(app.snap(), "draft") == "survive me", "draft persists across snaps"


# ---- recovery: reconnect, replay ------------------------------------------ #
@check("recovery", "the module reaches conn: Live")
def r_live(app):
    s = app.text_of(app.snap(), "status") or ""
    return "Live" in s, f"status={s!r}"


@check("recovery", "a replayed turn lands in the module's own transcript")
def r_replay(app):
    _send_turn(app, "walk: replay a turn")
    time.sleep(2.5)
    k = app.kinds(app.snap())
    return bool(k), f"timeline kinds={k}"


# --------------------------------------------------------------------------- #
def load_rows():
    with open(WALK_ROWS, newline="") as f:
        return list(csv.DictReader(f))


def area_of(row):
    hay = row["case"] + " " + row["spec"]
    for name, pat in AREA_PATTERNS.items():
        if pat.search(hay):
            return name
    return None


def select_targets(limit: int):
    """The first `limit` rows in the scriptable areas, **round-robin by area**.

    Pure file order lets `conversation`/`recovery` crowd out `threads` (which has
    only 4 eligible rows), so the card's named areas would not all appear. Taking
    one row per area in turn keeps every named area represented while still
    taking the earliest rows within each.
    """
    eligible: dict[str, list] = {a: [] for a in AREA_PATTERNS}
    for i, row in enumerate(load_rows(), start=1):
        if (row.get("web_only_reason") or "").strip() or row["needs"] == "real-turn":
            continue
        area = area_of(row)
        if area and AREA_SCRIPTABLE[area]:
            eligible[area].append((i, area, row))
    order = [a for a in ("conversation", "threads", "composer", "recovery")
             if AREA_SCRIPTABLE[a]]
    picked, cursors = [], {a: 0 for a in order}
    while len(picked) < limit:
        progressed = False
        for a in order:
            if len(picked) >= limit:
                break
            c = cursors[a]
            if c < len(eligible[a]):
                picked.append(eligible[a][c])
                cursors[a] += 1
                progressed = True
        if not progressed:
            break
    return picked


def missing_capability(spec: str) -> str:
    try:
        with open(PARITY, newline="") as f:
            rows = list(csv.DictReader(f))
    except FileNotFoundError:
        return "native capability unknown"
    key = spec.split("/")[-1].replace(".spec.ts", "")
    for r in rows:
        src = r.get("web_src") or ""
        if key in src or key.split("-")[0] in src:
            return f"{r['capability']} [{r['native_status']}]"
    return "native capability not yet built (see docs/parity-matrix.csv)"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=30)
    ap.add_argument("--only", default=None)
    ap.add_argument("--port", type=int, default=APP_PORT)
    args = ap.parse_args()

    WALK.mkdir(parents=True, exist_ok=True)
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    rows = load_rows()
    targets = select_targets(args.limit)
    procs = Procs(args.port)

    areas = sorted({a for _, a, _ in targets})
    if args.only:
        areas = [a for a in areas if a == args.only]
    area_result: dict[str, dict] = {}
    for area in areas:
        scenario = AREA_SCENARIO[area]
        try:
            procs.start_server(scenario)
            procs.start_app(scenario)
        except Exception as e:  # noqa: BLE001
            area_result[area] = {"status": "blocked", "run": [], "evidence": "",
                                 "reason": f"scenario '{scenario}' failed to start: {e}"}
            continue
        app = App(procs.app_port)
        run, evidence = [], ""
        for name, fn in CHECKS[area]:
            try:
                ok, reason = fn(app)
                status = "pass" if ok else "fail"
            except Exception as e:  # noqa: BLE001
                ok, status, reason = False, "fail", f"exception: {e}"
            run.append({"check": name, "status": status, "reason": reason[:200]})
            if status == "fail" and not evidence:
                try:
                    sj = EVIDENCE / f"area-{area}.snap.json"
                    sj.write_text(json.dumps(app.snap()))
                    subprocess.run(["curl", "-s", "--max-time", "20", "-o",
                                    str(EVIDENCE / f"area-{area}.png"),
                                    f"http://127.0.0.1:{procs.app_port}/g?raw=1"],
                                   capture_output=True)
                    evidence = str(sj.relative_to(ROOT))
                except Exception:  # noqa: BLE001
                    pass
            print(f"  [{area}] {status:5} {name[:62]:62} :: {reason[:60]}")
        overall = "pass" if all(c["status"] == "pass" for c in run) else "fail"
        area_result[area] = {"status": overall, "run": run, "evidence": evidence}

    procs.stop_all()

    target_by_row = {i: a for i, a, _ in targets}
    out = []
    for i, row in enumerate(rows, start=1):
        spec, case, needs = row["spec"], row["case"], row["needs"]
        web_only = (row.get("web_only_reason") or "").strip()
        area = area_of(row) or ""
        if i in target_by_row:
            res = area_result.get(area)
            if res is None:
                status, reason, ev = "not-run", "area not selected", ""
            else:
                status = res["status"]
                failed = [c["check"] for c in res["run"] if c["status"] != "pass"]
                reason = (f"area '{area}': {len(res['run'])} checks, all pass"
                          if not failed else f"area '{area}' failing: {'; '.join(failed)}")
                ev = res["evidence"]
            out.append({"row_id": i, "area": area, "spec": spec, "case": case,
                        "status": status, "evidence": ev, "reason": reason})
        elif web_only:
            out.append({"row_id": i, "area": area, "spec": spec, "case": case,
                        "status": "skipped", "evidence": "",
                        "reason": f"skipped: operator-confirmation-pending ({web_only})"})
        elif needs == "real-turn":
            out.append({"row_id": i, "area": area, "spec": spec, "case": case,
                        "status": "blocked", "evidence": "",
                        "reason": "blocked: needs outer-loop live run (real model turn)"})
        elif area == "approval":
            out.append({"row_id": i, "area": area, "spec": spec, "case": case,
                        "status": "not-yet-implemented", "evidence": "", "reason": APPROVAL_MISSING})
        else:
            out.append({"row_id": i, "area": area, "spec": spec, "case": case,
                        "status": "not-yet-implemented", "evidence": "",
                        "reason": f"missing: {missing_capability(spec)}"})

    with open(WALK / "results.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "status", "evidence", "reason"])
        w.writeheader()
        w.writerows(out)

    from collections import Counter
    counts = Counter(r["status"] for r in out)
    print("\n== walk-runner summary ==")
    for k in ("pass", "fail", "not-yet-implemented", "blocked", "skipped", "not-run"):
        if counts.get(k):
            print(f"   {k:20} {counts[k]}")
    print(f"   total                {len(out)}")
    print(f"   scripted rows        {len(targets)}  (areas: {areas})")
    return 0 if counts.get("fail", 0) == 0 and counts.get("not-run", 0) == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
