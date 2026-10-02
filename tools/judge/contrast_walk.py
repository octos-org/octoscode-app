#!/usr/bin/env python3
"""A18 — the contrast walk (web e2e/theme.spec.ts:72-112, walk results row 212).

    python3 tools/judge/contrast_walk.py <host-bin> <desktop|phone> <port> <rport> <outdir> [light|dark] [chrome|surfaces|seats]

The web runs axe's color-contrast rule on the conversation after a real turn, then on Settings, in MANUAL light mode
on a dark OS, and expects zero violations. Natively the theme is `OCTOSCODE_THEME=light` (the stored preference; the
OS stays as it is) and every surface is reached by CLICKS against a replay server (no model):

`chrome` (the replay `interrupt` scenario: the live-gate recording — a completed turn, then a turn whose terminal is
`interrupted`):
  the fresh chat (header, empty state, status strip, composer placeholder, sidebar), the completed turn, the
  stopped turn ("Turn stopped" notice), the phone's sidebar drawer, the composer's menus (model, permission, +),
  the session pane (the strip's tap), the sidebar's sort and workspace menus, and every Settings section;
`surfaces` (A6's `surfaces` scenario, r23's recorded turns): reasoning rows + answer, a tool row and delivered
  files, the question card, the approval card, the plan;
`seats` (A10's `a10` scenario, its seat simulator): the composer with both seats read back, the permission menu,
  the model menu (provider groups, rows), the session pane.

Each surface is measured by tools/judge/contrast.py on the app's own full-resolution framebuffer: an overlay (a
menu, the session pane, Settings) counts only its own nodes (those that appeared with it), a transcript row half
under the header is skipped. `<outdir>/<NN>-<surface>.*` keeps the PNG (<= 1400 px), the full grab, the snap and
the per-node TSV. A surface passes when no informational text node is below its threshold. The app and the replay
server are always stopped (a10_lib.run_session).
"""
from __future__ import annotations

import pathlib
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "walk"))
sys.path.insert(0, str(HERE))
import a10_lib  # noqa: E402
import contrast  # noqa: E402

PROMPTS = ["Why does main.rs print 5?", "Explain the borrow checker in one paragraph.",
           "Pick a color for the theme.", "Run the build with sudo.", "Plan the release."]
SECTIONS = ("Permissions", "Model", "Sandbox", "Connection", "Preferences", "About")
LISTS = ["timeline_list", "thread_list"]


def main() -> int:
    if len(sys.argv) < 6:
        print(__doc__)
        return 2
    app_bin, mode, port, rport, outdir = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
    theme = sys.argv[6] if len(sys.argv) > 6 else "light"
    flow = sys.argv[7] if len(sys.argv) > 7 else "chrome"
    out = pathlib.Path(outdir)
    phone = mode == "phone"
    n = [0]
    totals: list[tuple[str, int, float]] = []
    # A disabled control's style (board 3's DISABLED_INK, screens::theme::EXEMPT_INKS): axe skips a disabled
    # control. In light that ink is never text; in dark the tertiary text reads the same grey, so only its
    # disabled pill (on DISABLED_BG) is exempt there.
    exempt = [("#a1a1a6", None)] if theme == "light" else [("#a1a1a6", "#e9e9eb")]

    def record(w: a10_lib.Walk, stem: str, rows: list) -> None:
        scored = [m for m in rows if m["verdict"] in ("pass", "FAIL")]
        bad = [m for m in scored if m["verdict"] == "FAIL"]
        low = min((m["ratio"] for m in scored), default=0.0)
        totals.append((stem, len(bad), low))
        w.check(f"contrast {stem} ({theme}): every text node meets its threshold", not bad and bool(scored),
                f"{len(scored)} nodes, min {low:.2f}:1, below: "
                + "; ".join(f"{m['text'][:24]!r} {m['ratio']:.2f} {m['fg']} on {m['bg']}" for m in bad[:6]))

    def measure(w: a10_lib.Walk, name: str, **kw) -> None:
        n[0] += 1
        stem = f"{n[0]:02d}-{name}"
        time.sleep(0.8)  # the last frame settles (a hover-out, a fade)
        record(w, stem, contrast.measure_live(port, out, stem, exempt=exempt, **kw))

    def keys(w: a10_lib.Walk) -> set:
        return {contrast.node_key(x) for x in w.snap() if a10_lib.Walk.shown(x)}

    def overlay(w: a10_lib.Walk, name: str, trigger: str, close: str | None = None) -> None:
        """Click `trigger`, measure what appeared (the overlay's own nodes), dismiss it."""
        before = keys(w)
        if not w.click(trigger):
            w.check(f"{name}: its trigger {trigger} is on screen", False)
            return
        count, steady = 0, 0
        for _ in range(20):  # until the overlay's content is in: its node count holds for three polls
            time.sleep(0.4)
            now = len(keys(w) - before)
            steady = steady + 1 if (now and now == count) else 0
            count = now
            if steady >= 2:
                break
        if not count:
            w.note(f"{name}: no text appeared after {trigger} — not measured")
            return
        measure(w, name, before=before, clip=LISTS)
        if close and w.click(close):
            time.sleep(0.6)
        elif not phone:
            w.key("Escape")
            time.sleep(0.6)
        else:
            w.click(trigger)
            time.sleep(0.6)

    def idle(w: a10_lib.Walk) -> bool:
        sn = w.snap()
        return bool(w.visible("composer_send_icon", sn)) and not w.visible("composer_stop_icon", sn) \
            and not w.visible("composer_stop_busy", sn)

    def send(w: a10_lib.Walk, prompt: str) -> None:
        c = w.composer()
        if c is None:
            return
        w.click_xy(c["r"][0] + 12, c["r"][1] + c["r"][3] / 2)
        w.key("End")
        w.clear_field(80)
        w.type_text(prompt)
        w.key("Return")
        w.dismiss_keyboard()

    def chrome(w: a10_lib.Walk) -> None:
        measure(w, "fresh-chat", clip=LISTS)
        send(w, PROMPTS[0])
        w.wait(lambda: w.has_text("Worked for") or w.has_text("print"), 20)
        w.wait(lambda: idle(w), 30)
        measure(w, "turn-completed", clip=LISTS)
        send(w, PROMPTS[1])
        w.wait(lambda: w.has_text("Turn stopped"), 30)
        w.wait(lambda: idle(w), 20)
        measure(w, "turn-stopped", clip=LISTS)
        if phone and w.visible("sidebar_toggle_hit"):
            overlay(w, "sidebar-drawer", "sidebar_toggle_hit", close="drawer_close")
        for trigger, name in (("model_seat_hit", "model-menu"), ("approval_pill_hit", "permission-menu"),
                              ("plus_hit", "plus-menu"), ("b3_strip_tap", "session-pane"),
                              ("hd_defaults_change", "new-chat-defaults")):
            if w.visible(trigger):
                overlay(w, name, trigger)
        if not phone:
            for trigger, name in (("sb_sort", "sort-menu"), ("sb_g_more", "workspace-menu")):
                if w.visible(trigger):
                    overlay(w, name, trigger)
        # Settings (a centred dialog on desktop, a full sheet on the phone): each section counts what is not the
        # conversation beneath it — every node shown before Settings opened is left out.
        under = keys(w)
        if w.click("settings_open_hit") or w.click("hd_settings_label"):
            w.wait(lambda: bool(w.visible("set_title")), 10)
            time.sleep(1.0)
            measure(w, "settings-general", before=under)
            if w.visible("server_stop_request"):
                # Settings > General's "Stop server…": the confirm sheet (then Cancel — nothing is stopped).
                pre = keys(w)
                w.click("server_stop_request")
                if w.wait(lambda: bool(w.visible("server_stop_cancel")), 6):
                    time.sleep(0.8)
                    measure(w, "settings-stop-confirm", before=pre)
                    w.click("server_stop_cancel")
                    time.sleep(0.8)
            for sec in SECTIONS:
                hit = next((x for x in w.snap() if (x.get("t") or "").strip() == sec and a10_lib.Walk.shown(x)), None)
                if hit is None:
                    w.check(f"settings: the {sec} section is reachable", False, "no nav row")
                    continue
                r = hit["r"]
                w.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
                time.sleep(1.2)
                measure(w, f"settings-{sec.lower()}", before=under)
            w.click("settings_close") or w.click("set_back")

    def held(w: a10_lib.Walk, card: str, release: str, name: str, tries: int = 6) -> None:
        """A takeover card the turn waits on: measure the first one, then release every hold."""
        if not w.wait_shown(card, 25):
            w.check(f"{name}: the card shows", False)
            return
        time.sleep(1.0)
        measure(w, name, clip=LISTS)
        for _ in range(tries):
            if not w.visible(card):
                break
            w.click(release)
            time.sleep(1.5)
        w.wait(lambda: idle(w), 20)

    def surfaces(w: a10_lib.Walk) -> None:
        send(w, PROMPTS[0])
        w.wait(lambda: w.prefixed("b3_tl_think_"), 25)
        w.wait(lambda: idle(w), 25)
        measure(w, "reasoning-answer", clip=LISTS)
        send(w, PROMPTS[1])
        w.wait(lambda: w.prefixed("b3_tl_file_"), 25)
        w.wait(lambda: idle(w), 25)
        measure(w, "tool-and-files", clip=LISTS)
        send(w, PROMPTS[2])
        held(w, "cv_q_card", "cv_q_stop", "question-card")
        send(w, PROMPTS[3])
        held(w, "cv_ap_card", "cv_ap_deny", "approval-card")
        send(w, PROMPTS[4])
        if w.wait(lambda: w.prefixed("cv_pl_"), 25):
            time.sleep(1.0)
            measure(w, "plan", clip=LISTS)
        else:
            w.check("plan: the plan card shows", False)
        w.click("send_hit")
        w.wait(lambda: idle(w), 20)
        measure(w, "conversation-after", clip=LISTS)

    def seats(w: a10_lib.Walk) -> None:
        w.wait(lambda: bool(w.visible("i0_composer_2")) and bool(w.visible("i0_composer_model")), 15)
        time.sleep(1.5)  # the seats' read-back (the server's preset, the profile's model)
        measure(w, "seats-composer", clip=LISTS)
        for trigger, name in (("approval_pill_hit", "permission-menu"), ("model_seat_hit", "model-menu"),
                              ("b3_strip_tap", "session-pane")):
            if w.visible(trigger):
                overlay(w, name, trigger)

    def walk(w: a10_lib.Walk) -> None:
        {"surfaces": surfaces, "seats": seats}.get(flow, chrome)(w)
        print("== contrast surfaces: " + ", ".join(f"{s} {b} below (min {lo:.2f})" for s, b, lo in totals))

    scenario = {"surfaces": "surfaces", "seats": "a10"}.get(flow, "interrupt")
    return a10_lib.run_session(walk, mode=mode, outdir=outdir, port=port, replay_port=rport, scenario=scenario,
                               env={"OCTOSCODE_THEME": theme}, app_bin=app_bin, replay_args=["--adopt-turn-ids"])


if __name__ == "__main__":
    sys.exit(main())
