#!/usr/bin/env python3
"""A8 — the session FLOWS click walk (resume guards, delete, per-Session
drafts across a switch and an app restart, the reconnect re-open, the
workspace launch decision). Every step is a CLICK (or a typed slash command)
on the real app; each asserts the app's own effect (/snap text) AND what
reached the wire (the A8 fixture server's request log).

  phase1: python3 tools/walk/a8_flows_walk.py <port> <desktop|phone> phase1 <log> [shots]
  phase2: (after the wrapper restarted the APP)             phase2 <log> [shots]

phase1 restarts the FIXTURE SERVER once (the reconnect check) with
`--launch cross_profile`; the wrapper passes how to start it:
  A8_SERVE_BIN, A8_SERVE_PIDFILE (the walk only kills the pid it was given).
Exit status 0 when every step passes.
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

PORT = int(sys.argv[1])
MODE = sys.argv[2]
PHASE = sys.argv[3]
LOG = sys.argv[4]
SHOTS = sys.argv[5] if len(sys.argv) > 5 else None
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []


def get(path, timeout=20):
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read()
    except urllib.error.HTTPError:
        if path.startswith(("/m?", "/t?", "/k?", "/click?")):
            time.sleep(0.3)
            return b"{}"
        raise


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(s, wid):
    return [w for w in s if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def rect(wid, s=None, nth=0):
    hits = sorted(visible(s or snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def text(wid, s=None):
    hits = visible(s or snap(), wid)
    return hits[0].get("t", "") if hits else None


def shown(wid):
    return rect(wid) is not None


def tree_text(wid):
    """A widget's text whether or not it is laid out (the module's own
    connection readout, `status`, is never drawn)."""
    hits = [w for w in snap() if w.get("i") == wid]
    return hits[0].get("t", "") if hits else None


def labels():
    return [(w.get("i") or "", w.get("t") or "", w["r"]) for w in snap() if w.get("t") and w["r"][2] > 0 and w["r"][3] > 0]


def wait(pred, secs=8.0):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(0.25)
    return False


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.3)


def scroll_to(wid, min_h=8):
    """Bring `wid` fully into a dialog's scroll body (rewind first when it is
    out of view: its side is unknown). Outside a dialog: its rect as is.
    A rect the body clips ends exactly ON the body's edge, so "inside" keeps
    a 1 px margin; a body that stopped moving (the content's end) is final."""
    rewound = False
    last = None
    for _ in range(20):
        s = snap()
        r, body = rect(wid, s), rect("b3_scroll", s)
        if body is None:
            return r
        if r and r[3] >= min_h and r[1] > body[1] and r[1] + r[3] < body[1] + body[3]:
            return r
        if r is not None and r == last:
            return r
        last = r
        mx, my = body[0] + body[2] / 2, body[1] + body[3] / 2
        if r is None and not rewound:
            get(f"/m?k=scroll&x={mx}&y={my}&dy=-3000&wait=1")
            rewound = True
            time.sleep(0.15)
            continue
        dy = -150 if (r and r[1] < body[1]) else 150
        get(f"/m?k=scroll&x={mx}&y={my}&dy={dy}&wait=1")
        time.sleep(0.15)
    return rect(wid)


def click(wid, nth=0):
    r = scroll_to(wid) if nth == 0 else rect(wid, nth=nth)
    if r is None:
        return False
    click_rect(r)
    return True


def click_text(t, prefix=""):
    """Click the first laid-out label whose text is `t` (or `t` cut with "…")."""
    for i, tt, r in labels():
        if same_title(tt, t) and i.startswith(prefix):
            click_rect(r)
            return True
    return False


def key(code):
    get(f"/k?k=down&c={code}&wait=1")
    get(f"/k?k=up&c={code}&wait=1")


def type_text(t):
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))
    time.sleep(0.3)


def composer():
    """The composer's text ('' when it only shows its placeholder)."""
    t = text("i0_composer_0")
    return "" if t in (None, "Ask Octos anything") else t


def slash(cmd):
    click("i0_composer_0")
    type_text(cmd)
    wait(lambda: (composer() or "").strip() == cmd, 3)
    key("ReturnKey")
    time.sleep(0.6)


def wire(method):
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
    """The capture AND its /snap (the numeric UX checks read the snap)."""
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        with open(os.path.join(SHOTS, f"{MODE}-{name}.png"), "wb") as f:
            f.write(get("/g?raw=1", timeout=30))
        with open(os.path.join(SHOTS, f"{MODE}-{name}.json"), "wb") as f:
            f.write(get("/snap?all=1"))


def same_title(shown_t, title):
    """A row title as drawn: exact, or cut to the row's width with "…"."""
    if shown_t == title:
        return True
    return shown_t.endswith("…") and len(shown_t) > 4 and title.startswith(shown_t[:-1].rstrip())


def find_row(prefix, title):
    """The index N of `<prefix>N_title` whose text is `title` (all rows: a
    row scrolled out of a dialog body is still in the tree)."""
    for w in snap():
        i, t = w.get("i") or "", w.get("t") or ""
        if i.startswith(prefix) and i.endswith("_title") and same_title(t, title):
            try:
                return int(i[len(prefix):].split("_")[0])
            except ValueError:
                pass
    for i, t, _ in labels():
        if i.startswith(prefix) and i.endswith("_title") and t == title:
            try:
                return int(i[len(prefix):].split("_")[0])
            except ValueError:
                pass
    return None


def sidebar_open():
    if MODE == "phone" and not shown("sb_new_chat_hit"):
        click("sidebar_toggle_hit")
        wait(lambda: shown("sb_new_chat_hit"), 4)


def open_session_in_sidebar(title):
    sidebar_open()
    ok = click_text(title, "sb_r_title")
    time.sleep(1.0)
    return ok


def phase1():
    check("app is up with the strip", wait(lambda: shown("b3_strip_tap"), 20))
    # ---- resume guards ------------------------------------------------------
    slash("/resume")
    check("/resume opens 'Resume chat'", wait(lambda: text("b3_title") == "Resume chat"), str(text("b3_title")))
    check("resume: the catalog was read scoped (session/list {cwd, profile_id})",
          wait(lambda: any(p.get("profile_id") == "a8" for p in wire("session/list")), 6))
    legacy = find_row("b3_resume_row_", "Legacy chat")
    reason = None
    if legacy is not None and scroll_to(f"b3_resume_row_{legacy}_blocked"):
        reason = text(f"b3_resume_row_{legacy}_blocked")
    check("resume: a bare id is blocked with the grammar reason",
          bool(reason) and reason.startswith("This catalog ID does not identify a full Session"), str(reason)[:80])
    hollow = find_row("b3_resume_row_", "Why is hydrate slow?")
    check("resume: the hollow candidate is listed (unverified)", hollow is not None)
    shot("01-resume")
    if hollow is not None:
        click(f"b3_resume_row_{hollow}_tap")
        click("b3_resume_confirm")
        type_text("Why is hydrate slow?")
        check("resume: the exact title arms the primary control", wait(lambda: scroll_to("b3_resume_go") is not None, 4))
        n_open = len(wire("session/open"))
        click("b3_resume_go")
        check("resume: an empty history of listed messages is refused",
              wait(lambda: (text("b3_resume_caution_head") or "").startswith("Historical identity was not resolved")
                   or (scroll_to("b3_resume_caution_head") is not None and (text("b3_resume_caution_head") or "").startswith("Historical identity")), 8),
              str(text("b3_resume_caution_head")))
        check("wire: the candidate was opened and its history read",
              any(p.get("session_id") == "a8:api:hollow" for p in wire("session/open")[n_open:])
              and any(p.get("session_id") == "a8:api:hollow" and p.get("include") == ["messages"] for p in wire("session/hydrate")))
        check("wire: the previous Session was put back",
              wait(lambda: any(p.get("session_id") == "a8:main" for p in wire("session/open")[n_open:]), 6))
        shot("02-resume-refused")
    alpha = find_row("b3_resume_row_", "Add session fork")
    if alpha is not None:
        click(f"b3_resume_row_{alpha}_tap")
        click("b3_resume_confirm")
        type_text("Add session fork")
        if MODE == "phone":
            # The on-screen keyboard is up: the dialog panned the typed field
            # (and its button) above it.
            time.sleep(0.6)
            shot("01b-resume-keyboard")
        wait(lambda: scroll_to("b3_resume_go") is not None, 4)
        click("b3_resume_go")
    check("resume: a verified candidate opens and the dialog closes", wait(lambda: not shown("b3_resume_scope"), 8))
    check("header: the resumed Session's title", wait(lambda: text("hd_title") == "Add session fork", 8), str(text("hd_title")))
    check("wire: resuming never starts a turn", not wire("turn/start"))

    # ---- delete from the session list ---------------------------------------
    slash("/sessions")
    check("/sessions opens the switcher", wait(lambda: text("b3_title") == "Open a different session"), str(text("b3_title")))
    beta = find_row("b3_switch_row_", "Review PR #2566")
    check("switcher: a non-open row offers delete", beta is not None and shown(f"b3_switch_row_{beta}_delete"))
    if beta is not None:
        click(f"b3_switch_row_{beta}_delete")
        check("delete asks first ('Delete?')", wait(lambda: text(f"b3_switch_row_{beta}_confirm_q") == "Delete?"))
        shot("03-delete-confirm")
        click(f"b3_switch_row_{beta}_confirm_yes")
        check("wire: ONE session/delete for the confirmed row",
              wait(lambda: wire("session/delete") == [{"session_id": "a8:api:beta"}], 6), str(wire("session/delete")))
        check("delete: the row is gone", wait(lambda: find_row("b3_switch_row_", "Review PR #2566") is None, 6))
    locked = find_row("b3_switch_row_", "Bump octos-core to a6ea8505")
    if locked is not None:
        click(f"b3_switch_row_{locked}_delete")
        wait(lambda: shown(f"b3_switch_row_{locked}_confirm_yes"), 4)
        click(f"b3_switch_row_{locked}_confirm_yes")
        check("delete: a refusal keeps the row and says why",
              wait(lambda: text("b3_switch_error") == "Couldn't delete the session: session is busy", 6), str(text("b3_switch_error")))
        check("delete: the refused row stays", find_row("b3_switch_row_", "Bump octos-core to a6ea8505") is not None)
    open_ = find_row("b3_switch_row_", "Add session fork")
    check("switcher: the open Session offers no delete", open_ is not None and not shown(f"b3_switch_row_{open_}_delete"))
    click("b3_close")
    wait(lambda: not shown("b3_switch_error") and text("b3_title") is None, 4)

    # ---- per-Session drafts --------------------------------------------------
    click("i0_composer_0")
    type_text("unsent words")
    check("draft typed in 'Add session fork'", wait(lambda: composer() == "unsent words", 4), str(composer()))
    open_session_in_sidebar("Fix steer queue drop on reconnect")
    check("another Session shows its own (empty) draft", wait(lambda: composer() == "", 6), repr(composer()))
    open_session_in_sidebar("Add session fork")
    check("back: the draft returns to its Session", wait(lambda: composer() == "unsent words", 6), repr(composer()))
    shot("04-draft-restored")

    # ---- reconnect: restart the fixture server -------------------------------
    pidfile = os.environ.get("A8_SERVE_PIDFILE")
    binp = os.environ.get("A8_SERVE_BIN")
    if pidfile and binp:
        pid = int(open(pidfile).read().strip())
        n_open = len(wire("session/open"))
        opened = [p.get("session_id") for p in wire("session/open")]
        active = opened[-1] if opened else "a8:main"
        os.kill(pid, 15)
        # The outage itself, from the module's connection readout (what the
        # window shows while disconnected is the connection surface's).
        check("reconnect: the outage is seen (the transport left Live)",
              wait(lambda: (tree_text("status") or "").startswith("conn: Reconnecting"), 10), str(tree_text("status")))
        time.sleep(1.0)
        proc = subprocess.Popen([binp, "8428", "--launch", "cross_profile", "--log", LOG],
                                stdout=open(LOG + ".serve2.log", "w"), stderr=subprocess.STDOUT)
        open(pidfile, "w").write(str(proc.pid))
        check("reconnect: the active Session is re-opened",
              wait(lambda: any(p.get("session_id") == active for p in wire("session/open")[n_open:]), 40), str(active))
        check("reconnect: and hydrated canonically",
              wait(lambda: any(p.get("session_id") == active and p.get("include") == ["messages"] for p in wire("session/hydrate")[-4:]), 10))
        check("reconnect: Live again", wait(lambda: (tree_text("status") or "").startswith("conn: Live"), 15), str(tree_text("status")))
        check("strip: back to 'Ready'", wait(lambda: text("b3_strip_state") == "Ready", 15), str(text("b3_strip_state")))
        check("header: the re-opened Session is the one shown", wait(lambda: text("hd_title") == "Add session fork", 6), str(text("hd_title")))
        check("the unsent draft survived the outage", wait(lambda: composer() == "unsent words", 6), repr(composer()))
        shot("04b-after-reconnect")


def phase2():
    check("app is up again", wait(lambda: shown("b3_strip_tap"), 20))
    open_session_in_sidebar("Add session fork")
    check("restart: the unsent draft is restored, never sent", wait(lambda: composer() == "unsent words", 10) and not wire("turn/start"), repr(composer()))
    shot("05-draft-after-restart")
    # ---- the workspace launch (the server now answers cross_profile) ---------
    sidebar_open()
    click("sb_add_hit")
    check("+ Add workspace opens the picker", wait(lambda: shown("b1_pk_server"), 6))
    n_open = len(wire("session/open"))
    click("b1_pk_server")
    check("wire: the launch asked launch/resolve first", wait(lambda: len(wire("launch/resolve")) > 0, 6), str(wire("launch/resolve")[-1:]))
    check("the decision panel: 'Choose this workspace’s profile'", wait(lambda: text("b3_title") == "Choose this workspace’s profile", 6), str(text("b3_title")))
    check("nothing opened before the choice", len(wire("session/open")) == n_open)
    shot("06-launch-decision")
    click("b3_launch_choice_1")
    check("wire: the chosen profile opens the new Session",
          wait(lambda: any((p.get("session_id") or "").startswith("glm-coder:api:") and p.get("profile_id") == "glm-coder" for p in wire("session/open")[n_open:]), 8))
    check("the panel closes", wait(lambda: text("b3_title") is None, 6))
    check("the draft moved with the committed launch", wait(lambda: composer() == "unsent words", 6), repr(composer()))


if __name__ == "__main__":
    phase1() if PHASE == "phase1" else phase2()
    failed = [r for r in RESULTS if not r[1]]
    print(f"\n{len(RESULTS) - len(failed)}/{len(RESULTS)} passed ({PHASE})")
    sys.exit(1 if failed else 0)
