#!/usr/bin/env python3
"""A24 — the interface language by CLICKS (web `PreferencesDialog.tsx:42-55`
Language <select>, `ui-text.tsx` catalog, `model.ts` setLanguage/save).

Run 1 (live switch): Settings > Preferences shows Language (English |
简体中文) with English selected; a CLICK on 简体中文 re-renders every shell
surface at once — the Settings sheet (its nav, rows, the control itself),
the sidebar, the header, the composer + its seats, the status strip, the
slash menu, the Connect card and pairing after a Disconnect — each read
from /snap in Chinese and captured with numeric CJK checks (every Chinese
label inside its surface, not past the smallest box holding it, no overlaps,
no estimated truncation, controls >= 28 px). A CLICK on English restores the
English copy. Then 简体中文 again and Save: the display file holds
`language: "zh"` (only the whitelist).
Run 2 (relaunch): a fresh launch on the same preference file is Chinese from
its first frame (adopted at launch); English + Save restores the file.
Run 3 (phase 2, the surfaces): launched in Chinese (the saved preference), the
Fleet from the sidebar footer, the Session settings pane from the strip, the
Trajectory tab, and the /review, /thinking and /threads dialogs from the slash
menu — each captured with the same numeric CJK checks, and no English label
left on it but names and data (the allow-list below says which).

Against `replay_serve --scenario activity` (a recorded handshake with a
workspace), isolated state (a10_lib.run_session), the app hidden.
usage: OCTOSCODE_APP_BIN=<host octosense> a24_language.py <desktop|phone> <outdir>
"""
import json
import os
import pathlib
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, inside, overlap, run_session  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a24_language",
    "title": "Settings > Preferences > Language: 简体中文 re-renders the shell live, English restores, Save persists, a relaunch adopts it",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 1500,
    "rows": {
        76: {"checks": ["switch: 简体中文 CLICK", "switch: the Settings sheet reads Chinese",
                        "shell: the header reads Chinese", "restore: English CLICK"],
             "partial": "the language switch is walked live; themes, two busy Sessions and the queued image of "
                        "the web case are other agents' rows"},
        81: {"checks": ["save: the file holds language zh", "relaunch: the first frame is Chinese"],
             "partial": "the saved language and its relaunch are walked; the theme/Vim halves are A9's walk"},
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a24/zh-{MODE}"
CELL = "rl_hit" if MODE == "phone" else "nv_hit"
SECTIONS = ["general", "permissions", "model", "sandbox", "connection", "preferences", "about"]
PREFS = pathlib.Path(OUT).resolve() / "display-v1.json"


def is_cjk(text: str) -> bool:
    return any("一" <= c <= "鿿" or "　" <= c <= "〿" or "＀" <= c <= "￯" for c in text)


def em(c: str) -> float:
    """A glyph's advance in em (CJK full width; Inter latin ~0.56)."""
    if is_cjk(c):
        return 1.0
    if c == " ":
        return 0.28
    if c.isupper() or c.isdigit():
        return 0.64
    return 0.53


def cjk_checks(W: Walk, sn, frames: list[str]) -> dict:
    """Every shown Chinese label: inside the module view and inside the
    (first shown) surface frame that holds its origin, its right edge not
    past the smallest box holding its origin (overflow), no two Chinese
    labels overlapping, and no estimated truncation of a single line (the
    glyph estimate at the label's own line height wider than its box)."""
    # /snap lists the tree in order: a surface's own widgets follow its frame
    # (a modal's sheet comes after the conversation it covers), so a label
    # belongs to the surface when it comes after the frame and starts inside
    # it — what is drawn UNDER a modal is not judged with it.
    indexed = [(i, w) for i, w in enumerate(sn) if W.shown(w)]
    shown = [w for _, w in indexed]
    module = next((w["r"] for w in shown if w.get("ty") == "OctoscodeView"), None)
    frame_at = [(i, w["r"]) for f in frames for i, w in indexed if w.get("i") == f]
    frame_rects = [r for _, r in frame_at]

    def belongs(i, r):
        return any(i > fi and inside([r[0] + 0.5, r[1] + 0.5, 1, 1], fr) for fi, fr in frame_at)

    labels = [w for i, w in indexed if w.get("ty") == "Label" and is_cjk(w.get("t") or "") and belongs(i, w["r"])]
    boxes = [w for i, w in indexed if w.get("ty") not in ("Label", "Window", "Splash") and w["r"][2] > 4
             and w["r"][3] > 4 and belongs(i, w["r"])]
    out_module, out_frame, overflow, trunc, over = [], [], [], [], []
    for w in labels:
        r = w["r"]
        if module and not inside(r, module):
            out_module.append(w["t"])
        holder = [f for f in frame_rects if inside([r[0], r[1], 1, 1], f)]
        if holder and not inside(r, holder[0]):
            out_frame.append(w["t"])
        # The smallest box (not the label itself) holding the label's origin.
        cands = [b["r"] for b in boxes if inside([r[0] + 0.5, r[1] + 0.5, 1, 1], b["r"]) and b["r"] != r]
        if cands:
            box = min(cands, key=lambda b: b[2] * b[3])
            if r[0] + r[2] > box[0] + box[2] + 1.5:
                overflow.append((w["t"], r, box))
        # A wrapped label is several lines (~16-24 px each): estimate the run
        # at a line's SMALLEST plausible font against the box's capacity
        # (width x lines), so only a clear shortfall (an ellipsized or cut
        # run) is flagged; a Fit label measures its own run.
        lines = max(1, round(r[3] / 19.0))
        font = min((r[3] / lines) / 1.6, 15.0)
        est = sum(em(c) for c in w["t"]) * font
        if est > r[2] * lines * 1.12 + 6:
            trunc.append((w["t"], r, round(est)))
    for i, a in enumerate(labels):
        for b in labels[i + 1:]:
            if overlap(a["r"], b["r"]):
                over.append((a["t"], b["t"]))
    hits = [w for i, w in indexed if w.get("ty") in ("Button", "DesignNativeButton") and belongs(i, w["r"])]
    # A target the scroll body cuts at its top or bottom edge is scrolled,
    # not small: only whole targets are measured.
    views = [w["r"] for w in shown if w.get("i") in ("b3_scroll", "dialog_scroll", "set_body", "cv_tr_scroll")]

    def clipped(r):
        return any(inside([r[0] + 0.5, r[1] + 0.5, 1, 1], v) and (r[1] <= v[1] + 2 or r[1] + r[3] >= v[1] + v[3] - 2)
                   for v in views)

    under = [(w.get("i"), w["r"]) for w in hits if (w["r"][2] < 27.5 or w["r"][3] < 27.5) and not clipped(w["r"])]
    ok = not (out_module or out_frame or overflow or trunc or over or under)
    return {"ok": ok, "cjk": len(labels), "outside_module": out_module, "outside_frame": out_frame,
            "overflow": overflow, "truncated": trunc, "overlaps": over, "controls": len(hits), "under28": under}


def line(c: dict) -> str:
    return (f"cjk={c['cjk']} outside={len(c['outside_module']) + len(c['outside_frame'])} "
            f"overflow={len(c['overflow'])} truncated={len(c['truncated'])} overlaps={len(c['overlaps'])} "
            f"controls={c['controls']} under28={len(c['under28'])}"
            + (f" | {c['outside_module'][:2]}{c['outside_frame'][:2]}{c['overflow'][:2]}{c['truncated'][:2]}"
               f"{c['overlaps'][:2]}{c['under28'][:2]}" if not c["ok"] else ""))


SUMMARY: list[dict] = []


def capture(W: Walk, name: str, frames: list[str], surface: str):
    """One zh capture + its numeric checks, recorded for docs/ux-scores.csv."""
    time.sleep(0.6)
    sn = W.snap()
    c = cjk_checks(W, sn, frames)
    # A surface whose copy has no web key keeps English (cjk=0): nothing to
    # clip, and the count says so in the line.
    W.check(f"{surface}: no clipped or overflowing Chinese label ({name})", c["ok"], line(c))
    png = W.shot(name)
    # The shown labels' texts and rects (never an input's buffer: no secret
    # can ride this list).
    (W.out / f"{name}.snap.txt").write_text("\n".join(
        f"{w.get('i')}\t{w.get('ty')}\t{w['r']}\t{w.get('t')}" for w in sn
        if W.shown(w) and w.get("t") and w.get("ty") in ("Label", "Button")) + "\n")
    SUMMARY.append({"name": name, "surface": surface, "png": str(png), "checks": line(c), "ok": c["ok"]})
    return c


def texts(W: Walk, sn=None) -> list[str]:
    sn = sn if sn is not None else W.snap()
    return [w.get("t") or "" for w in sn if W.shown(w)]


def has(W: Walk, *needles: str, secs: float = 6.0) -> bool:
    return W.wait(lambda: all(any(n == t or n in t for t in texts(W)) for n in needles), secs)


def open_settings(W: Walk) -> bool:
    if not W.click("settings_open_hit"):
        return False
    return W.wait(lambda: bool(W.visible("settings_drawer")), 8)


def section(W: Walk, name: str) -> bool:
    W.click(CELL, SECTIONS.index(name))
    return W.wait(lambda: bool(W.visible(f"sec_{name}")), 6)


def close_settings(W: Walk):
    W.click("set_back" if MODE == "phone" else "settings_close")
    W.wait(lambda: not W.visible("settings_drawer"), 6)


def segment(W: Walk, seg: str):
    """The language segment's laid-out rect and its shown label."""
    sn = W.snap()
    r = W.rect(seg, sn=sn)
    on = [w for w in W.visible("sg_text_on", sn) if r and inside(w["r"], r)]
    return r, (on[0].get("t") if on else None)


def click_segment(W: Walk, seg: str) -> bool:
    r = W.rect(seg)
    if not r:
        W.note(f"CLICK {seg} — not visible")
        return False
    W.note(f"CLICK {seg} r={r}")
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    return True


def frames_for_settings():
    return ["settings_drawer", "set_body"]


def run_live(W: Walk):
    W.check("connected", W.wait(lambda: W.composer() is not None, 40))
    W.check("start: English (the device default here)", has(W, "Chat", "Ready"),
            f"{[t for t in texts(W) if t][:12]}")
    W.check("Settings opens (CLICK)", open_settings(W))
    W.check("Preferences shows (CLICK)", section(W, "preferences"))
    r_en, on_en = segment(W, "lang_en")
    r_zh, _ = segment(W, "lang_zh")
    W.check("Language: the control shows English | 简体中文, English selected",
            r_en is not None and r_zh is not None and on_en == "English", f"en={r_en} zh={r_zh} on={on_en!r}")
    W.check("Language: both segments are >= 28 px targets",
            bool(r_en and r_zh and r_en[3] >= 27.5 and r_zh[3] >= 27.5 and r_en[2] >= 28 and r_zh[2] >= 28),
            f"en={r_en} zh={r_zh}")
    W.shot("en-settings-preferences")
    W.mark()
    click_segment(W, "lang_zh")
    W.check("switch: 简体中文 CLICK", W.logged("a24 language -> zh (switched)"))
    # The phone sheet's nav is an icon rail: its section names are not text.
    want = ("语言", "Vim 编辑") if MODE == "phone" else ("语言", "Vim 编辑", "通用", "权限")
    W.check("switch: the Settings sheet reads Chinese", has(W, *want), f"{[t for t in texts(W) if is_cjk(t)][:10]}")
    _, on = segment(W, "lang_zh")
    W.check("switch: 简体中文 reads selected", on == "简体中文", f"{on!r}")
    W.check("switch: the status line says the change is unsaved", bool(W.text("prefs_status")), W.text("prefs_status"))
    capture(W, "zh-settings-preferences", frames_for_settings(), "Settings > Preferences")
    for name in ("general", "permissions", "model", "sandbox", "connection", "about"):
        section(W, name)
        capture(W, f"zh-settings-{name}", frames_for_settings(), f"Settings > {name.title()}")
    close_settings(W)
    # The shell: header, composer + seats, status strip (+ the sidebar on desktop).
    W.check("shell: the header reads Chinese", has(W, "聊天", "轨迹") if MODE != "phone" else has(W, "聊天"),
            f"{[t for t in texts(W) if is_cjk(t)][:12]}")
    W.check("shell: the status strip reads Chinese", has(W, "模型、权限、沙箱") or has(W, "就绪"),
            f"{W.text('b3_strip_caption')!r} {W.text('b3_strip_state')!r}")
    if MODE != "phone":
        W.check("shell: the sidebar reads Chinese", has(W, "添加工作区", "舰队"), f"{[t for t in texts(W) if is_cjk(t)][:12]}")
        capture(W, "zh-shell", ["threads_column", "conversation_column"], "shell (sidebar, header, composer, strip)")
    else:
        capture(W, "zh-shell", ["conversation_column"], "shell (header, composer, strip)")
        if W.click("sidebar_toggle_hit"):
            W.wait(lambda: bool(W.visible("drawer_scrim")), 6)
            W.check("shell: the drawer sidebar reads Chinese", has(W, "添加工作区", "舰队"),
                    f"{[t for t in texts(W) if is_cjk(t)][:12]}")
            capture(W, "zh-drawer", ["threads_column"], "sidebar drawer")
            W.click("drawer_close") or W.click("drawer_scrim_hit")
            W.wait(lambda: not W.visible("drawer_scrim"), 6)
    # The composer's permission seat menu (A10's seats).
    if W.click("approval_pill_hit"):
        if W.wait(lambda: bool(W.visible("b3_dialog")), 5):
            W.check("seats: the permission menu reads Chinese", has(W, "权限"), f"{W.text('b3_title')!r}")
            capture(W, "zh-permission-menu", ["b3_dialog"], "composer permission menu")
            # The web's outside press dismisses the popover.
            W.click("hd_title")
            W.check("seats: an outside press closes the menu", W.wait(lambda: not W.visible("b3_dialog"), 6))
    # The slash menu: its descriptions in Chinese (the web's command keys).
    c = W.composer()
    if c:
        x, y, w_, h = c["r"]
        W.click_xy(x + w_ / 2, y + h / 2)
        W.clear_field()
        W.type_text("/")
        if W.wait(lambda: bool(W.visible("palette_row_desc")), 6):
            W.check("slash menu: the descriptions read Chinese",
                    W.wait(lambda: any(is_cjk(w.get("t") or "") for w in W.visible("palette_row_desc")), 4),
                    f"{[w.get('t') for w in W.visible('palette_row_desc')][:4]}")
            capture(W, "zh-slash-menu", ["palette_dock", "palette_list", "palette"], "slash menu")
        W.key("Escape")
        W.clear_field(4)
        W.dismiss_keyboard("hd_title")
    # Board 1: Disconnect -> the Connect card in Chinese, then pairing.
    time.sleep(1.0)  # the phone keyboard's hide animation
    open_settings(W)
    if not section(W, "connection"):
        time.sleep(0.8)
        section(W, "connection")
    W.mark()
    W.check("board 1: Disconnect CLICK", W.click_in("settings_disconnect", "set_body"))
    W.check("board 1: the Connect card returns", W.wait(lambda: bool(W.visible("connect_card")), 12))
    if W.visible("connect_card"):
        W.check("board 1: the Connect card reads Chinese", has(W, "连接 Octos"), f"{W.text('connect_title')!r}")
        capture(W, "zh-connect", ["connect_card"], "Connect card (board 1)")
        if W.click("b1_connect_pair"):
            if W.wait(lambda: bool(W.prefixed("b1_pair")), 6):
                capture(W, "zh-pairing", ["b1_card"], "Pair with Octos (board 1 p4-01)")
                W.click("b1_pair_back")
                W.wait(lambda: bool(W.visible("connect_card")), 6)
        W.click("connect_btn")
        W.check("board 1: Connect reconnects", W.wait(lambda: W.composer() is not None, 20))
    # Restore English.
    open_settings(W)
    section(W, "preferences")
    W.mark()
    click_segment(W, "lang_en")
    W.check("restore: English CLICK", W.logged("a24 language -> en (switched)"))
    want = ("Language", "Vim editing") if MODE == "phone" else ("Language", "Vim editing", "General", "Permissions")
    W.check("restore: the sheet reads English again", has(W, *want), f"{[t for t in texts(W) if is_cjk(t)][:6]}")
    W.check("restore: no Chinese left on the sheet", not [t for t in texts(W) if is_cjk(t) and t != "简体中文"],
            f"{[t for t in texts(W) if is_cjk(t)]}")
    W.shot("en-restored")
    # Save the Chinese choice (row 81): only the whitelist is written.
    click_segment(W, "lang_zh")
    W.logged("a24 language -> zh")
    W.click("prefs_save")
    time.sleep(0.8)
    doc = json.loads(PREFS.read_text()) if PREFS.exists() else {}
    W.check("save: the file holds language zh", doc.get("language") == "zh" and set(doc) == {"version", "theme", "language", "vimMode"},
            f"{doc}")
    close_settings(W)


def run_relaunch(W: Walk):
    W.check("relaunch: connected", W.wait(lambda: W.composer() is not None, 40))
    W.check("relaunch: the first frame is Chinese", has(W, "聊天") or has(W, "设置"),
            f"{[t for t in texts(W) if is_cjk(t)][:8]}")
    W.check("relaunch: adopted at launch (log)", W.logged("a24 language at launch: zh", 4))
    open_settings(W)
    section(W, "preferences")
    _, on = segment(W, "lang_zh")
    W.check("relaunch: 简体中文 reads selected", on == "简体中文", f"{on!r}")
    click_segment(W, "lang_en")
    W.logged("a24 language -> en")
    W.click("prefs_save")
    time.sleep(0.8)
    doc = json.loads(PREFS.read_text()) if PREFS.exists() else {}
    W.check("relaunch: English saved back", doc.get("language") == "en", f"{doc}")
    close_settings(W)


# Labels a Chinese surface may still show in Latin script: names and data
# (the product, a model or a route, ids, numbers, keys), never sentences.
LATIN_OK = ("Octos", "OctosCode", "Octoscode", "MCP", "API", "Vim", "Esc", "ID", "URL", "Markdown", "LLM")
# Widgets that show data, never copy: the server's model and task names, a
# task's wire status, thread / scope rows, a peer's label (id fragments).
DATA_IDS = ("b3_sc_model_", "b3_sc_saved_value", "b3_sc_runtime_value", "b3_sc_turn_model_value",
            "cv_tr_task_", "cv_tr_plan_title_", "cv_tr_st_value_", "b3_insp_thread_", "b3_insp_scope_",
            "b3_fleet_row_", "b3_think_block_", "b3_sc_op_")


def english_left(W: Walk, frames: list[str]) -> list[str]:
    """Labels on the surface that read as English sentences or words (two or
    more Latin words, or one capitalised word that is not a name/data)."""
    sn = W.snap()
    indexed = [(i, w) for i, w in enumerate(sn) if W.shown(w)]
    frame_at = [(i, w["r"]) for f in frames for i, w in indexed if w.get("i") == f]
    out = []
    for i, w in indexed:
        t = (w.get("t") or "").strip()
        if w.get("ty") not in ("Label", "Button") or not t or is_cjk(t):
            continue
        if any(p in str(w.get("i") or "") for p in DATA_IDS):
            continue
        r = w["r"]
        if not any(i > fi and inside([r[0] + 0.5, r[1] + 0.5, 1, 1], fr) for fi, fr in frame_at):
            continue
        words = [x for x in t.replace("·", " ").replace("/", " ").split() if any(c.isalpha() for c in x)]
        if not words or all(x.strip(".,:…()") in LATIN_OK for x in words):
            continue
        # ids, model names, paths, numbers: lower-case or digit-bearing tokens with - _ . : /
        if all(any(c in x for c in "-_.:/@#0123456789") or x.islower() and len(words) == 1 for x in words):
            continue
        if len(words) >= 2 or words[0][:1].isupper():
            out.append(t)
    return out


# English the walk may still meet, each in a file another builder owns and
# converts after its merge (the coordinator's order): named, not hidden.
DEFERRED = {
    "Back": "board3/fleetview.rs:1109 (A30's file)",
    "Describe the task for the peer": "board3/fleetview.rs:906 (A30's file)",
}


def surface(W: Walk, name: str, frames: list[str], title: str):
    c = capture(W, name, frames, title)
    left = english_left(W, frames)
    owed = [t for t in left if t in DEFERRED]
    left = [t for t in left if t not in DEFERRED]
    W.check(f"{title}: no English copy left ({name})", not left,
            f"{left[:8]}" + (f" deferred: {[(t, DEFERRED[t]) for t in owed]}" if owed else ""))
    return c


def close_b3(W: Walk):
    if W.visible("b3_close"):
        W.click("b3_close")
    else:
        W.key("Escape")
    W.wait(lambda: not W.visible("b3_dialog"), 6)


def run_surfaces(W: Walk):
    W.check("surfaces: connected", W.wait(lambda: W.composer() is not None, 40))
    W.check("surfaces: launched in Chinese (log)", W.logged("a24 language at launch: zh", 4))
    # The slash menu's dialogs first (the composer is focused fresh): /review
    # (board 2), /thinking and /threads (board 3).
    W.check("review: /review CLICK opens the dialog", W.palette_run("revi", "/review") and W.wait_shown("dialog_frame", 10))
    if W.visible("dialog_frame"):
        surface(W, "zh-review", ["dialog_frame"], "Code review dialog")
        W.click("dialog_close")
        W.wait(lambda: not W.visible("dialog_frame"), 6)
    W.check("thinking: /thinking CLICK opens the dialog", W.palette_run("thin", "/thinking") and W.wait_shown("b3_dialog", 10))
    if W.visible("b3_dialog"):
        W.check("thinking: the dialog reads Chinese", has(W, "思考强度"), f"{W.text('b3_title')!r}")
        surface(W, "zh-thinking", ["b3_dialog"], "Thinking effort")
        close_b3(W)
    W.check("inspector: /threads CLICK opens the inspector", W.palette_run("thre", "/threads") and W.wait_shown("b3_dialog", 10))
    if W.visible("b3_dialog"):
        W.check("inspector: the inspector reads Chinese", has(W, "线程图"), f"{W.text('b3_title')!r}")
        surface(W, "zh-inspector", ["b3_dialog"], "Inspector (/threads)")
        close_b3(W)
    # The Fleet, from the sidebar footer (the drawer's on a phone).
    if MODE == "phone" and W.click("sidebar_toggle_hit"):
        W.wait(lambda: bool(W.visible("drawer_scrim")), 6)
    W.mark()
    W.check("fleet: the footer's Fleet CLICK", W.click("fleet_nav_hit") and W.wait_shown("b3_fleet_panel", 10))
    if W.visible("b3_fleet_panel"):
        W.check("fleet: the Fleet reads Chinese", has(W, "舰队"), f"{W.text('b3_title')!r}")
        surface(W, "zh-fleet", ["b3_fleet_panel"], "Fleet")
    # Back to the chat (the pane's Back, else Escape), and wait for the strip
    # the Fleet hid with the composer.
    if not (W.visible("b3_fleet_back") and W.click("b3_fleet_back")):
        W.key("Escape")
    W.wait(lambda: not W.visible("b3_fleet_panel"), 6)
    if W.visible("b3_close"):
        W.click("b3_close")
    W.wait_shown("b3_strip_tap", 10)
    time.sleep(0.6)
    # The Session settings pane, from the strip (the web's strip click).
    W.check("pane: the strip CLICK opens Session settings", W.click("b3_strip_tap") and W.wait_shown("b3_dialog", 10))
    if W.visible("b3_dialog"):
        W.check("pane: the pane reads Chinese", has(W, "会话设置", "模型"), f"{W.text('b3_title')!r}")
        surface(W, "zh-session-settings", ["b3_dialog"], "Session settings")
        close_b3(W)
    # The Trajectory tab (the header's second tab; desktop and phone).
    if W.click("hd_tab_traj_hit"):
        W.check("trajectory: the tab CLICK shows the pane", W.wait_shown("cv_tr_root", 10))
        W.check("trajectory: the pane reads Chinese", has(W, "轨迹"), f"{W.text('cv_tr_title')!r}")
        surface(W, "zh-trajectory", ["cv_tr_root"], "Trajectory")
        W.click("hd_tab_chat_hit")
        time.sleep(0.6)


if __name__ == "__main__":
    out = pathlib.Path(OUT)
    out.mkdir(parents=True, exist_ok=True)
    if PREFS.exists():
        PREFS.unlink()
    env = {"OCTOSCODE_DISPLAY_PREFS_PATH": str(PREFS), "OCTOS_WORKSPACE_CWD": "/home/user/src/octos"}
    # A24_SURFACES_ONLY=1 re-runs run 3 alone while developing it (the
    # evidence is always the whole walk).
    only3 = os.environ.get("A24_SURFACES_ONLY") == "1"
    rc1 = 0 if only3 else run_session(run_live, mode=MODE, outdir=OUT, scenario="activity", env=env)
    rc2 = 0 if only3 else run_session(run_relaunch, mode=MODE, outdir=str(out / "relaunch"), scenario="activity", env=env)
    # Run 3: the phase-2 surfaces, on the a10 fixture (/review answers there).
    PREFS.write_text(json.dumps({"version": 1, "theme": "slate", "language": "zh", "vimMode": False}) + "\n")
    # (no workspace cwd: the a10 fixture lists /review, /thinking and /threads for its own Session)
    rc3 = run_session(run_surfaces, mode=MODE, outdir=str(out / "surfaces"), scenario="a10",
                      env={"OCTOSCODE_DISPLAY_PREFS_PATH": str(PREFS)})
    (out / "captures.json").write_text(json.dumps(SUMMARY, indent=1, ensure_ascii=False) + "\n")
    if PREFS.exists():
        PREFS.unlink()
    ok = rc1 == 0 and rc2 == 0 and rc3 == 0
    print(f"== WALK a24 language {MODE}: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)
