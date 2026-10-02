"""A20 — the runner + helpers shared by tools/walk/a20_*.py.

Starts the a20 fixture server (`target/debug/examples/a20_serve`, parked
approvals/questions per Session, attested catalog roots; every request logged
as JSONL) and the app hidden on its own port with per-run isolated state
(brief §8), runs the walk, then ALWAYS stops both (the operator's
no-lingering-instances rule). The walks drive the app with A10's `Walk`
(every control CLICKED at its laid-out /snap rect) and read the WIRE from the
fixture's request log.
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, scrub  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent


class A20Walk(Walk):
    """A10's walk plus the a20 fixture's request log."""

    serve_log: pathlib.Path | None = None

    def wire(self, method: str, session: str | None = None) -> list[dict]:
        if not self.serve_log or not self.serve_log.exists():
            return []
        out = []
        for line in self.serve_log.read_text().splitlines():
            try:
                v = json.loads(line)
            except ValueError:
                continue
            if v.get("method") == method and (session is None or v.get("params", {}).get("session_id") == session):
                out.append(v.get("params") or {})
        return out

    def click_text(self, needle: str, wid: str) -> bool:
        """CLICK the visible `wid` whose text is `needle` (or its "…" cut)."""
        for w in self.snap():
            t = (w.get("t") or "").strip()
            if w.get("i") != wid or not self.shown(w):
                continue
            if t == needle or (t.endswith("…") and len(t) > 4 and needle.startswith(t[:-1].rstrip())):
                x, y, ww, h = w["r"]
                self.note(f"CLICK {wid} {needle!r} r={w['r']}")
                self.click_xy(x + ww / 2, y + h / 2)
                return True
        self.note(f"CLICK {wid} {needle!r} — not visible")
        return False

    def row_status(self, title: str, status_id: str = "sb_st_wait") -> bool:
        """Whether the sidebar row titled `title` shows `status_id` (the same
        PortalList item: the status dot sits on the title's row band)."""
        sn = self.snap()
        row = next((w for w in sn if w.get("i") == "sb_r_title" and self.shown(w)
                    and ((w.get("t") or "").strip() == title
                         or ((w.get("t") or "").endswith("…") and title.startswith((w.get("t") or "")[:-1].rstrip())))),
                   None)
        if row is None:
            return False
        cy = row["r"][1] + row["r"][3] / 2
        return any(w.get("i") == status_id and self.shown(w) and abs(w["r"][1] + w["r"][3] / 2 - cy) < 14 for w in sn)

    def sidebar_open(self) -> None:
        if self.mode == "phone" and not self.visible("sb_new_chat_hit"):
            self.click("sidebar_toggle_hit")
            self.wait(lambda: bool(self.visible("sb_new_chat_hit")), 4)

    def open_row(self, title: str) -> bool:
        self.sidebar_open()
        ok = self.click_text(title, "sb_r_title")
        time.sleep(1.2)
        return ok


def run(walk_fn, *, mode: str, outdir: str, port: int, fport: int, server_args: list | None = None,
        env: dict | None = None, app_bin: str | None = None, keep_state: pathlib.Path | None = None) -> int:
    app_bin = app_bin or os.environ.get("OCTOSCODE_APP_BIN")
    if not app_bin:
        raise SystemExit("set OCTOSCODE_APP_BIN to the HOST binary (outer/scripts/hostbuild.sh)")
    state = ROOT / "tmp" / "hs"
    state.mkdir(parents=True, exist_ok=True)
    iso = keep_state or (state / f"iso-a20-{port}-{int(time.time() * 1000)}")
    for sub in ("credentials", "recents", "downloads"):
        (iso / sub).mkdir(parents=True, exist_ok=True)
    serve_log = iso / "serve.jsonl"
    if serve_log.exists():
        serve_log.unlink()
    out_log = state / f"a20-serve-{fport}.log"
    bin_ = pathlib.Path(os.environ.get("CARGO_TARGET_DIR") or (ROOT / "target")) / "debug" / "examples" / "a20_serve"
    server = subprocess.Popen([str(bin_), str(fport), "--log", str(serve_log)] + list(server_args or []),
                              stdout=open(out_log, "w"), stderr=subprocess.STDOUT)
    end = time.time() + 15
    while time.time() < end and "listening on" not in out_log.read_text():
        if server.poll() is not None:
            raise SystemExit(f"a20_serve exited: {out_log.read_text()[-400:]}")
        time.sleep(0.2)
    e = dict(os.environ)
    e.update({
        "HEADLESS_STATE": str(state),
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if mode == "phone" else ""),
        # Brief §8: every piece of app state in this run's own dir.
        "OCTOSCODE_DRAFTS_FILE": str(iso / "composer-drafts.json"),
        "OCTOSCODE_CREDENTIALS_DIR": str(iso / "credentials"),
        "OCTOSCODE_PREF_PATH": str(iso / "display.json"),
        "OCTOSCODE_NOTIFICATIONS_FILE": str(iso / "notifications.json"),
        "OCTOSCODE_RECENTS_DIR": str(iso / "recents"),
        "OCTOSCODE_SHOW_THINKING_FILE": str(iso / "show-thinking.json"),
        "OCTOSCODE_DOWNLOAD_DIR": str(iso / "downloads"),
        "OCTOSCODE_DRIVER_ID_PATH": str(iso / "driver-id"),
        "OCTOSCODE_PANE_ADVANCED_FILE": str(iso / "session-pane-advanced.json"),
        "OCTOSCODE_CONNECTION_FILE": str(iso / "connection-v1.json"),
        "OCTOSCODE_DISPLAY_PREFS_PATH": str(iso / "display-v1.json"),
        "OCTOS_BASE_URL": f"http://127.0.0.1:{fport}",
        "OCTOS_PROFILE_ID": "a20",
        "OCTOS_BEARER": "fixture",
        "OCTOS_WORKSPACE_CWD": "/home/user/a20-ws",
    })
    if mode == "phone":
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    if env:
        e.update(env)
    rc = 1
    try:
        subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "start", app_bin, str(port)], env=e, check=True)
        w = A20Walk(port, outdir, mode)
        w.serve_log = serve_log
        w.wait(lambda: '"sz"' in w.get("/s", tolerant=True), 40, 1.0)
        time.sleep(4.0)
        if mode == "phone":
            w.open_phone_app()
        w.wait(lambda: w.composer() is not None, 30)
        walk_fn(w)
        rc = w.summary()
    finally:
        subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "stop", str(port)], env=e)
        server.terminate()
        try:
            server.wait(5)
        except Exception:
            server.kill()
        out = pathlib.Path(outdir)
        out.mkdir(parents=True, exist_ok=True)
        if serve_log.exists():
            (out / f"{mode}-wire.jsonl").write_text(scrub(serve_log.read_text()))
    return rc
