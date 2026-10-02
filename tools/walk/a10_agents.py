#!/usr/bin/env python3
"""A10 — the Agents panel click walk (web `AgentPanel.tsx`; no Stage-A board).

Every control is reached by a CLICK at its laid-out rect: the palette's
`/agents` row opens the panel; the roster comes from `agent/list`; Read
status / List artifacts / Read artifact / Read output / Load more / Interrupt
agent / the by-ID Close agent / Request parallel agents each run through the
real app against `replay_serve --scenario a10` (the recorded c24b agent
traffic + the faithful frames for the replies no recording carries). Each step
asserts the app's own effect (a /snap widget or text, and the routed log
line), and the replay log proves the method went on the wire.

usage: OCTOSCODE_APP_BIN=<host octosense> a10_agents.py <desktop|phone> <outdir>
"""
import sys

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/agents/{MODE}"


def shown(W: Walk, wid: str) -> bool:
    """Visible — scrolled into the panel body first when it is in there."""
    if W.visible(wid):
        return True
    return W.scroll_into(wid, "b3_scroll")


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    import time

    time.sleep(0.4)  # let a remount from the previous reply settle
    W.mark()
    ok = W.click_in(wid, "b3_scroll")
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        # A click that landed mid-remount routes nothing: click again (the
        # routed log line, not the click, is what the check counts).
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, "b3_scroll")
        logged = W.logged(needle, secs / 2) if ok else False
    seen = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and seen


def numeric(W: Walk, name: str):
    sn = W.snap()
    c = dialog_checks(sn, "b3_dialog", ("b3_agents_", "b3_title", "b3_close"), viewport="b3_scroll")
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))
    return c


def walk(W: Walk) -> None:
    W.note("== 1. /agents palette row CLICK opens the Agents panel; the roster loads (agent/list)")
    W.mark()
    opened = W.palette_run("age", "/agents") and W.wait_shown("b3_agents_row_0_name", 10)
    W.check("agents: /agents row CLICK -> panel with the agent/list roster", opened and W.logged("AgentsLoad", 6),
            f"rows={[w.get('t') for w in W.prefixed('b3_agents_row_') if w['i'].endswith('_name')]}")
    W.check("agents: the roster shows the running and the completed agent",
            shown(W, "b3_agents_row_1_name") and W.has_text("c24b-probe completed"))
    numeric(W, "roster")
    W.shot(f"01-roster-{MODE}")

    W.note("== 2. Read status (row 0) -> the status card")
    W.check("agents: Read status CLICK -> agent/status/read -> 'Status — <id>' card",
            click_logged(W, "b3_agents_row_0_act_status", "AgentStatus", lambda: shown(W, "b3_agents_status_title")))
    W.check("agents: the status card shows Owner session / Backend",
            shown(W, "b3_agents_status_backend") and "spawn_child_session" in W.text("b3_agents_status_backend")
            and shown(W, "b3_agents_status_session") and W.text("b3_agents_status_session").endswith(":main"))
    W.scroll_into("b3_agents_status_title", "b3_scroll")
    W.shot(f"02-status-{MODE}")

    W.note("== 3. List artifacts -> Read artifact (by id)")
    W.check("agents: List artifacts CLICK -> one detail viewer shows the artifacts (status card replaced)",
            click_logged(W, "b3_agents_row_0_act_artifacts", "AgentArtifacts", lambda: shown(W, "b3_agents_artifacts_title"))
            and not shown(W, "b3_agents_status_title"))
    W.check("agents: Read artifact CLICK -> agent/artifact/read -> the content card (markup inert)",
            click_logged(W, "b3_agents_artifact_read_0", "AgentArtifactRead", lambda: shown(W, "b3_agents_artifact_title"))
            and shown(W, "b3_agents_artifact_content") and "<script>not executable</script>" in W.text("b3_agents_artifact_content"))
    W.scroll_into("b3_agents_artifact_title", "b3_scroll")
    W.shot(f"03-artifact-{MODE}")

    W.note("== 4. Read output -> Load more (appends from next_cursor)")
    W.check("agents: Read output CLICK -> the output card with Load more",
            click_logged(W, "b3_agents_row_0_act_output", "AgentOutput", lambda: shown(W, "b3_agents_output_more")))
    W.check("agents: Load more CLICK -> the next page appended, Load more gone",
            click_logged(W, "b3_agents_output_more", "AgentOutput", lambda: not W.visible("b3_agents_output_more")
                         and shown(W, "b3_agents_output_text") and "12 passed, 0 failed" in W.text("b3_agents_output_text")))
    W.scroll_into("b3_agents_output_title", "b3_scroll")
    W.shot(f"04-output-{MODE}")

    W.note("== 5. Interrupt agent (row 0, running) -> the recorded receipt")
    W.check("agents: Interrupt agent CLICK -> agent/interrupt -> status 'interrupted' + activity line",
            click_logged(W, "b3_agents_row_0_act_interrupt", "AgentControl", lambda: shown(W, "b3_agents_activity")
                         and "interrupted" in W.text("b3_agents_activity")))
    W.check("agents: the interrupted row offers no further control",
            not shown(W, "b3_agents_row_0_act_interrupt") or not any(
                w["i"] == "b3_agents_row_0_act_interrupt" and w.get("ty") == "Button" for w in W.snap()))
    W.scroll_into("b3_agents_activity", "b3_scroll")
    W.shot(f"05-interrupted-{MODE}")

    W.note("== 6. Inspect or control an agent by ID -> Close agent")
    W.check("agents: the by-ID section opens by CLICK",
            W.click_in("b3_agents_by_id", "b3_scroll") and W.wait(lambda: shown(W, "b3_agents_query"), 8))
    W.scroll_into("b3_agents_query", "b3_scroll")
    r = W.rect("b3_agents_query")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text("<redacted-agent>")
        W.dismiss_keyboard()
    W.check("agents: Close agent (by ID) CLICK -> agent/close -> 'closed'",
            click_logged(W, "b3_agents_id_act_close", "AgentControl", lambda: shown(W, "b3_agents_activity") and "closed" in W.text("b3_agents_activity")))
    W.scroll_into("b3_agents_activity", "b3_scroll")
    W.shot(f"06-by-id-close-{MODE}")

    W.note("== 7. Request parallel agents: the composed text rides an ordinary turn/start")
    W.scroll_into("b3_agents_task", "b3_scroll")
    r = W.rect("b3_agents_task")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text("audit the parser")
        W.dismiss_keyboard()
    before = W.replay_saw("turn/start", 0)
    W.check("agents: Request parallel agents CLICK -> AgentsSpawn queued; the task field clears (the count stays)",
            click_logged(W, "b3_agents_spawn_go", "AgentsSpawn",
                         lambda: W.text("b3_agents_task") in ("", "Describe the task for the agents"), 10)
            and W.wait(lambda: W.text("b3_agents_count") == "1", 6))
    W.check("agents: exactly one ordinary turn/start reached the wire (replay log)",
            W.wait(lambda: W.replay_saw("turn/start", 0) == before + 1, 8), f"turn/start x{W.replay_saw('turn/start', 0)}")
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    numeric(W, "final")
    W.shot(f"07-spawned-{MODE}")
    W.note("== 8. the wire: every click's method reached the replay server")
    for method, n in [("agent/list", 1), ("agent/status/read", 1), ("agent/artifact/list", 1),
                      ("agent/artifact/read", 1), ("agent/output/read", 2), ("agent/interrupt", 1),
                      ("agent/close", 1)]:
        got = W.replay_saw(method, 2)
        W.check(f"wire: {method} x{n}", got == n, f"replay log: {got}")
    W.check("agents: the close glyph CLICK closes the panel",
            W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
