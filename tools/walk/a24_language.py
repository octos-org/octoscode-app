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
Fleet from the sidebar footer (closed by Escape — on a phone the shell's Back,
the app staying in front), the Session settings pane from the strip, the
Trajectory tab, the /review, /thinking and /threads dialogs from the slash
menu, and /model's Models dialog -> Manage providers -> Model providers -> Add
provider (the board-1 editor) — each captured with the same numeric CJK
checks, and no English label left on it but names and data (the allow-list
below says which).
Run 4 (phase 2, the merged surfaces), each launched in Chinese on its own
fixture: the Skills dialog's Background jobs (`skill-jobs`: every job's time
reads 刚刚 / N 分钟前 / N 小时前, never now / 2m / 1h), the diff review from an
approval card's 审查差异 (`surfaces --first-turn 3 --diff-words`), the /btw
aside answering and answered (`btw`), and the sidebar peer dock after one
Start from the Fleet (`fleet --peer-dock`: the dock's word for the peer is
the Fleet chip's word).

Against the replay server (recorded / faithful traffic, no model), isolated
state (a10_lib.run_session), the app hidden.
usage: OCTOSCODE_APP_BIN=<host octosense> a24_language.py <desktop|phone> <outdir>
"""
import json
import os
import pathlib
import re
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
        # A single line (<= 24 px tall) is measured at its own font: a tight
        # 12 px line box (<= 16 px) is 1.25 em, a taller one up to 1.7 em
        # (the form labels' 22 px rows). A run cut by a fixed box narrower
        # than its glyphs (a 36 px time column holding "20 分钟前") is caught.
        if r[3] <= 24 and "\n" not in w["t"]:
            est = max(est, sum(em(c) for c in w["t"]) * (r[3] / (1.25 if r[3] <= 16 else 1.7)))
            if est > r[2] * 1.1 + 4:
                trunc.append((w["t"], r, round(est)))
        elif est > r[2] * lines * 1.12 + 6:
            trunc.append((w["t"], r, round(est)))
    for i, a in enumerate(labels):
        for b in labels[i + 1:]:
            if overlap(a["r"], b["r"]):
                over.append((a["t"], b["t"]))
    hits = [w for i, w in indexed if w.get("ty") in ("Button", "DesignNativeButton") and belongs(i, w["r"])]
    # A target the scroll body cuts at its top or bottom edge is scrolled,
    # not small: only whole targets are measured.
    views = [w["r"] for w in shown if w.get("i") in ("b3_scroll", "dialog_scroll", "set_body", "cv_tr_scroll",
                                                       "b1_prov_scroll", "btw_aside_body", "pd_rows")]

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
    SUMMARY.append({"name": name, "surface": surface, "png": str(png), "checks": line(c), "ok": c["ok"], "frames": frames})
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
LATIN_OK = ("Octos", "OctosCode", "Octoscode", "MCP", "API", "Vim", "Esc", "ID", "URL", "Markdown", "LLM",
            "⌥Y", "⌥N", "⌥P")
# Widgets that show data, never copy: the server's model and task names, a
# task's wire status, thread / scope rows, a peer's label (id fragments; the
# web draws FleetRosterPeer.label raw, fleet-model.ts:416).
DATA_IDS = ("b3_sc_model_", "b3_sc_saved_value", "b3_sc_runtime_value", "b3_sc_turn_model_value",
            "cv_tr_task_", "cv_tr_plan_title_", "cv_tr_st_value_", "b3_insp_thread_", "b3_insp_scope_",
            "b3_fleet_row_", "b3_think_block_", "b3_sc_op_")
# The same, by pattern (an id prefix alone would also match copy): the diff's
# own lines, files and preview id; the aside's question, answer (the
# server's Markdown) and scope (workspace · Session title); a skill job's
# skill, action, file name, output line and the server's error cause; the
# dock row's tool target; the Fleet's lane value (a lane key) and the chosen
# lane's provider/model, initial and description (profile/sub_providers/list);
# an approval card's server title, reason and command, its `{tool} · {kind}`
# line and its key hint (ApprovalPanel.tsx draws them as sent); a configured
# provider's name, `family · route label` and `model · api type` lines
# (ModelManagementSection.tsx:1336 draws {familyLabel} · {route.label} raw),
# the Models dialog's `provider • route` group heads, and the editor's
# selected family / model / route (names).
DATA_RE = re.compile(r"^(b3_diff_(file_|preview_id)|btw_aside_(question|answer|scope)|"
                     r"dlg_skills_job_\d+_(skill|action|name|cause)$|pd_row_\d+_target$|"
                     r"b3_fleet_model_value$|b3_fleet_opt_|b3_fleet_lane_(initial|title|desc)$|"
                     r"cv_ap_(title|body|cmd|tool|hint)$|b3_routes_row_\d+_(name|meta|endpoint|base_url)$|"
                     r"dlg_models_t_(ds|kimi|glm|p\d+)_head$|b1_prov_(family|model|route)_value$)")
# A skill job's output line is the server's (`skill_jobs.rs` Message::Output:
# the first line of the job's `output`, drawn as sent): the fixture's.
SERVER_TEXT = ("Imported 14 pages",)
# A job's relative time in the web's zh (`formatRelativeTime` — the sidebar's
# wording): 刚刚 / N 分钟前 / N 小时前 / N 天前, or a date.
TIME_ZH = re.compile(r"刚刚|\d+ 分钟前|\d+ 小时前|\d+ 天前|\d+月\d+日")


def english_left(W: Walk, frames: list[str], data: tuple = (), sn=None) -> list[str]:
    """Labels on the surface that read as English sentences or words (two or
    more Latin words, or one capitalised word that is not a name/data). `sn`:
    a saved snap (the ux-scores rows re-read a capture's own)."""
    sn = sn if sn is not None else W.snap()
    indexed = [(i, w) for i, w in enumerate(sn) if W.shown(w)]
    frame_at = [(i, w["r"]) for f in frames for i, w in indexed if w.get("i") == f]
    out = []
    for i, w in indexed:
        t = (w.get("t") or "").strip()
        if w.get("ty") not in ("Label", "Button") or not t or is_cjk(t):
            continue
        if any(p in str(w.get("i") or "") for p in DATA_IDS) or DATA_RE.match(str(w.get("i") or "")) \
                or str(w.get("i") or "") in data:
            continue
        if t in SERVER_TEXT:
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
# (Phase 2's two — the Fleet's Back and its brief placeholder — are converted.)
DEFERRED: dict = {}


def surface(W: Walk, name: str, frames: list[str], title: str, data: tuple = ()):
    """A capture + no English copy left on it; `data` names this surface's
    own widgets that show server data (e.g. the diff review's title is the
    preview's)."""
    c = capture(W, name, frames, title)
    left = english_left(W, frames, data)
    SUMMARY[-1].update({"english": left, "data": list(data)})
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


def drawer_open(W: Walk) -> None:
    """The phone's sidebar is a drawer (its footer holds the Fleet entry and
    the peer dock); the desktop's column is always shown."""
    if MODE == "phone" and not W.visible("fleet_nav_hit") and W.click("sidebar_toggle_hit"):
        W.wait(lambda: bool(W.visible("drawer_scrim")), 6)


def open_fleet(W: Walk) -> bool:
    drawer_open(W)
    W.mark()
    return W.check("fleet: the footer's Fleet CLICK", W.click("fleet_nav_hit") and W.wait_shown("b3_fleet_panel", 10))


def escape_fleet(W: Walk) -> None:
    """Escape steps out of the Fleet pane: the desktop's key; on a phone the
    shell turns Escape into its Back, which the app takes while a surface is
    open (it used to leave the app with the pane still open)."""
    W.mark()
    W.key("Escape")
    W.check("fleet: Escape closes the pane" + (" (the phone's Back)" if MODE == "phone" else ""),
            W.wait(lambda: not W.visible("b3_fleet_panel"), 6))
    W.check("fleet: ...and the app stays in front (the composer is back)", W.wait(lambda: W.composer() is not None, 8))
    if W.visible("b3_fleet_panel") and W.visible("b3_fleet_back"):
        W.click("b3_fleet_back")
        W.wait(lambda: not W.visible("b3_fleet_panel"), 6)


def send(W: Walk, text: str) -> bool:
    """CLICK the composer, type, Return (the production send)."""
    c = W.composer()
    if c is None:
        return False
    x, y, w_, h = c["r"]
    W.click_xy(x + w_ / 2, y + h / 2)
    W.key("End")
    W.clear_field(40)
    W.type_text(text)
    time.sleep(0.3)
    W.key("Return")
    return True


def text_any(W: Walk, wid: str, sn=None) -> str:
    """A label's text from the tree, drawn or not (a row scrolled out of its
    viewport still carries its words)."""
    sn = sn if sn is not None else W.snap()
    hit = next((w for w in sn if w.get("i") == wid and w.get("t") is not None), None)
    return (hit.get("t") or "") if hit else ""


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
    open_fleet(W)
    if W.visible("b3_fleet_panel"):
        W.check("fleet: the Fleet reads Chinese", has(W, "舰队", "返回"), f"{W.text('b3_title')!r} {W.text('b3_fleet_back_label')!r}")
        surface(W, "zh-fleet", ["b3_fleet_panel"], "Fleet")
    escape_fleet(W)
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
    # The Models dialog -> Manage providers -> Model providers -> Add provider
    # (A23's board-3 dialog and board-1 editor), each by CLICK.
    W.check("models: /model CLICK opens the Models dialog", W.palette_run("mo", "/model") and W.wait_shown("dlg_models_t_title", 10))
    if not W.visible("dialog_frame"):
        return
    surface(W, "zh-models", ["dialog_frame"], "Models dialog")
    W.scroll_into("dlg_models_manage_providers_control", "dialog_scroll")
    W.check("providers: the 管理提供商 CLICK opens 模型提供商",
            W.click("dlg_models_manage_providers_control") and W.wait(lambda: W.text("b3_title") == "模型提供商", 10),
            repr(W.text("b3_title")))
    if not W.visible("b3_dialog"):
        return
    W.wait(lambda: bool(W.prefixed("b3_routes_row_")), 8)
    surface(W, "zh-providers", ["b3_dialog"], "Model providers")
    W.scroll_into("b3_routes_add_provider", "b3_scroll")
    W.check("provider editor: the 添加提供商 CLICK opens the board-1 editor",
            W.click("b3_routes_add_provider") and W.wait(lambda: W.text("b1_title") == "添加提供商", 10), repr(W.text("b1_title")))
    if W.visible("b1_card"):
        surface(W, "zh-provider-editor", ["b1_card"], "Provider editor (board 1)")
        # Escape steps back to the providers dialog (on a phone: its Back).
        W.key("Escape")
        W.check("provider editor: Escape steps back to 模型提供商",
                W.wait(lambda: not W.visible("b1_card") and W.text("b3_title") == "模型提供商", 10), repr(W.text("b3_title")))
    close_b3(W)


# ---------------------------------------------------------------- run 4

def run_skills(W: Walk):
    """`skill-jobs`: the Skills dialog's Background jobs in Chinese — the
    judge's item: a job's time read "now" / "2m" / "1h" in zh."""
    W.check("skills: connected", W.wait(lambda: W.composer() is not None, 40))
    ok = W.palette_run("ski", "/skills") and W.wait_shown("dlg_skills_jobs_head", 10)
    W.check("skills: the /skills CLICK opens Skills with its 后台作业", ok and W.text("dlg_skills_jobs_head") == "后台作业",
            repr(W.text("dlg_skills_jobs_head")))
    if not ok:
        return
    W.check("skills: the job rows arrive", W.wait(lambda: bool(text_any(W, "dlg_skills_job_5_name")), 10))
    time.sleep(0.6)
    sn = W.snap()
    times = [text_any(W, f"dlg_skills_job_{i}_time", sn) for i in range(6)]
    W.check("skills: every job's time reads Chinese (刚刚 / N 分钟前 / N 小时前), never now / 2m / 1h",
            all(TIME_ZH.fullmatch(t or "") for t in times), f"{times}")
    # Whole, not cut by the column (the 36 px column sized for "20m" cut
    # "20 分钟前" to "20 分钅"): each shown time's box holds its glyphs at
    # the 12 px meta font.
    cut = [(w.get("t"), w["r"]) for w in sn if W.shown(w) and str(w.get("i", "")).startswith("dlg_skills_job_")
           and str(w.get("i", "")).endswith("_time") and w["r"][2] + 2 < sum(em(c) for c in (w.get("t") or "")) * 12 * 0.95]
    W.check("skills: every shown time is whole (its box holds its words)", not cut, f"{cut}")
    surface(W, "zh-skills-jobs", ["dialog_frame"], "Skills > Background jobs")
    W.click("dialog_close")
    W.wait(lambda: not W.visible("dialog_frame"), 6)


def run_diff(W: Walk):
    """`surfaces --first-turn 3 --diff-words`: the approvals turn; the diff
    approval's 审查差异 opens the diff review."""
    W.check("diff: connected", W.wait(lambda: W.composer() is not None, 40))
    W.check("diff: a prompt + Return starts the approvals turn",
            send(W, "重试 steer 队列") and W.wait(lambda: W.replay_saw("turn/start", 0) >= 1, 8))
    W.check("diff: the command approval takes the composer over", W.wait(lambda: bool(W.visible("cv_ap_card")), 25))
    W.click("cv_ap_deny")
    W.check("diff: 拒绝 -> the diff approval with 审查差异", W.wait(lambda: bool(W.visible("cv_ap_diff")), 12) and has(W, "审查差异"))
    if W.visible("cv_ap_card"):
        surface(W, "zh-approval-diff", ["cv_ap_card"], "Diff approval card")
    W.check("diff: the 审查差异 CLICK opens the diff review", W.click("cv_ap_diff") and W.wait_shown("b3_dialog", 10))
    W.wait(lambda: any(str(w.get("i", "")).startswith("b3_diff_file_0_h0_l") for w in W.snap()), 8)
    if W.visible("b3_dialog"):
        # The title is the preview's own (DiffReviewDialog draws preview.title).
        surface(W, "zh-diff-review", ["b3_dialog"], "Diff review", data=("b3_title",))
        W.click("b3_close")
        W.wait(lambda: not W.visible("b3_dialog"), 6)


def run_btw(W: Walk):
    """`btw`: an aside asked in Chinese, answering then answered."""
    W.check("btw: connected", W.wait(lambda: W.composer() is not None, 40))
    send(W, "/btw 为什么要先排空队列？")
    W.check("btw: 旁问 — /btw · 正在回答…",
            W.wait(lambda: W.text("btw_aside_title") == "旁问 — /btw" and W.text("btw_aside_status_label") == "正在回答…", 10),
            f"{W.text('btw_aside_title')!r} {W.text('btw_aside_status_label')!r}")
    W.dismiss_keyboard("hd_title")
    if W.visible("btw_aside"):
        surface(W, "zh-btw-answering", ["btw_aside"], "/btw aside (answering)")
    W.check("btw: the answer arrives", W.wait(lambda: bool(W.text("btw_aside_answer")) or bool(W.prefixed("btw_aside_answer")), 20))
    time.sleep(0.6)
    if W.visible("btw_aside"):
        surface(W, "zh-btw-answered", ["btw_aside"], "/btw aside (answered)")
    W.click("btw_aside_close_hit")


# The phone shell's soft keyboard hide key (a10_fleet.py / a30_peer_dock.py).
PHONE_HIDE_KEYBOARD = (377, 573)


def fleet_dispatches(W: Walk) -> int:
    if not W.replay_log or not W.replay_log.exists():
        return 0
    return sum(1 for l in W.replay_log.read_text().splitlines() if "-> peer/dispatch (fleet sim)" in l)


def run_dock(W: Walk):
    """`fleet --peer-dock`: one peer from the Fleet's Start, then the
    sidebar's peer dock — its word for the peer is the Fleet chip's word."""
    W.check("dock: connected", W.wait(lambda: W.composer() is not None, 40))
    if not open_fleet(W):
        return
    W.check("dock: the Fleet's start form reads Chinese", W.wait_shown("b3_fleet_form_title", 10) and has(W, "启动一个同侪"),
            repr(W.text("b3_fleet_form_title")))
    time.sleep(1.5)
    for _ in range(3):
        W.scroll_into("b3_fleet_model_tap", "b3_scroll")
        W.click("b3_fleet_model_tap")
        if W.wait_shown("b3_fleet_opt_0", 6):
            W.scroll_into("b3_fleet_opt_0", "b3_scroll")
            W.click("b3_fleet_opt_0")
        if W.wait(lambda: W.text("b3_fleet_model_value") == "lane-glm", 4):
            break
    brief = "修复重连时 steer 队列丢失"
    for _ in range(3):
        W.scroll_into("b3_fleet_brief", "b3_scroll")
        r = W.rect("b3_fleet_brief")
        if r:
            W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
            W.type_text(brief)
            if MODE == "phone":
                W.click_xy(*PHONE_HIDE_KEYBOARD)
                time.sleep(0.6)
        time.sleep(0.6)
        if W.text("b3_fleet_brief") == brief:
            break
        W.clear_field(60)
    W.scroll_into("b3_fleet_start", "b3_scroll")
    W.check("dock: 启动 CLICK -> one peer/dispatch", W.click("b3_fleet_start") and W.wait(lambda: fleet_dispatches(W) == 1, 12))
    W.check("dock: the Fleet shows the peer's row", W.wait(lambda: bool(text_any(W, "b3_fleet_row_0_status")), 12))
    time.sleep(2.5)  # the peer's own frames (its approval) fold
    W.scroll_into("b3_fleet_row_0_status", "b3_scroll")
    surface(W, "zh-fleet-peer", ["b3_fleet_panel"], "Fleet (one peer)")
    escape_fleet(W)
    time.sleep(1.0)
    drawer_open(W)
    if MODE == "phone" and W.wait_shown("pd_pill", 8) and not W.visible("pd_row_0_label"):
        W.click("pd_pill")
    W.check("dock: the dock shows the peer's row", W.wait_shown("pd_row_0_status", 10))
    W.check("dock: the dock reads Chinese (同侪, 隐藏同侪)", has(W, "隐藏同侪") and W.text("pd_row_0_label").startswith("同侪 1"),
            repr(W.text("pd_row_0_label")))
    surface(W, "zh-peer-dock", ["peer_dock_row", "pd_dock"], "Peer dock (sidebar)")
    dock_word = text_any(W, "pd_row_0_status")
    dock_label = text_any(W, "pd_row_0_label")
    if MODE == "phone" and W.visible("drawer_close"):
        W.click("drawer_close")
        W.wait(lambda: not W.visible("drawer_scrim"), 6)
    # The same moment in the Fleet: the dock's word is the chip's word.
    open_fleet(W)
    chip = text_any(W, "b3_fleet_row_0_status")
    word = chip.split(" ", 1)[1] if " " in chip else chip
    W.check("dock: the dock's status word is the Fleet chip's (row 117)", bool(dock_word) and word == dock_word and is_cjk(word),
            f"dock={dock_word!r} fleet={chip!r}")
    fleet_label = text_any(W, "b3_fleet_row_0_label")
    W.check("dock: the dock names the peer as the Fleet does (同侪 1 · glm-4.6)",
            dock_label == fleet_label and dock_label.startswith("同侪 1"), f"dock={dock_label!r} fleet={fleet_label!r}")
    escape_fleet(W)


if __name__ == "__main__":
    out = pathlib.Path(OUT)
    out.mkdir(parents=True, exist_ok=True)
    if PREFS.exists():
        PREFS.unlink()
    env = {"OCTOSCODE_DISPLAY_PREFS_PATH": str(PREFS), "OCTOS_WORKSPACE_CWD": "/home/user/src/octos"}
    # A24_ONLY=<run>[,<run>…] re-runs some runs while developing them (live,
    # relaunch, surfaces, skills, diff, btw, dock); the evidence is always the
    # whole walk.
    only = [r for r in os.environ.get("A24_ONLY", "").split(",") if r]

    def want(name: str) -> bool:
        return not only or name in only

    rcs = []
    if want("live"):
        rcs.append(run_session(run_live, mode=MODE, outdir=OUT, scenario="activity", env=env))
    if want("relaunch"):
        rcs.append(run_session(run_relaunch, mode=MODE, outdir=str(out / "relaunch"), scenario="activity", env=env))
    # Runs 3 and 4 are launched in Chinese (the saved preference).
    PREFS.write_text(json.dumps({"version": 1, "theme": "slate", "language": "zh", "vimMode": False}) + "\n")
    zh = {"OCTOSCODE_DISPLAY_PREFS_PATH": str(PREFS)}
    # Run 3: the phase-2 surfaces, on the a10 fixture (/review answers there;
    # no workspace cwd: the fixture lists /review, /thinking and /threads for
    # its own Session).
    if want("surfaces"):
        rcs.append(run_session(run_surfaces, mode=MODE, outdir=str(out / "surfaces"), scenario="a10", env=zh))
    # Run 4: the merged surfaces, each on its own fixture.
    for name, fn, scenario, args in (
        ("skills", run_skills, "skill-jobs", []),
        ("diff", run_diff, "surfaces", ["--first-turn", "3", "--diff-words"]),
        ("btw", run_btw, "btw", ["--adopt-turn-ids", "--delay-ms", "40", "--btw-delay-ms", "4000"]),
        ("dock", run_dock, "fleet", ["--peer-dock"]),
    ):
        if want(name):
            rcs.append(run_session(fn, mode=MODE, outdir=str(out / name), scenario=scenario, env=zh, replay_args=args))
    (out / "captures.json").write_text(json.dumps(SUMMARY, indent=1, ensure_ascii=False) + "\n")
    if PREFS.exists():
        PREFS.unlink()
    ok = all(rc == 0 for rc in rcs)
    print(f"== WALK a24 language {MODE}: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)
