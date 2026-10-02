#!/usr/bin/env python3
"""A10 — the Profile's configured model providers by CLICK (web
`ModelManagementSection`; no Stage-A board — the native dialog kit).

`/model` opens the Models dialog; its "Manage providers" opens the
providers dialog (the recorded r2 config: the primary and the r2-route
fallback). On the fallback: "Add a model on this route" -> "Fetch available
models" (profile/llm/fetch_models) -> an "Available from endpoint"
suggestion fills the Model ID -> Save (profile/llm/test, then
profile/llm/upsert set_primary:false) -> the new route listed. Then its
"Delete" -> the typed confirmation (`DELETE <family>/<model>`, case-
sensitive: "Delete provider" armed only by the exact phrase) ->
profile/llm/delete -> the web's success line, the route gone.

Against `replay_serve --scenario a10` (its providers simulator: r2's
recorded config changed by each upsert/delete + a10-routes-faithful.jsonl).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_routes.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/routes/{MODE}"
VP = "b3_scroll"
PHRASE = "DELETE deepseek/deepseek-v4-pro"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_routes_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))


def seen(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, VP)


def click(W: Walk, wid: str, expect, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    ok = W.click_in(wid, VP) and W.wait(expect, secs)
    if not ok:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP) and W.wait(expect, secs)
    return ok


def routes(W: Walk) -> list[str]:
    return [W.text(f"b3_routes_row_{i}_endpoint") for i in range(4) if W.visible(f"b3_routes_row_{i}_endpoint")]


def walk(W: Walk) -> None:
    W.note("== 1. Models -> Manage providers")
    W.check("models: /model palette CLICK opens the Models dialog",
            W.palette_run("mo", "/model") and W.wait_shown("dlg_models_t_title", 10))
    W.scroll_into("dlg_models_manage_providers_control", "dialog_scroll")
    W.check("models: 'Manage providers' CLICK opens the providers dialog (the Models dialog closes)",
            W.click("dlg_models_manage_providers_control")
            and W.wait(lambda: W.text("b3_title") == "Model providers" and not W.visible("dialog_frame"), 10))
    W.check("providers: the recorded config — the primary and the r2-route fallback",
            W.wait(lambda: seen(W, "b3_routes_row_1_endpoint"), 8)
            and bool(W.visible("b3_routes_row_0_primary_chip")) and seen(W, "b3_routes_row_1_base_url")
            and W.text("b3_routes_row_1_base_url") == "http://127.0.0.1:9/v1",
            f"{routes(W)}")
    numeric(W, "providers")
    W.shot(f"01-providers-{MODE}")

    W.note("== 2. fetch the endpoint's models, pick one, save")
    W.check("add: 'Add a model on this route' (fallback) CLICK opens the editor",
            click(W, "b3_routes_row_1_add", lambda: seen(W, "b3_routes_model")))
    W.check("add: 'Fetch available models' CLICK -> three suggestions 'Available from endpoint'",
            click(W, "b3_routes_fetch", lambda: seen(W, "b3_routes_pick_2"))
            and W.text("b3_routes_fetched_label") == "Available from endpoint")
    W.check("add: the suggestion 'deepseek-v4-pro' CLICK fills the Model ID",
            click(W, "b3_routes_pick_1", lambda: seen(W, "b3_routes_model")
                  and (W.visible("b3_routes_model")[0].get("val") or "") == "deepseek-v4-pro"))
    W.scroll_into("b3_routes_editor_title", VP)
    numeric(W, "editor")
    W.shot(f"02-editor-{MODE}")
    W.mark()
    W.check("add: Save CLICK -> test then upsert -> 'Provider saved.'; the new route listed",
            click(W, "b3_routes_save", lambda: W.text("b3_routes_notice") == "Provider saved."
                  and seen(W, "b3_routes_row_2_endpoint"), 10)
            and W.text("b3_routes_row_2_endpoint").startswith("deepseek-v4-pro"))
    W.check("wire: profile/llm/test then profile/llm/upsert", W.replay_saw("profile/llm/test", 2) == 1
            and W.replay_saw("profile/llm/upsert", 2) == 1)
    W.shot(f"03-saved-{MODE}")

    W.note("== 3. delete behind the typed phrase")
    W.check("delete: 'Delete' (the new route) CLICK -> 'Delete model provider?' with the phrase to type",
            click(W, "b3_routes_row_2_delete", lambda: seen(W, "b3_routes_delete_prompt"))
            and PHRASE in W.text("b3_routes_delete_prompt"))
    W.check("delete: 'Delete provider' is inert before the phrase", bool(W.visible("b3_routes_delete_off")))
    W.scroll_into("b3_routes_phrase", VP)
    r = W.rect("b3_routes_phrase")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text(PHRASE.lower())
    W.dismiss_keyboard()
    W.check("delete: a phrase in the wrong case does not arm it", bool(W.visible("b3_routes_delete_off")))
    r = W.rect("b3_routes_phrase")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.key("End")
        W.clear_field(60)
        W.type_text(PHRASE)
    W.dismiss_keyboard()
    W.check("delete: the exact phrase arms 'Delete provider' (live, no remount)",
            W.wait(lambda: seen(W, "b3_routes_delete_go") and bool(W.visible("b3_routes_delete_on")), 6))
    W.scroll_into("b3_routes_delete_title", VP)
    numeric(W, "delete")
    W.shot(f"04-delete-{MODE}")
    W.check("delete: 'Delete provider' CLICK -> profile/llm/delete -> the web's success line; the route gone",
            click(W, "b3_routes_delete_go", lambda: W.text("b3_routes_notice").startswith("Provider deleted.")
                  and not W.visible("b3_routes_row_2_endpoint"), 10))
    W.check("wire: one profile/llm/delete", W.replay_saw("profile/llm/delete", 2) == 1)
    numeric(W, "deleted")
    W.shot(f"05-deleted-{MODE}")
    W.check("providers: the close glyph CLICK closes the dialog", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
