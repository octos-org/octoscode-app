#!/usr/bin/env python3
"""A10 — the native code review dialog by CLICK (web `NativeReviewDialog`;
A5's approved autonomy-02 card plus the web's own paragraphs and the
"Review instructions (optional)" field).

The palette's `/review` row opens the dialog; it says the workflow is a
Session turn, not a diff preview; instructions typed with markup are shown as
typed and sent verbatim (trimmed) as `review/start`'s `prompt`; Start
admits a NEW turn (a fresh turn id), closes the dialog, records the request
in the Session as text, and the receipt (the request's own turn echoed)
folds — reopening shows the running review.

Against `replay_serve --scenario a10` (its `review/start` arm answers the
octos-core `ReviewStartResult` shape).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_review.py <desktop|phone> <outdir>
"""
import re
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/review/{MODE}"
VP = "dialog_scroll"
TYPED = "<b>focus</b> on the parser"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("dialog_frame")), 6)
    c = dialog_checks(W.snap(), "dialog_frame", ("dlg_review_",), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))
    return c


def seen(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, VP)


def review_params(W: Walk) -> list[str]:
    if not W.replay_log or not W.replay_log.exists():
        return []
    return [l for l in W.replay_log.read_text().splitlines() if "-> review/start (seat simulator)" in l]


def walk(W: Walk) -> None:
    W.note("== 1. /review: the native workflow, not a diff preview")
    W.check("review: /review palette CLICK opens the dialog",
            W.palette_run("revi", "/review") and W.wait_shown("dlg_review_t_status", 10))
    W.check("review: the card says ready; the web's paragraphs say it is a Session turn, not a diff preview",
            W.text("dlg_review_t_status") == "Ready to review the current project changes."
            and seen(W, "dlg_review_review_not_preview")
            and W.text("dlg_review_review_not_preview").endswith("it is not a diff preview.")
            and seen(W, "dlg_review_review_results"))
    W.check("review: 'Review instructions (optional)' is a real field with the web's placeholder",
            seen(W, "dlg_review_prompt") and seen(W, "dlg_review_prompt_label")
            and W.text("dlg_review_prompt").startswith("Leave empty to review"))
    numeric(W, "review dialog")
    W.shot(f"01-review-{MODE}")

    W.note("== 2. instructions with markup, shown and sent as typed")
    W.scroll_into("dlg_review_prompt", VP)
    r = W.rect("dlg_review_prompt")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + 12)
        W.key("End")
        W.type_text(TYPED)
    W.dismiss_keyboard("dlg_review_t_title")
    hits = W.visible("dlg_review_prompt")
    W.check("review: the field shows the markup as text", bool(hits) and (hits[0].get("val") or "") == TYPED)
    W.shot(f"02-instructions-{MODE}")
    W.mark()
    W.scroll_into("dlg_review_start_review_control", VP)
    W.click("dlg_review_start_review_control")
    W.check("review: Start native review CLICK closes the dialog (admitted)",
            W.wait(lambda: not W.visible("dialog_frame"), 8))
    W.check("wire: review/start carried the instructions verbatim as `prompt`, delivery inline",
            W.wait(lambda: any(f'"prompt":"{TYPED}"' in l and '"delivery":"inline"' in l for l in review_params(W)), 8),
            "; ".join(l.split("(seat simulator) ", 1)[-1] for l in review_params(W))[:300])
    turns = [m.group(1) for l in review_params(W) for m in [re.search(r'"turn_id":"([0-9a-f-]{36})"', l)] if m]
    W.check("wire: a fresh protocol turn id (a new Session turn)", len(turns) == 1, f"turns={turns}")
    W.check("session: the request is recorded in the Session as text",
            W.wait(lambda: W.has_text(f"Native code review: {TYPED}"), 8))
    W.shot(f"03-recorded-{MODE}")

    W.note("== 3. the receipt folded: the review runs")
    W.check("review: reopened, the card shows the running review (the receipt's 3 specialists)",
            W.palette_run("revi", "/review") and W.wait(lambda: "3" in W.text("dlg_review_t_status")
                                                         or W.has_text("specialists"), 10))
    numeric(W, "review running")
    W.shot(f"04-running-{MODE}")
    W.click("dialog_close")
    W.wait_shown("dialog_frame", 5, gone=True)


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
