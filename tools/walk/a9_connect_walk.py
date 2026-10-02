#!/usr/bin/env python3
"""A9 — the Connect failure walk (§5.1 told honestly). The app starts on a
closed port, so it shows A1's Connect card; then by CLICKS and typing:

  1. an unreachable address -> "Can't reach <address>" + its actions;
  2. a reachable server with a WRONG token (the live server answers 401) ->
     "The server refused this token" + "Re-enter the token · Retry", the
     typed token kept and the token field focused (typing lands in it).

No real token is used and no turn is started.

  a9_walk.sh a9_connect_walk.py desktop <shots> OCTOS_BASE_URL=http://127.0.0.1:9
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a9_activity_walk as w  # noqa: E402

check, soon, snap, rect, is_shown, text_of, click, key, type_text, log_since, shot = (
    w.check, w.soon, w.snap, w.rect, w.is_shown, w.text_of, w.click, w.key, w.type_text, w.log_since, w.shot,
)
MODE = w.MODE
LIVE = os.environ.get("A9_LIVE_SERVER", "http://127.0.0.1:50190")


def set_field(wid, text, clear=48):
    click(wid)
    # A click lands the caret where it hit (mid-text in a narrow field):
    # clear forwards AND backwards from there.
    for _ in range(clear):
        key("Delete")
    for _ in range(clear):
        key("Backspace")
    type_text(text)


def field_text(wid):
    # `val` is the input's value (a masked field's `t` is bullets).
    v = w.visible(snap(), wid)
    return (v[0].get("val") or v[0].get("t")) if v else None


def attempt(server, token):
    set_field("connect_server", server)
    set_field("connect_token", token)
    log_since()
    click("connect_btn")


def main():
    if MODE == "phone" and not is_shown("conversation_column") and not is_shown("connect_card"):
        w.get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    check("the Connect card shows (nothing reachable at launch)", soon(lambda: is_shown("connect_card"), tries=60))

    # 1. Unreachable.
    attempt("http://127.0.0.1:1", "")
    ok = soon(lambda: (text_of("connect_error_text") or "").startswith("Can't reach"), tries=120, pause=0.25)
    check("an unreachable address: 'Can't reach <address>'", ok, f"{text_of('connect_error_text')!r}")
    detail = text_of("connect_error_detail") or ""
    check("…with its actions (check the address · Retry)", "check the address · Retry" in detail, detail)
    check("…the probe said Unreachable", any("a9 connect probe: Unreachable" in l for l in log_since()))
    shot(f"{MODE}-connect-unreachable")

    # 2. A reachable server, a wrong token.
    attempt(LIVE, "wrong-token-a9")
    ok = soon(lambda: text_of("connect_error_text") == "The server refused this token", tries=160, pause=0.25)
    check("a refused token: 'The server refused this token'", ok, f"{text_of('connect_error_text')!r}")
    lines = log_since()
    check("…told apart by the probe (401), not guessed", any("a9 connect probe: Rejected" in l for l in lines))
    detail = text_of("connect_error_detail") or ""
    check("…with its actions (Re-enter the token · Retry)", "Re-enter the token · Retry" in detail, detail)
    lines += log_since()
    # The focus is handed to the remounted token input once it has been drawn
    # (TextInput::take_key_focus). A hidden window's own focus state decides
    # whether injected keys then reach it (they did in some runs, not in
    # others; a click then typing always does), so the walk checks that the
    # focus was given, and the report says the real-keyboard effect is
    # unverified here.
    check("…the token field is given the key focus", any("the token field has the focus" in l for l in lines))
    tok = field_text("connect_token") or ""
    check("…and keeps the typed value", tok == "wrong-token-a9", f"{len(tok)} chars")
    s = snap()
    card, err = rect("connect_card", s=s), rect("connect_error", s=s)
    check("the error callout sits inside the card",
          card and err and err[0] >= card[0] and err[0] + err[2] <= card[0] + card[2] + 0.5, f"{err} in {card}")
    shot(f"{MODE}-connect-rejected")

    failed = [n for n, ok, _ in w.RESULTS if not ok]
    print(f"== WALK a9 connect {MODE}: {len(w.RESULTS) - len(failed)}/{len(w.RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
