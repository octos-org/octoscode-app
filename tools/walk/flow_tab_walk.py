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
  * a drag moves a card and the place is kept (the layout file names it); no
    default card ends up under the dragged one; a card dragged over another
    is drawn on top of it, and a click there opens the dragged card;
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


CARD = {"Peer ": (260.0, 92.0)}
SESSION = (240.0, 92.0)
# A card's title sits inside its padding (flow_canvas.rs: left 12, top 10).
PAD = (12.0, 10.0)


def cards(W: Walk) -> list[tuple[str, list[float]]]:
    """Every card as (title, [x, y, w, h]), from its title's rect and the
    card's laid-out size (the snap has the labels, not the card itself)."""
    out = []
    for w in W.visible("title"):
        t = w.get("t") or ""
        size = CARD["Peer "] if t.startswith("Peer ") else SESSION
        out.append((t, [w["r"][0] - PAD[0], w["r"][1] - PAD[1], size[0], size[1]]))
    return out


def overlaps(a, b) -> bool:
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def covered(cs) -> list[str]:
    return [f"{a[0]}/{b[0]}" for i, a in enumerate(cs) for b in cs[i + 1:] if overlaps(a[1], b[1])]


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
        cs = cards(W)
        W.check("flow: no card sits under another after the drag", len(cs) >= 4 and not covered(cs),
                f"{covered(cs)} {cs}")
    else:
        W.check("flow: a peer card to drag", False, f"{titles(W)}")

    W.note("== 4b. a card dragged over another is drawn on top and takes the click")
    # Both are placed by the person (a card with no place keeps clear of a
    # placed one), so the one dragged last must be the one drawn on top.
    cs = cards(W)
    sess = next((c for t, c in cs if not t.startswith("Peer ")), None)
    if sess:
        drag(W, sess[0] + sess[2] / 2, sess[1] + sess[3] / 2, 0, 24)
        time.sleep(0.6)
    cs = cards(W)
    peer1 = next((c for t, c in cs if t.startswith("Peer 1")), None)
    sess = next((c for t, c in cs if not t.startswith("Peer ")), None)
    if peer1 and sess:
        # Peer 1's left edge 24 over the session card's right edge.
        gx, gy = peer1[0] + peer1[2] / 2, peer1[1] + peer1[3] / 2
        drag(W, gx, gy, (sess[0] + sess[2] - 24) - peer1[0], (sess[1] + 30) - peer1[1])
        time.sleep(0.6)
        cs = cards(W)
        p1 = next((c for t, c in cs if t.startswith("Peer 1")), None)
        s1 = next((c for t, c in cs if not t.startswith("Peer ")), None)
        W.shot("flow-overlap")
        W.check("flow: the dragged card now covers the session card's edge", bool(p1 and s1) and overlaps(p1, s1),
                f"{p1} {s1}")
        others = [c for c in cs if not c[0].startswith("Peer 1") and c[0].startswith("Peer ")]
        W.check("flow: the unplaced peers stay clear of the placed one", all(not overlaps(p1, c[1]) for c in others),
                f"{p1} {others}")
        if p1 and s1:
            W.click_xy(s1[0] + s1[2] - 12, s1[1] + 50)  # inside both cards
            W.check("flow: a click where two cards meet opens the one drawn on top (the dragged peer, not the session)",
                    W.wait_shown("b3_fleet_panel", 8) and not W.visible("conversation_inner"))
            seek(W, "b3_fleet_back")
            W.click("b3_fleet_back")
            W.wait(lambda: not W.visible("b3_fleet_panel"), 8)
        # And the other way round: the session card, drawn first by default,
        # grabbed now, is on top of the peer it overlaps.
        # (The peers renumber after the Fleet pane: the one under the session's
        # edge is the one dragged there, whatever it is now called.)
        cs = cards(W)
        s1 = next((c for t, c in cs if not t.startswith("Peer ")), None)
        p1 = next((c for t, c in cs if t.startswith("Peer ") and s1 and overlaps(c, s1)), None)
        if p1 and s1:
            drag(W, s1[0] + 40, s1[1] + 40, 16, 0)
            time.sleep(0.6)
            cs = cards(W)
            s1 = next((c for t, c in cs if not t.startswith("Peer ")), None)
            p1 = next((c for t, c in cs if t.startswith("Peer ") and s1 and overlaps(c, s1)), None)
            W.shot("flow-overlap-session")
            W.check("flow: the session card now covers the peer's edge", bool(p1 and s1) and overlaps(p1, s1), f"{p1} {s1}")
            if p1 and s1:
                W.click_xy(p1[0] + 12, p1[1] + 60)  # inside both cards
                W.check("flow: ...and a click where they meet opens the session (the card grabbed last), not the peer",
                        W.wait_shown("conversation_inner", 8) and not W.visible("flow_view"))
                W.click("hd_tab_flow_hit")
                W.wait_shown("flow_view", 8)
    else:
        W.check("flow: cards to overlap", False, f"{cs}")

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
