#!/usr/bin/env python3
"""A9 — the Preferences walk (Settings > Preferences), by CLICKS.

  A9_PHASE=save    : Vim editing off -> click the toggle -> the composer's
                     Vim note shows and the status reads "Unsaved
                     preferences."; Save -> "Preferences saved." and the file
                     holds exactly {version:1, theme, language, vimMode:true}.
  A9_PHASE=relaunch: a fresh launch adopts the saved Vim editing (the note is
                     there before any click) and the toggle reads on; the
                     walk then turns it off and saves again (restores).

The prefs file is OCTOSCODE_DISPLAY_PREFS_PATH (set by the runner).
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a9_activity_walk as w  # noqa: E402
import a9_settings_walk as sw  # noqa: E402

check, soon, snap, rect, is_shown, text_of, click, log_since, shot = (
    w.check, w.soon, w.snap, w.rect, w.is_shown, w.text_of, w.click, w.log_since, w.shot,
)
MODE = w.MODE

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
# Two launches on one isolated state: save, then a relaunch that must adopt it.
WALK = {
    "name": "a9_prefs",
    "title": "Settings > Preferences: Vim editing toggle, explicit Save, the whitelist file, a relaunch adopts it",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "activity"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "a9walk"},
            "ready": ["i0_composer_0"]},
    "runs": [
        {"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"], "env": {"A9_PHASE": "save"}},
        {"restart": "app", "argv": ["{port}", "{mode}", "{fixture_log}", "{out}"], "env": {"A9_PHASE": "relaunch"}},
    ],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 600,
    "rows": {
        80: {"checks": ["the toggle flips Vim editing at once", "…and the change is unsaved",
                        "Save -> 'Preferences saved.'", "the file holds exactly the display whitelist",
                        "a fresh launch adopts the saved Vim editing"],
             "partial": "only an explicit Save persists and a relaunch restores it; the separate draft store "
                        "across that relaunch is not asserted here"},
    },
}
PHASE = os.environ.get("A9_PHASE", "save")
PREFS = os.environ.get("OCTOSCODE_DISPLAY_PREFS_PATH", "")


def vim_note():
    return text_of("b3_strip_vim")


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        w.get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    check("connected", sw.connected())
    if PHASE == "relaunch":
        check("a fresh launch adopts the saved Vim editing (the composer's note)", soon(lambda: vim_note() == "Vim · Insert"), f"{vim_note()!r}")
    else:
        check("Vim editing starts off (no composer note)", vim_note() is None, f"{vim_note()!r}")
    check("Settings opens", sw.open_settings())
    check("Preferences shows", sw.section("preferences"))
    s = snap()
    on = sw.inside("tg_on", "tg_vim", s)
    check("the toggle reads the current Vim editing", on == (PHASE == "relaunch"), f"on={on}")
    small = [(i, rect(i, s=s)) for i in ("tg_vim", "prefs_save") if not rect(i, s=s) or rect(i, s=s)[3] < 28]
    check("Preferences controls are >= 28 px targets", not small, f"{small}")
    for wid in ("tg_vim", "prefs_save", "prefs_status"):
        check(f"{wid} inside the body", sw.inside(wid, "set_body", s), f"{rect(wid, s=s)}")
    log_since()
    click("tg_vim")
    want_on = PHASE != "relaunch"
    check("the toggle flips Vim editing at once",
          soon(lambda: (vim_note() == "Vim · Insert") == want_on) and any("a9 prefs: vim editing" in l for l in log_since()),
          f"{vim_note()!r}")
    check("…and the change is unsaved", soon(lambda: text_of("prefs_status") == "Unsaved preferences."), f"{text_of('prefs_status')!r}")
    shot(f"{MODE}-prefs-{PHASE}-unsaved")
    click("prefs_save")
    check("Save -> 'Preferences saved.'", soon(lambda: text_of("prefs_status") == "Preferences saved."), f"{text_of('prefs_status')!r}")
    if PREFS:
        doc = json.load(open(PREFS))
        check("the file holds exactly the display whitelist",
              set(doc) == {"version", "theme", "language", "vimMode"} and doc["version"] == 1 and doc["vimMode"] == want_on,
              f"{doc}")
    shot(f"{MODE}-prefs-{PHASE}-saved")
    click("set_back" if MODE == "phone" else "settings_close")
    failed = [n for n, ok, _ in w.RESULTS if not ok]
    print(f"== WALK a9 prefs {PHASE} {MODE}: {len(w.RESULTS) - len(failed)}/{len(w.RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
