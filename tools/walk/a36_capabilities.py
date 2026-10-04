#!/usr/bin/env python3
"""A36 — Settings > Capabilities by CLICK: Skills, MCP servers, Tools and Memory are
reachable from Settings, not only by typing `/skills` or `/mcp`.

Phases (A36_PHASE; unset runs them all):
  main  : the server advertises every row's method. Settings (header) ->
          Capabilities (the nav cell on desktop, the rail chip on the phone)
          -> four rows; Skills "Open" -> the Skills dialog over Settings;
          MCP servers "Open" -> its own panel with the
          "configured on the server" note; Tools "Open" -> the Tools panel; Memory
          "Open" -> the Memory dialog (it reads memory/overview).
  none  : the open withdraws profile/skills/list, mcp/status/list, tool/status/list and the
          memory methods: no row, the section's note instead.
  dark  : the same section in the dark theme (Settings follows the theme).
  zh    : the same section in Chinese (Noto Sans SC), rows unclipped.

Against `replay_serve --scenario memory` (r1's handshake + the `screens`
replies + the memory simulator). usage: a36_capabilities.py <desktop|phone> <outdir>
"""
import json
import os
import pathlib
import sys
import time

from a10_lib import Walk, inside, overlap, run_session

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a36_capabilities",
    "title": "Settings > Capabilities (A36): Skills / MCP servers / Tools / Memory rows by CLICK, each gated by the "
             "advertised method; MCP and Tools open separate panels; "
             "none advertised -> the note; dark; Chinese",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "main"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "none"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "dark"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "zh"}},
    ],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a36/capabilities/{MODE}"
PHASE = os.environ.get("A36_PHASE", "")
PHONE = MODE == "phone"
NAV = "set_rail_capabilities" if PHONE else "set_nav_capabilities"
ROWS = [("skills", "set_cap_skills_row", "set_cap_skills"), ("mcp", "set_cap_mcp_row", "set_cap_mcp"),
        ("tools", "set_cap_tools_row", "set_cap_tools"),
        ("memory", "set_cap_memory_row", "set_cap_memory")]
MEMORY_METHODS = ["memory/overview", "memory/entity", "memory/search", "memory/load", "memory/ingest"]


def labels_in(W: Walk, box, sn):
    return [w for w in sn if W.shown(w) and w.get("ty") == "Label" and (w.get("t") or "").strip()
            and inside(w["r"], box, tol=400)]


def section_checks(W: Walk, name: str, expect_rows):
    """The section's numeric checks from one /snap: every row title / help /
    pill label inside the section and the Settings body, no two labels
    overlapping, each row's Open hit >= 28 px, each pill label inside its
    pill, the rows in order and left-aligned on one edge."""
    sn = W.snap()
    sec = W.rect("sec_capabilities", sn=sn)
    body = W.rect("settings_drawer", sn=sn)
    if not sec or not body:
        W.check(f"{name}: the section is laid out", False, f"sec={sec} drawer={body}")
        return
    labels = [w for w in sn if W.shown(w) and w.get("ty") == "Label" and (w.get("t") or "").strip()
              and inside(w["r"], sec)]
    outside = [w.get("t") for w in labels if not inside(w["r"], body)]
    over = [(a.get("t"), b.get("t")) for i, a in enumerate(labels) for b in labels[i + 1:] if overlap(a["r"], b["r"])]
    hits, pills, tops, lefts = [], [], [], []
    for rid, row, hit in ROWS:
        rr, hr = W.rect(row, sn=sn), W.rect(hit, sn=sn)
        if rid not in expect_rows:
            if rr:
                outside.append(f"{row} shown but not advertised")
            continue
        if not rr or not hr:
            outside.append(f"{row} missing")
            continue
        hits.append((hit, hr))
        tops.append(rr[1])
        row_labels = sorted([w for w in labels if inside(w["r"], rr)], key=lambda w: (w["r"][1], w["r"][0]))
        if row_labels:
            lefts.append(round(row_labels[0]["r"][0]))
        pill_label = [w for w in labels if inside(w["r"], hr)]
        pills.append((hit, bool(pill_label)))
    small = [(h, r) for h, r in hits if r[3] < 28 - 0.5 or r[2] < 28 - 0.5]
    ordered = tops == sorted(tops)
    one_edge = len(set(lefts)) <= 1
    ok = not outside and not over and not small and all(p for _, p in pills) and ordered and one_edge
    W.check(f"{name}: numeric checks (labels inside, no overlap, Open >= 28 px, rows ordered on one edge)", ok,
            f"labels={len(labels)} outside={outside} overlaps={over} small={small} pills={pills} "
            f"tops={tops} lefts={sorted(set(lefts))} sec={sec}")


def open_capabilities(W: Walk) -> bool:
    W.mark()
    if not W.click("settings_open_hit"):
        return False
    if not W.wait(lambda: bool(W.visible("settings_drawer")), 8):
        return False
    W.mark()
    W.click(NAV)
    return W.wait(lambda: bool(W.visible("sec_capabilities")), 6)


def close_settings(W: Walk):
    W.click("set_back" if PHONE else "settings_close")
    W.wait(lambda: not W.visible("settings_drawer"), 6)


def main_phase(W: Walk, tag: str = "main", zh: bool = False, dark: bool = False):
    W.note(f"== {tag}: Settings > Capabilities by CLICK")
    W.check(f"{tag}: Settings > Capabilities CLICK shows the section",
            open_capabilities(W) and W.logged("settings.section.capabilities", 4))
    W.check(f"{tag}: the four rows are offered (each method advertised)",
            all(W.visible(r) for _, r, _ in ROWS) and not W.visible("set_cap_none"),
            f"{[(r, bool(W.visible(r))) for _, r, _ in ROWS]}")
    section_checks(W, tag, {"skills", "mcp", "tools", "memory"})
    if zh:
        want = ["可用能力", "技能", "MCP 服务器", "工具", "记忆", "打开"]
        texts = [w.get("t") or "" for w in W.snap() if W.shown(w)]
        W.check(f"{tag}: the section reads Chinese", all(any(x == t or x in t for t in texts) for x in want),
                f"{[x for x in want if not any(x == t or x in t for t in texts)]}")
    W.shot(f"01-capabilities-{tag}-{MODE}")
    if zh or dark:
        close_settings(W)
        return

    W.note("== Skills: Open -> the Skills dialog")
    W.mark()
    W.click("set_cap_skills")
    W.check("skills: Open CLICK -> dialog.open.skills -> the Skills dialog over Settings",
            W.logged("dialog.open.skills", 4) and W.wait(lambda: bool(W.visible("dlg_skills_t_title")), 8))
    W.check("wire: the Skills dialog read profile/skills/list", W.replay_saw("profile/skills/list", 4) >= 1)
    W.shot(f"02-skills-from-settings-{MODE}")
    W.click("dialog_close")
    W.check("skills: the dialog closes back to Settings > Capabilities",
            W.wait(lambda: not W.visible("dlg_skills_t_title") and bool(W.visible("sec_capabilities")), 6))

    W.note("== MCP servers: Open -> the MCP servers panel")
    W.mark()
    W.click("set_cap_mcp")
    W.check("mcp: Open CLICK -> b3.open.mcp -> the MCP servers panel",
            W.logged("b3.open.mcp", 4) and W.wait(lambda: bool(W.visible("b3_inv_mcp_note")), 8))
    note = W.text("b3_inv_mcp_note")
    W.check("mcp: the view says servers are configured on the server, no add/remove",
            "configured on the server" in note and "can't add or remove" in note
            and not any("Add server" in (w.get("t") or "") or "Remove server" in (w.get("t") or "")
                        for w in W.snap() if W.shown(w)), repr(note))
    W.check("wire: the inventory read mcp/status/list", W.replay_saw("mcp/status/list", 4) >= 1)
    W.shot(f"03-mcp-from-settings-{MODE}")
    W.click("b3_close")
    W.check("mcp: the inventory closes back to Settings",
            W.wait(lambda: not W.visible("b3_inv_mcp_note") and bool(W.visible("sec_capabilities")), 6))

    W.note("== Tools: Open -> a separate Tools panel")
    W.mark()
    W.click("set_cap_tools")
    W.check("tools: Open CLICK -> b3.open.tools -> the Tools panel",
            W.logged("b3.open.tools", 4) and W.wait(lambda: bool(W.visible("b3_inv_count")), 8))
    W.check("tools: no MCP table or shared tab switcher",
            not W.visible("b3_inv_summary") and not W.visible("b3_inv_tab_0"))
    W.check("wire: the Tools panel read tool/status/list", W.replay_saw("tool/status/list", 4) >= 1)
    W.shot(f"03-tools-from-settings-{MODE}")
    W.click("b3_close")
    W.check("tools: the panel closes back to Settings",
            W.wait(lambda: not W.visible("b3_inv_count") and bool(W.visible("sec_capabilities")), 6))

    W.note("== Memory: Open -> the Memory dialog")
    W.mark()
    W.click("set_cap_memory")
    W.check("memory: Open CLICK -> b3.open.memory -> the Memory dialog",
            W.logged("b3.open.memory", 4) and W.wait(lambda: bool(W.visible("b3_mem_title")), 8))
    W.check("wire: the Memory dialog read memory/overview", W.replay_saw("memory/overview", 4) >= 1)
    W.shot(f"04-memory-from-settings-{MODE}")
    W.click("b3_close")
    W.check("memory: the dialog closes back to Settings",
            W.wait(lambda: not W.visible("b3_mem_title") and bool(W.visible("sec_capabilities")), 6))
    close_settings(W)


def none_phase(W: Walk):
    W.note("== none: nothing advertised -> no row, the note")
    W.check("none: Settings > Capabilities CLICK shows the section", open_capabilities(W))
    W.check("none: no row is offered; the section says why",
            not any(W.visible(r) for _, r, _ in ROWS) and bool(W.visible("set_cap_none")),
            f"{[(r, bool(W.visible(r))) for _, r, _ in ROWS]} note={bool(W.visible('set_cap_none'))}")
    W.shot(f"05-capabilities-none-{MODE}")
    close_settings(W)


def run_phase(phase: str) -> int:
    out = pathlib.Path(OUT)
    out.mkdir(parents=True, exist_ok=True)
    args, env = [], {}
    if phase == "none":
        for m in ["profile/skills/list", "mcp/status/list", "tool/status/list"] + MEMORY_METHODS:
            args += ["--drop-method", m]
    tmp = []
    if phase == "zh":
        prefs = (out / "display-zh.json").resolve()
        prefs.write_text(json.dumps({"version": 1, "theme": "terminal", "language": "zh", "vimMode": False}))
        env["OCTOSCODE_DISPLAY_PREFS_PATH"] = str(prefs)
        tmp.append(prefs)
    if phase == "dark":
        theme = (out / "theme-dark.json").resolve()
        theme.write_text(json.dumps({"version": 1, "theme": "dark", "language": "en", "vimMode": False}))
        env["OCTOSCODE_PREF_PATH"] = str(theme)
        tmp.append(theme)
    fn = {
        "main": main_phase,
        "none": none_phase,
        "dark": lambda W: main_phase(W, "dark", dark=True),
        "zh": lambda W: main_phase(W, "zh", zh=True),
    }[phase]
    try:
        scenario = os.environ.get("A36_SCENARIO", "memory")
        return run_session(fn, mode=MODE, outdir=str(out / phase), scenario=scenario, replay_args=args, env=env)
    finally:
        for f in tmp:
            if f.exists():
                f.unlink()


if __name__ == "__main__":
    phases = [PHASE] if PHASE else ["main", "none", "dark", "zh"]
    rc = 0
    for ph in phases:
        rc |= run_phase(ph)
        time.sleep(1.0)
    sys.exit(rc)
