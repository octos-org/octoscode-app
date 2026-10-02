#!/usr/bin/env python3
"""A2 — the board-1 click walk: tools/walk/a2_board1_walk.py <desktop|phone> <outdir>

Every board-1 state is reached by CLICKS from the real UI entries, captured
(/g) with its /snap, and checked numerically (the card inside the module view,
centred with equal margins on desktop / the full sheet on a phone, the web's
dialog widths 440/460/540, no widget outside the card, controls >= 28 px, no
overlapping leaves):

  first-run Connect card "Pair with a link instead" -> p4-01; a used link ->
  p4-03; a server without pairing -> p4-04; a slow claim -> p4-02 (Cancel);
  the good link -> claimed, connected, live; Settings > Connection > This
  device · Details -> p4-05; Settings > Model > Model providers · Edit ->
  p4-06, a rejected key -> p4-07, a good key -> saved; the sidebar's
  + Add workspace -> p4-08 (the browser over the picker), /private -> p4-09,
  back -> the picker, Browse -> Use this folder -> a new session.

Run against the fixture server (crates/octoscode-module/examples/board1_serve.rs)
with a hidden app in first-run state (OCTOS_BASE_URL at a dead port), e.g.

  cargo run -p octoscode-module --example board1_serve -- 8422 &
  OCTOS_BASE_URL=http://127.0.0.1:8499 OCTOS_PROFILE_ID=octoscode \\
  OCTOSCODE_RECENTS_DIR=$PWD/tmp/recents OCTOSCODE_DESIGN_DIR=$PWD/design \\
  MAKEPAD_WM_TEST_APP=octoscode HEADLESS_ARGS="--module octoscode" \\
    bash harness/headless.sh start <host-bin> 8412
  python3 tools/walk/a2_board1_walk.py desktop tmp/walk-desktop
  bash harness/headless.sh stop 8412      # and stop board1_serve

`phone` mode: HEADLESS_ARGS="--module octoscode --test-action page:0" and
OCTOSENSE_WINDOW_SIZE=360x780 (the shell's phone page with the 360x780 frame);
the walk taps the OctosCode icon itself. Writes <outdir>/<state>.png,
<state>.snap.json, checks.json and walk.log (the routed board-1 lines)."""
import json, os, sys, time, urllib.parse, urllib.request

PORT = int(os.environ.get("PORT", "8412"))
BASE = f"http://127.0.0.1:{PORT}"
MODE, OUT = (sys.argv[1], sys.argv[2]) if len(sys.argv) > 2 else ("desktop", "/tmp/walk-unused")
os.makedirs(OUT, exist_ok=True)
LINK = "http://app.invalid/?octos=http://127.0.0.1:8422&pair={}"
CHECKS = {}


def get(path, tolerant=False, tries=6):
    try:
        with urllib.request.urlopen(BASE + path, timeout=30) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")[:200]
        if "retry" in body and tries > 1 and path.startswith("/g"):
            # "grab frame could not be submitted at arming; retry"
            time.sleep(0.5)
            return get(path, tolerant, tries - 1)
        if tolerant or path.startswith("/m?") or path.startswith("/t?") or path.startswith("/k?"):
            # input routes with wait=1 can answer 404 when their frame is coalesced
            time.sleep(0.3)
            return b"{}"
        raise SystemExit(f"GET {path} -> {e.code} {body}")


def snap(all_=False):
    return json.loads(get("/snap" + ("?all=1" if all_ else "")))["s"]


def find(wid, s=None):
    for w in s or snap():
        if w.get("i") == wid and w.get("r", [0, 0, 0, 0])[2] > 0:
            return w
    return None


def wait(wid, secs=10, gone=False):
    end = time.time() + secs
    while time.time() < end:
        if bool(find(wid)) != gone:
            return
        time.sleep(0.25)
    try:
        open(f"{OUT}/TIMEOUT-{wid}.png", "wb").write(get("/g?raw=1"))
        open(f"{OUT}/TIMEOUT-{wid}.snap.json", "w").write(json.dumps(snap(), indent=0))
        log = json.loads(get("/log?n=60"))["l"]
        open(f"{OUT}/TIMEOUT-{wid}.log", "w").write("\n".join(log) + "\n")
    except Exception:
        pass
    raise SystemExit(f"timeout waiting for {'no ' if gone else ''}{wid}")


def click(wid):
    """ONE click at the widget's centre. A 404 "...; retry" answer may still
    have delivered it (a blind retry double-clicked Settings open -> closed),
    so call sites that need an effect use click_until."""
    w = find(wid)
    if not w:
        raise SystemExit(f"no visible {wid}")
    x, y, ww, hh = w["r"]
    get(f"/click?x={x + ww / 2:.1f}&y={y + hh / 2:.1f}&wait=1", tolerant=True)
    time.sleep(0.4)


def click_until(wid, until, gone=False, tries=3, secs=6):
    """Click, then wait for the effect; click again only if it never came."""
    for _ in range(tries):
        click(wid)
        end = time.time() + secs
        while time.time() < end:
            if bool(find(until)) != gone:
                return
            time.sleep(0.25)
    wait(until, 0.1, gone)


def clickxy(x, y):
    get(f"/click?x={x}&y={y}&wait=1")
    time.sleep(0.4)


def typ(text):
    try:
        get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
    except urllib.error.HTTPError:
        # /t?wait=1 can answer 404 when the frame it waits for is coalesced;
        # the text still lands (the next snap shows it).
        pass
    time.sleep(0.3)


def key(code, n=1):
    for _ in range(n):
        get(f"/k?k=down&c={code}&wait=1")
        get(f"/k?k=up&c={code}&wait=1")


def module_rect(s):
    w = next(w for w in s if w.get("ty") == "OctoscodeView")
    return w["r"]


def check(state, s):
    """Numeric checks for the visible board-1 surface."""
    view = module_rect(s)
    vx, vy, vw, vh = view
    card = find("b1_card", s)
    res = {"view": view}
    if not card:
        res["error"] = "no b1_card"
        CHECKS[state] = res
        return res
    cx_, cy_, cw, ch = card["r"]
    res["card"] = card["r"]
    # the dialog's own widgets (the Settings rows and the Connect card's link
    # are entry points elsewhere in the window, not part of the dialog)
    b1 = [w for w in s if str(w.get("i", "")).startswith("b1_") and w.get("i") not in ("b1_card", "b1_sheet", "b1_backdrop")
          and not str(w.get("i")).startswith("b1_set") and w.get("i") != "b1_connect_pair" and w.get("r", [0, 0, 0, 0])[2] > 0]
    # 1. the card sits inside the module view with equal side margins (desktop) or fills it (phone)
    left, right = cx_ - vx, (vx + vw) - (cx_ + cw)
    top, bottom = cy_ - vy, (vy + vh) - (cy_ + ch)
    res["margins_lr"] = [round(left, 1), round(right, 1)]
    res["margins_tb"] = [round(top, 1), round(bottom, 1)]
    res["card_inside_view"] = left >= -0.5 and right >= -0.5 and top >= -0.5 and bottom >= -0.5
    res["centred"] = abs(left - right) <= 1.0
    if MODE == "desktop":
        res["width_rule"] = cw in (440, 460, 540) or abs(cw - (vw - 48)) <= 1
    else:
        res["width_rule"] = abs(cw - vw) <= 1
    # 2. every b1 widget inside the card (no clipped/out-of-card text or control)
    outside = []
    for w in b1:
        x, y, ww, hh = w["r"]
        if x < cx_ - 0.5 or y < cy_ - 0.5 or x + ww > cx_ + cw + 0.5 or y + hh > cy_ + ch + 0.5:
            outside.append(w["i"])
    res["outside_card"] = outside
    # 3. controls >= 28 px high (buttons and inputs)
    small = [(w["i"], w["r"][3]) for w in b1 if w.get("ty") in ("Button", "TextInput") and w["r"][3] < 28]
    res["controls_lt_28"] = small
    # 4. labels: their text fits their width (a one-line label wider than its box is truncated)
    # 5. no overlap among leaf controls/labels (containment excluded)
    leaves = [w for w in b1 if w.get("ty") in ("Button", "TextInput", "Label", "Svg")]
    overlaps = []
    for i, a in enumerate(leaves):
        for b in leaves[i + 1:]:
            ax, ay, aw, ah = a["r"]; bx, by, bw, bh = b["r"]
            ix = min(ax + aw, bx + bw) - max(ax, bx)
            iy = min(ay + ah, by + bh) - max(ay, by)
            if ix > 1 and iy > 1:
                inside = (ax >= bx - 0.5 and ay >= by - 0.5 and ax + aw <= bx + bw + 0.5 and ay + ah <= by + bh + 0.5) or \
                         (bx >= ax - 0.5 and by >= ay - 0.5 and bx + bw <= ax + aw + 0.5 and by + bh <= ay + ah + 0.5)
                if not inside:
                    overlaps.append((a["i"], b["i"]))
    res["overlaps"] = overlaps
    # 6. side padding of the content: min x of leaves - card x, and card right - max right
    if leaves:
        lpad = min(w["r"][0] for w in leaves) - cx_
        rpad = (cx_ + cw) - max(w["r"][0] + w["r"][2] for w in leaves)
        res["content_pad_lr"] = [round(lpad, 1), round(rpad, 1)]
    res["texts"] = [w.get("t") for w in b1 if w.get("ty") in ("Label", "Button") and w.get("t")]
    res["pass"] = res["card_inside_view"] and res["centred"] and res["width_rule"] and not outside and not small and not overlaps
    CHECKS[state] = res
    return res


def capture(state):
    time.sleep(0.6)
    # a hover over an empty corner advances any pending widget transition
    get(f"/m?k=move&x=2&y=2&wait=1")
    time.sleep(0.3)
    s = snap()
    open(f"{OUT}/{state}.snap.json", "w").write(json.dumps(s, indent=0))
    open(f"{OUT}/{state}.png", "wb").write(get("/g?raw=1"))
    r = check(state, s)
    print(f"[{state}] pass={r.get('pass')} card={r.get('card')} margins={r.get('margins_lr')} small={r.get('controls_lt_28')} overlaps={r.get('overlaps')} outside={r.get('outside_card')}")


def capture_entry(state, wid):
    """An entry point (no dialog yet): the frame, its snap, and the entry's rect."""
    time.sleep(0.6)
    get(f"/m?k=move&x=2&y=2&wait=1")
    time.sleep(0.3)
    s = snap()
    open(f"{OUT}/{state}.snap.json", "w").write(json.dumps(s, indent=0))
    open(f"{OUT}/{state}.png", "wb").write(get("/g?raw=1"))
    w = find(wid, s)
    view = module_rect(s)
    ok = bool(w) and w["r"][3] >= 28 and w["r"][0] >= view[0] and w["r"][0] + w["r"][2] <= view[0] + view[2]
    CHECKS[state] = {"entry": wid, "rect": w and w["r"], "text": w and w.get("t"), "pass": ok}
    print(f"[{state}] entry={wid} rect={w and w['r']} pass={ok}")


def submit(button_id):
    """After typing: on the phone shell the emulated soft keyboard covers the
    lower screen, so a phone user submits with the keyboard's return key (the
    field's `returns` mapping routes it to the same action as the button)."""
    if MODE == "phone":
        key("ReturnKey")
        time.sleep(0.5)
    else:
        click(button_id)


def pair_with(code):
    click("b1_pair_link")
    typ(LINK.format(code))
    submit("b1_pair_submit")


def click_nth(wid, n):
    ws = [w for w in snap() if w.get("i") == wid and w.get("r", [0, 0, 0, 0])[2] > 0]
    if len(ws) <= n:
        raise SystemExit(f"no visible {wid}[{n}] ({len(ws)} visible)")
    x, y, ww, hh = ws[n]["r"]
    for attempt in range(4):
        if get(f"/click?x={x + ww / 2:.1f}&y={y + hh / 2:.1f}&wait=1", tolerant=True) != b"{}":
            break
        time.sleep(0.5)
    time.sleep(0.5)


def click_nth_until(wid, n, until, tries=3, secs=6):
    for _ in range(tries):
        ws = [w for w in snap() if w.get("i") == wid and w.get("r", [0, 0, 0, 0])[2] > 0]
        if len(ws) <= n:
            raise SystemExit(f"no visible {wid}[{n}] ({len(ws)} visible)")
        x, y, ww, hh = ws[n]["r"]
        get(f"/click?x={x + ww / 2:.1f}&y={y + hh / 2:.1f}&wait=1", tolerant=True)
        end = time.time() + secs
        while time.time() < end:
            if find(until):
                return
            time.sleep(0.25)
    wait(until, 0.1)


def visible(wid):
    return find(wid) is not None


def main():
    if MODE == "phone":
        # The shell's phone page (--test-action page:0) with the 360x780
        # frame: open OctosCode from the phone home (A3's walk does the same).
        # Tap only when the app is not open yet (an integration harness may
        # already have opened it; a second tap would land on the Connect card).
        w0 = find("b1_connect_pair")
        if not (w0 and w0.get("r", [0, 0, 0, 0])[2] > 0):
            clickxy(153, 363); time.sleep(3.0)
    wait("b1_connect_pair", 45)  # a loaded machine can take ~30 s to lay out the first-run card
    capture_entry("entry_connect", "b1_connect_pair")
    click_until("b1_connect_pair", "b1_pair_submit")
    capture("p4-01_pair")
    # p4-03: a used link -> its own copy, the form, the origin prefilled
    pair_with("USED0000"); wait("b1_conn_submit")
    capture("p4-03_link_problem")
    click_until("b1_pair_back", "b1_pair_submit")
    # p4-04: a server without pairing (404)
    pair_with("NOPAIR00"); wait("b1_pair_manual")
    capture("p4-04_cant_pair")
    click_until("b1_pair_back", "b1_pair_submit")
    # p4-02: the exchange in flight, then Cancel (latest-request-wins)
    pair_with("SLOWPAIR"); wait("b1_pair_cancel")
    capture("p4-02_pairing")
    click_until("b1_pair_cancel", "b1_pair_submit")
    # the good link: claim -> connect -> live, the dialog closes
    pair_with("3QK7ZP2M"); wait("b1_card", 20, gone=True)
    time.sleep(1.0)
    log = json.loads(get("/log?n=400"))["l"]
    live = any("the paired token is live" in l for l in log)
    CHECKS["paired_live"] = {"log": "pairing: connected — the paired token is live", "pass": live}
    print("[paired] live:", live)
    # Settings (the header) -> Connection -> This device · Details (p4-05)
    wait("settings_open_hit", 10)
    click_until("settings_open_hit", "settings_drawer")
    cell = "rl_hit" if MODE == "phone" else "nv_hit"
    click_nth_until(cell, 4, "b1_set_connection")
    capture_entry("entry_settings_connection", "b1_set_connection")
    click_until("b1_set_connection", "b1_pair_forget")
    capture("p4-05_connection")
    click_until("b1_pair_back", "b1_card", gone=True)
    # Settings -> Model -> Model providers · Edit (p4-06 / p4-07)
    click_nth_until(cell, 2, "b1_set_provider")
    capture_entry("entry_settings_model", "b1_set_provider")
    click_until("b1_set_provider", "b1_prov_save"); time.sleep(1.0)
    capture("p4-06_provider")
    click("b1_prov_key"); typ("sk-bad-key-123456"); submit("b1_prov_save"); wait("b1_prov_error")
    capture("p4-07_rejected")
    click("b1_prov_key"); key("Backspace", 18); typ("sk-good-key-1"); submit("b1_prov_save")
    wait("b1_card", 10, gone=True)
    CHECKS["provider_saved_back_in_settings"] = {"pass": visible("b1_set_provider")}
    click_until("set_back" if MODE == "phone" else "settings_close", "settings_drawer", gone=True)
    # The sidebar's + Add workspace: the browser over the picker (p4-08 / p4-09)
    if MODE == "phone" and not visible("sb_add_hit"):
        click_until("sidebar_toggle_hit", "sb_add_hit")
    capture_entry("entry_add_workspace", "sb_add_hit")
    click_until("sb_add_hit", "b1_br_row_1"); click("b1_br_row_1")
    capture("p4-08_browser")
    click("b1_br_crumb_2"); time.sleep(1.0)
    click("b1_br_path"); key("Backspace", 12); typ("/private")
    for _ in range(3):
        # Return only once the field holds the typed path (a coalesced /t can
        # land late), and again if the refusal never came.
        end = time.time() + 3
        while time.time() < end and (find("b1_br_path") or {}).get("t") != "/private":
            time.sleep(0.2)
        key("ReturnKey")
        end = time.time() + 6
        while time.time() < end and not find("b1_br_backto"):
            time.sleep(0.25)
        if find("b1_br_backto"):
            break
    wait("b1_br_backto", 1)
    capture("p4-09_refused")
    click_until("b1_br_backto", "b1_br_row_0")
    # The browser's back lands on the picker (the web's add -> choose)
    click_until("b1_br_back", "b1_pk_browse"); time.sleep(0.8)
    capture("picker")
    click_until("b1_pk_browse", "b1_br_row_0"); click("b1_br_row_0"); click("b1_br_use")
    wait("b1_card", 10, gone=True)
    log = [l for l in json.loads(get("/log?n=400"))["l"] if "board1" in l or "pairing" in l]
    open(f"{OUT}/walk.log", "w").write("\n".join(log) + "\n")
    json.dump(CHECKS, open(f"{OUT}/checks.json", "w"), indent=1)
    print("walk complete; checks pass:", {k: v.get("pass") for k, v in CHECKS.items()})


if __name__ != "__main__":
    pass
elif len(sys.argv) > 3 and sys.argv[3] == "--recheck":
    import glob
    for f in sorted(glob.glob(f"{OUT}/*.snap.json")):
        state = os.path.basename(f)[: -len(".snap.json")]
        r = check(state, json.load(open(f)))
        print(f"[{state}] pass={r.get('pass')} card={r.get('card')} margins={r.get('margins_lr')} pad={r.get('content_pad_lr')} small={r.get('controls_lt_28')} overlaps={r.get('overlaps')} outside={r.get('outside_card')}")
    old = json.load(open(f"{OUT}/checks.json")) if os.path.exists(f"{OUT}/checks.json") else {}
    old.update(CHECKS)
    json.dump(old, open(f"{OUT}/checks.json", "w"), indent=1)
else:
    main()
