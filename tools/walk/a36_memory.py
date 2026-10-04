#!/usr/bin/env python3
"""A36b — Memory (board 5) by CLICK, from Settings > Capabilities > Memory.

Against `replay_serve --scenario memory` (no model): the PROPOSAL world
(octos' own reply shapes + the upstream proposal's `profile_id` echo,
`docs/proposals/memory-profile-scope.md`) and TODAY's world (a6ea8505
RECORDED on a private serve: memory answers for the signed-in account, not
the Session's profile). Phases (A36_PHASE; unset runs them all):

  main       overview (frame 2/4) -> Show all (8) -> an entity page (7) ->
             search "steer queue" (5) -> the Documents filter -> a hit's
             record (6) with the untrusted callout -> clear -> Add note (9)
             -> the receipt -> the note is found by a search; the wire.
  truncated  Show all on a MEMORY.md the server cut: the notice (8).
  recent     the overview's Recent notes rows -> a day page.
  empty      an empty profile (10a).
  loading    the overview held 6 s: the loading state (10b).
  today      a6ea8505: the honest refusal at open (no Add note), and a
             search refused for the scope reason (10c, D1 bounded copy).
  notrunning the profile confirmed but not running: search / note refused.
  dark       the overview, results and record in the dark theme (11).
  zh         the overview and the note form in Chinese (12).

usage: a36_memory.py <desktop|phone> <outdir>
"""
import json
import os
import pathlib
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a36_memory",
    "title": "Memory (board 5) by CLICK from Settings > Capabilities: overview, long-term memory, entity page, search "
             "with the kind filter, a record with the untrusted callout, Add note with its receipt; truncated, recent, "
             "empty, loading; today's server refused honestly (scope), a profile not running; dark; Chinese",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "main"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "truncated"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "recent"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "empty"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "loading"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "today"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "notrunning"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "dark"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A36_PHASE": "zh"}},
    ],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a36/memory/{MODE}"
PHASE = os.environ.get("A36_PHASE", "")
PHONE = MODE == "phone"
NAV = "set_rail_capabilities" if PHONE else "set_nav_capabilities"
VP = "b3_scroll"
DOC_ID = "doc:octoscode:7f3a9c2e5b1d4a60"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_mem_", "b3_close", "b3_title"), viewport=VP)
    W.check(f"{name}: numeric checks (labels inside, no overlap, controls >= 28)", c["ok"], checks_line(c))
    return c


def texts(W: Walk, sn=None):
    sn = sn if sn is not None else W.snap()
    return [w.get("t") or "" for w in sn if W.shown(w)]


def has(W: Walk, *needles, secs: float = 6.0) -> bool:
    return W.wait(lambda: all(any(n in t for t in texts(W)) for n in needles), secs)


def replay_lines(W: Walk, needle: str):
    if not W.replay_log or not W.replay_log.exists():
        return []
    return [l for l in W.replay_log.read_text().splitlines() if needle in l]


def open_memory(W: Walk) -> bool:
    """Settings (header) -> Capabilities -> Memory "Open": the user's path."""
    W.mark()
    if not W.click("settings_open_hit") or not W.wait(lambda: bool(W.visible("settings_drawer")), 8):
        return False
    W.click(NAV)
    if not W.wait(lambda: bool(W.visible("set_cap_memory")), 6):
        return False
    W.click("set_cap_memory")
    return W.logged("b3.open.memory", 4) and W.wait(lambda: bool(W.visible("b3_mem_title")), 8)


def close_all(W: Walk):
    if W.visible("b3_close"):
        W.click("b3_close")
        W.wait(lambda: not W.visible("b3_mem_title"), 5)
    if W.visible("settings_drawer"):
        W.click("set_back" if PHONE else "settings_close")
        W.wait(lambda: not W.visible("settings_drawer"), 5)


def click_in(W: Walk, wid: str, nth: int = 0) -> bool:
    if not W.visible(wid):
        W.scroll_into(wid, VP)
    return W.click(wid, nth)


def type_into(W: Walk, wid: str, text: str):
    r = W.rect(wid)
    if not r:
        W.note(f"no field {wid}")
        return
    W.click_xy(r[0] + r[2] / 2, r[1] + min(r[3] / 2, 18))
    W.clear_field(40)
    W.type_text(text)


def search(W: Walk, query: str):
    type_into(W, "b3_mem_query", query)
    W.mark()
    W.key("Return")


def scope_ok(W: Walk, tag: str):
    want = "服务器配置档案：dsflash" if tag == "zh" else "Server Profile: dsflash"
    got = W.text("b3_mem_scope")
    W.check(f"{tag}: the scope line names the Session's profile (D2 A), plain text (D1)", got == want, repr(got))


# ------------------------------------------------------------------ phases
def main_phase(W: Walk):
    W.note("== main: Settings > Capabilities > Memory")
    W.check("open: Settings > Capabilities > Memory 'Open' CLICK -> the Memory dialog", open_memory(W))
    W.check("wire: memory/overview names the Session's profile",
            W.wait(lambda: any('"profile_id":"dsflash"' in l for l in replay_lines(W, "-> memory/overview")), 6),
            str(replay_lines(W, "-> memory/overview")[-1:]))
    W.check("overview: long-term memory and today (frame 2/4)",
            has(W, "Long-term memory", "Prefers Rust 2024 edition", "Show all", "Today", "Fixed the steer queue redelivery"))
    scope_ok(W, "overview")
    W.check("overview: Add note is offered (the profile is confirmed, ingest advertised)", bool(W.visible("b3_mem_add")))
    numeric(W, "overview")
    W.shot(f"01-overview-{MODE}")
    W.check("overview: the entities (name-sorted by the server) and the staging line below",
            W.scroll_into("b3_mem_staging_text", VP)
            and has(W, "Entities", "octos-core", "steer-queue", "2 notes waiting for the next memory refresh."))
    numeric(W, "overview scrolled")
    W.shot(f"01b-overview-scrolled-{MODE}")

    W.note("== Show all -> long-term memory (frame 8)")
    W.check("long-term: Show all CLICK opens MEMORY.md in full",
            click_in(W, "b3_mem_lt_more") and has(W, "MEMORY.md", "Sam reviews the octos PRs."))
    numeric(W, "long-term")
    W.shot(f"02-long-term-{MODE}")
    W.click("b3_mem_back")
    W.check("long-term: ‹ Memory returns to the overview", W.wait(lambda: bool(W.visible("b3_mem_lt")), 5))

    W.note("== an entity row -> its page (frame 7)")
    ok = click_in(W, "b3_mem_entity_2") and has(W, "Entity page", "Backoff starts at 250 ms")
    W.check("entity: the steer-queue row CLICK -> memory/entity {name} -> the page", ok)
    W.check("wire: memory/entity {name: steer-queue, profile_id}",
            any('"name":"steer-queue"' in l and '"profile_id":"dsflash"' in l for l in replay_lines(W, "-> memory/entity")))
    numeric(W, "entity")
    W.shot(f"03-entity-{MODE}")
    W.click("b3_mem_back")
    W.wait(lambda: bool(W.visible("b3_mem_query")), 5)

    W.note("== search (frame 5)")
    search(W, "steer queue")
    W.check("search: Enter -> memory/search -> 3 results with kind chips and the untrusted chip",
            W.wait(lambda: bool(W.visible("b3_mem_count")), 8) and has(W, "3 results", "Knowledge", "Episode", "Document")
            and len([w for w in W.snap() if W.shown(w) and (w.get("t") or "") == "untrusted"]) == 2)
    W.check("wire: memory/search {query, limit: 20, profile_id}, no kinds for All",
            any('"query":"steer queue"' in l and '"limit":20' in l and '"kinds"' not in l
                for l in replay_lines(W, "-> memory/search")))
    W.dismiss_keyboard("b3_mem_title")
    numeric(W, "results")
    W.shot(f"04-results-{MODE}")
    W.mark()
    W.click("b3_mem_kind_3")
    W.check("filter: Documents CLICK -> kinds [document] -> 1 result",
            W.wait(lambda: W.text("b3_mem_count") == "1 result", 8)
            and any('"kinds":["document"]' in l for l in replay_lines(W, "-> memory/search")))
    W.shot(f"05-results-documents-{MODE}")
    W.click("b3_mem_kind_0")
    W.wait(lambda: W.text("b3_mem_count") == "3 results", 8)

    W.note("== a hit -> its record (frame 6)")
    ok = click_in(W, "b3_mem_hit_2") and has(W, "Backoff for redelivery", "Added from an app",
                                               "Octos reads it as data, never as instructions.", DOC_ID)
    W.check("record: the document hit CLICK -> memory/load -> the record with the untrusted callout", ok)
    W.check("record: opened N times (the load counted a visit)", has(W, "opened 3 times"), str(W.text("b3_mem_rec_meta")))
    W.check("wire: memory/load {id, profile_id}",
            any(DOC_ID in l and '"profile_id":"dsflash"' in l for l in replay_lines(W, "-> memory/load")))
    numeric(W, "record")
    W.shot(f"06-record-{MODE}")
    W.click("b3_mem_back")
    W.check("record: ‹ Results returns to the results", W.wait(lambda: bool(W.visible("b3_mem_count")), 5))
    W.click("b3_mem_clear")
    W.check("clear: × returns to the overview", W.wait(lambda: bool(W.visible("b3_mem_lt")), 5))

    W.note("== Add note (frame 9)")
    W.click("b3_mem_add")
    W.check("add: Add note CLICK -> the form", W.wait(lambda: bool(W.visible("b3_mem_add_note")), 5)
            and has(W, "Add a note", "Search matches the title and the start of the note."))
    W.check("add: the submit is unarmed while the note is empty",
            bool(W.visible("b3_mem_add_submit_off")) and not W.visible("b3_mem_add_submit_on"))
    type_into(W, "b3_mem_add_title", "Dentist appointment")
    type_into(W, "b3_mem_add_note", "Tuesday 14 October at 9:30 with Dr. Lin, Elm Street clinic.")
    W.dismiss_keyboard("b3_mem_add_head")
    W.check("add: typing the note arms Add to memory", W.wait(lambda: bool(W.visible("b3_mem_add_submit_on")), 4))
    numeric(W, "add form")
    W.shot(f"07-add-note-{MODE}")
    W.mark()
    click_in(W, "b3_mem_add_submit")
    W.check("add: Add to memory CLICK -> memory/ingest -> 'Added to memory.' on the overview",
            W.wait(lambda: W.text("b3_mem_receipt") == "Added to memory.", 8))
    ing = replay_lines(W, "-> memory/ingest")
    W.check("wire: ONE document record doc:octoscode:<16 hex>, source octoscode, the title and the note",
            len(ing) == 1 and '"kind":"document"' in ing[0] and '"source":"octoscode"' in ing[0]
            and '"id":"doc:octoscode:' in ing[0] and '"title":"Dentist appointment"' in ing[0]
            and '"profile_id":"dsflash"' in ing[0], ing[0][:200] if ing else "none")
    W.shot(f"08-added-{MODE}")
    search(W, "dentist")
    W.check("add: the note is found by a search, marked untrusted",
            W.wait(lambda: W.text("b3_mem_count") == "1 result", 8) and has(W, "Dentist appointment", "untrusted"))
    W.dismiss_keyboard("b3_mem_title")
    W.shot(f"09-note-found-{MODE}")
    close_all(W)


def truncated_phase(W: Walk):
    W.check("truncated: open", open_memory(W))
    W.check("truncated: Show all -> the notice leads the page: what the server kept, before it is read",
            click_in(W, "b3_mem_lt_more")
            and has(W, "Showing the first 96 KB of 140 KB. The rest stays on the server.", secs=10))
    numeric(W, "truncated")
    W.shot(f"10-long-term-truncated-{MODE}")
    close_all(W)


def recent_phase(W: Walk):
    W.check("recent: open", open_memory(W))
    W.scroll_into("b3_mem_day_0", VP)
    W.check("recent: the Recent notes rows", has(W, "Recent notes", "Reviewed PR #2566"))
    numeric(W, "recent")
    W.shot(f"11-recent-{MODE}")
    W.check("recent: a day row CLICK -> the day's note", click_in(W, "b3_mem_day_0")
            and has(W, "Reviewed PR #2566 (session fork)"))
    W.shot(f"12-day-{MODE}")
    close_all(W)


def empty_phase(W: Walk):
    W.check("empty: open", open_memory(W))
    W.check("empty: 'No knowledge pages yet' with its line (frame 10a)",
            has(W, "No knowledge pages yet", "No long-term pages or daily notes here yet. Recall records may still be available through search."))
    numeric(W, "empty")
    W.shot(f"13-empty-{MODE}")
    # Regression: rebuilding a dialog during a click must not leave a second
    # script callback pointing at the replaced body. No note is submitted.
    since = json.loads(W.get("/log?since=0")).get("n", 0)
    W.click("b3_mem_add")
    W.check("empty: Add note opens", W.wait(lambda: bool(W.visible("b3_mem_add_note")), 5))
    W.click("b3_mem_add_cancel")
    W.check("empty: Cancel returns to Memory", W.wait(lambda: bool(W.visible("b3_mem_add")), 5))
    time.sleep(0.4)
    lines = json.loads(W.get(f"/log?since={since}")).get("l", [])
    errors = [line for line in lines if "[E]" in line or "empty stack" in line or "mes empty" in line]
    W.check("empty: navigation has no deferred script errors", not errors, f"{len(errors)} errors")
    close_all(W)


def loading_phase(W: Walk):
    W.mark()
    W.click("settings_open_hit")
    W.wait(lambda: bool(W.visible("settings_drawer")), 8)
    W.click(NAV)
    W.wait(lambda: bool(W.visible("set_cap_memory")), 6)
    W.click("set_cap_memory")
    W.check("loading: the held overview shows 'Loading memory…' (frame 10b)",
            W.wait(lambda: bool(W.visible("b3_mem_loading")), 4) and has(W, "Loading memory…"))
    W.shot(f"14-loading-{MODE}")
    W.check("loading: the reply lands", W.wait(lambda: bool(W.visible("b3_mem_lt")), 12))
    close_all(W)


def today_phase(W: Walk):
    W.check("today: open (a6ea8505 answers the signed-in account's memory)", open_memory(W))
    W.check("today: the honest refusal — the problem and the next step, no raw error (D1, D2 A)",
            has(W, "Couldn't read memory.", "The server did not confirm the memory scope for this session in dsflash.",
                "Update octos to a version that reports session memory scope.")
            and not any(x in " ".join(texts(W)) for x in ("ProfileRuntime", "admin", "No knowledge pages yet")))
    W.check("today: no Add note (a note must never land in memory the server cannot attribute)",
            not W.visible("b3_mem_add"))
    scope_ok(W, "today")
    numeric(W, "today refused")
    W.shot(f"15-today-refused-{MODE}")
    search(W, "steer queue")
    W.check("today: a search is refused for the same reason (frame 10c)",
            has(W, "Couldn't search memory.", "not for dsflash.") and "ProfileRuntime" not in " ".join(texts(W)))
    W.check("wire: the recorded refusal came back (-32603 runtime_unavailable)",
            any("-32603" in l for l in replay_lines(W, "-> memory/search refused")))
    W.dismiss_keyboard("b3_mem_title")
    numeric(W, "today search refused")
    W.shot(f"16-today-search-refused-{MODE}")
    close_all(W)


def notrunning_phase(W: Walk):
    W.check("notrunning: open", open_memory(W))
    search(W, "steer queue")
    W.check("notrunning: a confirmed profile without a runtime: 'Couldn't search memory.' + the next step",
            has(W, "Couldn't search memory.", "The server isn't running dsflash yet",
                "Add a model provider for this profile, then try again."))
    W.dismiss_keyboard("b3_mem_title")
    numeric(W, "not running")
    W.shot(f"17-search-not-running-{MODE}")
    W.click("b3_mem_clear")
    W.wait(lambda: bool(W.visible("b3_mem_add")), 5)
    W.click("b3_mem_add")
    W.wait(lambda: bool(W.visible("b3_mem_add_note")), 5)
    type_into(W, "b3_mem_add_note", "Tuesday 9:30, Dr. Lin.")
    W.dismiss_keyboard("b3_mem_add_head")
    click_in(W, "b3_mem_add_submit")
    W.check("notrunning: a refused note keeps the draft and says why",
            has(W, "Couldn't add the note.", "Tuesday 9:30, Dr. Lin."))
    W.shot(f"18-note-refused-{MODE}")
    close_all(W)


def dark_phase(W: Walk):
    W.check("dark: open", open_memory(W))
    W.check("dark: the overview", has(W, "Long-term memory", "Entities"))
    numeric(W, "dark overview")
    W.shot(f"19-dark-overview-{MODE}")
    search(W, "steer queue")
    W.wait(lambda: bool(W.visible("b3_mem_count")), 8)
    W.dismiss_keyboard("b3_mem_title")
    W.shot(f"20-dark-results-{MODE}")
    click_in(W, "b3_mem_hit_2")
    W.check("dark: the record with its callout", has(W, "Added from an app"))
    W.shot(f"21-dark-record-{MODE}")
    close_all(W)


def zh_phase(W: Walk):
    W.check("zh: open", open_memory(W))
    W.check("zh: the overview reads Chinese (frame 12)", has(W, "记忆", "搜索记忆", "长期记忆", "今天", "实体", "显示全部"))
    scope_ok(W, "zh")
    numeric(W, "zh overview")
    W.shot(f"22-zh-overview-{MODE}")
    W.click("b3_mem_add")
    W.check("zh: the note form reads Chinese", W.wait(lambda: bool(W.visible("b3_mem_add_note")), 5)
            and has(W, "添加笔记", "搜索会匹配标题和笔记开头。", "添加到记忆"))
    numeric(W, "zh add")
    W.shot(f"23-zh-add-{MODE}")
    close_all(W)


PHASES = {
    "main": (main_phase, []),
    "truncated": (truncated_phase, ["--memory-truncated"]),
    "recent": (recent_phase, ["--memory-recent"]),
    "empty": (empty_phase, ["--memory-mode", "empty"]),
    "loading": (loading_phase, ["--slow", "memory/overview=6000"]),
    "today": (today_phase, ["--memory-mode", "today"]),
    "notrunning": (notrunning_phase, ["--memory-refuse", "search", "--memory-refuse", "ingest"]),
    "dark": (dark_phase, []),
    "zh": (zh_phase, []),
}


def run_phase(phase: str) -> int:
    out = pathlib.Path(OUT)
    out.mkdir(parents=True, exist_ok=True)
    fn, args = PHASES[phase]
    env, tmp = {}, []
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
    try:
        return run_session(fn, mode=MODE, outdir=str(out / phase), scenario="memory", replay_args=args, env=env)
    finally:
        for f in tmp:
            if f.exists():
                f.unlink()


if __name__ == "__main__":
    phases = [PHASE] if PHASE else list(PHASES)
    rc = 0
    for ph in phases:
        rc |= run_phase(ph)
        time.sleep(1.0)
    sys.exit(rc)
