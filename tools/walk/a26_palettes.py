#!/usr/bin/env python3
"""Unified Theme selector by native clicks, desktop and phone.

Choose checks System/Light/Dark and four named themes, then saves Solarized.
Relaunch verifies persistence and restores Light. The replay fixture keeps
all conversations and preferences isolated from the user's app.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a26_lib as L  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a26_palettes",
    "title": "Settings > Preferences: one unified theme selector, each re-theming the whole app at once; Save; a relaunch adopts it",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "history"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "dsflash",
                    "OCTOS_BEARER": "replay", "OCTOSCODE_THEME": "light"},
            "ready": ["i0_composer_0"]},
    "runs": [
        {"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"], "env": {"A26_PHASE": "choose"}},
        {"restart": "app", "argv": ["{port}", "{mode}", "{fixture_log}", "{out}"], "env": {"A26_PHASE": "relaunch"}},
    ],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

PHASE = os.environ.get("A26_PHASE", "choose")
PREFS = os.environ.get("OCTOSCODE_DISPLAY_PREFS_PATH", "")


def radio_on(w, p, sn=None):
    """The palette row whose radio shows its on layer."""
    sn = sn if sn is not None else w.snap()
    row = w.rect(f"pal_{p}", sn=sn)
    ons = [x["r"] for x in sn if x.get("i") == "rd_on" and w.shown(x)]
    return bool(row and any(L.inside(r, row) for r in ons))


def rows_ok(w):
    sn = w.snap()
    body = w.rect("set_body", sn=sn)
    rows = [w.rect(f"pal_{p}", sn=sn) for p in L.PALETTES]
    titles = [w.text("pl_title", sn=sn)]
    labels = [x.get("t") for x in sorted((x for x in sn if x.get("i") == "pl_title" and w.shown(x)), key=lambda x: x["r"][1])]
    hits = [x["r"] for x in sn if x.get("i") == "pl_hit" and w.shown(x)]
    return sn, body, rows, labels, hits, titles


def pick(w, p):
    """Click palette `p`'s row; True when the app re-themed to it."""
    w.mark()
    hits = sorted((x["r"] for x in w.snap() if x.get("i") == "pl_hit" and w.shown(x)), key=lambda r: r[1])
    i = L.PALETTES.index(p)
    if len(hits) <= i:
        return False
    r = hits[i]
    w.note(f"CLICK the {L.LABELS[p]} row r={r}")
    w.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    return w.wait(lambda: w.text("sb_theme_label") == L.LABELS[p], 8)


def main():
    w = L.walk_from_argv()
    mode = w.mode
    w.mark()
    if PHASE == "relaunch":
        ok, detail = L.draws_look(w, "solarized")
        w.check("a fresh launch draws the saved palette (Solarized) from its first frame", ok, detail)
        w.shot(f"{mode}-relaunch-solarized")
    else:
        ok, detail = L.draws_look(w, "light")
        w.check("Terminal on the light theme draws the light shell", ok, detail)
        w.shot(f"{mode}-terminal-light")
    w.check("Settings opens from the sidebar footer", L.open_settings_from_footer(w))
    w.check("Preferences shows", L.open_preferences(w))
    sn, body, rows, labels, hits, _ = rows_ok(w)
    w.check("seven mutually exclusive theme choices", labels == [L.LABELS[p] for p in L.PALETTES], f"{labels}")
    w.check("every palette row lies inside the Settings body",
            body is not None and all(r and L.inside(r, body) for r in rows), f"body={body} rows={rows}")
    small = [r for r in hits if r[3] < 28 or r[2] < 28]
    w.check("palette rows are >= 28 px targets", len(hits) == 7 and not small, f"{hits}")
    save, foot = w.rect("prefs_save", sn=sn), w.rect("set_footer", sn=sn)
    # Save lives in Preferences' fixed footer under the scrolling body.
    w.check("Save sits whole inside the fixed footer (not clipped)", bool(save and foot and L.inside(save, foot) and save[3] >= 34),
            f"save={save} footer={foot}")
    w.check("the footer sits below the body", bool(body and foot and foot[1] >= body[1] + body[3] - 1),
            f"body={body} footer={foot}")
    current = "solarized" if PHASE == "relaunch" else "system"
    w.check(f"the {L.LABELS[current]} radio is on", radio_on(w, current, sn))
    if PHASE == "relaunch":
        w.shot(f"{mode}-relaunch-prefs")
        # Restore: Light, saved.
        w.check("the Light row re-themes back to the light shell", pick(w, "light"))
        w.click("prefs_save")
        w.check("Save -> 'Preferences saved.'", w.wait(lambda: w.text("prefs_status") == "Preferences saved.", 6),
                repr(w.text("prefs_status")))
        if PREFS:
            doc = json.load(open(PREFS))
            w.check("the file holds the whitelist with light", doc.get("theme") == "light" and doc.get("version") == 2 and len(doc) == 4, f"{doc}")
        L.close_settings(w)
        ok, detail = L.draws_look(w, "light")
        w.check("…and the app is light again", ok, detail)
        sys.exit(w.summary())
    w.shot(f"{mode}-prefs-terminal")
    for p in ["dark", "light", "codex", "claude", "slate", "solarized"]:
        w.check(f"the {L.LABELS[p]} row re-themes the app (log)", pick(w, p))
        time.sleep(0.6)
        sn = w.snap()
        w.check(f"{L.LABELS[p]}: its radio is on and System's is off", radio_on(w, p, sn) and not radio_on(w, "system", sn))
        w.check(f"{L.LABELS[p]}: applied but unsaved", w.text("prefs_status", sn=sn) == "Unsaved preferences.",
                repr(w.text("prefs_status", sn=sn)))
        w.shot(f"{mode}-prefs-{p}")
        w.check(f"{L.LABELS[p]}: Settings closes", L.close_settings(w))
        time.sleep(0.5)
        ok, detail = L.draws_look(w, p)
        w.check(f"{L.LABELS[p]}: the whole app draws the palette (the window's own background)", ok, detail)
        w.shot(f"{mode}-conversation-{p}")
        if p != "solarized":
            w.check(f"{L.LABELS[p]}: Settings reopens from the footer", L.open_settings_from_footer(w) and L.open_preferences(w))
    # Save the last one (Solarized): only the whitelist is written.
    w.check("Settings reopens for Save", L.open_settings_from_footer(w) and L.open_preferences(w))
    w.click("prefs_save")
    w.check("Save -> 'Preferences saved.'", w.wait(lambda: w.text("prefs_status") == "Preferences saved.", 6),
            repr(w.text("prefs_status")))
    if PREFS:
        doc = json.load(open(PREFS))
        w.check("the file holds exactly the display whitelist with solarized",
                set(doc) == {"version", "theme", "language", "vimMode"} and doc["theme"] == "solarized", f"{doc}")
    L.close_settings(w)
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
