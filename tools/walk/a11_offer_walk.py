#!/usr/bin/env python3
"""A11 — the pairing-discovery click walk (walk row 114, "a pairing-capable
server this browser has seen offers itself once"; row 113, its 404 twin; and
the pairing-link rows 108-111 the walk passes through on the way).

  python3 tools/walk/a11_offer_walk.py <host-bin> <desktop|phone> [port] [fixture-port] [out-dir]

The walk launches what it needs and ALWAYS stops it (the operator rule): A2's
fixture server (`target/debug/examples/board1_serve`, build it with
`cargo build -p octoscode-module --example board1_serve`) and the hidden app,
each launch with its own isolated state (`walk_env.isolated_env`, brief §8 —
never the operator's ~/.octoscode). Five launches per mode:

1. nothing remembered — no probe, no offer; then, by clicks, the pairing
   link's refusals (an expired code, a malformed link, a link to another
   computer, a used link — each its own bounded copy) and the good link
   (paired, live); Settings > Connection > Details > Forget this device. The
   device now remembers the server WITHOUT a credential — made by the UI.
2. relaunch on that state (the fixture restarted: a fresh code) — ONE GET
   /pair/info, ONE offer "Found Octos on <host>." (no alert); Connect to <host>
   opens pairing (p4-01); used, the offer is gone for the run (also after a
   Forget); the fresh link pairs.
3. a remembered server that 404s /pair/info (seeded, as the web e2e seeds
   localStorage) — one GET, no offer, no complaint; the manual form connects.
4. a remembered server that needs no token — Connect to <host> connects at
   once (no pairing dialog).
5. a remembered server off this computer — no probe at all, no offer.

Prints one `PASS|FAIL <name> — <detail>` line per check (names carry their
walk-row tag, `[114]`); exit 0 iff every check passed. Evidence: <out>/<mode>/
walk.log, the fixture and app logs (scrubbed), captures + snaps.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from walk_env import ROOT, App, Fixture, app_env, crop_png, scrub  # noqa: E402

# The aggregator's convention (docs/walk/README.md "Native click walks"): a
# literal, read with `ast` — never imported.
WALK = {
    "name": "a11_offer",
    "title": "pairing discovery (offer once) + pairing-link refusals",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{bin}", "{mode}", "{port}", "{fport}", "{out}"]}],
    "needs": ["target/debug/examples/board1_serve"],
    "timeout": 900,
    "rows": {
        108: ["[108]"],
        109: ["[109]"],
        110: ["[110]"],
        111: ["[111]"],
        112: {"checks": ["[112]"],
              "partial": "the web's tab scope (a fresh tab has no credential) does not apply: the native "
                         "credential is device-scoped by design (parity 'Tab-scoped vs durable credential split' C)"},
        113: ["[113]"],
        114: ["[114]"],
    },
}

BIN = sys.argv[1] if len(sys.argv) > 1 else ""
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8421
FPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8434
OUT = (pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else ROOT / "tmp" / "walk" / "a11").resolve() / MODE
PHONE = MODE == "phone"
# The launch default points nowhere: first run. Port 9 (discard) is never a
# fixture's: 8499 was, once A10's walks shifted this walk's fixtures to
# 8497-8499 under the aggregator (the "dead" default then answered).
DEAD = "http://127.0.0.1:9"
FIXTURE = ROOT / "target" / "debug" / "examples" / "board1_serve"
RESULTS = []
WALK_LOG = []
SHOTS = [0]


def say(line):
    print(line, flush=True)
    WALK_LOG.append(line)


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    say(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
    return bool(ok)


def origin(port):
    return f"http://127.0.0.1:{port}"


def link(port, code):
    return f"http://app.invalid/?octos={origin(port)}&pair={code}"


# ------------------------------------------------------------------ helpers

def texts(app, s=None):
    s = s if s is not None else app.snap()
    return [w.get("t", "") for w in s if App.shown(w) and w.get("t")]


def has_text(app, needle, s=None):
    return any(needle in t for t in texts(app, s))


def logged(app, lines, needle):
    return any(needle in l for l in lines)


def live(app):
    """The paired / connected app: the first-run card is gone, the
    conversation's composer is up."""
    s = app.snap()
    return not app.find("connect_card", s) and app.find("i0_composer_0", s) is not None


def capture(app, name):
    """A PNG + its snap (input values dropped: a field can hold a secret)."""
    app.move_away()
    time.sleep(0.6)
    s = app.snap()
    for w in s:
        w.pop("val", None)
    SHOTS[0] += 1
    stem = OUT / f"{SHOTS[0]:02d}-{name}"
    stem.with_suffix(".snap.json").write_text(json.dumps(s))
    data = app.png()
    if data:
        stem.with_suffix(".png").write_bytes(data)
        view = next((w["r"] for w in s if w.get("ty") == "OctoscodeView" and App.shown(w)), None)
        if view:
            k = 2.0  # the capture is at the window's 2x density
            x, y, w, h = view
            if not PHONE:
                # The desktop capture is the whole 1400x900 shell desktop:
                # keep the OctosCode window (the module view + its 32 pt title bar).
                y, h = max(y - 32, 0), h + min(32, y)
            else:
                # The phone shell frame is 402 wide; the module draws in its
                # 360x780 window frame at the left (A3's env frame): keep the
                # frame from the status bar down.
                h, y = h + y, 0
            crop_png(stem.with_suffix(".png"), int(x * k), int(y * k), int(w * k), int(h * k))
    say(f"  shot {stem.name}")
    return s


def inside(a, b, tol=0.5):
    return a[0] >= b[0] - tol and a[1] >= b[1] - tol and a[0] + a[2] <= b[0] + b[2] + tol \
        and a[1] + a[3] <= b[1] + b[3] + tol


def overlap(a, b):
    ix = min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0])
    iy = min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1])
    return ix > 1 and iy > 1


def offer_layout(app, s):
    """Numeric /snap checks of the offer inside the Connect card."""
    card = app.find("connect_card", s)
    box = app.find("connect_offer_box", s)
    txt = app.find("connect_offer_text", s)
    pill = app.find("connect_offer_wrap", s)
    lab = app.find("connect_offer_label", s)
    hit = app.find("connect_offer", s)
    title = app.find("connect_title", s)
    server = app.find("connect_server", s)
    if not all((card, box, txt, pill, lab, hit, title, server)):
        return False, "offer widgets missing"
    c, b = card["r"], box["r"]
    probs = []
    if not inside(b, c):
        probs.append("box outside the card")
    for w in (txt, pill, lab, hit):
        if not inside(w["r"], b):
            probs.append(f"{w['i']} outside the box")
    if not inside(lab["r"], pill["r"]):
        probs.append("label outside its pill")
    if hit["r"][3] < 28 or hit["r"][2] < 28:
        probs.append(f"hit {hit['r'][2]}x{hit['r'][3]} < 28")
    if overlap(txt["r"], pill["r"]):
        probs.append("text overlaps the pill")
    # The box spans the card's content column (the fields' edges).
    field = app.find("connect_server_error", s) or server
    col_l = title["r"][0]
    col_r = c[0] + c[2] - (title["r"][0] - c[0])
    if abs(b[0] - col_l) > 1 or abs(b[0] + b[2] - col_r) > 1:
        probs.append(f"box {b[0]}..{b[0] + b[2]} vs column {col_l}..{col_r}")
    # Order: title, offer, the Server field.
    if not (title["r"][1] + title["r"][3] <= b[1] and b[1] + b[3] <= server["r"][1]):
        probs.append("not between the title and the fields")
    # The one-line message is not cut (its text fits the box width).
    gap_top = txt["r"][1] - b[1]
    gap_bottom = (b[1] + b[3]) - (pill["r"][1] + pill["r"][3])
    if abs(gap_top - gap_bottom) > 3:
        probs.append(f"unbalanced padding top {gap_top} bottom {gap_bottom}")
    detail = (f"card {c}, box {b}, pill {pill['r']} hit {hit['r'][2]}x{hit['r'][3]}, "
              f"padding t/b {gap_top:.0f}/{gap_bottom:.0f}") + (f"; {probs}" if probs else "")
    return not probs, detail


def card_checks(s):
    """A1's numeric Connect-card checks (tools/a1/ux_checks.py connect)."""
    path = OUT / "card-check.snap.json"
    path.write_text(json.dumps({"s": s}))
    r = subprocess.run([sys.executable, str(ROOT / "tools" / "a1" / "ux_checks.py"), str(path), "connect"],
                       capture_output=True, text=True)
    path.unlink()
    last = (r.stdout.strip().splitlines() or ["(no output)"])[-1]
    return r.returncode == 0, last


def submit(app, button):
    """Submit a typed field: on the phone the emulated keyboard covers the
    lower screen, so its return key submits (the field's `returns` route)."""
    if PHONE:
        app.key("ReturnKey")
        time.sleep(0.5)
    else:
        app.click(button)


def pair_with(app, text):
    """p4-01 is open: paste `text` into the link field and press Pair."""
    app.click("b1_pair_link")
    app.type(text)
    submit(app, "b1_pair_submit")


def open_pairing(app):
    return app.click_until("b1_connect_pair", lambda: app.find("b1_pair_link"))


def back_to_pair(app):
    return app.click_until("b1_pair_back", lambda: app.find("b1_pair_link"))


def forget(app):
    """Settings > Connection > This device · Details > Forget this device."""
    if not app.click_until("settings_open_hit", lambda: app.find("settings_drawer")):
        return False
    cell = "rl_hit" if PHONE else "nv_hit"
    # Connection (A36: Capabilities sits after Model, so Connection is the sixth cell).
    if not app.click_until(cell, lambda: app.find("b1_set_connection"), nth=5):
        return False
    if not app.click_until("b1_set_connection", lambda: app.find("b1_pair_forget")):
        return False
    if not app.click_until("b1_pair_forget", lambda: app.find("connect_card")):
        return False
    # The web returns to "Connect to Octos" with no dialog (or its dimmer) left.
    return app.wait(lambda: app.find("connect_card") and not app.find("settings_drawer")
                    and not app.find("dimmer"), timeout=6) is not None


def launch(name, fixture_args=None, fport=None, seed=None, state=None):
    """One hidden app (and its fixture) with isolated state."""
    st = state or (OUT / "state" / name)
    if state is None:
        shutil.rmtree(st, ignore_errors=True)
    hs = OUT / "state" / f"hs-{name}"
    env = app_env(MODE, st, {"OCTOS_BASE_URL": DEAD, "OCTOS_PROFILE_ID": "octoscode"}, hs=hs)
    if seed is not None:
        # The web e2e seeds the durable endpoint (localStorage, `version: 2`);
        # natively the remembered server is the credentials' last-server.
        (st / "cred").mkdir(parents=True, exist_ok=True)
        (st / "cred" / "last-server").write_text(seed)
    fx = None
    if fixture_args is not None:
        fx = Fixture([str(FIXTURE), str(fport)] + fixture_args, OUT / f"fixture-{name}.log").start()
    app = App(PORT, env)
    try:
        ok = app.start(BIN)
    except Exception as e:  # noqa: BLE001
        say(f"  launch {name} failed: {e!r}")
        ok = False
    return app, fx, st, ok


def stop(app, fx, name):
    try:
        lines = app.all_logs()
        kept = [scrub(l) for l in lines if "octoscode" in l or "[E]" in l]
        (OUT / f"app-{name}.log").write_text("\n".join(kept) + "\n")
    except Exception as e:  # noqa: BLE001
        say(f"  (app log not saved: {e!r})")
    app.stop()
    if fx:
        fx.stop()
        (OUT / f"fixture-{name}.log").write_text(scrub(fx.text()))


# ------------------------------------------------------------------- phases

def phase_seen_via_ui():
    say("== 1. nothing remembered; the pairing link's refusals; pair; Forget (the device now remembers the server, no credential)")
    app, fx, st, ok = launch("p1", ["--pair", "ok"], FPORT)
    try:
        if not check("[114] launch 1: the first-run Connect card shows", ok and app.find("connect_card")):
            return None
        time.sleep(1.5)
        lines = app.logs()
        check("[114] nothing remembered: no probe, no offer",
              not app.find("connect_offer_box") and fx.count("/pair/info") == 0
              and logged(app, lines, "discovery: no probe (no remembered server)"),
              f"GET /pair/info x{fx.count('/pair/info')}")
        if not check("[110] 'Pair with a link instead' opens p4-01", open_pairing(app)):
            return st
        # Row 110: an expired code and a malformed link get their own copy.
        pair_with(app, link(FPORT, "EXPIRED0"))
        app.wait(lambda: app.find("b1_conn_submit"))
        s = app.snap()
        check("[110] an expired code: 'This pairing link has expired.' + its next step",
              has_text(app, "This pairing link has expired.", s)
              and has_text(app, "Restart Octos on your computer for a fresh link.", s))
        capture(app, "p4-03-expired")
        back_to_pair(app)
        pair_with(app, link(FPORT, "not-a-code"))
        app.wait(lambda: app.find("b1_conn_submit"))
        s = app.snap()
        check("[110] a malformed link: its own copy, the origin prefilled",
              has_text(app, "This pairing link isn’t complete.", s)
              and app.text("b1_conn_server", s) == origin(FPORT),
              repr(app.text("b1_conn_server", s)))
        # Row 111: a link to another computer — refused with no request.
        claims = fx.count("POST /pair/claim")
        back_to_pair(app)
        pair_with(app, "http://app.invalid/?octos=http://192.168.1.20:50190&pair=3QK7ZP2M")
        app.wait(lambda: app.find("b1_conn_submit"))
        time.sleep(0.8)
        s = app.snap()
        check("[111] a link to another computer is refused without a request, not prefilled",
              has_text(app, "This link points to another computer.", s)
              and fx.count("POST /pair/claim") == claims
              and app.text("b1_conn_server", s) in ("", "http://127.0.0.1:50190"),
              f"claims {claims}->{fx.count('POST /pair/claim')}, server field {app.text('b1_conn_server', s)!r}")
        capture(app, "p4-03-foreign")
        # Row 109: a used link says so, the form keeps the origin.
        back_to_pair(app)
        pair_with(app, link(FPORT, "USED0000"))
        app.wait(lambda: app.find("b1_conn_submit"))
        s = app.snap()
        check("[109] a used link says so and the form keeps the origin",
              has_text(app, "This pairing link was already used.", s)
              and app.text("b1_conn_server", s) == origin(FPORT))
        # Row 108: the good link — no token box, the connection is live.
        back_to_pair(app)
        pair_with(app, link(FPORT, "3QK7ZP2M"))
        on = app.wait(lambda: live(app), timeout=25)
        lines = app.logs()
        check("[108] a good link connects with no token box (live)",
              on and logged(app, lines, "pairing: connected — the paired token is live"))
        stored = sorted(p.name for p in (st / "cred").glob("*"))
        check("[114] connecting remembers the server", (st / "cred" / "last-server").read_text().strip() == origin(FPORT)
              if (st / "cred" / "last-server").exists() else False, f"{stored}")
        check("[112] the paired credential is kept for the next start (one token file, owner-only)",
              len(list((st / "cred").glob("*.token"))) == 1
              and all((p.stat().st_mode & 0o777) == 0o600 for p in (st / "cred").glob("*.token")), f"{stored}")
        check("[112][114] Settings > Connection > Forget this device: the Connect card is back, no dialog or dimmer left",
              forget(app))
        app.wait(lambda: app.text("connect_token") in ("", "Paste your server token"), timeout=4)
        s = app.snap()
        tokens = list((st / "cred").glob("*.token"))
        field = app.text("connect_token", s)
        check("[112][114] seen, no credential: the server stays remembered, its token is gone, the field is empty",
              not tokens and (st / "cred" / "last-server").exists() and field in ("", "Paste your server token"),
              f"token files {len(tokens)}, token field {'empty' if field in ('', 'Paste your server token') else 'not empty'}")
        capture(app, "forgotten")
    finally:
        stop(app, fx, "p1")
    return st


def phase_offer(st):
    say("== 2. relaunch on that state (a fresh code): one probe, one offer, Connect to -> pairing, used once")
    app, fx, _, ok = launch("p2", ["--pair", "ok"], FPORT, state=st)
    try:
        if not check("[114] launch 2: the Connect card shows", ok and app.find("connect_card")):
            return
        shown = app.wait(lambda: app.find("connect_offer_box"), timeout=10)
        time.sleep(1.0)
        s = app.snap()
        check("[112] after Forget, a restart prefills no token (the form stays empty)",
              app.text("connect_token", s) in ("", "Paste your server token"))
        msgs = [w for w in s if w.get("i") == "connect_offer_text" and App.shown(w)]
        check("[114] one unauthenticated GET /pair/info on the remembered origin",
              fx.count("GET /pair/info") == 1, f"x{fx.count('GET /pair/info')}")
        check(f"[114] the offer, once: 'Found Octos on 127.0.0.1:{FPORT}.'",
              shown and len(msgs) == 1 and msgs[0].get("t") == f"Found Octos on 127.0.0.1:{FPORT}."
              and sum(1 for t in texts(app, s) if t.startswith("Found Octos on")) == 1,
              repr([w.get("t") for w in msgs]))
        check(f"[114] its one button: 'Connect to 127.0.0.1:{FPORT}'",
              app.text("connect_offer_label", s) == f"Connect to 127.0.0.1:{FPORT}")
        check("[114] a status, not an alert: no error on the card",
              not app.find("connect_error", s) and not app.find("connect_error_text", s))
        ok_l, detail = offer_layout(app, s)
        check("[114] layout: the offer inside the card's column, between the title and the fields, >= 28 px hit, no overlap",
              ok_l, detail)
        ok_c, last = card_checks(s)
        check("[114] layout: the Connect card's own checks (tools/a1/ux_checks.py)", ok_c, last)
        capture(app, "offer")
        app.logs()
        opened = app.click_until("connect_offer", lambda: app.find("b1_pair_link"))
        lines = app.logs()
        check("[114] CLICK Connect to <host> -> pairing (p4-01)",
              opened and logged(app, lines, "discovery: offer used -> pairing"))
        capture(app, "offer-pairing")
        closed = app.click_until("b1_pair_back", lambda: not app.find("b1_card"))
        time.sleep(0.8)
        s = app.snap()
        check("[114] used once: the offer is gone and the form names the server",
              closed and not app.find("connect_offer_box", s) and app.text("connect_server", s) == origin(FPORT),
              repr(app.text("connect_server", s)))
        capture(app, "offer-used")
        open_pairing(app)
        pair_with(app, link(FPORT, "3QK7ZP2M"))
        on = app.wait(lambda: live(app), timeout=25)
        check("[114] the fresh link pairs with the offered server (live)", on)
        forget(app)
        time.sleep(1.0)
        check("[114] never offered twice in a run (after a Forget too); still one GET",
              not app.find("connect_offer_box") and fx.count("GET /pair/info") == 1,
              f"x{fx.count('GET /pair/info')}")
    finally:
        stop(app, fx, "p2")


def phase_seen_and_offered():
    """Launches 1 and 2 share one state tree: the second sees what the first made."""
    phase_offer(phase_seen_via_ui())


def phase_404():
    say("== 3. a remembered server that 404s /pair/info: no offer, no complaint; the manual form connects")
    fp = FPORT + 1
    app, fx, _, ok = launch("p3", ["--pair", "unsupported"], fp, seed=origin(fp))
    try:
        if not check("[113] launch: the Connect card shows", ok and app.find("connect_card")):
            return
        app.wait(lambda: fx.count("/pair/info") >= 1, timeout=8)
        time.sleep(1.5)
        s = app.snap()
        lines = app.logs()
        check("[113] one GET /pair/info, answered 404",
              fx.count("GET /pair/info (404)") == 1, f"x{fx.count('/pair/info')}")
        check("[113] no offer and no complaint of any kind",
              not app.find("connect_offer_box", s) and not has_text(app, "Found Octos on", s)
              and not app.find("connect_error", s)
              and logged(app, lines, "does not offer pairing (404)"))
        capture(app, "no-pairing-404")
        app.click("connect_token")
        app.type("tab-scoped-e2e-token")
        app.key("ReturnKey")
        on = app.wait(lambda: live(app), timeout=25)
        check("[113] the manual form is untouched: a token + Connect opens the server", on)
    finally:
        stop(app, fx, "p3")


def phase_open():
    say("== 4. a remembered server that needs no token: Connect to <host> connects at once")
    fp = FPORT + 2
    app, fx, _, ok = launch("p4", ["--pair", "open"], fp, seed=origin(fp))
    try:
        if not check("[114] tokenless: the Connect card shows", ok and app.find("connect_card")):
            return
        shown = app.wait(lambda: app.find("connect_offer_box"), timeout=10)
        check(f"[114] tokenless: offered once ('Found Octos on 127.0.0.1:{fp}.')",
              shown and app.text("connect_offer_text") == f"Found Octos on 127.0.0.1:{fp}.")
        app.logs()
        app.click("connect_offer")
        on = app.wait(lambda: live(app), timeout=25)
        lines = app.logs()
        check("[114] tokenless: CLICK Connect to <host> connects at once (no pairing dialog)",
              on and logged(app, lines, "discovery: offer used -> connect")
              and not logged(app, lines, "b1.open.pairing"))
    finally:
        stop(app, fx, "p4")


def phase_foreign():
    say("== 5. a remembered server off this computer: no probe at all")
    app, fx, _, ok = launch("p5", None, None, seed="http://192.168.1.20:50190")
    try:
        if not check("[114] foreign: the Connect card shows", ok and app.find("connect_card")):
            return
        time.sleep(2.0)
        lines = app.logs()
        check("[114] a remembered server off this computer: no request, no offer",
              not app.find("connect_offer_box")
              and logged(app, lines, "discovery: no probe (the remembered server is not an http(s) origin on this computer")
              and not logged(app, lines, "discovery: probing"))
    finally:
        stop(app, fx, "p5")


def main():
    if not BIN:
        print(__doc__)
        return 2
    if not FIXTURE.is_file():
        print(f"missing {FIXTURE}: cargo build -p octoscode-module --example board1_serve", file=sys.stderr)
        return 2
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    say(f"== A11 offer walk ({MODE}): app :{PORT}, fixtures :{FPORT}-{FPORT + 2}")
    parts = [phase_seen_and_offered, phase_404, phase_open, phase_foreign]
    # Development aid: A11_WALK_PARTS=1,3 runs those parts only.
    only = [int(x) for x in os.environ.get("A11_WALK_PARTS", "").split(",") if x.strip().isdigit()]
    for n, part in enumerate(parts, start=1):
        if only and n not in only:
            continue
        try:
            part()
        except Exception as e:  # noqa: BLE001 — a broken phase fails, the walk goes on
            check(f"[114] walk phase completes ({getattr(part, '__name__', 'phase')})", False, repr(e))
    failed = [r for r in RESULTS if not r[1]]
    say(f"== {len(RESULTS) - len(failed)}/{len(RESULTS)} passed ({MODE})")
    (OUT / "walk.log").write_text("\n".join(scrub(l) for l in WALK_LOG) + "\n")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
