#!/usr/bin/env python3
"""A21 — unsent drafts restore only for the matching identity / principal
(parity row 196), walked by CLICKS on the real app against the A8 fixture
server with `--principal-from-token` (`alice.1` and `alice.2` are one user —
a rotated token — `bob.1` another; `durable-session-drafts.ts:10-35`), on one
isolated state tree (brief §8).

  1. Connect as alice.1, open "Add session fork", type a draft (unsent);
  2. Disconnect; Connect as bob.1 (another principal), open the SAME
     Session: the composer is empty — never alice's text [196];
  3. Disconnect; Connect as alice.2 (alice's rotated token): her draft is
     back in that Session, unsent [196] (`connection-storage.spec.ts:63`);
  4. Settings > Forget server; Connect as alice.2 again: the draft is GONE —
     Forget cleared the principal's drafts (`ConnectionGate.tsx:319-330`) [196];
  5. a new draft, then a relaunch (the same identity restores itself): the
     draft is back, never sent (A7's recovery kept for the same principal);
  6. Disconnect, relaunch, Connect as bob.1: that Session's composer is
     empty — the tab drafts of alice's identity restore for nobody else [196].

  python3 tools/walk/a21_drafts_walk.py <host-bin> <desktop|phone> [port] [fixture-port] [out-dir]
"""
from __future__ import annotations

import json
import pathlib
import shutil
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from a21_lib import EXAMPLES, ROOT, Fixture, Walk, composer_checks, origin  # noqa: E402

WALK = {
    "name": "a21_drafts",
    "title": "unsent drafts restore only for the matching identity/principal; Forget clears them",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{bin}", "{mode}", "{port}", "{fport}", "{out}"]}],
    "needs": ["target/debug/examples/a8_serve"],
    "timeout": 900,
}

BIN = sys.argv[1] if len(sys.argv) > 1 else ""
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8489
FPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8491
OUT = (pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else ROOT / "tmp" / "walk" / "a21-drafts").resolve() / MODE
CWD = "/home/user/octos"
SESSION = "Add session fork"          # a8_serve's a8:api:alpha
ALICE = "alice's unsent words"
LATER = "a draft that waits for the restart"


def bound(w: Walk, since: int) -> list[str]:
    return [l for l in w.all_logs()[since:] if "drafts: the principal's scope is bound" in l]


def turn_starts(log: pathlib.Path) -> int:
    if not log.exists():
        return 0
    return sum(1 for l in log.read_text().splitlines() if '"method": "turn/start"' in l or '"method":"turn/start"' in l)


def session_live(w: Walk, who: str) -> bool:
    """Connected, the principal's binding done, the Session opened."""
    return bool(w.wait(w.live, 40))


def open_and_read(w: Walk, settle: float = 4.0) -> str:
    """Open the walk's Session from the sidebar, let the binding settle, read
    the composer."""
    if not w.open_session(SESSION):
        return "<the Session row was not found>"
    time.sleep(settle)
    return w.composer()


def main() -> int:
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True, exist_ok=True)
    log = OUT / "serve.jsonl"
    fx = Fixture([str(EXAMPLES / "a8_serve"), str(FPORT), "--principal-from-token", "--advertise", "--log", str(log)], OUT / "serve.log")
    w = Walk(BIN, MODE, PORT, OUT, {"OCTOS_BASE_URL": origin(FPORT), "OCTOS_WORKSPACE_CWD": CWD})
    try:
        fx.start()
        w.launch()
        w.check("first launch: the Connect card (nothing remembered)", w.wait(w.card, 30))
        # ---- 1. alice types a draft --------------------------------------
        n = len(w.all_logs())
        w.connect(None, "alice.1")
        w.check("Connect as alice (a click) goes live", session_live(w, "alice"))
        w.check("…alice's principal is resolved and her scope bound", w.wait(lambda: bound(w, n), 10))
        w.check(f"'{SESSION}' opens from the sidebar with an empty composer", open_and_read(w) == "", repr(w.composer()))
        w.type_in_composer(ALICE)
        w.check("the draft sits unsent in alice's composer", w.wait(lambda: w.composer() == ALICE, 5), repr(w.composer()))
        time.sleep(1.5)

        # ---- 2. bob: another principal ------------------------------------
        w.check("Settings > Disconnect (the unsaved-input confirmation confirmed) returns the Connect card", w.leave("disconnect"))
        n = len(w.all_logs())
        w.connect(None, "bob.1")
        w.check("Connect as bob (another principal) goes live", session_live(w, "bob"))
        w.check("…bob's scope is bound with nothing restored",
                w.wait(lambda: any("nothing restored" in l for l in bound(w, n)), 10), "; ".join(bound(w, n))[-160:])
        got = open_and_read(w)
        w.check(f"[196] bob opens the SAME Session: an empty composer — never alice's draft", got == "", repr(got))
        composer_checks(w, "another principal")
        w.shot(f"01-another-principal-empty-{MODE}")

        # ---- 3. alice again, with a rotated token -------------------------
        w.check("Settings > Disconnect returns the Connect card", w.leave("disconnect"))
        w.connect(None, "alice.2")
        w.check("Connect as alice with a rotated token goes live", session_live(w, "alice"))
        got = open_and_read(w, settle=2.0)
        if got != ALICE:
            w.wait(lambda: w.composer() == ALICE, 8)
            got = w.composer()
        w.check("[196] the same principal (a rotated token) gets her draft back in that Session", got == ALICE, repr(got))
        w.check("…restored, never sent (no turn/start)", turn_starts(log) == 0)
        composer_checks(w, "the same principal")
        w.shot(f"02-same-principal-restored-{MODE}")

        # ---- 4. Forget clears the principal's drafts ----------------------
        w.check("Settings > Forget server (the unsaved-input confirmation confirmed) returns the Connect card", w.leave("forget"))
        w.check("…the composer's text is gone with the identity", w.composer() == "" or not w.shown("i0_composer_0"))
        w.connect(None, "alice.2")
        w.check("Connect as alice again goes live", session_live(w, "alice"))
        got = open_and_read(w)
        w.check("[196] after Forget the same principal gets NOTHING back (her drafts were cleared)", got == "", repr(got))
        composer_checks(w, "after Forget")
        w.shot(f"03-after-forget-empty-{MODE}")

        # ---- 5. a relaunch with the same identity keeps the draft ---------
        w.type_in_composer(LATER)
        w.check("a new draft sits unsent", w.wait(lambda: w.composer() == LATER, 5), repr(w.composer()))
        time.sleep(1.5)
        w.stop()
        w.launch()
        w.check("relaunch: the connection restores itself", w.wait(w.live, 40))
        w.wait(lambda: w.composer() == LATER, 10)
        got = w.composer()
        if got != LATER:
            got = open_and_read(w)
        w.check("[196] the same identity's draft survives the restart, never sent (A7's recovery kept)",
                got == LATER and turn_starts(log) == 0, repr(got))
        w.shot(f"04-restart-same-identity-{MODE}")

        # ---- 6. another identity after a relaunch -------------------------
        w.check("Settings > Disconnect returns the Connect card", w.leave("disconnect"))
        w.stop()
        w.launch()
        w.check("relaunch after Disconnect: the Connect card", w.wait(w.card, 30))
        w.connect(None, "bob.1")
        w.check("Connect as bob goes live", session_live(w, "bob"))
        got = open_and_read(w)
        w.check("[196] the tab drafts of alice's identity restore for nobody else", got == "", repr(got))
        tab = OUT / "state" / "drafts.json"
        raw = tab.read_text() if tab.exists() else ""
        w.check("[196] the tab-drafts file holds no token and no draft of alice's",
                "alice." not in raw and "bob." not in raw and LATER not in raw, f"{len(raw)} bytes")
    except Exception as e:  # noqa: BLE001
        w.check("the walk ran to its end", False, f"{type(e).__name__}: {e}")
    finally:
        w.stop()
        fx.stop()
    return w.finish("a21 drafts")


if __name__ == "__main__":
    sys.exit(main())
