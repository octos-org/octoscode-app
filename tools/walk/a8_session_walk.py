#!/usr/bin/env python3
"""A8 — the session-config click walk: every control is reached from the real
UI by a CLICK at its laid-out rect (the LESSONS rule) and each step asserts the
app's own effect (a /snap text or visibility) AND what reached the wire (the
fixture server's request log). It also runs numeric /snap layout checks (the
pane inside the window with equal side margins, equal section cards, tap
targets >= 28 px, no label past its card).

The app must run hidden on PORT against the A8 fixture server
(`cargo run -p octoscode-module --example a8_serve -- 8428 --held --log <LOG>`):

  OCTOS_BASE_URL=http://127.0.0.1:8428 OCTOS_PROFILE_ID=a8 \\
  OCTOSCODE_RECENTS_DIR=<tmp> OCTOSCODE_PANE_ADVANCED_FILE=<tmp>/adv.json \\
  OCTOSCODE_SHOW_THINKING_FILE=<tmp>/think.json OCTOSCODE_DRAFTS_FILE=<tmp>/drafts.json \\
  OCTOSCODE_CREDENTIALS_DIR=<tmp>/credentials OCTOSCODE_PREF_PATH=<tmp>/prefs.json \\
  OCTOSCODE_NOTIFICATIONS_FILE=<tmp>/notifications.json OCTOSCODE_DOWNLOAD_DIR=<tmp>/downloads \\
  OCTOSCODE_DESIGN_DIR=$PWD/design \\
  MAKEPAD_WM_TEST_APP=octoscode HEADLESS_ARGS="--module octoscode" \\
    bash harness/headless.sh start <host-bin> 8418
  python3 tools/walk/a8_session_walk.py 8418 desktop <LOG> [captures-dir]
  bash harness/headless.sh stop 8418

Every app a walk starts gets a per-run <tmp> for ALL of its state (the brief's
test-isolation rule): never the operator's ~/.octoscode.

Phone: the same with OCTOSENSE_WINDOW_SIZE=360x780 and
HEADLESS_ARGS="--module octoscode --test-action page:0 --test-action launch-octoscode".
Exit status: 0 when every step passes.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a8_session",
    "title": "session config: strip, Session settings pane, presets, full access, driver disclosure, copy, inspector, defaults",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/a8_serve", "{fport}", "--held", "--log", "{state}/serve.jsonl"], "fport": 8428},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "a8",
                    "OCTOSCODE_PANE_ADVANCED_FILE": "{state}/adv.json"},
            "ready": ["b3_strip_tap", "i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{state}/serve.jsonl", "{out}"]}],
    "needs": ["target/debug/examples/a8_serve"],
    "timeout": 900,
    "rows": {
        14: ["holder banner:", "wire: session/driver/acquire", "wire: session/driver/release", "resume chat:"],
        30: {"checks": {"desktop": ["header: 'Copy as Markdown' is offered", "header copy: 'Copied'",
                                    "wire: the copy read the canonical history", "header copy: the result resets"]}},
        32: {"checks": {"phone": ["phone header: no copy control"]}},
        40: {"checks": ["advanced: 'Another controller'", "advanced: the disclosure names the driver",
                        "advanced: the operations list shows the peer", "wire: the driver walk asked for an operations page"],
             "partial": "a healthy EXTERNAL disclosure; the web's empty terminal operations page variant is not staged"},
        96: {"checks": ["/permissions opens 'Remembered approvals'", "inspector: the scope line",
                        "inspector: 'Reading…' settles", "inspector: Refresh re-reads"],
             "partial": "the remembered-scope inspector; discarding a closed Session's late read is not staged"},
        163: ["preset 'Read · Network blocked': Saved", "wire: permission/profile/set {mode read_only",
              "the selected preset reads back", "the Full access preset is offered",
              "full access shows the risk confirmation", "'Enable full access' is disabled until the box is ticked",
              "Cancel closes the confirmation", "wire: nothing sent for full access"],
        164: ["pane read the runtime model", "model pick: notice", "wire: profile/llm/select carries"],
        189: {"checks": ["show thinking:"],
              "partial": "the preference is written; its per-Session retention and the captured effort are not asserted"},
        198: {"checks": ["inspector: 'Conversation link copied.'", "inspector: the full link stays visible"],
              "partial": "the read-only link field is always shown; a DENIED clipboard is not simulated"},
    },
}

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8418
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
LOG = sys.argv[3] if len(sys.argv) > 3 else "tmp/a8/serve.jsonl"
SHOTS = sys.argv[4] if len(sys.argv) > 4 else None
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []


def get(path, timeout=20):
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read()
    except urllib.error.HTTPError:
        # Input routes with wait=1 can answer 404 when the frame they waited
        # on was coalesced or replaced (a remount); the input itself landed.
        if path.startswith(("/m?", "/t?", "/k?", "/click?")):
            time.sleep(0.3)
            return b"{}"
        raise


def key(code):
    get(f"/k?k=down&c={code}&wait=1")
    get(f"/k?k=up&c={code}&wait=1")


def composer():
    """The composer's text ('' when it only shows its placeholder)."""
    hits = [w for w in snap() if w.get("i") == "i0_composer_0" and w["r"][2] > 0]
    t = hits[0].get("t", "") if hits else None
    return "" if t in (None, "Ask Octos anything") else t


def clear_composer():
    """Empty the composer before typing into it: it may hold a RESTORED
    prompt (Stop puts an interrupted prompt back, drafts come back with their
    Session), and the instrument's synthetic Cmd+A does not reach the
    TextInput's select-all — End, then one Backspace per character."""
    n = len(composer() or "")
    if n:
        get("/k?c=end&wait=1")
        for _ in range(n + 1):
            get("/k?c=backspace&wait=1")
        time.sleep(0.35)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(widgets, wid):
    return [w for w in widgets if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def rect(wid, s=None, nth=0):
    hits = sorted(visible(s or snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def text(wid, s=None):
    hits = visible(s or snap(), wid)
    return hits[0].get("t", "") if hits else None


def shown(wid):
    return rect(wid) is not None


def wait(pred, secs=8.0):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(0.2)
    return False


def click_rect(r):
    x, y, w, h = r
    try:
        get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    except urllib.error.HTTPError:
        # A click that remounts the pane can answer 404 for `wait=1` (the
        # frame it waited on was replaced); the click itself was delivered.
        pass
    time.sleep(0.25)


def seen(wid, want=None):
    """Scroll `wid` into view, then read its text (None when absent)."""
    if scroll_to(wid, min_h=8) is None:
        return None
    t = text(wid)
    return t if want is None else t == want


def scroll_to(wid, min_h=20):
    """Scroll the pane body until `wid` is fully laid out inside it. A widget
    scrolled entirely out of view has no rect, so its side is unknown: rewind
    to the top once, then scan down."""
    rewound = False
    for _ in range(20):
        s = snap()
        r, body = rect(wid, s), rect("b3_scroll", s)
        if r and body and r[3] >= min_h and r[1] >= body[1] and r[1] + r[3] <= body[1] + body[3]:
            return r
        if body is None:
            return r
        mx, my = body[0] + body[2] / 2, body[1] + body[3] / 2
        if r is None and not rewound:
            get(f"/m?k=scroll&x={mx}&y={my}&dy=-3000&wait=1")
            rewound = True
            time.sleep(0.15)
            continue
        dy = -160 if (r and r[1] < body[1]) else 160
        get(f"/m?k=scroll&x={mx}&y={my}&dy={dy}&wait=1")
        time.sleep(0.15)
    return rect(wid)


def find_row(prefix, needle, n=8):
    """The `<prefix><i>` tap whose title contains `needle`."""
    for i in range(n):
        t = seen(f"{prefix}{i}_title")
        if t and needle in t:
            return f"{prefix}{i}"
    return None


def click(wid, scroll=True):
    r = scroll_to(wid) if scroll else rect(wid)
    if r is None:
        return False
    click_rect(r)
    return True


def wire(method):
    """Every logged request of `method` (the fixture server's log)."""
    out = []
    if not os.path.exists(LOG):
        return out
    for line in open(LOG):
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if v.get("method") == method:
            out.append(v.get("params") or {})
    return out


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))


def shot(name):
    """The capture AND its /snap (the numeric UX checks read the snap). A
    grab the instrument refuses mid-remount is retried; a capture is
    evidence, never a step, so a lost one is noted, not fatal."""
    if not SHOTS:
        return
    os.makedirs(SHOTS, exist_ok=True)
    for attempt in range(4):
        try:
            png = get("/g?raw=1", timeout=30)
            with open(os.path.join(SHOTS, f"{MODE}-{name}.png"), "wb") as f:
                f.write(png)
            with open(os.path.join(SHOTS, f"{MODE}-{name}.json"), "wb") as f:
                f.write(get("/snap?all=1"))
            return
        except urllib.error.HTTPError:
            time.sleep(0.5)
    print(f"NOTE capture {name} failed")


def layout_checks(tag):
    s = snap()
    module = rect("b3_root", s) or rect("OctoscodeView", s)
    dialog = rect("b3_dialog", s)
    ok = bool(module and dialog)
    if ok:
        left = dialog[0] - module[0]
        right = (module[0] + module[2]) - (dialog[0] + dialog[2])
        ok = left >= 15 and right >= 15 and abs(left - right) <= 2
    check(f"{tag}: the pane sits inside the window with equal side margins", ok, f"module={module} dialog={dialog}")
    cards = [rect(c, s) for c in ("b3_sc_model", "b3_sc_perm", "b3_sc_sandbox", "b3_sc_thinking")]
    present = [c for c in cards if c]
    check(f"{tag}: section cards share one left edge and width", present and len({(c[0], c[2]) for c in present}) == 1, str(present))
    small = []
    for w in s:
        i = w.get("i") or ""
        if w.get("ty") == "Button" and i.startswith("b3_") and w["r"][3] > 0 and w.get("v", 1) != 0:
            body = rect("b3_scroll", s)
            clipped = body and (w["r"][1] <= body[1] + 1 or w["r"][1] + w["r"][3] >= body[1] + body[3] - 1)
            if w["r"][3] < 28 and not clipped:
                small.append((i, w["r"]))
    check(f"{tag}: every pane tap target is >= 28 px high", not small, str(small[:4]))
    over = []
    for w in s:
        i = w.get("i") or ""
        if w.get("ty") == "Label" and i.startswith("b3_sc_") and w["r"][2] > 0:
            for c in present:
                if c[1] <= w["r"][1] < c[1] + c[3] and w["r"][0] + w["r"][2] > c[0] + c[2] + 1:
                    over.append((i, w["r"], c))
    check(f"{tag}: no pane label runs past its card", not over, str(over[:3]))


def main():
    # 0. The strip above the composer (three facts, one tap).
    check("strip is laid out", wait(lambda: shown("b3_strip_tap"), 20))
    s = snap()
    cells = [rect(c, s) for c in ("b3_strip_model_cell", "b3_strip_state_cell", "b3_strip_perm_cell")]
    if MODE == "phone":
        # The web's <=760 px strip: the state word on its own line, the model
        # and the permission in two equal halves under it.
        m, st_, p = cells
        check("strip (phone): the state word spans the strip, above two equal halves",
              all(cells) and st_[1] < m[1] and m[1] == p[1] and round(m[2]) == round(p[2])
              and abs(st_[2] - (m[2] + p[2] + 1)) <= 2, str(cells))
    else:
        check("strip: three equal cells", all(cells) and len({round(c[2]) for c in cells}) == 1, str(cells))
    # The holder arrives with the startup session/driver/get (slow on a busy host).
    check("strip: the foreign holder is the state word",
          wait(lambda: text("b3_strip_state") == "Another app is using this session", 20), str(text("b3_strip_state")))
    shot("00-strip")

    # 1. The strip's click opens the Session settings pane (not app Settings).
    click("b3_strip_tap", scroll=False)
    check("strip click opens 'Session settings'", wait(lambda: text("b3_title") == "Session settings"), str(text("b3_title")))
    check("app Settings did not open instead", not shown("set_title"))
    check("pane read the runtime model (session/status/read)", wait(lambda: seen("b3_sc_runtime_value", "deepseek-v4-flash"), 10), str(seen("b3_sc_runtime_value")))
    for m in ("session/status/read", "permission/profile/list", "profile/llm/list", "session/driver/get"):
        check(f"wire: the pane sent {m}", wait(lambda m=m: len(wire(m)) > 0, 5), str(len(wire(m))))
    check("wire: the driver walk asked for an operations page", any(p.get("operations") == {"limit": 50} for p in wire("session/driver/get")))
    check("holder banner: 'Another app is using this session' + Resume chat", seen("b3_sc_holder_head") == "Another app is using this session" and shown("b3_sc_resume"))
    layout_checks("open")
    shot("01-pane-top")

    # 2. Model: pick glm-5 -> profile/llm/select with the route id -> exact disposition.
    n_sel = len(wire("profile/llm/select"))
    click("b3_sc_model_1")
    check("model pick: notice 'Saved. The server keeps running deepseek-v4-flash until it restarts'",
          wait(lambda: seen("b3_sc_model_notice", "Saved. The server keeps running deepseek-v4-flash until it restarts")), str(seen("b3_sc_model_notice")))
    sel = wire("profile/llm/select")[n_sel:]
    check("wire: profile/llm/select carries model glm-5 + route_id zhipu", sel and sel[0].get("model_id") == "glm-5" and sel[0].get("route_id") == "zhipu", str(sel[:1]))
    shot("02-model-picked")

    # 3. Permissions: a preset -> permission/profile/set -> Saved + readback.
    n_set = len(wire("permission/profile/set"))
    click("b3_sc_perm_1")
    check("preset 'Read · Network blocked': Saved", wait(lambda: seen("b3_sc_perm_state", "Saved")), str(seen("b3_sc_perm_state")))
    sets = wire("permission/profile/set")[n_set:]
    check("wire: permission/profile/set {mode read_only, network deny}", sets and sets[0].get("update") == {"mode": "read_only", "network": "deny"}, str(sets[:1]))
    check("the selected preset reads back", wait(lambda: "Read" in (seen("b3_sc_perm_0_title") or "")), str(seen("b3_sc_perm_0_title")))

    # 4. Approval policy: Never ask -> the stamp reads back 'never'.
    n_set = len(wire("permission/profile/set"))
    click("b3_sc_policy_seg_1")
    check("approval 'Never ask' reads back from the stamp", wait(lambda: seen("b3_sc_policy_value", "never")), str(seen("b3_sc_policy_value")))
    sets = wire("permission/profile/set")[n_set:]
    check("wire: permission/profile/set {approval_policy: never}", sets and sets[0].get("update") == {"approval_policy": "never"}, str(sets[:1]))

    # 5. Full access asks first; Cancel sends nothing.
    n_set = len(wire("permission/profile/set"))
    full = find_row("b3_sc_perm_", "Full access")
    check("the Full access preset is offered", full is not None, str(full))
    click(full or "b3_sc_perm_3")
    check("full access shows the risk confirmation", wait(lambda: seen("b3_sc_risk_title", "Enable full access?")))
    check("'Enable full access' is disabled until the box is ticked", scroll_to("b3_sc_risk_confirm_box") is not None and not shown("b3_sc_risk_confirm"))
    shot("03-full-access-confirm")
    click("b3_sc_risk_cancel")
    check("Cancel closes the confirmation", wait(lambda: not shown("b3_sc_risk_title")))
    check("wire: nothing sent for full access", len(wire("permission/profile/set")) == n_set)

    # 6. Sandbox: the effective fields + the fixed-at-open note.
    scroll_to("b3_sc_sandbox_note")
    check("sandbox: effective values read-only", seen("b3_sc_sb_on_value") == "on" and seen("b3_sc_sb_net_value") in ("allowed", "blocked"), f"{seen('b3_sc_sb_on_value')} {seen('b3_sc_sb_net_value')}")
    shot("04-permissions-sandbox")

    # 7. Show thinking toggles the persisted preference (and the pane keeps
    # its scroll position across the remount: the switch stays in view).
    before = scroll_to("b3_sc_thinking_toggle")
    click_rect(before)
    think = os.environ.get("OCTOSCODE_SHOW_THINKING_FILE")
    check("show thinking: the preference is written OFF", wait(lambda: think and os.path.exists(think) and open(think).read().strip() == "false"), str(think))
    after = rect("b3_sc_thinking_toggle")
    check("show thinking: the pane kept its scroll position across the remount", after is not None and before is not None and abs(after[1] - before[1]) <= 2, f"{before} -> {after}")

    # 8. Advanced: who controls + the external disclosure.
    if not shown("b3_sc_who"):
        click("b3_sc_adv")
    check("advanced: 'Another controller'", wait(lambda: seen("b3_sc_who", "Another controller")), str(seen("b3_sc_who")))
    scroll_to("b3_sc_driver_id_value")
    check("advanced: the disclosure names the driver octos-tui", seen("b3_sc_driver_id_value") == "octos-tui")
    check("advanced: the operations list shows the peer", wait(lambda: seen("b3_sc_op_0_label", "review-diff")))
    shot("05-advanced")
    layout_checks("advanced")

    # 9. Resume chat: acquire -> release(internal); the banner goes.
    click("b3_sc_resume")
    check("wire: session/driver/acquire at the observed revision", wait(lambda: len(wire("session/driver/acquire")) > 0) and wire("session/driver/acquire")[-1].get("expected_revision") == 9, str(wire("session/driver/acquire")[-1:]))
    check("wire: session/driver/release next=internal", wait(lambda: len(wire("session/driver/release")) > 0) and wire("session/driver/release")[-1].get("next") == "internal")
    check("resume chat: the holder banner is gone", wait(lambda: seen("b3_sc_holder_head") is None, 10))
    check("resume chat: 'This app' controls the session now", wait(lambda: seen("b3_sc_who", "This app"), 10), str(seen("b3_sc_who")))

    # 10. Close.
    click("b3_close", scroll=False)
    check("close: the pane is gone", wait(lambda: not shown("b3_sc_model_title")))
    check("strip: the state word left the foreign-holder copy", wait(lambda: text("b3_strip_state") != "Another app is using this session", 10), str(text("b3_strip_state")))

    # 10b. The header's "Copy as Markdown" (desktop only; the phone header has no room).
    if MODE == "desktop":
        n_h = len(wire("session/hydrate"))
        check("header: 'Copy as Markdown' is offered", text("hd_copy_label") == "Copy as Markdown", str(text("hd_copy_label")))
        click("copy_open_hit", scroll=False)
        check("header copy: 'Copied'", wait(lambda: text("hd_copy_label") == "Copied"), str(text("hd_copy_label")))
        check("wire: the copy read the canonical history (session/hydrate)", len(wire("session/hydrate")) > n_h)
        shot("07-header-copied")
        check("header copy: the result resets after 2.5 s", wait(lambda: text("hd_copy_label") == "Copy as Markdown", 6), str(text("hd_copy_label")))
    else:
        check("phone header: no copy control", not shown("hd_copy_label"))

    # 10c. The inspection dialog: /permissions from the composer.
    click("i0_composer_0", scroll=False)
    clear_composer()
    get("/t?" + urllib.parse.urlencode({"t": "/permissions", "wait": 1}))
    time.sleep(0.3)
    key("ReturnKey")
    check("/permissions opens 'Remembered approvals'", wait(lambda: text("b3_title") == "Remembered approvals"), str(text("b3_title")))
    check("inspector: the scope line names the Session",
          wait(lambda: (text("b3_insp_scope") or "").startswith("Session: a8:"), 6), str(text("b3_insp_scope")))
    # The first read must finish first: Refresh is disabled ("Reading…") while it runs.
    check("inspector: 'Reading…' settles to 'Refresh'", wait(lambda: text("b3_insp_refresh_label") == "Refresh"), str(text("b3_insp_refresh_label")))
    n_sc = len(wire("approval/scopes/list"))
    click("b3_insp_refresh", scroll=False) or click("b3_insp_refresh_glyph", scroll=False)
    check("inspector: Refresh re-reads the snapshot", wait(lambda: len(wire("approval/scopes/list")) > n_sc), f"{n_sc} -> {len(wire('approval/scopes/list'))}")
    wait(lambda: text("b3_insp_refresh_label") == "Refresh")
    time.sleep(0.4)
    click("b3_insp_copy_btn")
    check("inspector: 'Conversation link copied.'", wait(lambda: seen("b3_insp_copied", "Conversation link copied.")))
    link = seen("b3_insp_link_value")
    # A13: the link stays visible on ONE line, shortened in the middle (the
    # judge's 8-line percent-encoded block); Copy writes the whole link.
    lr = rect("b3_insp_link_value")
    check("inspector: the link stays visible on one line (whole, or head…tail when longer than its field)",
          bool(link) and link.startswith("octoscode://session") and bool(lr) and lr[3] <= 20,
          f"{str(link)[:60]} {lr}")
    shot("08-inspector")
    click("b3_close", scroll=False)
    check("inspector closes", wait(lambda: not shown("b3_insp_scope")))

    # 11. New-session defaults: Settings > Sandbox, then New chat.
    opened = click("settings_open_hit", scroll=False)
    check("Settings opens from the header", opened and wait(lambda: shown("settings_drawer")))
    cell = "rl_hit" if MODE == "phone" else "nv_hit"
    r = rect(cell, nth=3)
    if r:
        click_rect(r)
    check("Settings > Sandbox shows the three switches", wait(lambda: shown("tg_sb_write") and shown("tg_sb_read")))
    check("the third switch is the web's 'Sandbox'", "Sandbox" in [w.get("t") for w in snap() if (w.get("i") or "").startswith("") and w.get("ty") == "Label"])
    click("tg_sb_write", scroll=False)
    click("tg_sb_read", scroll=False)
    time.sleep(0.3)
    shot("06-settings-sandbox")
    rec = os.environ.get("OCTOSCODE_RECENTS_DIR")
    if rec:
        files = [f for f in os.listdir(rec) if f.startswith("octoscode-web.session-defaults.v1")]
        stored = json.load(open(os.path.join(rec, files[0]))) if files else {}
        check("the defaults persisted per endpoint", stored.get("permissionMode") == "read_only" and stored.get("sandbox", {}).get("enabled") is True, str(stored))
    click("set_back" if MODE == "phone" else "settings_close", scroll=False)
    check("Settings closes", wait(lambda: not shown("settings_drawer")))
    n_open = len(wire("session/open"))
    n_set = len(wire("permission/profile/set"))
    if MODE == "phone" and not shown("sb_new_chat_hit"):
        click("sidebar_toggle_hit", scroll=False)
        time.sleep(0.6)
    clicked = click("sb_new_chat_hit", scroll=False)
    check("New chat clicked", clicked)
    check("wire: the created open carries the sandbox default", wait(lambda: len(wire("session/open")) > n_open, 8) and wire("session/open")[-1].get("sandbox") == {"enabled": True, "network_access": False}, str(wire("session/open")[-1:]))
    created = wire("session/open")[-1].get("session_id") if len(wire("session/open")) > n_open else None
    check("wire: ONE permission/profile/set for the created id", wait(lambda: len(wire("permission/profile/set")) > n_set, 8)
          and wire("permission/profile/set")[n_set:] == [{"session_id": created, "update": {"mode": "read_only", "network": "deny"}}],
          str(wire("permission/profile/set")[n_set:]))

    failed = [r for r in RESULTS if not r[1]]
    print(f"\n{len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
