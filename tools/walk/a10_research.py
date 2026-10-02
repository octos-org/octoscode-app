#!/usr/bin/env python3
"""A10 — the Research provider lanes dialog by CLICK (web
`ResearchDialog.tsx`; no Stage-A board — built with the native dialog kit,
the Agents panel's style).

Every control is reached by a CLICK at its laid-out rect: the palette's
`/research` row opens the dialog; the lanes come from
`profile/sub_providers/list`; closing and reopening while that list is in
flight drops the stale reply (the generation guard: the app logs
`ResearchLoad(..): superseded`); Edit fills the form, Clear lane draft
empties it; a typed lane (with a dummy credential in the masked field) goes
through Review lane save -> the confirmation -> Confirm save (upsert with the
dispatch-only credential; the receipt's notice; the masked field reads empty
again); Remove -> Confirm removal holds the Profile lease while the server
answers (the lock line, Close inert) and then reports the receipt.

Against `replay_serve --scenario a10 --slow profile/sub_providers/list=4000
--slow profile/sub_providers/remove=2500` (r2 recordings + the faithful
`a10-research-faithful.jsonl`).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_research.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/research/{MODE}"
VP = "b3_scroll"
# Not a credential: a visible stand-in typed into the masked field.
DUMMY = "walk-dummy-not-a-secret"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_research_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))
    return c


def seen(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, VP)


def val(W: Walk, wid: str) -> str:
    """An input's value (the instrument's `val`), scrolled into view first."""
    seen(W, wid)
    hits = W.visible(wid)
    return (hits[0].get("val") or "") if hits else ""


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    time.sleep(0.4)  # let a remount from the previous reply settle
    W.mark()
    ok = W.click_in(wid, VP)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP)
        logged = W.logged(needle, secs / 2) if ok else False
    got = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and got


def click_local(W: Walk, wid: str, expect, secs: float = 6.0) -> bool:
    """A UI-local control (no job): the CLICK and its visible effect."""
    time.sleep(0.4)
    ok = W.click_in(wid, VP)
    got = W.wait(expect, secs) if ok else False
    if ok and not got:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP)
        got = W.wait(expect, secs) if ok else False
    return ok and got


def type_into(W: Walk, wid: str, text: str) -> None:
    # Phone: the emulated soft keyboard of the previous field covers the
    # lower sheet; drop it first so the CLICK reaches the next field.
    W.dismiss_keyboard()
    W.scroll_into(wid, VP)
    r = W.rect(wid)
    if not r:
        W.note(f"no field {wid}")
        return
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.clear_field(30)
    W.type_text(text)


def walk(W: Walk) -> None:
    W.note("== 1. /research palette CLICK; close + reopen while the list is in flight (generation guard)")
    W.mark()
    W.check("research: /research row CLICK opens the dialog; 'Waiting for the server…' while it lists",
            W.palette_run("resea", "/research") and W.wait_shown("b3_research_busy", 10))
    W.shot(f"01-waiting-{MODE}")
    W.check("research: the close glyph CLICK closes it mid-list",
            W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))
    W.check("research: /research again (the alias row is /research too) -> a fresh dialog",
            W.palette_run("resea", "/research") and W.wait_shown("b3_dialog", 10))
    W.check("research: the first list's late reply is dropped (logged superseded); the current one publishes the lanes",
            W.logged("superseded", 14) and W.wait(lambda: seen(W, "b3_research_lane_1_key"), 14))
    W.check("research: lane rows carry the web's lines (key / provider · model / API style)",
            W.text("b3_research_lane_0_key") == "strong"
            and W.text("b3_research_lane_0_route") == "moonshot · kimi-k3"
            and W.text("b3_research_lane_0_style") == "API style: openai")
    W.scroll_into("b3_research_intro", VP)
    numeric(W, "lanes")
    W.shot(f"02-lanes-{MODE}")

    W.note("== 2. Edit fills the form; Clear lane draft empties it")
    W.check("research: 'Edit strong' CLICK fills the form (key, provider, model, API style, env name)",
            click_local(W, "b3_research_lane_0_edit",
                        lambda: val(W, "b3_research_key") == "strong" and val(W, "b3_research_provider") == "moonshot"
                        and val(W, "b3_research_api_type") == "openai"
                        and val(W, "b3_research_api_key_env") == "MOONSHOT_API_KEY", 10))
    W.check("research: the form's Review lane save is armed (key + provider)",
            seen(W, "b3_research_review") and bool(W.visible("b3_research_review_on")))
    numeric(W, "edit")
    W.shot(f"03-edit-{MODE}")
    W.check("research: 'Clear lane draft' CLICK empties the form; Review is disarmed",
            click_local(W, "b3_research_clear",
                        lambda: val(W, "b3_research_key") == "" and val(W, "b3_research_api_key_env") == ""
                        and seen(W, "b3_research_review_disabled_box") and bool(W.visible("b3_research_review_off")), 10))

    W.note("== 3. a new lane with a credential -> Review -> the confirmation -> Confirm save")
    for wid, text in [("b3_research_key", "web"), ("b3_research_provider", "zhipu"),
                      ("b3_research_model", "glm-4-flash"), ("b3_research_api_key_env", "ZHIPU_API_KEY"),
                      ("b3_research_credential", DUMMY), ("b3_research_description", "Web research lane")]:
        type_into(W, wid, text)
    W.dismiss_keyboard()
    W.check("research: each typed value landed in its own field",
            val(W, "b3_research_key") == "web" and val(W, "b3_research_api_key_env") == "ZHIPU_API_KEY"
            and val(W, "b3_research_description") == "Web research lane")
    W.check("research: the credential field is masked (bullets drawn, never the text)",
            seen(W, "b3_research_credential") and DUMMY not in W.text("b3_research_credential")
            and len(W.text("b3_research_credential")) == len(DUMMY))
    W.check("research: typing armed Review lane save without a remount",
            seen(W, "b3_research_review") and bool(W.visible("b3_research_review_on")))
    W.shot(f"04-draft-{MODE}")
    W.check("research: Review lane save CLICK -> 'Confirm lane save' with 'web · zhipu · glm-4-flash'",
            click_local(W, "b3_research_review",
                        lambda: seen(W, "b3_research_confirm_detail")
                        and W.text("b3_research_confirm_detail") == "web · zhipu · glm-4-flash"))
    W.check("research: the confirmation names the Profile and the restart report",
            "This changes server Profile dsflash." in W.text("b3_research_confirm_body"))
    W.scroll_into("b3_research_confirm_title", VP)
    numeric(W, "confirm save")
    W.shot(f"05-confirm-save-{MODE}")
    W.check("research: Confirm save CLICK -> upsert -> 'Saved on the server; the server reports no restart requirement.'",
            click_logged(W, "b3_research_confirm_go", "ResearchSave",
                         lambda: seen(W, "b3_research_notice")
                         and W.text("b3_research_notice") == "Saved on the server; the server reports no restart requirement.", 10))
    W.check("research: the receipt's lanes show the saved lane with its description",
            W.wait(lambda: seen(W, "b3_research_lane_2_key") and W.text("b3_research_lane_2_key") == "web", 6)
            and seen(W, "b3_research_lane_2_desc") and W.text("b3_research_lane_2_desc") == "Web research lane")
    W.check("research: the masked credential field reads empty after dispatch",
            seen(W, "b3_research_credential") and val(W, "b3_research_credential") == "")
    W.scroll_into("b3_research_notice", VP)
    numeric(W, "saved")
    W.shot(f"06-saved-{MODE}")

    W.note("== 4. Remove the saved lane: the confirmation, the lease while the server answers, the receipt")
    W.check("research: 'Remove web' CLICK -> 'Confirm lane removal' naming the key",
            click_local(W, "b3_research_lane_2_remove",
                        lambda: seen(W, "b3_research_confirm_detail") and W.text("b3_research_confirm_detail") == "web"))
    W.scroll_into("b3_research_confirm_title", VP)
    W.shot(f"07-confirm-remove-{MODE}")
    W.mark()
    W.click_in("b3_research_confirm_go", VP)
    W.check("research: while the removal is in flight the Profile lock line shows (the lease)",
            W.wait(lambda: bool(W.visible("b3_research_locked")) or W.scroll_into("b3_research_locked", VP), 3))
    W.scroll_into("b3_research_intro", VP)
    W.shot(f"08-locked-{MODE}")
    W.click("b3_close")
    W.check("research: a close CLICK under the mutation is inert (the dialog stays)",
            W.wait(lambda: bool(W.visible("b3_dialog")), 1.0) and bool(W.visible("b3_dialog")))
    W.check("research: the removal receipt -> the restart notice, the lane gone, the lock lifted",
            W.logged("ResearchRemove", 8)
            and W.wait(lambda: seen(W, "b3_research_notice") and W.text("b3_research_notice").startswith(
                "Saved on the server. A serve restart is required"), 8)
            and not W.visible("b3_research_lane_2_key") and not W.visible("b3_research_locked"))
    W.scroll_into("b3_research_notice", VP)
    numeric(W, "removed")
    W.shot(f"09-removed-{MODE}")

    W.note("== 5. the wire")
    for method, n in [("profile/sub_providers/list", 2), ("profile/sub_providers/upsert", 1),
                      ("profile/sub_providers/remove", 1)]:
        got = W.replay_saw(method, 2)
        W.check(f"wire: {method} x{n}", got >= n if method.endswith("list") else got == n, f"replay log: {got}")
    W.check("research: the close glyph CLICK closes the dialog",
            W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10",
                         replay_args=["--slow", "profile/sub_providers/list=4000",
                                      "--slow", "profile/sub_providers/remove=2500"]))
