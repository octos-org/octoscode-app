#!/usr/bin/env python3
"""A16 — the parity re-audit's click walks. Every step CLICKS a control at
its laid-out rect (or types into a field the user would) on the real app,
launched hidden with isolated state (tools/walk/a10_lib.py `run_session`),
and asserts the app's own effect (/snap text or visibility) and what reached
the wire (the fixture server's log).

Walks:
  profile-ext  (parity row 284, web ProfileExtensionsDialog.tsx) — Skills and
               Research share ONE Profile scope and ONE lease: a skill install
               in flight (replay --slow profile/skills/install) locks the
               Research lanes opened from the palette; the lock lifts when
               the install lands.
  readonly     (row 280) — the server lists the models but offers no
               profile/llm/select: the rows read-only, a CLICK routes nothing.
  restart      (rows 272/280, web profileDefaultNeedsRestart +
               ModelsSettingsContent's notice) — the Session settings pane's
               Model card: no notice while the runtime IS the Profile
               default; the model menu's restart_required answer lights it;
               a Profile default that is not the runtime needs it on its own.
  sidebar      (row 242, web SessionSidebar / ProductSidebar SessionRow) —
               one sidebar row per listed Session; a row CLICK opens that
               Session; a Session deleted from the switcher leaves the
               sidebar. Against the A8 fixture server (a8_serve), started
               here on the replay port.

usage: OCTOSCODE_APP_BIN=<host octosense> A10_PORT=<app port> A10_REPLAY_PORT=<server port> \\
         a16_parity_walk.py <profile-ext|restart|readonly|sidebar> <desktop|phone> [outdir]
"""
import json
import os
import pathlib
import subprocess
import sys
import time

from a10_lib import ROOT, Walk, checks_line, dialog_checks, env_replay_port, run_session, scrub

WHICH = sys.argv[1] if len(sys.argv) > 1 else "restart"
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
OUT = sys.argv[3] if len(sys.argv) > 3 else f"docs/ux/a16/{WHICH}/{MODE}"


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0, viewport: str | None = None) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click_in(wid, viewport) if viewport else W.click(wid)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, viewport) if viewport else W.click(wid)
        logged = W.logged(needle, secs / 2) if ok else False
    got = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and got


# ------------------------------------------------------------ profile-ext

SK_VP = "dialog_scroll"
RS_VP = "b3_scroll"


def seen(W: Walk, wid: str, vp: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, vp)


def profile_ext(W: Walk) -> None:
    W.note("== 1. /skills (palette CLICK): the Profile scope")
    W.check("skills: /skills palette CLICK opens the dialog",
            W.palette_run("ski", "/skills") and W.wait_shown("dlg_skills_skills_warning", 10))
    W.check("skills: scoped to 'Server Profile: dsflash'", W.text("dlg_skills_scope") == "Server Profile: dsflash",
            W.text("dlg_skills_scope"))
    W.check("skills: no lock line while the Profile is idle", not W.visible("dlg_skills_skills_locked"))

    W.note("== 2. Install from source -> the confirm card -> Confirm (the server answers in 25 s)")
    W.scroll_into("dlg_skills_src_repo", SK_VP)
    r = W.rect("dlg_skills_src_repo")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.clear_field(30)
        W.type_text("octos-org/review-kit")
    W.dismiss_keyboard("dlg_skills_t_title")
    W.check("skills: Review installation CLICK -> the confirm names 'octos-org/review-kit · branch main'",
            click_logged(W, "dlg_skills_src_review_control", "dialog confirm asked: skills.install_source",
                         lambda: "octos-org/review-kit · branch main" in W.text("dlg_skills_cf_detail"), viewport=SK_VP))
    W.mark()
    W.click_in("dlg_skills_cf_confirm_control", SK_VP)
    W.check("skills: Confirm install CLICK -> the install holds the lease: the Skills lock line",
            W.wait(lambda: bool(W.visible("dlg_skills_skills_locked")) or W.scroll_into("dlg_skills_skills_locked", SK_VP), 4))
    W.shot(f"01-skills-locked-{MODE}")

    W.note("== 3. close Skills; /research (palette CLICK) while the install is still in flight")
    W.check("skills: the close glyph CLICK closes the dialog (the install keeps running)",
            W.click("dialog_close") and W.wait_shown("dialog_frame", 6, gone=True))
    W.check("research: /research palette CLICK opens the lanes dialog",
            W.palette_run("resea", "/research") and W.wait_shown("b3_dialog", 10))
    W.check("research: the same Profile scope ('Server Profile: dsflash')",
            W.wait(lambda: W.text("b3_research_scope") == "Server Profile: dsflash", 6), W.text("b3_research_scope"))
    W.check("research: the lanes are listed", W.wait(lambda: seen(W, "b3_research_lane_0_key", RS_VP), 10))
    W.check("research: the SHARED lease locks it — the Profile lock line while the skill install is in flight",
            W.wait(lambda: seen(W, "b3_research_locked", RS_VP), 4)
            and W.text("b3_research_locked") == "Configuration changes are paused while known Profile work is running.")
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_research_", "b3_title", "b3_close"), viewport=RS_VP)
    W.check("research (locked): dialog numeric checks", c["ok"], checks_line(c))
    W.shot(f"02-research-locked-{MODE}")
    W.mark()
    W.scroll_into("b3_research_lane_0_remove_box", RS_VP)
    r = W.rect("b3_research_lane_0_remove_box")
    if r:
        W.note(f"CLICK the drawn pill b3_research_lane_0_remove_box (no tap target while locked) r={r}")
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.check("research: a 'Remove strong' CLICK while locked routes nothing (no confirmation; still locked)",
            r is not None and not W.wait(lambda: bool(W.visible("b3_research_confirm_detail")), 1.5)
            and seen(W, "b3_research_locked", RS_VP))

    W.note("== 4. the install lands: the lease is released, Research unlocks")
    W.check("research: the lock line goes once the skill install is answered",
            W.wait(lambda: not W.visible("b3_research_locked"), 30))
    W.check("research: 'Remove strong' CLICK now asks ('Confirm lane removal' naming the key)",
            W.click_in("b3_research_lane_0_remove", RS_VP)
            and W.wait(lambda: seen(W, "b3_research_confirm_detail", RS_VP) and W.text("b3_research_confirm_detail") == "strong", 6))
    W.shot(f"03-research-unlocked-{MODE}")
    W.check("research: 'Cancel lane change' CLICK sends nothing",
            W.click_in("b3_research_cancel", RS_VP) and W.wait(lambda: not W.visible("b3_research_confirm_detail"), 6))
    W.check("research: the close glyph CLICK closes it", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))
    W.note("== 5. the wire: one install, no lane mutation, every read scoped to Profile dsflash")
    W.check("wire: profile/skills/install x1", W.replay_saw("profile/skills/install", 2) == 1)
    W.check("wire: no profile/sub_providers/remove (the locked Remove and the cancelled one sent nothing)",
            W.replay_saw("profile/sub_providers/remove", 1) == 0)
    W.check("wire: the lanes were listed", W.replay_saw("profile/sub_providers/list", 2) >= 1)


# ---------------------------------------------------------------- restart

NOTICE_R2 = ("Profile default is DeepSeek V4 Flash. This Octos process is still serving deepseek-v4-flash. "
             "Restart Octos to apply the new default.")
NOTICE_KIMI = ("Profile default is Kimi K3. This Octos process is still serving deepseek-v4-flash. "
               "Restart Octos to apply the new default.")
PANE_VP = "b3_scroll"


def open_pane(W: Walk) -> bool:
    time.sleep(0.4)
    ok = W.click("b3_strip_tap") and W.wait(lambda: bool(W.visible("b3_sc_model_title")), 8)
    if not ok:
        W.note("RETRY b3_strip_tap")
        ok = W.click("b3_strip_tap") and W.wait(lambda: bool(W.visible("b3_sc_model_title")), 8)
    # The pane's reads (status, presets, models) land after it opens.
    return ok and W.wait(lambda: W.text("b3_sc_runtime_value") != "", 8)


def close_pane(W: Walk) -> bool:
    return W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True)


def pane_numeric(W: Walk, name: str):
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_sc_", "b3_title", "b3_close"), viewport=PANE_VP)
    W.check(f"{name}: pane numeric checks", c["ok"], checks_line(c))
    r, card = W.rect("b3_sc_restart"), W.rect("b3_sc_model")
    if r and card:
        txt = W.rect("b3_sc_restart_text")
        inside = card[0] - 1 <= r[0] and r[0] + r[2] <= card[0] + card[2] + 1
        fits = bool(txt) and r[0] <= txt[0] and txt[0] + txt[2] <= r[0] + r[2] + 1 and txt[1] + txt[3] <= r[1] + r[3] + 1
        W.check(f"{name}: the notice block sits inside the Model card and holds its whole text",
                inside and fits, f"block={r} text={txt} card={card}")


def choose_model(W: Walk, row: str, notice: str) -> bool:
    time.sleep(0.4)
    ok = W.click("model_seat_hit") and W.wait(lambda: bool(W.visible("b3_model_opt_0_name")), 8)
    if not ok:
        W.note("RETRY model_seat_hit")
        ok = W.click("model_seat_hit") and W.wait(lambda: bool(W.visible("b3_model_opt_0_name")), 8)
    if not ok:
        return False
    got = click_logged(W, row, "ModelSelect", lambda: W.text("b3_model_notice") == notice, 10)
    # An outside press dismisses the menu (the web's outside-press close; on
    # the phone shell Escape would leave the app for the home screen).
    d = W.rect("b3_dialog")
    if d:
        x, y = d[0] + d[2] / 2, d[1] - 40
        W.note(f"CLICK outside the menu at ({x:.0f},{y:.0f})")
        W.click_xy(x, y)
    closed = W.wait(lambda: not W.visible("b3_dialog"), 6)
    return got and closed


def restart(W: Walk) -> None:
    W.note("== 1. the strip CLICK opens Session settings: the Model card's truth, no restart notice")
    W.check("pane: strip CLICK -> 'Session settings' with the Model card", open_pane(W))
    W.check("pane: 'Saved for this profile:' names the Profile default",
            W.text("b3_sc_saved_value") == "DeepSeek V4 Flash", W.text("b3_sc_saved_value"))
    W.check("pane: 'Session runtime' names the status's runtime model",
            W.text("b3_sc_runtime_value") == "deepseek-v4-flash", W.text("b3_sc_runtime_value"))
    W.check("pane: no restart notice — the runtime IS the Profile default and nothing hinted a restart",
            not W.visible("b3_sc_restart"))
    W.shot(f"01-pane-no-restart-{MODE}")
    W.check("pane: the close glyph CLICK closes it", close_pane(W))

    W.note("== 2. the model menu: the r2-route fallback answers r2's recorded restart_required")
    W.check("model: seat CLICK -> r2-route CLICK -> 'Saved. The server keeps running deepseek-v4-flash until it restarts'",
            choose_model(W, "b3_model_opt_1", "Saved. The server keeps running deepseek-v4-flash until it restarts"))
    W.check("pane: strip CLICK reopens it", open_pane(W))
    W.check("pane: the web's restart notice (the last answer was restart_required; equal strings never disprove it)",
            W.wait(lambda: W.text("b3_sc_restart_text") == NOTICE_R2, 6), W.text("b3_sc_restart_text"))
    pane_numeric(W, "restart notice")
    W.shot(f"02-pane-restart-hint-{MODE}")
    W.check("pane: closed", close_pane(W))

    W.note("== 3. Kimi K3 answers reloaded (the hint goes out) — but the runtime is not the new Profile default")
    W.check("model: seat CLICK -> Kimi K3 CLICK -> 'Saved. Your next message uses kimi-k3'",
            choose_model(W, "b3_model_opt_2", "Saved. Your next message uses kimi-k3"))
    W.check("pane: strip CLICK reopens it", open_pane(W))
    W.check("pane: 'Saved for this profile:' is now Kimi K3", W.wait(lambda: W.text("b3_sc_saved_value") == "Kimi K3", 6),
            W.text("b3_sc_saved_value"))
    W.check("pane: the runtime (deepseek-v4-flash) is not the Profile default -> the notice on its own",
            W.wait(lambda: W.text("b3_sc_restart_text") == NOTICE_KIMI, 6), W.text("b3_sc_restart_text"))
    W.check("pane: this app's own menu choice is not 'changed in another tab or app' (one last-seen selection)",
            not W.visible("b3_sc_model_external"))
    pane_numeric(W, "identity notice")
    W.shot(f"03-pane-restart-identity-{MODE}")
    W.check("pane: closed", close_pane(W))
    W.note("== 4. the wire")
    W.check("wire: profile/llm/select x2", W.replay_saw("profile/llm/select", 2) == 2)
    W.check("wire: session/status/read on every pane open (>= 3)", W.replay_saw("session/status/read", 2) >= 3)


def readonly(W: Walk) -> None:
    """The server offers the list but not `profile/llm/select` (replay
    --drop-method): the Profile's models read-only, a row CLICK routes
    nothing."""
    W.note("== 1. the strip CLICK opens Session settings: the list without a select")
    W.check("pane: strip CLICK -> 'Session settings' with the Model card", open_pane(W))
    W.check("pane: the models are listed", W.wait(lambda: bool(W.visible("b3_sc_model_2_title")), 6))
    W.check("pane: 'Profile defaults are read-only on this server.'",
            W.wait(lambda: W.text("b3_sc_models_readonly") == "Profile defaults are read-only on this server.", 6),
            W.text("b3_sc_models_readonly"))
    pane_numeric(W, "read-only list")
    W.shot(f"01-pane-read-only-{MODE}")
    W.mark()
    r = W.rect("b3_sc_model_2_box") or W.rect("b3_sc_model_2_title")
    if r:
        W.note(f"CLICK the Kimi K3 row (no tap target while read-only) r={r}")
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.check("pane: a row CLICK routes nothing (no 'Saving…', no select on the wire)",
            r is not None and not W.wait(lambda: bool(W.visible("b3_sc_model_saving")), 1.5)
            and W.replay_saw("profile/llm/select", 1) == 0)
    W.check("pane: closed", close_pane(W))


# ---------------------------------------------------------------- sidebar

A8_TITLES = ["Fix steer queue drop on reconnect", "Add session fork", "Review PR #2566",
             "Why is hydrate slow?", "Bump octos-core to a6ea8505", "Legacy chat"]


def a8_wire(log: pathlib.Path, method: str) -> list:
    out = []
    if not log.exists():
        return out
    for line in log.read_text().splitlines():
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if v.get("method") == method:
            out.append(v.get("params") or {})
    return out


def sidebar_open(W: Walk) -> None:
    if MODE == "phone" and not W.visible("sb_new_chat_hit"):
        W.click("sidebar_toggle_hit")
        W.wait(lambda: bool(W.visible("sb_new_chat_hit")), 4)


def sidebar_close(W: Walk) -> None:
    """Phone: the drawer covers the composer — its close control (the web's
    compact navigation surface dismiss)."""
    if MODE == "phone" and W.visible("sb_new_chat_hit"):
        if not W.click("drawer_close"):
            W.click("drawer_scrim_hit")
        W.wait(lambda: not W.visible("sb_new_chat_hit"), 4)


def row_titles(W: Walk) -> list:
    sb = W.rect("thread_list")
    rows = W.visible("sb_r_title")
    return [w.get("t") or "" for w in rows if not sb or (w["r"][1] >= sb[1] - 1 and w["r"][1] < sb[1] + sb[3])]


def click_row(W: Walk, title: str) -> bool:
    for w in W.visible("sb_r_title"):
        t = w.get("t") or ""
        if t == title or (t.endswith("…") and title.startswith(t[:-1].rstrip())):
            x, y, ww, hh = w["r"]
            W.note(f"CLICK sidebar row {title!r} r={w['r']}")
            W.click_xy(x + ww / 2, y + hh / 2)
            return True
    W.note(f"no sidebar row {title!r}")
    return False


def selected_row(W: Walk) -> str:
    sel = W.visible("sb_r_sel")
    if not sel:
        return ""
    sy = sel[0]["r"][1] + sel[0]["r"][3] / 2
    for w in W.visible("sb_r_title"):
        r = w["r"]
        if r[1] <= sy <= r[1] + r[3] + 8:
            return w.get("t") or ""
    return ""


def sidebar_walk_for(log: pathlib.Path):
    def walk(W: Walk) -> None:
        W.note("== 1. the sidebar draws ONE row per listed Session")
        W.check("wire: the app listed the Sessions (session/list)", W.wait(lambda: len(a8_wire(log, "session/list")) >= 1, 10))
        sidebar_open(W)
        W.check("sidebar: one row per Session — the six listed titles, each once",
                W.wait(lambda: sorted(row_titles(W)) == sorted(A8_TITLES), 10), str(row_titles(W)))
        rows = W.visible("sb_r_title")
        hs = sorted({round(w["r"][3]) for w in rows})
        W.check("sidebar: the rows' labels share one height (uniform rows)", len(hs) == 1, str(hs))
        W.shot(f"01-sidebar-rows-{MODE}")

        W.note("== 2. a row CLICK opens that Session")
        n_open = len(a8_wire(log, "session/open"))
        W.check("sidebar: 'Review PR #2566' CLICK", click_row(W, "Review PR #2566"))
        W.check("wire: session/open for a8:api:beta",
                W.wait(lambda: any(p.get("session_id") == "a8:api:beta" for p in a8_wire(log, "session/open")[n_open:]), 8))
        W.check("header: the opened Session's title", W.wait(lambda: W.text("hd_title") == "Review PR #2566", 8), W.text("hd_title"))
        sidebar_open(W)
        W.check("sidebar: the opened row is the selected one", W.wait(lambda: selected_row(W) == "Review PR #2566", 6), selected_row(W))
        W.shot(f"02-sidebar-selected-{MODE}")

        W.note("== 3. a Session deleted from the switcher (/sessions) leaves the sidebar")
        sidebar_close(W)
        c = W.composer()
        if c:
            x, y, w, h = c["r"]
            W.click_xy(x + w / 2, y + h / 2)
        W.clear_field()
        W.type_text("/sessions")
        W.key("Return")
        W.check("switcher: /sessions opens 'Open a different session'",
                W.wait(lambda: W.text("b3_title") == "Open a different session", 8), W.text("b3_title"))
        hollow = None
        for w in W.snap():
            i, t = str(w.get("i") or ""), w.get("t") or ""
            if i.startswith("b3_switch_row_") and i.endswith("_title") and t == "Why is hydrate slow?":
                hollow = i[len("b3_switch_row_"):].split("_")[0]
        W.check("switcher: the row 'Why is hydrate slow?' offers delete",
                hollow is not None and W.scroll_into(f"b3_switch_row_{hollow}_delete", "b3_scroll"))
        if hollow is not None:
            W.click(f"b3_switch_row_{hollow}_delete")
            W.check("switcher: delete asks first ('Delete?')",
                    W.wait(lambda: W.text(f"b3_switch_row_{hollow}_confirm_q") == "Delete?", 6))
            W.click(f"b3_switch_row_{hollow}_confirm_yes")
            W.check("wire: ONE session/delete for a8:api:hollow",
                    W.wait(lambda: a8_wire(log, "session/delete") == [{"session_id": "a8:api:hollow"}], 8),
                    str(a8_wire(log, "session/delete")))
        W.click("b3_close")
        W.wait(lambda: not W.visible("b3_dialog"), 4)
        sidebar_open(W)
        want = sorted(t for t in A8_TITLES if t != "Why is hydrate slow?")
        W.check("sidebar: the deleted Session's row is gone; the other five stay, each once",
                W.wait(lambda: sorted(row_titles(W)) == want, 8), str(row_titles(W)))
        W.shot(f"03-sidebar-after-delete-{MODE}")

    return walk


def run_sidebar() -> int:
    port = env_replay_port()
    state = ROOT / "tmp" / "hs"
    state.mkdir(parents=True, exist_ok=True)
    log = state / f"a8-serve-{port}-{int(time.time())}.jsonl"
    bin_ = pathlib.Path(os.environ.get("CARGO_TARGET_DIR") or (ROOT / "target")) / "debug" / "examples" / "a8_serve"
    out = open(state / f"a8-serve-{port}.out", "w")
    serve = subprocess.Popen([str(bin_), str(port), "--log", str(log)], stdout=out, stderr=subprocess.STDOUT)
    try:
        end = time.time() + 15
        while time.time() < end and "listening on" not in (state / f"a8-serve-{port}.out").read_text():
            if serve.poll() is not None:
                raise SystemExit("a8_serve exited")
            time.sleep(0.2)
        env = {"OCTOS_BASE_URL": f"http://127.0.0.1:{port}", "OCTOS_PROFILE_ID": "a8", "OCTOS_BEARER": "a8-walk"}
        rc = run_session(sidebar_walk_for(log), mode=MODE, outdir=OUT, replay_port=None, env=env)
        pathlib.Path(OUT, "serve.jsonl").write_text(scrub(log.read_text() if log.exists() else ""))
        return rc
    finally:
        serve.terminate()
        try:
            serve.wait(5)
        except Exception:
            serve.kill()


if __name__ == "__main__":
    if WHICH == "profile-ext":
        sys.exit(run_session(profile_ext, mode=MODE, outdir=OUT, scenario="a10",
                             replay_args=["--slow", "profile/skills/install=25000"]))
    if WHICH == "restart":
        sys.exit(run_session(restart, mode=MODE, outdir=OUT, scenario="a10"))
    if WHICH == "readonly":
        sys.exit(run_session(readonly, mode=MODE, outdir=OUT, scenario="a10",
                             replay_args=["--drop-method", "profile/llm/select"]))
    if WHICH == "sidebar":
        sys.exit(run_sidebar())
    raise SystemExit(f"unknown walk {WHICH!r}")
