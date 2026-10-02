#!/usr/bin/env python3
"""A21 — the pre-connection bootstrap (parity row 199) and the one-identity
credential split (row 195), walked by CLICKS on the real app, relaunched on
the SAME isolated state between phases (brief §8: never ~/.octoscode).

The product path only: the app gets OCTOS_BASE_URL (the build's default
endpoint, the web's VITE_OCTOS_DEFAULT_ENDPOINT) and a startup folder — never
OCTOS_BEARER / OCTOS_PROFILE_ID (the TEST-ONLY harness start). Two A8 fixture
servers (`a8_serve --principal-from-token`) count every socket they accept,
so "no socket" is read off the server, not inferred.

  1. first launch, nothing remembered: the Connect card on the default
     endpoint, no token — and NO socket at the server [199];
  2. Connect (a click): one socket; the connection authenticates [199];
  3. relaunch: the remembered server restores itself with no click — the
     remembered Session re-opened directly, no launch/resolve [199];
  4. Settings > Disconnect, relaunch: the Connect card with the server and
     token kept, NO socket [199];
  5. Connect to ANOTHER server: only its token is kept, the first server's
     restore hints are gone [195];
  6. a token saved for a third origin (the former per-origin device memory)
     is purged at the next start; the connection's own token restores [195];
  7. Settings > Forget server: the initial draft (default endpoint, no token),
     no token for any origin, no remembered address; relaunch: the Connect
     card, NO socket [199, 195].

  python3 tools/walk/a21_bootstrap_walk.py <host-bin> <desktop|phone> [port] [fixture-port] [out-dir]

Prints `PASS|FAIL <name> — <detail>` per check; exit 0 iff all passed.
Evidence: <out>/<mode>/walk.log, captures + scrubbed snaps.
"""
from __future__ import annotations

import json
import shutil
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from a21_lib import EXAMPLES, ROOT, Fixture, Walk, card_checks, composer_checks, origin  # noqa: E402

WALK = {
    "name": "a21_bootstrap",
    "title": "pre-connection bootstrap (restore only a remembered connection) + one connection identity",
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
FPORT2 = FPORT + 2
OUT = (pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else ROOT / "tmp" / "walk" / "a21-bootstrap").resolve() / MODE
CWD = "/home/user/octos"
LEGACY = "http://127.0.0.1:8499"


def hexname(o: str) -> str:
    return "".join(f"{b:02x}" for b in o.encode()) + ".token"


def token_origins(w: Walk) -> list[str]:
    """The origins that have a saved token (file NAMES only — never read)."""
    d = w.state / "cred"
    out = []
    for p in sorted(d.glob("*.token")) if d.exists() else []:
        try:
            out.append(bytes.fromhex(p.name[: -len(".token")]).decode())
        except ValueError:
            out.append("?")
    return out


def last_server(w: Walk) -> str | None:
    p = w.state / "cred" / "last-server"
    return p.read_text().strip() if p.exists() else None


def remembered_origins(w: Walk) -> list[str]:
    p = w.state / "connection-v1.json"
    if not p.exists():
        return []
    return sorted(json.loads(p.read_text()).get("servers", {}).keys())


def wire(log: pathlib.Path, method: str) -> list[dict]:
    out = []
    if log.exists():
        for line in log.read_text().splitlines():
            try:
                v = json.loads(line)
            except ValueError:
                continue
            if v.get("method") == method:
                out.append(v.get("params") or {})
    return out


def sockets(*fx: Fixture) -> int:
    return sum(f.count("ui-protocol socket open") for f in fx)


def main() -> int:
    # A fresh state tree every run: "nothing remembered" must be true.
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True, exist_ok=True)
    log_a, log_b = OUT / "serve-a.jsonl", OUT / "serve-b.jsonl"
    fa = Fixture([str(EXAMPLES / "a8_serve"), str(FPORT), "--principal-from-token", "--advertise", "--log", str(log_a)], OUT / "serve-a.log")
    fb = Fixture([str(EXAMPLES / "a8_serve"), str(FPORT2), "--principal-from-token", "--advertise", "--log", str(log_b)], OUT / "serve-b.log")
    w = Walk(BIN, MODE, PORT, OUT, {"OCTOS_BASE_URL": origin(FPORT), "OCTOS_WORKSPACE_CWD": CWD})
    try:
        fa.start()
        fb.start()
        # ---- 1. first launch, nothing remembered ---------------------------
        w.launch()
        w.check("[199] first launch, nothing remembered: the Connect card", w.wait(w.card, 30))
        time.sleep(4.0)
        s = w.snap()
        w.check("[199] …prefilled with the default endpoint (OCTOS_BASE_URL)", w.val("connect_server", s) == origin(FPORT),
                repr(w.val("connect_server", s)))
        w.check("[199] …and no token (the initial draft)", len(w.val("connect_token", s)) == 0)
        w.check("[199] …and NO socket: the server saw no connection and no REST read",
                sockets(fa, fb) == 0 and fa.count("/api/auth/me") == 0, f"sockets={sockets(fa, fb)}")
        logs = w.all_logs()
        w.check("[199] …the bootstrap decided: no auto-start",
                any("bootstrap: None" in l for l in logs) and any("no auto-start" in l for l in logs))
        card_checks(w, "first launch")
        w.shot(f"01-first-launch-{MODE}")

        # ---- 2. Connect by click ---------------------------------------------
        w.connect(None, "alice.1")
        w.check("[199] Connect (a click) dials the prefilled server and goes live", w.wait(w.live, 40))
        w.check("[199] …one socket, opened by the click", sockets(fa) == 1, f"sockets={sockets(fa)}")
        w.check("[199] …the connection authenticated: it may restore at the next launch",
                w.wait(lambda: any("restores at the next launch" in l for l in w.all_logs()), 10))
        opened = wire(log_a, "session/open")
        first = opened[-1].get("session_id") if opened else None
        w.check("[199] …a Session opened in the startup folder", bool(first) and opened[-1].get("cwd") == CWD, repr(opened[-1:]))
        w.wait(lambda: w.state.joinpath("connection-v1.json").exists(), 8)
        w.stop()

        # ---- 3. relaunch: the remembered connection restores itself ----------
        n_sock, n_open, n_resolve = sockets(fa), len(wire(log_a, "session/open")), len(wire(log_a, "launch/resolve"))
        w.launch()
        w.check("[199] relaunch: the remembered server restores itself — no click", w.wait(w.live, 40))
        w.check("[199] …a socket to it, opened by the restore", sockets(fa) == n_sock + 1, f"sockets={sockets(fa)}")
        reopened = wire(log_a, "session/open")[n_open:]
        w.check("[199] …the remembered Session re-opened directly (no launch/resolve)",
                bool(reopened) and reopened[0].get("session_id") == first and len(wire(log_a, "launch/resolve")) == n_resolve,
                f"{[o.get('session_id') for o in reopened]} vs {first}")
        w.check("[199] …the bootstrap decided: restore", any("bootstrap: Restore(" in l for l in w.all_logs()))
        composer_checks(w, "restored at launch")
        w.shot(f"02-restored-{MODE}")

        # ---- 4. Disconnect, relaunch: no unattended restore ------------------
        w.check("Settings > Connection > Disconnect returns the Connect card", w.leave("disconnect"))
        w.stop()
        n_sock = sockets(fa, fb)
        w.launch()
        w.check("[199] after a Disconnect the relaunch shows the Connect card", w.wait(w.card, 30))
        time.sleep(4.0)
        s = w.snap()
        w.check("[199] …NO socket (a Disconnect stops the unattended restore)", sockets(fa, fb) == n_sock,
                f"sockets={sockets(fa, fb)} before={n_sock}")
        w.check("[199] …the server and its token kept on the card (the tab's identity)",
                w.val("connect_server", s) == origin(FPORT) and len(w.val("connect_token", s)) > 0, repr(w.val("connect_server", s)))
        card_checks(w, "relaunch after Disconnect")
        w.shot(f"03-after-disconnect-relaunch-{MODE}")

        # ---- 5. another server: one connection identity -----------------------
        w.connect(origin(FPORT2), "carol.1")
        w.check("[195] Connect to another server goes live", w.wait(w.live, 40))
        w.wait(lambda: any("restores at the next launch" in l for l in w.all_logs()[-60:]), 10)
        w.check("[195] one identity: only the new server keeps a token", token_origins(w) == [origin(FPORT2)], repr(token_origins(w)))
        w.check("[195] …the durable address is the new server", last_server(w) == origin(FPORT2), repr(last_server(w)))
        w.wait(lambda: remembered_origins(w) == [origin(FPORT2)], 8)
        w.check("[195] …the first server's restore hints went with its identity", origin(FPORT) not in remembered_origins(w),
                repr(remembered_origins(w)))
        w.stop()

        # ---- 6. the former device memory is purged at start -------------------
        legacy = w.state / "cred" / hexname(LEGACY)
        legacy.write_text("walk-dummy-legacy")
        legacy.chmod(0o600)
        n_sock = sockets(fb)
        w.launch()
        w.check("[195] relaunch: a token saved for another origin (the former device memory) is purged at start",
                not legacy.exists(), repr(token_origins(w)))
        w.check("[195] …the connection's own token stays and it restores itself",
                token_origins(w) == [origin(FPORT2)] and w.wait(w.live, 40) and sockets(fb) == n_sock + 1)
        composer_checks(w, "restored after the switch")
        w.shot(f"04-restored-after-switch-{MODE}")

        # ---- 7. Forget: back to the initial draft ----------------------------
        w.check("Settings > Connection > Forget server returns the Connect card", w.leave("forget"))
        s = w.snap()
        w.check("[199] Forget: the initial draft — the default endpoint, no token",
                w.val("connect_server", s) == origin(FPORT) and len(w.val("connect_token", s)) == 0, repr(w.val("connect_server", s)))
        w.check("[195] Forget: no token for any origin, no remembered address, no restore hint",
                token_origins(w) == [] and last_server(w) is None and remembered_origins(w) == [],
                f"{token_origins(w)} {last_server(w)} {remembered_origins(w)}")
        w.stop()
        n_sock = sockets(fa, fb)
        w.launch()
        w.check("[199] after Forget the relaunch shows the Connect card", w.wait(w.card, 30))
        time.sleep(4.0)
        s = w.snap()
        w.check("[199] …NO socket", sockets(fa, fb) == n_sock, f"sockets={sockets(fa, fb)} before={n_sock}")
        w.check("[199] …on the default endpoint, no token", w.val("connect_server", s) == origin(FPORT) and len(w.val("connect_token", s)) == 0)
        card_checks(w, "relaunch after Forget")
        w.shot(f"05-after-forget-relaunch-{MODE}")
    except Exception as e:  # noqa: BLE001 — a crash is a FAIL line, never a lingering app
        w.check("the walk ran to its end", False, f"{type(e).__name__}: {e}")
    finally:
        w.stop()
        fa.stop()
        fb.stop()
    return w.finish("a21 bootstrap")


if __name__ == "__main__":
    sys.exit(main())
