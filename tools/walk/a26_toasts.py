#!/usr/bin/env python3
"""A26 — the error toasts walk (parity row error/error "error toasts
(transient, bounded queue)"), by CLICKS, against a server that REALLY refuses:
replay_serve `--refuse session/open@1` (the launch's open is accepted, every
later one refused — a New chat whose open is refused, which before A26 the
window never reported).

Checks: the toast shows the plain lead and the server's reason, under the
header and right-aligned in the conversation (full width on a phone), clear
of the composer (which still takes typing); four failures keep three and say
one was pushed out; a modal (Settings; the phone drawer) HOLDS the stack
(hidden) and it returns when the modal closes; × dismisses one; the rest
leave on their own; the app log announces each arrival, push-out, dismissal
and expiry; the same in the dark theme.
"""
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a26_lib as L  # noqa: E402

WALK = {
    "name": "a26_toasts",
    "title": "Error toasts: a refused New chat says so; bounded (3 + note), held under a modal, dismissible, transient",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "history", "--refuse", "session/open@1"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "dsflash",
                    "OCTOS_BEARER": "replay", "OCTOSCODE_THEME": "light"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

LEAD = "Couldn't start a new chat."


def toasts(w, sn=None):
    """The shown toasts, newest first: (id, lead, cause, card rect, × rect)."""
    sn = sn if sn is not None else w.snap()
    out = []
    for x in sn:
        m = re.fullmatch(r"a26_toast_lead_(\d+)", str(x.get("i", "")))
        if m and w.shown(x):
            n = m.group(1)
            out.append((int(n), x.get("t"), w.text(f"a26_toast_cause_{n}", sn=sn), w.rect(f"a26_toast_{n}", sn=sn),
                        w.rect(f"a26_toast_x_{n}", sn=sn)))
    return sorted(out, key=lambda t: t[3][1] if t[3] else 0)


def new_chat(w):
    if w.mode == "phone":
        L.open_drawer(w)
    w.click("sb_new_chat_hit")
    time.sleep(0.4)


def main():
    w = L.walk_from_argv()
    mode = w.mode
    w.mark()
    w.check("no toast before anything failed", not w.visible("toast_dock") and not toasts(w))
    # 1 — a refused New chat.
    new_chat(w)
    ok = w.wait(lambda: len(toasts(w)) == 1, 8)
    shown = toasts(w)
    w.check("a refused New chat raises one toast", ok, f"{[(t[1], t[2]) for t in shown]}")
    if not shown:
        sys.exit(w.summary())
    _, lead, cause, card, x = shown[0]
    w.check("its lead says what failed, plainly", lead == LEAD, repr(lead))
    w.check("its cause is the server's own reason", bool(cause) and "refused session/open #2" in cause, repr(cause))
    w.check("the server really refused it (the wire)", w.replay_saw("session/open", 4) >= 2
            and "session/open refused" in (w.replay_log.read_text() if w.replay_log else ""))
    w.check("the arrival is announced in the app log", w.logged(f"toast: {LEAD}", 4))
    sn = w.snap()
    col, head, comp = w.rect("conversation_column", sn=sn), w.rect("oc_header", sn=sn), w.rect("composer_row", sn=sn)
    module = w.module_rect(sn)
    w.check("the toast sits inside the conversation, under its header",
            bool(card and col and head and L.inside(card, col) and card[1] >= head[1] + head[3]), f"card={card} col={col} head={head}")
    if mode == "desktop":
        w.check("right-aligned in the conversation (16 px inset), at most 360 px",
                bool(card and col) and abs((col[0] + col[2] - 16) - (card[0] + card[2])) <= 2 and card[2] <= 361, f"card={card} col={col}")
    else:
        w.check("full width on the phone (12 px gutters)",
                bool(card and module) and abs(card[0] - (module[0] + 12)) <= 2 and abs((module[0] + module[2] - 12) - (card[0] + card[2])) <= 2,
                f"card={card} module={module}")
    w.check("clear of the composer", bool(card and comp) and card[1] + card[3] <= comp[1] - 8, f"card={card} composer={comp}")
    w.check("× is a >= 28 px target inside the toast", bool(x and card and x[2] >= 28 and x[3] >= 28 and L.inside(x, card)), f"{x}")
    for wid in (f"a26_toast_lead_{shown[0][0]}", f"a26_toast_cause_{shown[0][0]}"):
        r = w.rect(wid, sn=sn)
        w.check(f"{wid} lies whole inside its toast", bool(r and card and L.inside(r, card)), f"{r} in {card}")
    w.shot(f"{mode}-toast-one-light")
    # The composer still takes typing with a toast up.
    c = w.composer()
    if c:
        cx, cy, cw, ch = c["r"]
        w.click_xy(cx + cw / 2, cy + ch / 2)
        w.type_text("still typing")
        typed = (w.composer() or {}).get("val") or ""
        w.check("the composer still takes typing under a toast", "still typing" in typed, repr(typed))
        w.clear_field(14)
        w.dismiss_keyboard()
    # 2 — bounded: four failures keep three and say so.
    for _ in range(3):
        new_chat(w)
    ok = w.wait(lambda: len(toasts(w)) == 3 and w.text("a26_toasts_dropped") == "1 earlier error no longer shown", 8)
    shown = toasts(w)
    w.check("four failures keep three toasts and say one was pushed out", ok,
            f"{len(shown)} shown; note={w.text('a26_toasts_dropped')!r}")
    w.check("the newest is on top", bool(shown) and "#5" in (shown[0][2] or ""), f"{[t[2] for t in shown]}")
    w.check("the push-out is announced", w.logged("toast pushed out", 4))
    sn = w.snap()
    stack, comp = w.rect("a26_toasts", sn=sn), w.rect("composer_row", sn=sn)
    w.check("the whole stack stays clear of the composer", bool(stack and comp) and stack[1] + stack[3] <= comp[1] - 8,
            f"stack={stack} composer={comp}")
    w.shot(f"{mode}-toast-stack-light")
    # 3 — held under a modal.
    if mode == "phone":
        L.open_drawer(w)
        w.check("the open drawer holds the stack (hidden under it)", w.wait(lambda: not w.visible("toast_dock"), 4))
        w.click("drawer_close")
        w.check("…and it returns when the drawer closes", w.wait(lambda: bool(w.visible("toast_dock")), 4))
    w.check("Settings opens from the footer", L.open_settings_from_footer(w))
    w.check("Settings holds the stack: no toast over the dialog", w.wait(lambda: not w.visible("toast_dock"), 4))
    w.shot(f"{mode}-toast-held-settings")
    L.close_settings(w)
    w.check("the stack returns when Settings closes", w.wait(lambda: len(toasts(w)) == 3, 4))
    # 4 — × dismisses one.
    shown = toasts(w)
    w.mark()
    r = shown[0][4]
    w.note(f"CLICK × on the newest toast r={r}")
    w.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    w.check("× dismisses that toast", w.wait(lambda: len(toasts(w)) == 2, 4) and w.logged("toast dismissed", 4))
    # 5 — transient: the rest leave on their own.
    w.check("the rest leave on their own (8 s on screen)", w.wait(lambda: not w.visible("toast_dock"), 14))
    w.check("their expiry is announced", w.logged("toast expired", 4))
    # 6 — the dark theme.
    if mode == "phone":
        L.open_drawer(w)
    for _ in range(2):
        w.click("sb_theme_hit")
        time.sleep(0.8)
    w.check("the footer's toggle reaches Dark", w.wait(lambda: w.text("sb_theme_label") == "Dark", 4))
    if mode == "phone":
        w.click("drawer_close")
        time.sleep(0.6)
    new_chat(w)
    ok = w.wait(lambda: len(toasts(w)) == 1, 8)
    w.check("dark: a refused New chat raises its toast", ok)
    time.sleep(0.5)
    sn = w.snap()
    shown = toasts(w, sn)
    if shown:
        card = shown[0][3]
        px = L.pixel(w, card[0] + card[2] - 40, card[1] + card[3] - 8)
        # The card is RAISED over a dark window (the dark twin of the chip
        # fill, #2C2C2E), never the window's own #1C1F22.
        w.check("dark: the toast card is drawn raised on the dark window (#2C2C2E)",
                px is not None and L.near(px, L.hex_rgb("#2c2c2e"), 8), f"pixel {px}")
    w.shot(f"{mode}-toast-one-dark")
    new_chat(w)
    w.wait(lambda: len(toasts(w)) == 2, 6)
    w.shot(f"{mode}-toast-stack-dark")
    # Restore the light theme.
    if mode == "phone":
        L.open_drawer(w)
    w.click("sb_theme_hit")
    w.check("back to Light", w.wait(lambda: w.text("sb_theme_label") == "Light", 4))
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
