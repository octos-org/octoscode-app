#!/usr/bin/env python3
"""A10 — the header's Review entry by CLICK (web `DiffReviewDialog`; no
Stage-A board — the board-3 dialog kit, A5 style).

Judge finding: the header "Review" opened a mostly blank docked sheet (a
"Review" title colliding with the session title, a "Last turn" pill, a
28x22 close, nothing else). Now:

1. no preview announced yet: the header Review CLICK opens the modal with
   the eyebrow, "Review changes", the honest empty state and — when the
   server has native review — "Code review…" (the /review dialog); nothing is
   read; the close control is 28 px and closes it;
2. a typed DIFF approval (A6's `surfaces` replay, turn 4: r5's command
   approval, then the diff approval whose `diff/preview/get` is answered in
   the octos-core shape): its "Review diff" CLICK opens the same dialog on
   ONE `diff/preview/get` — the preview's title, +4 −1, status · source ·
   id, src/main.rs with its hunk and numbered, marked lines; Refresh CLICK =
   one more read; Escape / the close returns to the card;
3. the header Review CLICK now opens the announced preview too.

Against `replay_serve --scenario surfaces --first-turn 3` (the first prompt
plays the approvals turn).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_diff_review.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, inside, run_session

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a10_diff_review",
    "title": "the diff review dialog: header Review, a typed diff approval's Review diff, Refresh, back to the card",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        85: {"checks": {"desktop": ["diff: 'Review diff' CLICK opens the dialog on ONE diff/preview/get",
                                    "diff: Escape returns to the approval card"],
                        "phone": ["diff: 'Review diff' CLICK opens the dialog on ONE diff/preview/get",
                                  "diff: the close CLICK returns to the approval card"]},
             "partial": "the review over the approval and Escape back to the card; Tab ownership is not walked"},
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/diff-review/{MODE}"
VP = "b3_scroll"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    sn = W.snap()
    c = dialog_checks(sn, "b3_dialog", ("b3_diff_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))
    close = W.rect("b3_close", sn=sn)
    W.check(f"{name}: the close control is >= 28 x 28 px", bool(close) and close[2] >= 28 and close[3] >= 28, f"b3_close={close}")
    title, dialog = W.rect("b3_title", sn=sn), W.rect("b3_dialog", sn=sn)
    W.check(f"{name}: the title sits inside the modal card (no header collision)",
            bool(title and dialog) and inside(title, dialog), f"title={title} dialog={dialog}")


def code_text(W: Walk, lid: str) -> str:
    """A28: a decorated line is drawn as its runs `<line>_c<k>_<class>`; its
    text is their texts in order (a plain block's `<hunk>_b<n>_code` past
    the decoration bound)."""
    import re
    pat = re.compile(re.escape(lid) + r"_(?:w\d+_)?c(\d+)_[a-z]+$")
    runs = sorted(((int(m.group(1)), w.get("t") or "") for w in W.snap()
                   for m in [pat.match(str(w.get("i", "")))] if m), key=lambda t: t[0])
    return "".join(t for _, t in runs)


def header_review(W: Walk) -> bool:
    ok = W.click("review_open_hit") and W.wait(lambda: bool(W.visible("b3_diff_eyebrow")), 8)
    if not ok:
        W.note("RETRY review_open_hit")
        ok = W.click("review_open_hit") and W.wait(lambda: bool(W.visible("b3_diff_eyebrow")), 8)
    return ok


def send_prompt(W: Walk, text: str) -> bool:
    c = W.composer()
    if c is None:
        return False
    x, y, w, h = c["r"]
    W.note(f"CLICK composer r={c['r']}")
    W.click_xy(x + w / 2, y + h / 2)
    W.key("End")
    W.clear_field(40)
    W.type_text(text)
    W.key("Return")
    return W.wait(lambda: W.replay_saw("turn/start", 0) >= 1, 8)


def walk(W: Walk) -> None:
    W.note("== 1. no preview yet: the header Review entry")
    W.check("header: Review CLICK opens the diff review modal (eyebrow + 'Review changes')",
            header_review(W) and W.text("b3_title") == "Review changes"
            and W.text("b3_diff_eyebrow") == "Authoritative diff preview")
    W.check("empty: 'No diff preview yet' + the reason; nothing is read",
            W.text("b3_diff_empty_head") == "No diff preview yet" and bool(W.text("b3_diff_empty_body"))
            and W.replay_saw("diff/preview/get", 0) == 0)
    numeric(W, "empty")
    W.shot(f"01-empty-{MODE}")
    if W.visible("b3_diff_native"):
        W.check("empty: 'Code review…' CLICK opens the Code review dialog (/review)",
                W.click("b3_diff_native") and W.wait(lambda: W.has_text("Code review") and not W.visible("b3_diff_eyebrow"), 8))
        W.shot(f"01b-code-review-{MODE}")
        # A phone has no Escape (the shell's Escape leaves the app): the
        # dialog's own close control.
        if MODE != "phone":
            W.key("Escape")
            W.wait(lambda: not W.visible("dialog_close"), 4)
        if W.visible("dialog_close"):
            W.click("dialog_close")
        W.check("code review: closed again (the composer is back)", W.wait(lambda: not W.visible("dialog_close"), 6))
    else:
        W.note("(the server does not advertise native review: no Code review… button)")
        W.check("empty: the close control CLICK closes it", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))

    W.note("== 2. a typed diff approval's Review diff")
    W.check("turn: a prompt CLICK + Return -> turn/start (the approvals turn)", send_prompt(W, "apply the version flag"))
    W.check("approval: the command approval takes the composer over", W.wait(lambda: bool(W.visible("cv_ap_card")), 25))
    W.click("cv_ap_deny")
    W.check("approval: Deny -> the typed DIFF approval with 'Review diff'",
            W.wait(lambda: W.text("cv_ap_title").startswith("Apply a patch") and bool(W.visible("cv_ap_diff")), 10),
            repr(W.text("cv_ap_title")))
    W.check("diff: 'Review diff' CLICK opens the dialog on ONE diff/preview/get",
            W.click("cv_ap_diff") and W.wait(lambda: W.text("b3_title") == "Add a --version flag", 8)
            and W.replay_saw("diff/preview/get", 4) == 1, f"title={W.text('b3_title')!r}")
    W.check("diff: the totals, status · source · id, the file, its hunk and lines",
            W.text("b3_diff_add") == "+4" and W.text("b3_diff_del") == "−1"
            and W.text("b3_diff_status") == "ready" and W.text("b3_diff_source") == "pending_store"
            and "01920000-0000-7000-8000-0000000000f1".startswith(W.text("b3_diff_preview_id").rstrip("…")[:12])
            and W.text("b3_diff_file_0_path") == "src/main.rs" and W.text("b3_diff_file_0_status") == "modified"
            and W.text("b3_diff_file_0_h0_header").startswith("@@ -10,6 +10,9 @@")
            and code_text(W, "b3_diff_file_0_h0_l2").strip() == "if args.version {"
            and W.text("b3_diff_file_0_h0_l2_prefix") == "+" and W.text("b3_diff_file_0_h0_l1_prefix") == "−",
            f"add={W.text('b3_diff_add')!r} del={W.text('b3_diff_del')!r} status={W.text("b3_diff_status")!r} "
            f"path={W.text('b3_diff_file_0_path')!r} l2={code_text(W, 'b3_diff_file_0_h0_l2')!r}")
    numeric(W, "preview")
    W.shot(f"02-preview-{MODE}")
    W.check("diff: Refresh CLICK -> one more diff/preview/get",
            W.click("b3_diff_refresh") and W.wait(lambda: W.replay_saw("diff/preview/get", 0) == 2, 6)
            and W.wait(lambda: W.text("b3_title") == "Add a --version flag", 6))
    if MODE == "phone":
        closed = W.click("b3_close")
    else:
        W.key("Escape")
        closed = True
    W.check("diff: " + ("the close CLICK" if MODE == "phone" else "Escape") + " returns to the approval card",
            closed and W.wait(lambda: not W.visible("b3_dialog") and bool(W.visible("cv_ap_session")), 6))

    W.note("== 3. the header Review entry now opens the announced preview")
    W.check("header: Review CLICK -> the preview (one more read)",
            header_review(W) and W.wait(lambda: W.text("b3_title") == "Add a --version flag", 8)
            and W.replay_saw("diff/preview/get", 0) == 3)
    W.shot(f"03-header-preview-{MODE}")
    W.check("header: the close control CLICK closes it", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))
    W.click("cv_ap_session")
    time.sleep(0.5)


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="surfaces", replay_args=["--first-turn", "3"]))
