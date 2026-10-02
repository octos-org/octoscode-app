#!/usr/bin/env python3
"""A10 — the composer's two control seats by CLICK (web `SessionControlBar`:
the permission seat left, the model seat right; no Stage-A board draws the
menus — built with the native dialog kit, the web component as reference).

Every control is reached by a CLICK at its laid-out rect: the approval pill
opens the permission menu above it (permission/profile/list: the current
preset first); a standard preset applies directly (permission/profile/set
{mode, network}) and the menu closes; Full access opens the web's
confirmation, whose Enable stays inert until the acknowledgement box is
ticked; the model label opens the model menu above it (profile/llm/list
{session_id}: grouped by provider, the unavailable model disabled with its
reason); selecting the r2-route fallback answers with r2's recorded
restart_required ("Saved. The server keeps running deepseek-v4-flash until it
restarts"), Kimi with reloaded; the seat names the selected model; an outside
press dismisses a menu; the Session's notice is still there on reopen.

Against `replay_serve --scenario a10` (its seat simulator: the recorded r2
permission state + `a10-seats-faithful.jsonl`).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_seats.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a10_seats",
    "title": "the composer's permission and model seats: presets, the full-access confirmation, model menu",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        163: ["permission: approval pill CLICK -> the menu above it", "permission: 'Full access' CLICK -> the confirmation",
              "risk: 'Enable full access' is inert until the box is ticked", "risk: 'Enable full access' CLICK"],
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/seats/{MODE}"


def numeric(W: Walk, name: str, prefix=("b3_perm_", "b3_model_", "b3_risk_", "b3_title")):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", prefix)
    W.check(f"{name}: menu numeric checks", c["ok"], checks_line(c))
    return c


def above(W: Walk, seat: str, right: bool) -> bool:
    """The menu sits above its seat (bottom 8 px over it) and is aligned to
    the seat's left (permission) or right (model) edge."""
    d, s = W.rect("b3_dialog"), W.rect(seat)
    if not d or not s:
        W.note(f"no rect: dialog={d} seat={s}")
        return False
    gap = s[1] - (d[1] + d[3])
    edge = abs((d[0] + d[2]) - (s[0] + s[2])) if right else abs(d[0] - s[0])
    # A menu wider than the room beside its seat is held inside the frame's
    # 16 px margin instead (a narrow phone).
    frame = W.rect("b3_backdrop_hit")
    clamped = bool(frame) and (abs(d[0] - (frame[0] + 16)) <= 2 if right
                                else abs((d[0] + d[2]) - (frame[0] + frame[2] - 16)) <= 2)
    W.note(f"menu {d} over seat {s}: gap={gap:.1f} edge={edge:.1f} clamped={clamped}")
    return 4 <= gap <= 14 and (edge <= 3 or clamped)


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click(wid)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click(wid)
        logged = W.logged(needle, secs / 2) if ok else False
    got = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and got


def open_menu(W: Walk, hit: str, first_row: str) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click(hit) and W.wait(lambda: bool(W.visible(first_row)), 8)
    if not ok:
        W.note(f"RETRY {hit}")
        ok = W.click(hit) and W.wait(lambda: bool(W.visible(first_row)), 8)
    return ok


def walk(W: Walk) -> None:
    W.note("== 1. the seats: permission left, model right")
    W.check("seats: both seats are drawn (the server offers both menus)",
            W.wait(lambda: bool(W.visible("i0_composer_2")) and bool(W.visible("i0_composer_model")), 10))
    W.check("seats: the permission seat names the server's preset before its menu opens (read at Session open)",
            W.wait(lambda: W.text("i0_composer_2_0") == "Write · Network allowed", 10), W.text("i0_composer_2_0"))

    W.note("== 2. the permission menu (approval pill CLICK)")
    W.check("permission: approval pill CLICK -> the menu above it (permission/profile/list)",
            open_menu(W, "approval_pill_hit", "b3_perm_opt_0_name") and W.logged("PermissionLoad", 6))
    W.check("permission: the current preset first, then the other profiles (shield / name / check)",
            W.wait(lambda: W.text("b3_perm_opt_0_name") == "Write · Network allowed"
                   and W.text("b3_perm_opt_3_name") == "Full access · Network allowed", 6)
            and bool(W.visible("b3_perm_opt_0_check")) and bool(W.visible("b3_perm_opt_3_icon")))
    W.check("permission: the menu sits above the seat, left-aligned to it", above(W, "i0_composer_2", right=False))
    numeric(W, "permission menu")
    W.shot(f"01-permission-menu-{MODE}")
    W.check("permission: 'Read · Network blocked' CLICK -> permission/profile/set; the menu closes",
            click_logged(W, "b3_perm_opt_1", "PermissionSet", lambda: not W.visible("b3_dialog"), 10))
    W.check("permission: the seat shows the read-back (the web's trigger label)",
            W.wait(lambda: W.text("i0_composer_2_0") == "Read · Network blocked", 8), W.text("i0_composer_2_0"))
    W.shot(f"01b-permission-applied-{MODE}")

    W.note("== 3. full access only through the acknowledged confirmation")
    W.check("permission: reopened, the read-back is current (Read · Network blocked first, checked)",
            open_menu(W, "approval_pill_hit", "b3_perm_opt_0_name")
            and W.wait(lambda: W.text("b3_perm_opt_0_name") == "Read · Network blocked", 8)
            and bool(W.visible("b3_perm_opt_0_check")))
    full = next((w["i"][:-5] for w in W.prefixed("b3_perm_opt_") if w["i"].endswith("_name") and W.text(w["i"]) == "Full access · Network allowed"), "b3_perm_opt_3")
    W.check("permission: 'Full access' CLICK -> the confirmation, not a set",
            W.click(full) and W.wait(lambda: bool(W.visible("b3_risk_summary")), 6)
            and not W.logged("PermissionSet", 1.0))
    W.check("risk: the web's copy — title, summary (Full access / Network allowed), the box, the hint",
            W.text("b3_title") == "Enable full access?" and W.text("b3_risk_access_v") == "Full access"
            and W.text("b3_risk_network_v") == "Network allowed" and bool(W.visible("b3_risk_hint")))
    W.mark()
    r = W.rect("b3_risk_confirm_box")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.check("risk: 'Enable full access' is inert until the box is ticked", r is not None and not W.logged("PermissionSet", 1.5)
            and bool(W.visible("b3_risk_summary")))
    numeric(W, "risk confirmation", ("b3_risk_", "b3_title", "b3_close"))
    W.shot(f"02-risk-{MODE}")
    W.check("risk: the acknowledgement CLICK ticks the box; the hint goes; Enable is armed",
            W.click("b3_risk_ack") and W.wait(lambda: not W.visible("b3_risk_hint") and bool(W.visible("b3_risk_confirm")), 6))
    W.shot(f"03-risk-ack-{MODE}")
    W.check("risk: 'Enable full access' CLICK -> permission/profile/set {danger_full_access, allow}; closed",
            click_logged(W, "b3_risk_confirm", "PermissionSet", lambda: not W.visible("b3_dialog"), 10)
            and W.wait(lambda: W.text("i0_composer_2_0") == "Full access · Network allowed", 8))

    W.note("== 4. the model menu (model seat CLICK)")
    W.check("model: the model label CLICK -> the menu above it (profile/llm/list {session_id})",
            open_menu(W, "model_seat_hit", "b3_model_opt_0_name") and W.logged("ModelsLoad", 6))
    W.check("model: the two deepseek routes read apart (model · route)",
            W.text("b3_model_opt_0_desc") == "deepseek-v4-flash · deepseek"
            and W.text("b3_model_opt_1_desc") == "deepseek-v4-flash · r2-route")
    W.check("model: grouped by provider; the unavailable model disabled with its reason",
            W.text("b3_model_group_0") == "Deepseek" and W.text("b3_model_group_1") == "Moonshot"
            and W.text("b3_model_opt_3_desc").startswith("This configured model is unavailable")
            and not any(w["i"] == "b3_model_opt_3" for w in W.snap() if Walk.shown(w)))
    W.check("model: the selected model is checked; the seat names it",
            bool(W.visible("b3_model_opt_0_check")) and W.text("i0_composer_4") == "DeepSeek V4 Flash")
    W.check("model: the menu sits above the seat, right-aligned to it", above(W, "i0_composer_model", right=True))
    numeric(W, "model menu")
    W.shot(f"04-model-menu-{MODE}")
    W.check("model: the r2-route fallback CLICK -> profile/llm/select -> the restart_required notice",
            click_logged(W, "b3_model_opt_1", "ModelSelect",
                         lambda: W.text("b3_model_notice") == "Saved. The server keeps running deepseek-v4-flash until it restarts", 10))
    numeric(W, "model notice")
    W.shot(f"05-model-restart-{MODE}")
    W.check("model: Kimi K3 CLICK -> reloaded: 'Saved. Your next message uses kimi-k3'; checked",
            click_logged(W, "b3_model_opt_2", "ModelSelect",
                         lambda: W.text("b3_model_notice") == "Saved. Your next message uses kimi-k3"
                         and bool(W.visible("b3_model_opt_2_check")), 10))
    W.shot(f"06-model-reloaded-{MODE}")

    W.note("== 5. an outside press dismisses; the seat and the notice persist")
    # An outside press over the transcript, just above the menu (the
    # header row sits above the module's dialogs on the phone shell).
    d = W.rect("b3_dialog")
    if d:
        x, y = d[0] + d[2] / 2, d[1] - 40
        W.note(f"CLICK outside the menu at ({x:.0f},{y:.0f})")
        W.click_xy(x, y)
    closed = W.wait(lambda: not W.visible("b3_dialog"), 6)
    named = W.wait(lambda: W.text("i0_composer_4") == "Kimi K3", 6)
    W.check("model: an outside press closes the menu; the seat now names Kimi K3", closed and named,
            f"closed={closed} seat={W.text('i0_composer_4')!r} dialog={W.rect('b3_dialog')}")
    W.check("model: reopened, the Session's notice is still there",
            open_menu(W, "model_seat_hit", "b3_model_opt_0_name")
            and W.wait(lambda: W.text("b3_model_notice") == "Saved. Your next message uses kimi-k3", 6))
    W.shot(f"07-model-sticky-{MODE}")
    W.key("Escape")
    W.wait(lambda: not W.visible("b3_dialog"), 4)
    W.note("== 6. the wire")
    for method, n in [("permission/profile/set", 2), ("profile/llm/select", 2)]:
        got = W.replay_saw(method, 2)
        W.check(f"wire: {method} x{n}", got == n, f"replay log: {got}")
    for method in ["permission/profile/list", "profile/llm/list"]:
        got = W.replay_saw(method, 2)
        W.check(f"wire: {method} read", got >= 2, f"replay log: {got}")


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
