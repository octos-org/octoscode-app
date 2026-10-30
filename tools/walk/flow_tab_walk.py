#!/usr/bin/env python3
"""The Flow tab click walk (the session and the work it sent out, as cards;
`screens/flow_graph.rs`, `flow_canvas.rs`).

Against `replay_serve --scenario fleet --peer-dock` (the faithful external-
driver fixture the A30 walk uses; no model is called), every control is
reached by a click or a drag at its laid-out rect:
  * no peers: the header shows no Flow tab;
  * three peers started from the Fleet pane's production Start: the Flow tab
    appears; a click shows the canvas in place of the transcript and the
    composer, with the session's card and one card per peer in the Fleet's
    words ("Peer N · model", the status word);
  * a click on a peer's card opens the Fleet pane; on the session's card,
    the Chat tab;
  * a drag moves a card and the place is kept (the layout file names it);
  * the Chat tab brings the transcript and the composer back;
  * after a switch to the dark palette the Flow still draws its cards.
usage: OCTOSCODE_APP_BIN=<octoscode or host octosense> flow_tab_walk.py <outdir>
"""
import json
import os
import pathlib
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, run_session  # noqa: E402
from a30_peer_dock import fleet_start, seek  # noqa: E402
import a26_lib as L  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast,
# never imported). The Flow tab is native-only: no parity row to claim.
WALK = {
    "name": "flow_tab",
    "title": "Flow tab: hidden with nothing sent out; the session and its peers as cards in the Fleet's words; "
             "a drag keeps a card's place; a click opens a peer's Fleet pane or the session's chat; the dark palette",
    "modes": ["desktop"],
    "app": "self",
    "runs": [{"argv": ["{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "OCTOSCODE_APP_BIN": "{bin}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
}


def titles(W: Walk) -> list[str]:
    return [w.get("t") or "" for w in W.visible("title")]


def card_rect(W: Walk, prefix: str):
    """The rect of the first card title starting with `prefix`."""
    for w in W.visible("title"):
        if (w.get("t") or "").startswith(prefix):
            return w["r"]
    return None


def drag(W: Walk, x: float, y: float, dx: float, dy: float, steps: int = 8) -> None:
    W.get(f"/m?k=down&x={x:.1f}&y={y:.1f}&wait=1")
    for i in range(1, steps + 1):
        W.get(f"/m?k=move&x={x + dx * i / steps:.1f}&y={y + dy * i / steps:.1f}&wait=1")
    W.get(f"/m?k=up&x={x + dx:.1f}&y={y + dy:.1f}&wait=1")


def walk(W: Walk) -> None:
    W.note("== 1. no peers: no Flow tab")
    W.check("header: no Flow tab while the session sent nothing out", not W.visible("hd_tab_flow"))

    W.note("== 2. three peers from the Fleet's production Start")
    W.click("fleet_nav_hit")
    W.check("fleet: the footer's Fleet entry opens the pane", W.wait_shown("b3_fleet_panel", 10))
    started = all(fleet_start(W, n) for n in range(3))
    W.check("fleet: three Starts dispatched", started)
    seek(W, "b3_fleet_back")
    W.click("b3_fleet_back")
    W.wait(lambda: not W.visible("b3_fleet_panel"), 8)

    W.note("== 3. the Flow tab")
    W.check("header: the Flow tab appears once a peer was sent", W.wait_shown("hd_tab_flow", 10))
    W.click("hd_tab_flow_hit")
    W.check("flow: the canvas replaces the transcript and the composer",
            W.wait_shown("flow_view", 8) and not W.visible("conversation_inner") and not W.visible("composer_dock"))
    W.wait(lambda: len([t for t in titles(W) if t.startswith("Peer ")]) >= 3, 10)
    t = titles(W)
    peers = sorted(x for x in t if x.startswith("Peer "))
    W.check("flow: one card per peer, in the Fleet's words", len(peers) == 3, f"{t}")
    statuses = [w.get("t") or "" for w in W.visible("status")]
    W.check("flow: the cards carry the Fleet's status words",
            any("Working" in s or "Waiting" in s or "Starting" in s for s in statuses), f"{statuses}")
    W.check("flow: the session's card is drawn too", len(t) >= 4, f"{t}")
    W.shot("flow-canvas")

    W.note("== 4. a drag moves a card and the place is kept")
    r = card_rect(W, "Peer 1")
    layout = pathlib.Path(os.environ["FLOW_LAYOUT_FILE"])
    if r:
        x, y = r[0] + r[2] / 2, r[1] + r[3] / 2
        drag(W, x, y, 40, 120)
        time.sleep(0.6)
        r2 = card_rect(W, "Peer 1")
        moved = bool(r2) and abs((r2[1] - r[1]) - 120) < 3 and abs((r2[0] - r[0]) - 40) < 3
        W.check("flow: the dragged card follows the pointer", moved, f"{r} -> {r2}")
        saved = json.loads(layout.read_text()) if layout.exists() else {}
        W.check("flow: the place is written to the layout file", any(k.startswith("p:") for k in saved), f"{saved}")
        W.shot("flow-dragged")
    else:
        W.check("flow: a peer card to drag", False, f"{titles(W)}")

    W.note("== 5. clicks open what a card stands for")
    r = card_rect(W, "Peer 2")
    if r:
        W.mark()
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.check("flow: a peer's card opens the Fleet pane", W.wait_shown("b3_fleet_panel", 8))
        seek(W, "b3_fleet_back")
        W.click("b3_fleet_back")
        W.wait(lambda: not W.visible("b3_fleet_panel"), 8)
    session = next((w for w in W.visible("title") if not (w.get("t") or "").startswith("Peer ")), None)
    if session:
        r = session["r"]
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.check("flow: the session's card goes back to its chat",
                W.wait_shown("conversation_inner", 8) and not W.visible("flow_view") and W.wait_shown("composer_dock", 4))
    else:
        W.check("flow: a session card", False, f"{titles(W)}")

    W.note("== 6. the tabs switch back and forth")
    W.click("hd_tab_flow_hit")
    W.check("flow: back on the Flow tab", W.wait_shown("flow_view", 8))
    W.click("hd_tab_chat_hit")
    W.check("chat: the Chat tab shows the transcript and the composer again",
            W.wait_shown("conversation_inner", 8) and W.wait_shown("composer_dock", 4))

    W.note("== 7. the dark palette (a re-theme re-applies the DSL)")
    opened = L.open_settings_from_footer(W) and L.open_preferences(W)
    hits = sorted((x["r"] for x in W.snap() if x.get("i") == "pl_hit" and W.shown(x)), key=lambda r: r[1])
    dark = L.PALETTES.index("dark")
    if opened and len(hits) > dark:
        r = hits[dark]
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.check("theme: the Dark row re-themes the app", W.wait(lambda: W.text("sb_theme_label") == "Dark", 8))
        L.close_settings(W)
        time.sleep(0.6)
        W.click("hd_tab_flow_hit")
        W.check("flow (dark): the canvas still draws every card",
                W.wait_shown("flow_view", 8) and W.wait(lambda: len([t for t in titles(W) if t.startswith("Peer ")]) == 3, 8),
                f"{titles(W)}")
        W.shot("flow-dark")
    else:
        W.check("theme: the palette selector opens", False, f"opened={opened} rows={len(hits)}")


def main() -> int:
    out = sys.argv[1] if len(sys.argv) > 1 else "tmp/flow-tab-walk"
    pathlib.Path(out).mkdir(parents=True, exist_ok=True)
    layout = pathlib.Path(out).resolve() / "flow-layout-v1.json"
    layout.unlink(missing_ok=True)
    os.environ["FLOW_LAYOUT_FILE"] = str(layout)
    return run_session(walk, mode="desktop", outdir=out, scenario="fleet", replay_args=["--peer-dock"],
                       env={"OCTOSCODE_FLOW_LAYOUT_FILE": str(layout)})


if __name__ == "__main__":
    sys.exit(main())
