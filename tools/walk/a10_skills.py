#!/usr/bin/env python3
"""A10 — the Skills dialog by CLICK (web `SkillsDialog.tsx`, the approved
setup-10 card): the warning paragraph; a registry search whose rows carry the
web's fields (description, repo, "Provides executable tools" | "Instruction
skills" · licence, "Requires:", "Installed:"); Install from a row through the
confirm card; "Install from source" (repo + branch) through the confirm card;
Remove; and the Profile lock: while a mutation holds the Profile lease, the
lock line shows and no mutation is wired.

Against `replay_serve --scenario a10 --slow profile/skills/install=2500` (r2
recordings + the faithful `a10-skills-faithful.jsonl`; the slowed install keeps
the Profile lease held long enough to see the lock).
usage: a10_skills.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/skills/{MODE}"
VP = "dialog_scroll"


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("dialog_frame")), 6)
    c = dialog_checks(W.snap(), "dialog_frame", ("dlg_skills_",), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))


def seen(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, VP)


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click_in(wid, VP)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP)
        logged = W.logged(needle, secs / 2) if ok else False
    got = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and got


def type_into(W: Walk, wid: str, text: str) -> None:
    W.scroll_into(wid, VP)
    r = W.rect(wid)
    if not r:
        W.note(f"no field {wid}")
        return
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.clear_field(30)
    W.type_text(text)


def walk(W: Walk) -> None:
    W.note("== 1. /skills: the warning paragraph and the sections")
    W.check("skills: /skills palette CLICK opens the dialog",
            W.palette_run("ski", "/skills") and W.wait_shown("dlg_skills_skills_warning", 10))
    numeric(W, "skills open")
    W.shot(f"01-skills-{MODE}")

    W.note("== 2. search the registry: rows with the web's fields")
    r = W.rect("dlg_skills_skills_query")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text("lint")
        W.mark()
        W.key("Return")
    W.check("skills: Enter in the search box -> profile/skills/registry/search -> rows",
            W.logged("skills search", 6) and W.wait(lambda: seen(W, "dlg_skills_reg_0_kind"), 10))
    W.check("skills: row fields — description, repo, tools/licence, Requires, Installed",
            "Provides executable tools · MIT" in W.text("dlg_skills_reg_0_kind")
            and seen(W, "dlg_skills_reg_0_requires") and "ripgrep" in W.text("dlg_skills_reg_0_requires")
            and seen(W, "dlg_skills_reg_1_installed") and "api-client" in W.text("dlg_skills_reg_1_installed"))
    W.scroll_into("dlg_skills_reg_0_name", VP)
    numeric(W, "registry rows")
    W.shot(f"02-registry-{MODE}")

    W.note("== 3. Install (row 0) through the confirm card")
    W.check("skills: Install CLICK -> 'Confirm server installation'",
            click_logged(W, "dlg_skills_reg_0_install_control", "dialog confirm asked: skills.install_3",
                         lambda: "code-linter · branch main" in W.text("dlg_skills_cf_detail")))
    W.shot(f"03-install-confirm-{MODE}")
    W.check("skills: Confirm install CLICK -> installed; the receipt line; the row in Installed",
            click_logged(W, "dlg_skills_cf_confirm_control", "dialog confirmed: skills.install_3",
                         lambda: "Server installed: code-linter" in W.text("dlg_skills_dialog_notice"), 10)
            and W.wait(lambda: seen(W, "dlg_skills_t_name3") and W.text("dlg_skills_t_name3") == "code-linter", 6))
    numeric(W, "after install")
    W.shot(f"04-installed-{MODE}")

    W.note("== 4. Install from source: repo + branch through the confirm card")
    type_into(W, "dlg_skills_src_repo", "octos-org/review-kit")
    type_into(W, "dlg_skills_src_branch", "dev")
    W.dismiss_keyboard("dlg_skills_t_title")
    W.check("skills: Review installation CLICK -> the confirm names 'octos-org/review-kit · branch dev'",
            click_logged(W, "dlg_skills_src_review_control", "dialog confirm asked: skills.install_source",
                         lambda: "octos-org/review-kit · branch dev" in W.text("dlg_skills_cf_detail")))
    W.shot(f"05-source-confirm-{MODE}")
    W.mark()
    W.click("dlg_skills_cf_confirm_control")
    W.note("== 4b. the Profile lock while the install is in flight (the server answers in 2.5 s)")
    W.check("skills: the lease holds -> the lock line shows",
            W.wait(lambda: bool(W.visible("dlg_skills_skills_locked")), 4))
    W.shot(f"07-locked-{MODE}")
    W.mark()
    r = W.rect("dlg_skills_t_remove0")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.check("skills: a Remove CLICK while locked routes nothing (no confirm asked)",
            r is not None and not W.logged("dialog confirm asked", 1.5) and not W.visible("dlg_skills_cf_detail"))
    W.check("skills: the source installs; the receipt names the skipped skill; the lock lifts",
            W.wait(lambda: "Server installed: review-kit. Skipped: code-linter." in W.text("dlg_skills_dialog_notice"), 10)
            and not W.visible("dlg_skills_skills_locked"))

    W.note("== 5. Remove (row 0) through the confirm card")
    W.check("skills: Remove CLICK -> 'Confirm removal' -> Confirm -> 'Removed code-linter …'",
            click_logged(W, "dlg_skills_t_remove0_hit", "dialog confirm asked: skills.remove_0",
                         lambda: W.text("dlg_skills_cf_detail") == "code-linter")
            and click_logged(W, "dlg_skills_cf_confirm_control", "dialog confirmed: skills.remove_0",
                             lambda: "Removed code-linter from server Profile dsflash." in W.text("dlg_skills_dialog_notice"), 10))
    numeric(W, "after remove")
    W.shot(f"06-removed-{MODE}")
    for method, n in [("profile/skills/registry/search", 1), ("profile/skills/install", 2), ("profile/skills/remove", 1)]:
        got = W.replay_saw(method, 2)
        W.check(f"wire: {method} x{n}", got == n, f"replay log: {got}")
    W.click("dialog_close")
    W.wait_shown("dialog_frame", 5, gone=True)



if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10", replay_args=["--slow", "profile/skills/install=2500"]))
