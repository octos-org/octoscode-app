#!/usr/bin/env python3
"""A9 — the Settings connection walk: every A9 Settings control is reached by
a CLICK at its laid-out rect and each step asserts the app's effect (/snap,
the routed log line).

  General: the Octos server row (state + dot + origin), Current workspace,
  Profile; Connection: Disconnect (nothing to lose -> at once; the Connect
  card returns on the same server), reconnect, then Forget server with an
  unsent draft -> the leave confirmation (Cancel keeps everything; Confirm
  forgets the address + token and returns to the default Connect card).

Run against the replay server's `activity` scenario (any recorded handshake
with a workspace root works), the same way as a9_activity_walk.py:

  python3 tools/walk/a9_settings_walk.py 8419 desktop tmp/replay-8429.log [shots]
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a9_activity_walk as w  # noqa: E402  (shares PORT/MODE/argv)

check, soon, snap, rect, visible, is_shown, text_of, click, click_rect, key, type_text, log_since, shot = (
    w.check, w.soon, w.snap, w.rect, w.visible, w.is_shown, w.text_of, w.click, w.click_rect,
    w.key, w.type_text, w.log_since, w.shot,
)
MODE = w.MODE
CELL = "rl_hit" if MODE == "phone" else "nv_hit"
# The replay server this walk runs against (A9's recipe: 8429). A11: the walk
# aggregator allocates the port (8429 can be another agent's) and says so.
SERVE = os.environ.get("A9_SERVE_PORT", "8429")

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a9_settings",
    "title": "Settings General (server row, workspace, profile) + Connection (Disconnect, Forget server + confirm)",
    "modes": ["desktop", "phone"],
    # The walk asserts the server it was run against (A9_SERVE_PORT, the
    # aggregator's fixture port) and a known workspace (the session opened in
    # OCTOS_WORKSPACE_CWD): A9's recipe.
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "activity"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "a9walk",
                    "OCTOS_WORKSPACE_CWD": "/home/user/src/octos"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"], "env": {"A9_SERVE_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 600,
    "rows": {
        165: ["General: the state reads Connected", "General: the origin is the connected server",
              "General: Current workspace shows", "General: Profile shows the opened Profile",
              "Disconnect acts at once", "…closes the transport and returns to the Connect card",
              "…on the same server (remembered)", "Connect reconnects to the remembered server",
              "Confirm forgets: transport closed, the Connect card returns",
              "…on the default address (the saved one is gone)"],
        234: {"checks": ["Forget with an unsent draft asks first", "…'Forget this server?' with the draft warning",
                         "Cancel closes the confirmation and keeps the connection", "Confirm forgets:"],
              "partial": "the confirmation is walked for Forget with an unsent draft; Disconnect / Forget during a "
                         "RUNNING turn are not staged"},
    },
}
SECTIONS = ["general", "permissions", "model", "sandbox", "connection", "preferences", "about"]


def open_settings():
    if not click("settings_open_hit"):
        return False
    return soon(lambda: is_shown("settings_drawer"))


def section(name):
    click(CELL, SECTIONS.index(name))
    return soon(lambda: is_shown(f"sec_{name}"))


def inside(child, parent, s):
    c, p = rect(child, s=s), rect(parent, s=s)
    if c is None or p is None:
        return False
    return c[0] >= p[0] - 0.5 and c[1] >= p[1] - 0.5 and c[0] + c[2] <= p[0] + p[2] + 0.5 and c[1] + c[3] <= p[1] + p[3] + 0.5


def general_checks():
    s = snap()
    check("General: the state reads Connected", text_of("set_server_status", s) == "Connected", f"{text_of('set_server_status', s)!r}")
    check("General: the green dot shows (one dot)",
          is_shown("set_server_dot_ok", s) and not any(is_shown(f"set_server_dot_{d}", s) for d in ("busy", "err", "idle")))
    check("General: the origin is the connected server", text_of("set_server_value", s) == f"127.0.0.1:{SERVE}", f"{text_of('set_server_value', s)!r}")
    check("General: Current workspace shows (known)", is_shown("set_ws_row", s) and bool(text_of("set_ws_value", s)), f"{text_of('set_ws_value', s)!r} {text_of('set_ws_path', s)!r}")
    check("General: Profile shows the opened Profile", text_of("set_profile_value", s) == "a9walk", f"{text_of('set_profile_value', s)!r}")
    body = rect("set_body", s=s)
    bad = []
    for wid in ("set_server_row", "set_ws_row", "set_profile_row", "set_stop_row", "tg_notify", "set_theme"):
        r = rect(wid, s=s)
        if r and body and r[1] + r[3] > body[1] + body[3] + 0.5:
            bad.append((wid, r))
    check("General: every row fits the body (no overflow)", not bad, f"body {body} {bad}")
    # The state and the origin never collide on the row.
    st, ov = rect("set_server_status", s=s), rect("set_server_value", s=s)
    check("General: state and origin do not overlap",
          st and ov and (st[0] + st[2] <= ov[0] or st[1] + st[3] <= ov[1]), f"{st} {ov}")
    for wid in ("set_ws_value", "set_profile_value", "set_server_value"):
        check(f"General: {wid} inside the body", inside(wid, "set_body", s), f"{rect(wid, s=s)}")


def connection_checks():
    s = snap()
    small = [(i, rect(i, s=s)) for i in ("settings_disconnect", "settings_forget") if rect(i, s=s) and rect(i, s=s)[3] < 28]
    check("Connection: Disconnect / Forget server are >= 28 px targets", not small and rect("settings_forget", s=s) is not None, f"{small}")
    for wid in ("settings_disconnect", "settings_forget"):
        check(f"Connection: {wid} inside the body", inside(wid, "set_body", s), f"{rect(wid, s=s)}")


def connected():
    return soon(lambda: is_shown("i0_composer_0") and not is_shown("connect_card"), tries=60)


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        w.get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    check("connected to the replay server", connected())
    log_since()
    check("Settings opens from the header", open_settings())
    check("General shows", section("general"))
    general_checks()
    shot(f"{MODE}-settings-general")
    check("Connection shows", section("connection"))
    connection_checks()
    shot(f"{MODE}-settings-connection")

    # Disconnect with nothing to lose: at once (no confirmation).
    log_since()
    click("settings_disconnect")
    lines = log_since()
    check("Disconnect acts at once (no work, no draft)",
          any("a9 leave Disconnect" in l and "-> now" in l for l in lines) and not is_shown("a9_lv_dialog"), "")
    check("…closes the transport and returns to the Connect card",
          soon(lambda: is_shown("connect_card")) and any("a9 leave done: Disconnect" in l for l in lines + log_since()), "")
    srv = w.visible(snap(), "connect_server")
    check("…on the same server (remembered)", srv and srv[0].get("t") == f"http://127.0.0.1:{SERVE}", f"{srv and srv[0].get('t')!r}")
    shot(f"{MODE}-after-disconnect")
    # Reconnect from the card.
    click("connect_btn")
    check("Connect reconnects to the remembered server", connected())

    # An unsent draft: Forget asks first.
    comp = rect("i0_composer_0")
    if comp:
        click_rect(comp)
    w.clear_composer()
    type_text("unsent idea")
    check("Settings reopens", open_settings())
    section("connection")
    log_since()
    click("settings_forget")
    check("Forget with an unsent draft asks first", soon(lambda: is_shown("a9_lv_dialog")))
    s = snap()
    check("…'Forget this server?' with the draft warning",
          text_of("a9_lv_title", s) == "Forget this server?"
          and text_of("a9_lv_p1", s) == "This input has not been saved. Copy it before leaving this conversation.",
          f"{text_of('a9_lv_title', s)!r} {text_of('a9_lv_p1', s)!r}")
    dlg = rect("a9_lv_dialog", s=s)
    check("…the confirmation is centred, min(440, w - 40) wide",
          dlg is not None and abs(dlg[2] - min(440, (rect("a9_lv_root", s=s) or [0, 0, 0, 0])[2] - 40)) <= 1, f"{dlg}")
    small = [(i, rect(i, s=s)) for i in ("a9_lv_cancel", "a9_lv_confirm") if not rect(i, s=s) or rect(i, s=s)[3] < 44]
    check("…its buttons are the web's 44 px", not small, f"{small}")
    shot(f"{MODE}-forget-confirm")
    click("a9_lv_cancel")
    check("Cancel closes the confirmation and keeps the connection",
          soon(lambda: not is_shown("a9_lv_dialog")) and is_shown("settings_drawer") and not is_shown("connect_card"))
    click("settings_forget")
    soon(lambda: is_shown("a9_lv_dialog"))
    log_since()
    click("a9_lv_confirm")
    lines = log_since()
    check("Confirm forgets: transport closed, the Connect card returns",
          soon(lambda: is_shown("connect_card")) and any("a9 leave done: Forget" in l for l in lines + log_since()), "")
    srv = w.visible(snap(), "connect_server")
    # A21: Forget returns to the web's initialConnection, whose address is
    # the DEFAULT endpoint (`connection-bootstrap.ts:11-23`) — this launch's
    # OCTOS_BASE_URL, the build's VITE_OCTOS_DEFAULT_ENDPOINT (the built-in
    # http://127.0.0.1:50190 when unset). That the saved address and token
    # are gone is walked by tools/walk/a21_bootstrap_walk.py (relaunch: no
    # restore, no token for any origin).
    default = os.environ.get("A9_DEFAULT_ENDPOINT", f"http://127.0.0.1:{SERVE}")
    check("…on the default address (the saved one is gone)", srv and srv[0].get("t") == default, f"{srv and srv[0].get('t')!r}")
    shot(f"{MODE}-after-forget")

    failed = [n for n, ok, _ in w.RESULTS if not ok]
    print(f"== WALK a9 settings {MODE}: {len(w.RESULTS) - len(failed)}/{len(w.RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
