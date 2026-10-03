#!/usr/bin/env python3
"""A32 — the judge's "below 9" item 1: no single-line title may end in two
dots. A sweep of the ellipsized titles the sidebar rows, the header and the
peer dock draw, across the widths the real UI produces, in the real app
(hidden, isolated state).

The two-dot ellipsis was makepad's (decision D10e,
`patches/makepad/layouter-ellipsis-pen.patch`): a row soft-wrapped at a
space drew its ellipsis one space advance short, over its last glyph. This
walk loads that path where the app draws it:

 btw   (replay_serve --scenario btw, A29's three Sessions)
   desktop, the default window: `/btw` asked in X, a CLICK on Y's row: X's row
     carries the marker chip (A29 steps 1-2) -> the rows; a CLICK on X -> its
     long title in the header; the sidebar collapsed to its rail (CLICK) -> a
     wider header; the rail expanded (CLICK) and Y re-opened -> the rows with
     the chip again. The window's opening animation also sweeps every title's
     bound frame by frame (each geometry is one trace verdict).
   phone, one launch per frame width 300..368 (the drawer is min(320, w - 48)
     px, chrome.rs, so its titles' bound moves across ~70 px): the drawer rows
     with the chip, then X's title in the phone header.
 fleet (replay_serve --scenario fleet --peer-dock, A30's peers)
   three peers from the Fleet pane's production Start (A30's steps): the dock
   rows and the session rows.

Per state, from /snap: every title lies inside the column and overlaps
neither the marker chip nor the time on its row; the dock labels lie inside
the column. Per launch, from the A32 trace instrument when it is compiled
into the host tree (`docs/ux/a32/trace-instrument.diff`; the app appends to
$A32_TRACE_FILE, with A32_TRACE_TEXT='*'): `A32CHECK ok=` for every truncated
text (the ellipsis starts where the last kept glyph ends and ends inside the
label's rect) and `A32KIT fits=` for every text our kit truncated itself
(board3 `fit_w`). Without the instrument the glyph verdicts are reported as
unavailable.

Not used: the makepad tweaker (`/tweak/apply` switches its overlay on, which
panics over the OctosCode module: "value belongs to a different heap"), and
OCTOSENSE_WINDOW_SIZE on desktop (the module then lays out wider than its
window), and `/w?k=maximize` (the hidden window keeps its size). Widths come
from the real UI instead.

usage: OCTOSCODE_APP_BIN=<host octosense> [A32_TRACE_TEXT='*' A32_TRACE_FILE=<file>]
       a32_ellipsis_sweep.py <desktop|phone> <outdir> [btw|fleet|all]
       (ports: A32_PORT, default 8624; A32_REPLAY_PORT, default 8626)
"""
import json
import os
import pathlib
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, inside, overlap, run_session  # noqa: E402  (a10_lib imports bridgeauth: D10c)

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"tmp/a32/sweep/{MODE}"
PHASES = sys.argv[3] if len(sys.argv) > 3 else "all"
PORT = int(os.environ.get("A32_PORT", "8624"))
REPLAY = int(os.environ.get("A32_REPLAY_PORT", "8626"))
TRACE = os.environ.get("A32_TRACE_FILE", "")

# Phone frame widths (the drawer is min(320, w - 48)); None = the default
# window (desktop).
SIZES = {
    "phone": ["300x780", "310x780", "320x780", "330x780", "340x780", "350x780", "360x780", "368x780"],
    "desktop": [None],
}
DOCK_SIZES = {"phone": ["300x780", "330x780", "360x780"], "desktop": [None]}

X = "Fix steer queue drop on reconnect"
Y = "Why is hydrate slow?"
Q1 = "why does redeliver drain the whole queue first?"
SUMMARY: dict = {"mode": MODE, "states": [], "launches": []}
LAUNCH = {"since": 0}


# ------------------------------------------------------------------ helpers
def trace_lines() -> list[str]:
    if not TRACE or not os.path.exists(TRACE):
        return []
    return pathlib.Path(TRACE).read_text(errors="replace").splitlines()


def geometry_key(line: str) -> str:
    """A verdict without the label's position: one per geometry, however
    many frames an animation drew it at."""
    return re.sub(r"label=\[[^,\]]+,[^,\]]+,", "label=[", line)


def verdicts(since: int) -> dict:
    """The trace's verdicts appended since line `since`, one per geometry."""
    lines = trace_lines()[since:]
    checks = sorted({geometry_key(l) for l in lines if l.startswith("A32CHECK")})
    kits = sorted({geometry_key(l) for l in lines if l.startswith("A32KIT")})
    return {
        "truncated": len(checks),
        "bad": [l for l in checks if not l.startswith("A32CHECK ok=true")],
        "kit": len(kits),
        "kit_bad": [l for l in kits if not l.startswith("A32KIT fits=true")],
        "checks": checks,
        "kits": kits,
    }


def on_row(w: Walk, sn: list, wid: str, cy: float) -> list:
    return [s["r"] for s in sn if s.get("i") == wid and w.shown(s) and abs(s["r"][1] + s["r"][3] / 2 - cy) < 14]


def sidebar_rows(w: Walk, sn: list) -> tuple[list, list]:
    """Every shown session-row title, and the ones that leave the column or
    overlap their row's marker chip or time."""
    col = w.rect("threads_column", sn=sn)
    rows, bad = [], []
    for t in (s for s in sn if s.get("i") == "sb_r_title" and w.shown(s)):
        r = t["r"]
        cy = r[1] + r[3] / 2
        side = on_row(w, sn, "sb_r_aside", cy) + on_row(w, sn, "sb_r_time", cy)
        ok = bool(col) and inside(r, col) and not any(overlap(r, o) for o in side) \
            and all(inside(o, col) for o in side)
        rows.append({"title": t.get("t"), "r": r, "chip": bool(on_row(w, sn, "sb_r_aside", cy))})
        if not ok:
            bad.append({"title": t.get("t"), "r": r, "side": side, "col": col})
    return rows, bad


def dock_rows(w: Walk, sn: list) -> tuple[list, list]:
    """The peer dock's row labels: inside the column, not overlapping one
    another."""
    col = w.rect("threads_column", sn=sn)
    labels = [s for s in sn if str(s.get("i", "")).startswith("pd_row_") and s.get("ty") == "Label" and w.shown(s)]
    bad = []
    for a in labels:
        if col and not inside(a["r"], col):
            bad.append({"label": a.get("i"), "t": a.get("t"), "r": a["r"], "col": col})
        for b in labels:
            if a.get("i", "") < b.get("i", "") and overlap(a["r"], b["r"]):
                bad.append({"overlap": [a.get("i"), b.get("i")], "r": [a["r"], b["r"]]})
    return [{"id": a.get("i"), "t": a.get("t"), "r": a["r"]} for a in labels], bad


def measure(w: Walk, state: str, size: str, dock: bool = False, rows_expected: bool = True) -> None:
    """/snap geometry and the trace's verdicts so far this launch, for one
    state; a capture."""
    time.sleep(0.8)
    sn = w.snap()
    rows, bad = sidebar_rows(w, sn)
    dock_labels: list = []
    if dock:
        dock_labels, dbad = dock_rows(w, sn)
        bad += dbad
    head = next((s for s in sn if s.get("i") == "hd_title" and w.shown(s)), None)
    hd_left = w.rect("hd_left", sn=sn)
    if head and hd_left and not inside(head["r"], hd_left):
        bad.append({"hd_title": head["r"], "hd_left": hd_left})
    v = verdicts(LAUNCH["since"])
    entry = {"state": state, "size": size, "rows": rows, "dock": dock_labels,
             "hd_title": head and {"t": head.get("t"), "r": head["r"]}, "hd_left": hd_left,
             "geometry_bad": bad}
    SUMMARY["states"].append(entry)
    what = f"{len(rows)} rows" + (f", {len(dock_labels)} dock labels" if dock else "") + (", the header" if head else "")
    w.check(f"{size} {state}: titles inside their rows, clear of the chip and the time ({what})",
            not bad and (bool(rows) or not rows_expected), json.dumps(bad[:3]))
    if TRACE:
        w.check(f"{size} {state}: every truncated text so far this launch starts its ellipsis at the kept text's end "
                f"and ends it inside its label ({v['truncated']} geometries)", not v["bad"], "; ".join(v["bad"][:2]))
        w.check(f"{size} {state}: every kit-truncated text so far this launch fits its label ({v['kit']})",
                not v["kit_bad"], "; ".join(v["kit_bad"][:2]))
    w.dismiss_keyboard()
    w.shot(f"{w.mode}-{state}-{size}")


def sidebar_open(w: Walk) -> None:
    if w.mode == "phone" and not w.visible("sb_new_chat_hit"):
        w.dismiss_keyboard()
        w.click("sidebar_toggle_hit")
        w.wait(lambda: bool(w.visible("sb_new_chat_hit")), 5)


def sidebar_close(w: Walk) -> None:
    if w.mode == "phone" and w.visible("sb_new_chat_hit"):
        w.click("drawer_close")
        w.wait(lambda: not w.visible("sb_new_chat_hit"), 5)
        time.sleep(0.4)


def open_row(w: Walk, title: str) -> bool:
    sidebar_open(w)
    for s in w.snap():
        if s.get("i") == "sb_r_title" and w.shown(s) and s.get("t") == title:
            x, y, ww, h = s["r"]
            w.click_xy(x + ww / 2, y + h / 2)
            return w.wait(lambda: w.text("hd_title") == title, 8)
    return False


def send(w: Walk, text: str) -> bool:
    comp = w.composer()
    if comp is None:
        return False
    x, y, ww, h = comp["r"]
    w.click_xy(x + ww / 2, y + h / 2)
    w.clear_field()
    w.type_text(text)
    time.sleep(0.3)
    w.key("Return")
    time.sleep(0.4)
    return True


def ask_aside_then_switch(w: Walk, size: str) -> None:
    """A29 steps 1-2 by CLICKS: /btw in X, then Y opened from its row."""
    w.check(f"{size}: the composer is on screen", w.wait(lambda: w.composer() is not None, 20))
    w.check(f"{size}: /btw asked in X (the panel shows)",
            send(w, f"/btw {Q1}") and w.wait(lambda: bool(w.visible("btw_aside")), 8))
    w.check(f"{size}: CLICK Y's row -> the header names Y", open_row(w, Y), repr(w.text("hd_title")))
    sidebar_open(w)
    w.check(f"{size}: X's row carries the /btw chip", w.wait(lambda: bool(w.visible("sb_r_aside")), 6))


# ---------------------------------------------------------------- phases
def btw_desktop(size: str):
    def phase(w: Walk) -> None:
        ask_aside_then_switch(w, size)
        measure(w, "rows-chip", size)
        w.check(f"{size}: CLICK X's row -> the header names X", open_row(w, X), repr(w.text("hd_title")))
        measure(w, "header-x", size)
        w.check(f"{size}: CLICK collapse -> the sidebar rail", w.click("sidebar_collapse")
                and w.wait(lambda: bool(w.visible("oc_sidebar_rail")), 6))
        measure(w, "header-x-rail", size, rows_expected=False)
        w.check(f"{size}: CLICK the rail's expand -> the sidebar", w.click("rb_hit", 0)
                and w.wait(lambda: bool(w.visible("sb_new_chat_hit")), 6))
        w.check(f"{size}: CLICK Y's row again", open_row(w, Y), repr(w.text("hd_title")))
        measure(w, "rows-chip-again", size)
    return phase


def btw_phone(size: str):
    def phase(w: Walk) -> None:
        ask_aside_then_switch(w, size)
        measure(w, "rows-chip", size)
        w.check(f"{size}: CLICK X's row -> the header names X", open_row(w, X), repr(w.text("hd_title")))
        sidebar_close(w)
        measure(w, "header-x", size, rows_expected=False)
    return phase


def fleet_at(size: str):
    def phase(w: Walk) -> None:
        import a30_peer_dock as P  # noqa: PLC0415 — A30's production Start steps

        P.drawer_open(w)
        w.click("fleet_nav_hit")
        w.check(f"{size}: the footer's Fleet entry opens the pane", w.wait_shown("b3_fleet_panel", 10))
        started = all(P.fleet_start(w, n) for n in range(3))
        w.check(f"{size}: three peers started from the pane", started, f"{len(P.dispatches(w))} dispatches")
        P.seek(w, "b3_fleet_back")
        w.click("b3_fleet_back")
        w.wait(lambda: not w.visible("b3_fleet_panel"), 8)
        time.sleep(2.5)
        P.drawer_open(w)
        if w.mode == "phone" and w.wait_shown("pd_pill", 6) and not w.visible("pd_row_0_label"):
            w.click("pd_pill")
        w.check(f"{size}: the dock rows show", w.wait_shown("pd_row_0_label", 8))
        measure(w, "peer-dock", size, dock=True, rows_expected=False)
    return phase


def launch(phase, name: str, size, scenario: str, replay_args: list) -> int:
    LAUNCH["since"] = len(trace_lines())
    label = size or "default"
    env = {"OCTOSENSE_WINDOW_SIZE": size} if size else None
    rc = run_session(phase(label), mode=MODE, outdir=os.path.join(OUT, f"{name}-{label}"), port=PORT,
                     replay_port=REPLAY, scenario=scenario, env=env, replay_args=replay_args)
    v = verdicts(LAUNCH["since"])
    SUMMARY["launches"].append({"phase": name, "size": label, "rc": rc, "truncated_geometries": v["truncated"],
                                "two_dot_or_outside": v["bad"], "kit_geometries": v["kit"],
                                "kit_clipped": v["kit_bad"], "checks": v["checks"], "kits": v["kits"]})
    return rc


def main() -> int:
    os.makedirs(OUT, exist_ok=True)
    rc = 0
    if PHASES in ("btw", "all"):
        for size in SIZES[MODE]:
            rc |= launch(btw_desktop if MODE == "desktop" else btw_phone, "btw", size, "btw",
                         ["--adopt-turn-ids", "--delay-ms", "40", "--btw-delay-ms", "60000"])
    if PHASES in ("fleet", "all"):
        for size in DOCK_SIZES[MODE]:
            rc |= launch(fleet_at, "fleet", size, "fleet", ["--peer-dock"])
    launches = SUMMARY["launches"]
    SUMMARY["totals"] = {
        "launches": len(launches),
        "states": len(SUMMARY["states"]),
        "rows_measured": sum(len(s["rows"]) + len(s["dock"]) for s in SUMMARY["states"]),
        "geometry_bad": sum(len(s["geometry_bad"]) for s in SUMMARY["states"]),
        "truncated_geometries": sum(l["truncated_geometries"] for l in launches),
        "two_dot_or_outside": sum(len(l["two_dot_or_outside"]) for l in launches),
        "kit_geometries": sum(l["kit_geometries"] for l in launches),
        "kit_clipped": sum(len(l["kit_clipped"]) for l in launches),
        "trace": bool(TRACE),
    }
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump(SUMMARY, f, indent=1, ensure_ascii=False)
    print(f"== A32 ellipsis sweep {MODE}: {json.dumps(SUMMARY['totals'])} {'PASS' if rc == 0 else 'FAIL'}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
