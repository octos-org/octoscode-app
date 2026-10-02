#!/usr/bin/env python3
"""A30 — the sidebar peer dock click walk (parity row 270; web
`features/peers/PeerDock.tsx`, `features/shell/ProductSidebar.tsx:959-967`,
`app/App.tsx:1088-1119`; board 4 regions 6 / 7).

Every control is reached by a CLICK at its laid-out rect (⌥P / ⌥Y / ⌥N by the
instrument's key events): three peers are started from the Fleet pane's
production Start (lanes glm-4.6, gpt-5.4, deepseek-v4-flash), then the dock in
the sidebar — between the session tree and the footer — is walked against
`replay_serve --scenario fleet --peer-dock`, the faithful external-driver
fixture whose peers ask for approvals with real ids and targets:

  * hidden with no peers; on a phone it lives in the drawer and starts FOLDED;
  * the rows: "Peer N · model", the Fleet's status words, the web's elapsed,
    "↓ tokens"; the session tree's alignment (numeric /snap checks);
  * the threaded approval: Approve once / Deny / Approve for session (+ ⌥N on
    the focused row) / Stop each send EXACTLY ONE `peer/control` bound to the
    clicked row's own operation, adopted turn and pending approval id — never
    another row's (the replay log is the wire proof);
  * ⌥P folds to the one pill ("Peers · 3 · 1 working · ⚠ 1 waiting · 1/3
    finished") and back; the pill and "Hide peers" toggle it;
  * zh: the dock in Chinese (Settings > Preferences > Language), the CJK labels
    unclipped.

usage: OCTOSCODE_APP_BIN=<host octosense> a30_peer_dock.py <desktop|phone> <outdir>
"""
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, inside, overlap, run_session  # noqa: E402  (a10_lib imports bridgeauth: D10c)

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast,
# never imported). run_session takes the aggregator's ports.
WALK = {
    "name": "a30_peer_dock",
    "title": "Sidebar peer dock: rows in the Fleet's words, the threaded approval's four actions each one targeted "
             "peer/control, Alt+P fold to the pill, hidden with no peers, phone drawer starts folded, zh",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A30_PORT": "{port}", "A30_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 1500,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"tmp/a30/{MODE}"
PORT = int(os.environ.get("A30_PORT", "8612"))
REPLAY = int(os.environ.get("A30_REPLAY_PORT", "8614"))
LANES = [("glm-4.6", 0), ("gpt-5.4", 1), ("deepseek-v4-flash", 2)]
BRIEFS = ["Fix steer queue drop on reconnect", "Run the CLI test suite", "Review PR 2566"]
CAPTURES: list[dict] = []

# The phone shell's soft keyboard hide key (a10_fleet.py PHONE_HIDE_KEYBOARD).
PHONE_HIDE_KEYBOARD = (377, 573)


# ------------------------------------------------------------------ helpers

def replay_lines(W: Walk, needle: str) -> list[str]:
    if not W.replay_log or not W.replay_log.exists():
        return []
    return [l for l in W.replay_log.read_text().splitlines() if needle in l]


def kv(line: str) -> dict:
    return dict(re.findall(r"(\w+)=(\S+)", line))


def controls(W: Walk) -> list[dict]:
    return [kv(l) for l in replay_lines(W, "-> peer/control (fleet sim)")]


def dispatches(W: Walk) -> list[dict]:
    return [kv(l) for l in replay_lines(W, "-> peer/dispatch (fleet sim)")]


def asked(W: Walk, session: str) -> list[str]:
    """The approval ids the replay asked for in `session`, oldest first."""
    return [kv(l).get("approval_id", "") for l in replay_lines(W, "=> approval/requested (fleet)")
            if kv(l).get("session_id") == session]


def key_alt(W: Walk, code: str) -> None:
    W.note(f"KEY Alt+{code}")
    W.get(f"/k?k=down&c={code}&alt=1&wait=1", tolerant=True)
    W.get(f"/k?k=up&c={code}&alt=1&wait=1", tolerant=True)
    time.sleep(0.4)


def hide_keyboard(W: Walk) -> None:
    if W.mode == "phone":
        W.click_xy(*PHONE_HIDE_KEYBOARD)
        time.sleep(0.6)


def drawer_open(W: Walk) -> bool:
    if W.mode != "phone":
        return True
    if W.visible("sb_settings_hit"):
        return True
    W.click("sidebar_toggle_hit")
    return W.wait_shown("sb_settings_hit", 8)


def drawer_close(W: Walk) -> None:
    if W.mode == "phone" and W.visible("drawer_close"):
        W.click("drawer_close")
        W.wait(lambda: not W.visible("sb_settings_hit"), 6)
        time.sleep(0.4)


def where(W: Walk, wid: str) -> str:
    sn = W.snap()
    vp, r = W.rect("b3_scroll", sn=sn), W.rect(wid, sn=sn)
    if not (vp and r and inside(r, vp)):
        return "out"
    if r[1] <= vp[1] + 1:
        return "cut-top"
    if r[1] + r[3] >= vp[1] + vp[3] - 1:
        return "cut-bottom"
    return "in"


def seek(W: Walk, wid: str, steps: int = 40) -> bool:
    """Bring a Fleet pane widget into its scroll viewport with the wheel."""
    if where(W, wid) == "in":
        return True
    vp = W.rect("b3_scroll")
    for _ in range(6):
        if vp:
            W.get(f"/m?k=scroll&x={vp[0] + 6:.0f}&y={vp[1] + vp[3] / 3:.0f}&dy=-600&wait=1", tolerant=True)
    last = None
    for _ in range(steps):
        at = where(W, wid)
        if at == "in":
            return True
        r = W.rect(wid)
        if at.startswith("cut") and r == last:
            return True
        last = r
        vp = W.rect("b3_scroll")
        dy = -60 if at == "cut-top" else 60 if at == "cut-bottom" else 90
        if vp:
            W.get(f"/m?k=scroll&x={vp[0] + 6:.0f}&y={vp[1] + vp[3] / 3:.0f}&dy={dy}&wait=1", tolerant=True)
        time.sleep(0.2)
    return where(W, wid) == "in"


def fleet_start(W: Walk, n: int) -> bool:
    """One Start from the Fleet pane: lane `n`, its brief, Start; wait for the
    n-th dispatch on the wire. The previous peer's own frames (its approval,
    the pane's announcement) remount the pane, so every step is read back
    before the next one."""
    model, opt = LANES[n]
    time.sleep(2.0)  # the previous peer's attach frames settle first
    lane_keys = ["lane-glm", "lane-primary", "lane-deepseek"]
    for _ in range(3):
        seek(W, "b3_fleet_model_tap")
        W.click("b3_fleet_model_tap")
        if W.wait_shown(f"b3_fleet_opt_{opt}", 6):
            seek(W, f"b3_fleet_opt_{opt}")
            W.click(f"b3_fleet_opt_{opt}")
        if W.wait(lambda: W.text("b3_fleet_model_value") == lane_keys[n], 4):
            break
    for _ in range(3):
        seek(W, "b3_fleet_brief")
        r = W.rect("b3_fleet_brief")
        if r:
            W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
            if W.text("b3_fleet_brief") not in ("", "Describe the task for the peer"):
                W.clear_field(60)
            W.type_text(BRIEFS[n])
            hide_keyboard(W)
        time.sleep(0.6)
        if W.text("b3_fleet_brief") == BRIEFS[n]:
            break
        W.note(f"RETRY the brief (read back {W.text('b3_fleet_brief')!r})")
    seek(W, "b3_fleet_start")
    W.wait_shown("b3_fleet_start", 6)
    W.click("b3_fleet_start")
    return W.wait(lambda: len(dispatches(W)) == n + 1, 12)


def text_any(W: Walk, wid: str, sn=None) -> str:
    """A label's text from the tree, drawn or not: a dock row the capped
    rows region scrolled out of view still carries its words."""
    sn = sn if sn is not None else W.snap()
    hit = next((w for w in sn if w.get("i") == wid and w.get("t") is not None), None)
    return (hit.get("t") or "") if hit else ""


def dock_rows(W: Walk, sn=None) -> list[str]:
    sn = sn if sn is not None else W.snap()
    out = []
    for i in range(8):
        t = text_any(W, f"pd_row_{i}_label", sn=sn)
        if not t:
            break
        out.append(t)
    return out


def row_status(W: Walk, i: int, sn=None) -> str:
    return text_any(W, f"pd_row_{i}_status", sn=sn)


def dock_seek(W: Walk, wid: str, tries: int = 16) -> bool:
    """Bring a dock control wholly into view: on a short desktop column the
    rows region scrolls (the user's own wheel over it)."""
    for _ in range(tries):
        sn = W.snap()
        r = W.rect(wid, sn=sn)
        # The rows region is the viewport (a ScrollYView when capped; the
        # instrument may name its type View).
        vp = next((w["r"] for w in sn if w.get("i") == "pd_rows" and Walk.shown(w)), None)
        in_rows = wid.startswith("pd_row_")
        if r and (not in_rows or vp is None or (r[1] >= vp[1] - 0.5 and r[1] + r[3] <= vp[1] + vp[3] + 0.5)):
            return True
        if vp is None:
            # A remount not drawn yet (or a folded dock): wait for the frame.
            time.sleep(0.3)
            continue
        # Scrolled out below (or no rect yet): wheel down; above: up.
        dy = -60 if (r and r[1] < vp[1]) else 60
        if r is None:
            # Not drawn: find whether it lies above or below by its row index.
            m = re.match(r"pd_row_(\d+)_", wid)
            first = next((int(re.match(r"pd_row_(\d+)_", w["i"]).group(1)) for w in sn
                          if re.match(r"pd_row_\d+_head$", str(w.get("i", ""))) and Walk.shown(w)), None)
            dy = -60 if (m and first is not None and int(m.group(1)) < first) else 60
        W.get(f"/m?k=scroll&x={vp[0] + 20:.0f}&y={vp[1] + vp[3] / 2:.0f}&dy={dy}&wait=1", tolerant=True)
        time.sleep(0.25)
    sn = W.snap()
    row = re.match(r"(pd_row_\d+)_", wid)
    probe = {w.get("i"): (w.get("ty"), w.get("r"), w.get("v", 1)) for w in sn
             if w.get("i") in ("pd_rows", "peer_dock_row", wid, f"{wid}_box")
             or (row and str(w.get("i", "")).startswith(row.group(1)) and str(w.get("i", "")).endswith(("_head", "_card")))}
    W.note(f"SEEK {wid} failed: {probe}")
    return False


def dock_click(W: Walk, wid: str) -> bool:
    dock_seek(W, wid)
    return W.click(wid)


def dock_checks(W: Walk, name: str, sn=None) -> bool:
    """The dock's numeric UX checks from one /snap: inside the sidebar
    column, between the tree and the footer, aligned with the session tree,
    no label outside its column or overlapping another, controls >= 28 px,
    the tree keeps room."""
    sn = sn if sn is not None else W.snap()
    shown = [w for w in sn if Walk.shown(w)]
    col = W.rect("threads_column", sn=sn)
    slot = W.rect("peer_dock_row", sn=sn)
    tree = W.rect("thread_list", sn=sn)
    foot = W.rect("oc_sidebar_foot", sn=sn)
    if not (col and slot and tree and foot):
        return W.check(f"{name}: dock numeric checks", False, f"col={col} slot={slot} tree={tree} foot={foot}")
    content = [col[0] + 10, col[1], col[2] - 20, col[3]]
    mine = [w for w in shown if str(w.get("i", "")).startswith("pd_")]
    vp = next((w["r"] for w in shown if w.get("i") == "pd_rows"), None)
    if vp:
        # The capped rows region scrolls: a row the viewport's edge cuts is
        # clipped BY DESIGN (the instrument reports its clipped rect, which
        # ends or starts exactly on the edge) — only what lies wholly inside
        # is judged.
        top, bot = vp[1], vp[1] + vp[3]

        def clipped(r):
            return (r[1] < top - 0.5 or r[1] + r[3] > bot + 0.5
                    or abs(r[1] + r[3] - bot) <= 1.5 or abs(r[1] - top) <= 1.5)

        mine = [w for w in mine if not (str(w.get("i", "")).startswith("pd_row_") and clipped(w["r"]))]
    labels = [w for w in mine if w.get("ty") == "Label" and (w.get("t") or "").strip()]
    hits = [w for w in mine if w.get("ty") in ("Button", "DesignNativeButton")]
    outside = [w["i"] for w in labels if not inside(w["r"], content, 1.0)]
    over = [(a["i"], b["i"]) for k, a in enumerate(labels) for b in labels[k + 1:] if overlap(a["r"], b["r"])]
    under = [(w["i"], w["r"]) for w in hits if w["r"][3] < 27.5 or w["r"][2] < 27.5]
    order = tree[1] + tree[3] <= slot[1] + 1.0 and slot[1] + slot[3] <= foot[1] + 1.0
    # The session tree's grid: the dock's glyph column and labels line up
    # with the tree's (`SbRowTpl` padding 6, glyph 16, gap 6), its elapsed
    # ends where the tree's time does (padding 8).
    tree_title = W.rect("sb_r_title", sn=sn) or W.rect("sb_g_label", sn=sn)
    # The first row wholly in view (a capped region may have scrolled).
    first = next((i for i in range(8) if any(w.get("i") == f"pd_row_{i}_label" for w in mine)), 0)
    label0 = W.rect(f"pd_row_{first}_label", sn=sn) if any(w.get("i") == f"pd_row_{first}_label" for w in mine) else None
    elapsed0 = W.rect(f"pd_row_{first}_elapsed", sn=sn) if label0 else None
    glyph0 = W.rect(f"pd_row_{first}_glyph", sn=sn) if label0 else None
    align = []
    if tree_title and label0:
        align.append(("label_x", round(label0[0] - tree_title[0], 1)))
    if elapsed0:
        align.append(("elapsed_right", round(elapsed0[0] + elapsed0[2] - (content[0] + content[2] - 8), 1)))
    if glyph0:
        align.append(("glyph_x", round(glyph0[0] - (content[0] + 6), 1)))
    pill = W.rect("pd_pill_box", sn=sn)
    if pill:
        align.append(("pill_inside", inside(pill, content, 1.0)))
    aligned = all(abs(v) <= 1.0 if isinstance(v, (int, float)) and not isinstance(v, bool) else v for _, v in align)
    tree_room = tree[3]
    ok = not outside and not over and not under and order and aligned and tree_room >= 30
    detail = (f"dock={[round(x) for x in slot]} tree_h={round(tree_room)} order={order} labels={len(labels)} "
              f"outside={outside[:3]} overlaps={over[:3]} controls={len(hits)} under28={under[:3]} "
              f"align={align}")
    return W.check(f"{name}: dock numeric checks (inside the column, between tree and footer, the tree's grid, "
                   f"no clipped/overlapping labels, controls >= 28 px)", ok, detail)


def capture(W: Walk, name: str, state: str) -> None:
    """A capture of the whole app plus a crop of the sidebar column, for the
    side-by-side with board 4 regions 6 / 7."""
    png = W.shot(name)
    sn = json.loads((W.out / f"{name}.snap.json").read_text())
    col = next((w["r"] for w in sn if w.get("i") == "threads_column" and Walk.shown(w)), None)
    win = next((w["r"] for w in sn if w.get("ty") == "Window" and Walk.shown(w)), None)
    mod = next((w["r"] for w in sn if w.get("ty") == "OctoscodeView" and Walk.shown(w)), None)
    crop = None
    try:
        from PIL import Image  # noqa: PLC0415

        img = Image.open(png)
        if col and win and mod:
            if W.mode == "desktop":
                # The desktop shot is the module + its 32 px title bar, downscaled.
                k = img.width / mod[2]
                ox, oy = mod[0], mod[1] - 32
            else:
                k = img.width / win[2]
                ox, oy = win[0], win[1]
            box = (int((col[0] - ox) * k), int((col[1] - oy) * k), int((col[0] + col[2] - ox) * k),
                   int((col[1] + col[3] - oy) * k))
            crop = W.out / f"{name}-sidebar.png"
            img.crop(box).save(crop)
    except Exception as e:  # noqa: BLE001 — the full capture stays
        W.note(f"sidebar crop skipped: {e}")
    CAPTURES.append({"name": name, "state": state, "png": str(png), "sidebar": str(crop) if crop else None})


# --------------------------------------------------------------------- walk

def walk(W: Walk) -> None:
    mode = W.mode
    W.note("== 1. no peers: the dock is hidden (the web renders nothing)")
    drawer_open(W)
    W.check("dock: hidden while there are no peers (the sidebar shows, the dock does not)",
            W.wait_shown("sb_add_hit", 6) and not W.visible("peer_dock_row") and not W.visible("pd_dock"))
    W.mark()
    key_alt(W, "KeyP")
    W.check("dock: Alt+P with no peers draws no dock (and flips nothing)",
            not W.visible("pd_dock") and W.logged("shortcut Alt+P: no peer dock on screen", 4))
    drawer_close(W)

    W.note("== 2. three peers from the Fleet's production Start (glm-4.6, gpt-5.4, deepseek-v4-flash)")
    drawer_open(W)
    W.mark()
    W.click("fleet_nav_hit")
    W.check("fleet: the footer's Fleet entry opens the pane", W.wait_shown("b3_fleet_panel", 10)
            and W.wait_shown("b3_fleet_form_title", 10))
    started = all(fleet_start(W, n) for n in range(3))
    d = dispatches(W)
    W.check("fleet: three Starts -> three dispatches on lanes glm / primary / deepseek",
            started and [x.get("model") for x in d] == ["lane-glm", "lane-primary", "lane-deepseek"],
            f"{[(x.get('model'), x.get('adopted_session_id')) for x in d]}")
    peers = [(x.get("operation_id"), x.get("adopted_session_id"), x.get("adopted_turn_id")) for x in d]
    if len(peers) < 3:
        W.note("ABORT: the three peers did not start")
        return
    seek(W, "b3_fleet_back")
    W.click("b3_fleet_back")
    W.wait(lambda: not W.visible("b3_fleet_panel"), 8)
    time.sleep(2.5)  # the peers' own frames fold (approvals, tokens, the third finishes)

    W.note("== 3. the dock between the session tree and the footer")
    drawer_open(W)
    if mode == "phone":
        W.check("phone: the dock starts FOLDED in the drawer (one pill)",
                W.wait_shown("pd_pill", 8) and not W.visible("pd_row_0_label"))
        capture(W, f"{mode}-01-folded-start", "collapsed (phone default)")
        dock_checks(W, "phone folded")
        W.mark()
        W.click("pd_pill")
        W.check("phone: the pill CLICK expands the dock", W.logged("peer dock tap: pd.fold", 6)
                and W.wait_shown("pd_row_0_label", 6))
    else:
        W.check("desktop: the dock starts expanded", W.wait_shown("pd_row_0_label", 8) and W.visible("pd_hide"))
    W.wait(lambda: row_status(W, 2) == "Finished" and "Waiting for your approval" == row_status(W, 1), 10)
    sn = W.snap()
    rows = dock_rows(W, sn)
    W.check("rows: 'Peer N · model' in the Fleet's order and words (never the slug)",
            rows == ["Peer 1 · glm-4.6", "Peer 2 · gpt-5.4", "Peer 3 · deepseek-v4-flash"]
            and not any("fix-steer" in t or "#peer-" in t for t in rows), f"{rows}")
    words = [row_status(W, i, sn) for i in range(3)]
    W.check("rows: the Fleet's status words (waiting, waiting, finished)",
            words == ["Waiting for your approval", "Waiting for your approval", "Finished"], f"{words}")
    elapsed = [text_any(W, f"pd_row_{i}_elapsed", sn) for i in range(3)]
    W.check("rows: elapsed in the web's format (Ns / MmSSs)",
            all(re.fullmatch(r"\d+s|\d+m\d\ds|\d+h\d\dm", e or "") for e in elapsed), f"{elapsed}")
    metas = [text_any(W, f"pd_row_{i}_meta", sn) for i in range(3)]
    W.check("rows: '↓ tokens' (12.4k on Peer 1, 31k on Peer 3)",
            "↓ 12.4k" in metas[0] and "↓ 31k" in metas[2], f"{metas}")
    has = lambda wid: any(w.get("i") == wid for w in sn)  # noqa: E731
    W.check("rows: only the approval-blocked rows grow the threaded card",
            has("pd_row_0_card") and has("pd_row_1_card") and not has("pd_row_2_card"))
    W.check("card: 'asks to run shell' + the target, Approve once / Deny / Stop + Approve for session",
            text_any(W, "pd_row_1_asks", sn) == "asks to run shell" and text_any(W, "pd_row_1_target", sn) == "cargo test -p octos-cli"
            and [text_any(W, f"pd_row_1_{b}_label", sn) for b in ("approve", "deny", "stop")] == ["Approve once", "Deny", "Stop"]
            and text_any(W, "pd_row_1_session_label", sn) == "Approve for session" and text_any(W, "pd_row_1_keys", sn) == "⌥Y / ⌥N",
            f"asks={text_any(W, 'pd_row_1_asks', sn)!r} target={text_any(W, 'pd_row_1_target', sn)!r}")

    W.note("== 4. Approve once on Peer 1: ONE frame bound to Peer 1's own ids")
    p1_op, p1_sess, p1_turn = peers[0]
    p2_op, p2_sess, p2_turn = peers[1]
    p1_ask, p2_ask = asked(W, p1_sess), asked(W, p2_sess)
    before = len(controls(W))
    W.mark()
    dock_click(W, "pd_row_0_approve")
    sent = W.wait(lambda: len(controls(W)) == before + 1, 8)
    time.sleep(0.8)
    c = controls(W)[before:] if sent else []
    W.check("Approve once CLICK -> exactly ONE peer/control approval_respond with Peer 1's approval id, operation "
            "and adopted turn (never Peer 2's)",
            sent and len(c) == 1 and c[0].get("command") == "approval_respond" and c[0].get("decision") == "approve"
            and "approval_scope" not in c[0] and c[0].get("approval_id") == (p1_ask or [None])[-1]
            and c[0].get("target_operation_id") == p1_op and c[0].get("expected_turn_id") == p1_turn
            and c[0].get("approval_id") not in p2_ask,
            f"{c} p1_ask={p1_ask} p2_ask={p2_ask}")
    W.check("Peer 1 -> Working · ↓ 12.4k · Sent (the acknowledgment, the card gone)",
            W.wait(lambda: row_status(W, 0) == "Working" and not any(w.get("i") == "pd_row_0_card" for w in W.snap()), 8)
            and "Sent" in text_any(W, "pd_row_0_meta"), f"{row_status(W, 0)!r} {text_any(W, 'pd_row_0_meta')!r}")

    W.note("== 5. the board's state: Peer 1 working, Peer 2 waiting (threaded card), Peer 3 finished")
    time.sleep(0.6)
    dock_checks(W, f"{mode} expanded")
    capture(W, f"{mode}-02-expanded", "expanded: working / waiting + card / finished")

    W.note("== 6. Alt+P folds the dock to ONE pill; Alt+P again expands it")
    drawer_close(W) if mode == "phone" else None
    drawer_open(W)
    W.mark()
    key_alt(W, "KeyP")
    folded = W.wait(lambda: W.visible("pd_pill") and not W.visible("pd_row_0_label"), 6)
    sn = W.snap()
    pill = [W.text(i, sn) for i in ("pd_pill_peers", "pd_pill_total", "pd_pill_working", "pd_pill_warn", "pd_pill_waiting",
                                     "pd_pill_finished")]
    W.check("Alt+P -> the dock folds to one pill (log + /snap)", folded and W.logged("shortcut Alt+P -> peer dock folded", 4))
    W.check("pill: 'Peers · 3 · 1 working · ⚠ 1 waiting · 1/3 finished'",
            pill == ["Peers", "3", "1 working", "⚠", "1 waiting", "1/3 finished"], f"{pill}")
    W.check("pill: the 'Show peers · ⌥P' hint", W.text("pd_hint", sn) == "Show peers · ⌥P", repr(W.text("pd_hint", sn)))
    dock_checks(W, f"{mode} collapsed")
    capture(W, f"{mode}-03-collapsed", "collapsed pill")
    W.mark()
    key_alt(W, "KeyP")
    W.check("Alt+P again -> expanded", W.wait_shown("pd_row_0_label", 6) and W.logged("peer dock expanded", 4))

    W.note("== 7. Deny on Peer 2: ONE frame with Peer 2's pending id; Peer 2 asks again")
    p2_ask = asked(W, p2_sess)
    before = len(controls(W))
    dock_click(W, "pd_row_1_deny")
    sent = W.wait(lambda: len(controls(W)) == before + 1, 8)
    time.sleep(0.6)
    c = controls(W)[before:] if sent else []
    W.check("Deny CLICK -> exactly ONE approval_respond deny with Peer 2's approval id, operation and turn",
            sent and len(c) == 1 and c[0].get("decision") == "deny" and c[0].get("approval_id") == (p2_ask or [None])[-1]
            and c[0].get("target_operation_id") == p2_op and c[0].get("expected_turn_id") == p2_turn, f"{c} p2_ask={p2_ask}")
    W.check("Peer 2 asks again (a NEW approval id, a new target on its card)",
            W.wait(lambda: len(asked(W, p2_sess)) == len(p2_ask) + 1
                   and text_any(W, "pd_row_1_target") == "cargo clippy --workspace", 8), f"{asked(W, p2_sess)}")

    W.note("== 8. Approve for session on Peer 2: ONE frame with the session scope")
    p2_ask = asked(W, p2_sess)
    before = len(controls(W))
    dock_click(W, "pd_row_1_session")
    sent = W.wait(lambda: len(controls(W)) == before + 1, 8)
    time.sleep(0.6)
    c = controls(W)[before:] if sent else []
    W.check("Approve for session CLICK -> exactly ONE approval_respond approve + approval_scope=session on Peer 2's new id",
            sent and len(c) == 1 and c[0].get("decision") == "approve" and c[0].get("approval_scope") == "session"
            and c[0].get("approval_id") == p2_ask[-1] and c[0].get("target_operation_id") == p2_op, f"{c}")
    W.check("Peer 2 asks a third time", W.wait(lambda: len(asked(W, p2_sess)) == len(p2_ask) + 1
                                               and text_any(W, "pd_row_1_target") == "rm -rf target/debug/incremental", 8))

    W.note("== 9. Alt+Y / Alt+N act on the FOCUSED row only")
    W.mark()
    dock_click(W, "pd_row_0_focus")
    W.check("focus: a row CLICK focuses it (log); a row with no card shows no ring",
            W.logged("peer dock pd.focus", 4) and not any(w.get("i") == "pd_row_0_ring" for w in W.snap()))
    before = len(controls(W))
    key_alt(W, "KeyY")
    time.sleep(1.0)
    W.check("Alt+Y on the focused Peer 1 (no pending approval) sends NOTHING", len(controls(W)) == before,
            f"{controls(W)[before:]}")
    dock_click(W, "pd_row_1_focus")
    W.check("focus: Peer 2's row CLICK draws the ring around the row ⌥Y / ⌥N will answer",
            W.wait(lambda: any(w.get("i") == "pd_row_1_ring" for w in W.snap()), 4))
    p2_ask = asked(W, p2_sess)
    key_alt(W, "KeyN")
    sent = W.wait(lambda: len(controls(W)) == before + 1, 8)
    time.sleep(0.6)
    c = controls(W)[before:] if sent else []
    W.check("Alt+N on the focused Peer 2 -> exactly ONE deny on Peer 2's pending id",
            sent and len(c) == 1 and c[0].get("decision") == "deny" and c[0].get("approval_id") == p2_ask[-1]
            and c[0].get("target_operation_id") == p2_op, f"{c}")
    W.wait(lambda: len(asked(W, p2_sess)) == len(p2_ask) + 1 and text_any(W, "pd_row_1_target") == "cargo publish --dry-run", 8)

    W.note("== 10. Stop on Peer 2: ONE interrupt bound to Peer 2's operation and turn")
    before = len(controls(W))
    dock_click(W, "pd_row_1_stop")
    sent = W.wait(lambda: len(controls(W)) == before + 1, 8)
    time.sleep(0.6)
    c = controls(W)[before:] if sent else []
    W.check("Stop CLICK -> exactly ONE peer/control interrupt on Peer 2's operation + adopted turn",
            sent and len(c) == 1 and c[0].get("command") == "interrupt" and c[0].get("target_operation_id") == p2_op
            and c[0].get("expected_turn_id") == p2_turn, f"{c}")
    W.check("Peer 2 -> Stopped (the turn's own terminal)", W.wait(lambda: row_status(W, 1) == "Stopped", 8),
            repr(row_status(W, 1)))
    W.check("wire: every peer/control so far went to the row clicked (Peer 1 x1, Peer 2 x4)",
            [x.get("target_operation_id") for x in controls(W)] == [p1_op, p2_op, p2_op, p2_op, p2_op],
            f"{[x.get('target_operation_id') for x in controls(W)]}")

    W.note("== 11. Hide peers / the pill toggle the fold")
    dock_click(W, "pd_hide")
    W.check("'Hide peers' CLICK folds", W.wait(lambda: W.visible("pd_pill") and not W.visible("pd_row_0_label"), 6))
    W.check("pill after the stop: 'Peers · 3 · 1 working · 2/3 finished' (no waiting segment)",
            [W.text(i) for i in ("pd_pill_total", "pd_pill_working", "pd_pill_finished")] == ["3", "1 working", "2/3 finished"]
            and not W.visible("pd_pill_warn"))
    W.wait_shown("pd_pill", 6)
    W.click("pd_pill")
    W.check("the pill CLICK expands", W.wait_shown("pd_row_0_label", 6))
    drawer_close(W)


def zh_phase(W: Walk) -> None:
    """The dock in Chinese: Settings > Preferences > Language 简体中文, the
    dock expanded (a waiting card) and folded, the CJK labels unclipped."""
    mode = W.mode
    cell = "rl_hit" if mode == "phone" else "nv_hit"
    sections = ["general", "permissions", "model", "sandbox", "connection", "preferences", "about"]

    def open_prefs() -> bool:
        drawer_open(W)
        W.click("sb_settings_hit")
        if not W.wait(lambda: bool(W.visible("settings_drawer")), 8):
            return False
        W.click(cell, sections.index("preferences"))
        return W.wait(lambda: bool(W.visible("sec_preferences")), 6)

    def close_prefs() -> None:
        W.click("set_back" if mode == "phone" else "settings_close")
        W.wait(lambda: not W.visible("settings_drawer"), 6)

    def segment(seg: str) -> bool:
        r = W.rect(seg)
        if not r:
            return False
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        return True

    W.note("== zh: the dock in Chinese")
    W.check("zh: Settings > Preferences opens from the footer", open_prefs())
    W.mark()
    segment("lang_zh")
    W.check("zh: 简体中文 CLICK switches the language", W.logged("a24 language -> zh", 6))
    close_prefs()
    drawer_open(W)
    if not W.visible("pd_row_0_label"):
        W.click("pd_pill")
        W.wait_shown("pd_row_0_label", 6)
    time.sleep(0.8)
    sn = W.snap()
    want = {"pd_title": "同侪", "pd_hide_label": "隐藏同侪", "pd_row_0_status": "工作中", "pd_row_1_status": "已停止",
            "pd_row_2_status": "已完成"}
    got = {k: text_any(W, k, sn) for k in want}
    W.check("zh: the dock reads Chinese (同侪 / 隐藏同侪 / 工作中 / 已停止 / 已完成)", got == want, f"{got}")
    W.check("zh: 'Peer N' reads 同侪 N", text_any(W, "pd_row_0_label", sn).startswith("同侪 1 · "), repr(text_any(W, "pd_row_0_label", sn)))
    dock_checks(W, f"{mode} zh expanded", sn)
    capture(W, f"{mode}-04-zh-expanded", "zh expanded")
    key_alt(W, "KeyP")
    W.wait_shown("pd_pill", 6)
    sn = W.snap()
    W.check("zh: the pill reads Chinese (同侪 · 3 · 1 个工作中 · 2/3 已完成; 显示同侪 · ⌥P)",
            [W.text(i, sn) for i in ("pd_pill_peers", "pd_pill_working", "pd_pill_finished", "pd_hint")]
            == ["同侪", "1 个工作中", "2/3 已完成", "显示同侪 · ⌥P"],
            f"{[W.text(i, sn) for i in ('pd_pill_peers', 'pd_pill_working', 'pd_pill_finished', 'pd_hint')]}")
    dock_checks(W, f"{mode} zh collapsed", sn)
    capture(W, f"{mode}-05-zh-collapsed", "zh collapsed")
    key_alt(W, "KeyP")
    W.check("zh: Settings reopens", open_prefs())
    W.mark()
    segment("lang_en")
    W.check("zh: English restores", W.logged("a24 language -> en", 6))
    close_prefs()
    drawer_close(W)


def main_walk(W: Walk) -> None:
    walk(W)
    zh_phase(W)


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    rc = run_session(main_walk, mode=MODE, outdir=OUT, port=PORT, replay_port=REPLAY, scenario="fleet",
                     replay_args=["--peer-dock"])
    with open(os.path.join(OUT, "captures.json"), "w") as f:
        json.dump(CAPTURES, f, indent=1, ensure_ascii=False)
    print(f"== WALK a30 peer dock {MODE}: {'PASS' if rc == 0 else 'FAIL'}")
    sys.exit(rc)
