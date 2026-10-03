#!/usr/bin/env python3
"""A20 — parity row 247: a saved conversation link whose workspace differs is
refused BEFORE any session/open or session/hydrate. The click walk, on the
real app (hidden) against the a20 fixture server.

The link is the one the inspector copies (`octoscode://session?s=<tuple>`),
handed to the app at launch (`OCTOSCODE_SESSION_LINK`, the native analog of
the web's `?s=` address). Three launches:

  A. the link names /home/user/a20-link-ws, which the server attests as
     /home/user/a20-real-ws (a moved / symlinked folder): the "Open saved
     conversation" panel; CLICK "Open conversation" -> refused on the panel,
     the wire shows the catalog read and NO session/open or session/hydrate
     for the linked Session; CLICK "Dismiss link" -> normal entry.
  B. the link names the exact workspace: CLICK "Open conversation" ->
     session/open {cwd: <exact>} + its history read; the panel closes.
  C. an invalid link: the panel says so; CLICK "Dismiss link".

  cargo build -p octoscode-module --example a20_serve
  OCTOSCODE_APP_BIN=<host-bin> python3 tools/walk/a20_saved_link_walk.py desktop|phone [port] [fport] [out]
"""
import json
import sys
import time
import urllib.parse

from a10_lib import checks_line, dialog_checks
from a20_lib import run

WALK = {
    "name": "a20_saved_link",
    "title": "saved conversation link: a different workspace is refused before any open or history read",
    "modes": ["desktop", "phone"],
    # A34: the aggregator's mode, ports and scratch dir (see a20_interaction_walk.py).
    "runs": [{"argv": ["{mode}", "{port}", "{fport}", "{out}"], "env": {"OCTOSCODE_APP_BIN": "{bin}"}}],
    "needs": ["target/debug/examples/a20_serve"],
    # Walk rows of docs/walk-rows.csv (A34: this said 247, the PARITY row whose
    # e2e spec is walk row 196's case; 4 of its 5 patterns matched no check).
    # Launch A proves row 196, launch C row 195 (the native link is handed over
    # at launch, so the panel keeping or dropping it stands for the address).
    "rows": {
        196: ["the panel 'Open saved conversation' is offered", "refused on the panel, with both workspaces",
              "wire: the precondition read the server's attestation", "wire: NO session/open for the linked",
              "wire: NO session/hydrate for the linked", "the link stays on the panel"],
        195: ["C: an invalid link says so", "C: wire: nothing opened for it", "C: CLICK Dismiss link",
              "C: dismissed, normal entry"],
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8483
FPORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8485
OUT = sys.argv[4] if len(sys.argv) > 4 else "docs/ux/a20/walk"
XID = "a20:api:xray"
LINK_WS = "/home/user/a20-link-ws"
REAL_WS = "/home/user/a20-real-ws"


def link(ws, profile="a20", session=XID):
    tuple_ = json.dumps([ws, profile, session], separators=(",", ":"))
    return "octoscode://session?s=" + urllib.parse.quote(tuple_, safe="")


def panel_checks(w, name):
    sn = w.snap()
    c = dialog_checks(sn, "b3_dialog", ("b3_link_", "b3_title", "b3_close"), module=w.module_rect(sn), viewport="b3_scroll")
    w.check(f"{name}: layout {checks_line(c)}", bool(c.get("ok")), json.dumps(c)[:300])
    return c


def refused(w):
    shot = lambda n: w.shot(f"{MODE}-{n}")
    w.check("the panel 'Open saved conversation' is offered",
            w.wait(lambda: w.text("b3_title") == "Open saved conversation", 20), repr(w.text("b3_title")))
    w.check("panel: the workspace the link names", w.text("b3_link_workspace") == LINK_WS, repr(w.text("b3_link_workspace")))
    w.check("panel: the server in use", f"127.0.0.1:{FPORT}" in (w.text("b3_link_server") or ""), repr(w.text("b3_link_server")))
    w.check("panel: the conversation details start closed (the web's <details>)", not w.visible("b3_link_session"))
    panel_checks(w, "panel")
    shot("11-link-panel")
    w.check("CLICK Conversation details", w.click("b3_link_details"))
    w.check("details: the Session and the Profile",
            w.wait(lambda: w.text("b3_link_session") == XID and w.text("b3_link_profile") == "a20", 5),
            f"{w.text('b3_link_session')!r} {w.text('b3_link_profile')!r}")
    panel_checks(w, "details panel")
    shot("11b-link-details")
    opens, reads = len(w.wire("session/open", XID)), len(w.wire("session/hydrate", XID))
    w.check("CLICK Open conversation", w.click("b3_link_open"))
    w.check("refused on the panel, with both workspaces",
            w.wait(lambda: (w.text("b3_link_error") or "").startswith("Not opened: the server resolves the saved link"), 10)
            and LINK_WS in (w.text("b3_link_error_0") or "").replace("\n", "")
            and REAL_WS in (w.text("b3_link_error_1") or "").replace("\n", ""),
            f"{w.text('b3_link_error')!r} {w.text('b3_link_error_0')!r} {w.text('b3_link_error_1')!r}")
    w.check("wire: the precondition read the server's attestation (session/list {cwd: <link>})",
            any(p.get("cwd") == LINK_WS for p in w.wire("session/list")))
    time.sleep(1.0)
    w.check("wire: NO session/open for the linked Session", len(w.wire("session/open", XID)) == opens == 0,
            str(w.wire("session/open", XID)))
    w.check("wire: NO session/hydrate for the linked Session", len(w.wire("session/hydrate", XID)) == reads == 0,
            str(w.wire("session/hydrate", XID)))
    w.check("the link stays on the panel (Open conversation offered again)", bool(w.visible("b3_link_open")))
    panel_checks(w, "refused panel")
    shot("12-link-refused")
    w.check("CLICK Dismiss link", w.click("b3_link_dismiss"))
    w.check("dismissed: the panel closes, the conversation shows",
            w.wait(lambda: not w.visible("b3_dialog") and w.composer() is not None, 6))
    w.check("wire: still nothing opened for the link", not w.wire("session/open", XID) and not w.wire("session/hydrate", XID))
    shot("13-link-dismissed")


def exact(w):
    shot = lambda n: w.shot(f"{MODE}-{n}")
    w.check("B: the panel is offered", w.wait(lambda: w.text("b3_title") == "Open saved conversation", 20))
    w.check("B: CLICK Open conversation", w.click("b3_link_open"))
    w.check("B: wire: session/open {session: <link>, cwd: <the exact workspace>}",
            w.wait(lambda: any(p.get("cwd") == REAL_WS for p in w.wire("session/open", XID)), 10), str(w.wire("session/open", XID)))
    w.check("B: wire: its history is read after the accepted open",
            w.wait(lambda: len(w.wire("session/hydrate", XID)) >= 1, 8))
    w.check("B: the panel closes", w.wait(lambda: not w.visible("b3_dialog"), 8))
    w.check("B: the linked Session is on screen", w.wait(lambda: w.has_text("Session X — clean the scratch dir"), 8))
    shot("14-link-opened")


def invalid(w):
    shot = lambda n: w.shot(f"{MODE}-{n}")
    w.check("C: an invalid link says so", w.wait(lambda: w.text("b3_title") == "This conversation link is invalid", 20),
            repr(w.text("b3_title")))
    shot("15-link-invalid")
    w.check("C: CLICK Dismiss link", w.click("b3_link_dismiss"))
    w.check("C: dismissed, normal entry", w.wait(lambda: not w.visible("b3_dialog") and w.composer() is not None, 6))
    w.check("C: wire: nothing opened for it", not w.wire("session/open", XID))


if __name__ == "__main__":
    attest = ["--attest", f"{LINK_WS}={REAL_WS}"]
    rc = run(refused, mode=MODE, outdir=f"{OUT}/link-{MODE}-A", port=PORT, fport=FPORT, server_args=attest,
             env={"OCTOSCODE_SESSION_LINK": link(LINK_WS)})
    rc |= run(exact, mode=MODE, outdir=f"{OUT}/link-{MODE}-B", port=PORT, fport=FPORT, server_args=attest,
              env={"OCTOSCODE_SESSION_LINK": link(REAL_WS)})
    rc |= run(invalid, mode=MODE, outdir=f"{OUT}/link-{MODE}-C", port=PORT, fport=FPORT,
              env={"OCTOSCODE_SESSION_LINK": "octoscode://session?s=" + urllib.parse.quote('["relative/path","a20"]')})
    sys.exit(rc)
