#!/usr/bin/env python3
"""Judge tour: visit every reachable OctosCode screen in a RUNNING hidden app, capture it and run generic /snap checks.

    python3 tools/judge/tour.py <port> <desktop|phone> <outdir> [first-run]

The app must already run (outer/scripts/judge-tour.sh launches it with the capture seeds). For each screen:
<outdir>/<NN>-<name>.png (downscaled to <= 1400 px), <name>.snap.json, and one line in <outdir>/checks.tsv.
Generic checks (the human judge still looks at every PNG):
  outside  - a laid-out text node lies (partly) outside the window
  collapsed- a node with text has width or height < 2
  overlap  - two sibling text nodes overlap by more than 25% of the smaller one
  small    - a Button / hit narrower or shorter than 28 px (touch / pointer target)
"""
import json, os, re, subprocess, sys, time, urllib.error, urllib.parse, urllib.request
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "walk"))
from snapsafe import scrub as _scrub  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../walk"))  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

PORT, MODE, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
PHASE = sys.argv[4] if len(sys.argv) > 4 else "live"   # live | commands | first-run
FIRST_RUN = PHASE == "first-run"
BASE = f"http://127.0.0.1:{PORT}"
os.makedirs(OUT, exist_ok=True)
N = [0]


def get(path, timeout=15):
    # An input route with wait=1 answers 404 when its frame was coalesced: the
    # input WAS delivered (A11), so it is never re-sent (a retry would click
    # twice). A read that 404s (the UI thread missed its 5 s window on a loaded
    # machine) is retried once after a pause, which tells a stall from a dead app.
    if path.startswith(("/click", "/t?", "/k?", "/m?")):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return b""
            raise
    # Under a saturated machine (eight agents building) a frame can miss the
    # window more than once; the pauses grow so a slow frame gets its turn.
    pauses = (1.5, 3.0, 6.0)
    for attempt in range(len(pauses) + 1):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if attempt == len(pauses):
                raise
            time.sleep(pauses[attempt])


def snap():
    return json.loads(get("/snap?all=1"))


def nodes(tree=None):
    out = []

    def walk(x, parent):
        if isinstance(x, dict):
            me = parent
            if "r" in x:
                out.append((x, parent))
                me = x
            for k, v in x.items():
                if k != "r":
                    walk(v, me)
        elif isinstance(x, list):
            for v in x:
                walk(v, parent)

    walk(tree if tree is not None else snap(), None)
    return out


def find(wid):
    for n, _ in nodes():
        if n.get("i") == wid and n["r"][2] > 0 and n["r"][3] > 0:
            return n
    return None


def find_text(text):
    for n, _ in nodes():
        if (n.get("t") or "").strip() == text and n["r"][2] > 0 and n["r"][3] > 0:
            return n
    return None


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.6)


def click(wid):
    n = find(wid)
    if n:
        click_rect(n["r"])
    return bool(n)


def click_text(text):
    n = find_text(text)
    if n:
        click_rect(n["r"])
    return bool(n)


def key(c, **mods):
    q = {"c": c, "wait": 1}
    q.update({k: 1 for k, v in mods.items() if v})
    get("/k?" + urllib.parse.urlencode(q))
    time.sleep(0.4)


def type_text(t):
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))
    time.sleep(0.3)


def window():
    s = json.loads(get("/s"))
    sz = s["w"][0]["sz"]
    return [0, 0, sz[0], sz[1]]


def checks(tree):
    win = window()
    found = []
    ns = nodes(tree)
    for n, parent in ns:
        r, t = n["r"], (n.get("t") or "").strip()
        if r[2] <= 0 or r[3] <= 0:
            if t and n.get("ty") in ("Label", "TextInput") and n.get("v", 1) != 0 and parent and parent["r"][2] > 0:
                found.append(f"collapsed:{n.get('i')}")
            continue
        if t and n.get("ty") == "Label":
            if r[0] < win[0] - 1 or r[1] < win[1] - 1 or r[0] + r[2] > win[2] + 1 or r[1] + r[3] > win[3] + 1:
                found.append(f"outside:{n.get('i')}")
        if n.get("ty") == "Button" and (r[2] < 27.5 or r[3] < 27.5) and n.get("v", 1) != 0:
            found.append(f"small:{n.get('i')}:{round(r[2])}x{round(r[3])}")
    by_parent = {}
    win_area = win[2] * win[3]
    for n, parent in ns:
        # Siblings inside a SMALL container only: overlay layers (a dialog over
        # the conversation) share a window-sized ancestor and are not defects.
        if not parent or parent["r"][2] * parent["r"][3] > 0.4 * win_area:
            continue
        if (n.get("t") or "").strip() and n.get("ty") == "Label" and n["r"][2] > 0 and n["r"][3] > 0 and n.get("v", 1) != 0:
            by_parent.setdefault(id(parent), []).append(n)
    for sib in by_parent.values():
        for i in range(len(sib)):
            for j in range(i + 1, len(sib)):
                a, b = sib[i]["r"], sib[j]["r"]
                ix = max(0, min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0]))
                iy = max(0, min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1]))
                small = min(a[2] * a[3], b[2] * b[3])
                if small > 0 and ix * iy > 0.25 * small:
                    found.append(f"overlap:{sib[i].get('i')}/{sib[j].get('i')}")
    return found


def capture(name):
    N[0] += 1
    stem = f"{N[0]:02d}-{name}"
    png = os.path.join(OUT, stem + ".png")
    try:
        grab = get("/g?raw=1", timeout=30)
    except Exception as e:  # recorded, never an empty PNG; the tour goes on
        with open(os.path.join(OUT, "checks.tsv"), "a") as f:
            f.write(f"{stem}\t{MODE}\tgrab-timeout\t{type(e).__name__}: {e}\n")
        print(f"{stem}: grab-timeout ({e})")
        return
    with open(png, "wb") as f:
        f.write(grab)
    subprocess.run(["sips", "-Z", "1400", png, "--out", png], capture_output=True)
    tree = snap()
    with open(os.path.join(OUT, stem + ".snap.json"), "w") as f:
        json.dump(_scrub(tree), f)
    found = checks(tree)
    with open(os.path.join(OUT, "checks.tsv"), "a") as f:
        f.write(f"{stem}\t{MODE}\t{len(found)}\t{' '.join(found[:12])}\n")
    print(f"{stem}: {len(found)} flags {' '.join(found[:6])}")


CLOSERS = ("b3_img_close_btn", "dialog_close", "b3_close", "review_close", "set_back", "settings_close", "drawer_close",
           "a9_act_close")
CLOSE_ID = re.compile(r"(^|_)close(_btn)?$")


def app_in_front():
    return bool(find("hd_bar") or find("i0_composer_0") or find("connect_btn"))


def open_closers():
    return [n for n, _ in nodes() if n.get("ty") == "Button" and n["r"][2] > 0 and n["r"][3] > 0
            and n.get("v", 1) != 0 and ((n.get("i") or "") in CLOSERS or CLOSE_ID.search(n.get("i") or ""))]


def close_overlays():
    # On the shell's phone page Escape means "home": it closes the app, so phone
    # tours close overlays by their own controls only. The topmost (last laid
    # out) close control goes first; an overlay that leaves one behind is
    # recorded, because every later capture would show it instead.
    if MODE != "phone":
        for _ in range(3):
            key("escape")
    for _ in range(4):
        left = open_closers()
        if not left:
            break
        click_rect(left[-1]["r"])
    else:
        left = open_closers()
        if left:
            with open(os.path.join(OUT, "checks.tsv"), "a") as f:
                f.write(f"close\t{MODE}\tstuck\t{' '.join(n.get('i') for n in left)}\n")
    if MODE == "phone" and not app_in_front():
        raise SystemExit("phone: the app left the foreground - tour stopped")


def clear_composer():
    n = find("i0_composer_0")
    if not n:
        return
    k = len(n.get("t") or "")
    get("/k?c=end&wait=1")
    for _ in range(k + 1):
        get("/k?c=backspace&wait=1")


def run_command(cmd):
    if not click("i0_composer_0"):
        return False
    clear_composer()
    type_text("/" + cmd)
    key("return")
    time.sleep(1.2)
    return True


def tour_live():
    capture("conversation")
    if MODE == "phone":
        if click("sidebar_toggle_hit"):
            capture("drawer")
            click("drawer_close")
    # sidebar search
    if MODE != "phone" and click("sb_search"):
        type_text("fix")
        capture("sidebar-search")
        for _ in range(4):
            key("backspace")
    # settings sections
    if click("settings_open_hit"):
        capture("settings-general")
        # By id: the desktop nav row (set_nav_<id>) or, on phone, the icon-only
        # rail (set_rail_<id>) — the rail has no text to click.
        for sec in ("permissions", "model", "sandbox", "connection", "preferences", "about"):
            if click(f"set_nav_{sec}") or click(f"set_rail_{sec}"):
                capture(f"settings-{sec}")
        close_overlays()
    if click("review_open_hit"):
        capture("review")
        close_overlays()
    if click("fleet_nav_hit") or click_text("Fleet"):
        capture("fleet")
        click_text("Back")
    if click("sb_add_hit") or click_text("Add workspace"):
        capture("add-workspace")
        close_overlays()


def dismiss(trigger):
    # Menus and popovers have no close control: Escape on desktop, the trigger
    # again (a toggle) on the phone, where Escape would leave the app.
    if MODE != "phone":
        key("escape")
    elif find(trigger):
        click(trigger)
    close_overlays()


def tour_controls():
    """The connected shell's own controls (header, composer seats, sidebar menus), each opened by a click."""
    for wid, name in (("b3_strip_tap", "session-pane"), ("hd_defaults_change", "new-chat-defaults"),
                      ("copy_open_hit", "copy-markdown"), ("model_seat_hit", "model-menu"),
                      ("approval_pill_hit", "permission-menu"), ("plus_hit", "plus-menu"),
                      ("sb_g_more", "workspace-menu"), ("sb_sort", "sort-menu")):
        try:
            if click(wid):
                capture(name)
                dismiss(wid)
        except SystemExit:
            raise
        except Exception as e:
            print(f"{name}: ERROR {e}")
    if click("hd_tab_traj_hit"):
        capture("trajectory")
        click("hd_tab_chat_hit")
    if MODE != "phone" and click("sidebar_collapse"):
        capture("sidebar-collapsed")
        click("sidebar_toggle_hit") or click("sidebar_collapse")


def tour_commands():
    """Every implemented command that opens a surface, against a CONNECTED app (a replay server):
    with no transport (synthetic seeds) the actions are dropped, so commands can only be judged here."""
    tour_controls()
    if click("i0_composer_0"):
        clear_composer()
        type_text("/")
        capture("palette")
        close_overlays()
    for cmd in ("review", "undo", "rewind", "fork", "peer", "btw", "threads", "turn", "permissions", "gather",
                "thinking", "images", "ps", "activity", "status", "cost", "model", "sessions", "tools", "mcp",
                "skills", "research", "agents", "goal", "loop", "monitor", "resume"):
        try:
            if run_command(cmd):
                capture(f"cmd-{cmd}")
            close_overlays()
            clear_composer()
        except Exception as e:
            with open(os.path.join(OUT, "checks.tsv"), "a") as f:
                f.write(f"cmd-{cmd}\t{MODE}\terror\t{type(e).__name__}: {e}\n")
            print(f"cmd-{cmd}: ERROR {e}")
            try:
                get("/s", timeout=5)
            except Exception:
                print("app gone - stopping the tour")
                break
    if click("sb_new_chat_hit"):
        capture("new-chat")


def tour_first_run():
    capture("connect")
    if click("b1_connect_pair"):
        capture("pair")
        close_overlays()


if __name__ == "__main__":
    open(os.path.join(OUT, "checks.tsv"), "w").close()
    if FIRST_RUN:
        tour_first_run()
    elif PHASE == "commands":
        tour_commands()
    else:
        tour_live()
    print("tour done:", OUT)
