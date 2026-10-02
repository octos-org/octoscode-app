#!/usr/bin/env python3
"""A31 — parity row 15: the Skills dialog's "Background jobs" section, by
CLICKS, desktop and phone (board 4 region 3, README "Row 15").

Against `replay_serve --scenario skill-jobs` (the jobs synthetic, in
octos-cli's own shapes; no model is called):

  A31_PHASE=seeded   : the server advertises `skill.action_jobs.v1`. The
                       /skills palette row CLICK opens Skills; its on-open
                       read `skill/action/job/list` names the dialog's Profile
                       and Session; the server sends two NEWER updates before
                       the (stale) reply. The section shows every status
                       (queued, running, succeeded, failed, cancelled,
                       abandoned) newest first with the board's chips and
                       message lines and the active count; another Session's
                       and another Profile's jobs never show. A live update
                       (the trigger file) changes two rows while the dialog
                       is open. Then the sidebar row of the other Session
                       CLICK + /skills shows that Session's own jobs only.
  A31_PHASE=announced: the open withdraws the feature: no list is read; the
                       section shows the jobs announced since connecting and
                       the muted note; a live update still applies.
  A31_PHASE=zh       : seeded, the interface in Chinese (the saved display
                       preference): the section and the dialog read Chinese.

Unset A31_PHASE runs the three in turn.
usage: A10_PORT=<app> A10_REPLAY_PORT=<replay> OCTOSCODE_APP_BIN=<host octosense> a31_skill_jobs.py <desktop|phone> [outdir]
"""
import json
import os
import pathlib
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
try:
    import bridgeauth  # noqa: F401  (D10c: the bridge token on every request)
except ImportError:  # an older checkout without the token
    pass
from a10_lib import Walk, dialog_checks, checks_line, inside, overlap, run_session  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a31_skill_jobs",
    "title": "Skills > Background jobs (row 15): the /skills CLICK reads skill/action/job/list for the dialog's Profile + "
             "Session; every status newest first with its chip, message and the active count; a stale list never "
             "regresses a newer update; a live update; another Session's / Profile's jobs never show; without "
             "skill.action_jobs.v1 only announced jobs with a note; Chinese",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A31_PHASE": "seeded"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A31_PHASE": "announced"}},
        {"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "A31_PHASE": "zh"}},
    ],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a31/{MODE}"
PHASE = os.environ.get("A31_PHASE", "")
VP = "dialog_scroll"
P = "dlg_skills_"
HOME, OTHER, PROFILE = "dsflash:main", "dsflash:imports", "dsflash"
NOTE = "Only jobs announced since the app connected are shown; this server doesn't list earlier jobs."

SEEDED = [
    # name, chip, message, cause
    ("report.pdf", "● Running", None, None),
    ("notes.md", "○ Queued", None, None),
    ("q3-review.pdf", "✓ Done", "Imported 14 pages", None),
    ("scan-007.pdf", "✕ Failed", "Couldn't finish this job.", "pdftotext exited with status 1"),
    ("q3-board.pptx", "✕ Stopped", None, None),
    ("archive.zip", "✕ Stopped", "The server restarted before this job finished.", None),
]
ZH_CHIPS = ["● 运行中", "○ 排队中", "✓ 已完成", "✕ 已失败", "✕ 已停止", "✕ 已停止"]
FOREIGN = ["invoice.pdf", "secret.pdf"]


# The replay server's live-transition trigger (set per phase by run_phase).
TRIGGER = pathlib.Path(OUT).resolve() / "live.trigger"


def list_requests(W: Walk) -> list:
    """Every `skill/action/job/list` the replay server answered: its params."""
    if not W.replay_log or not W.replay_log.exists():
        return []
    out = []
    for line in W.replay_log.read_text().splitlines():
        m = re.search(r"-> skill/action/job/list (\S+) \((\d+) jobs\) (\{.*\})\s*$", line)
        if m:
            try:
                out.append(json.loads(m.group(3)))
            except ValueError:
                pass
    return out


def race_order(W: Walk) -> bool:
    """The replay sent both newer updates BEFORE the home list's reply."""
    lines = W.replay_log.read_text().splitlines() if W.replay_log and W.replay_log.exists() else []
    racing = [i for i, l in enumerate(lines) if "(before the list reply)" in l]
    sent = [i for i, l in enumerate(lines) if "skill/action/job/list reply sent" in l]
    return len(racing) == 2 and bool(sent) and max(racing) < sent[0]


def row_ids(i: int) -> dict:
    b = f"{P}job_{i}"
    return {k: f"{b}{s}" for k, s in [("row", ""), ("skill", "_skill"), ("action", "_action"), ("name", "_name"),
                                       ("chip", "_state_chip"), ("state", "_state"), ("time", "_time"),
                                       ("msg", "_msg"), ("cause", "_cause")]}


def read_rows(W: Walk, n: int) -> list:
    """Rows 0..n-1, each scrolled into the dialog's viewport: its texts and
    the row-level geometry checks (the row's labels inside the row, the name
    clear of the chip, the chip clear of the time, nothing cut)."""
    rows = []
    for i in range(n):
        ids = row_ids(i)
        if not W.visible(ids["name"]):
            W.scroll_into(ids["row"], VP)
        sn = W.snap()
        rec = {k: W.text(v, sn=sn) for k, v in ids.items() if k not in ("row", "chip")}
        rr, nr, cr, tr = (W.rect(ids[k], sn=sn) for k in ("row", "name", "chip", "time"))
        problems = []
        if rr and nr and cr and tr:
            for k in ("skill", "action", "name", "chip", "time", "msg", "cause"):
                r = W.rect(ids[k], sn=sn)
                if r and not inside(r, rr):
                    problems.append(f"{k} outside its row")
            if overlap(nr, cr):
                problems.append("name overlaps the chip")
            if cr[0] + cr[2] > tr[0]:
                problems.append("chip overlaps the time")
        else:
            problems.append(f"row {i} not laid out: row={rr} name={nr} chip={cr} time={tr}")
        if "…" in rec["name"]:
            problems.append("name cut")
        rec["chip_r"] = (cr[0] + cr[2]) if cr else None
        rec["time_r"] = (tr[0] + tr[2]) if tr else None
        rec["problems"] = problems
        rows.append(rec)
    return rows


def numeric(W: Walk, name: str) -> dict:
    """The dialog's numeric UX checks (a10_lib: labels inside the frame, no
    overlaps, controls >= 28 px) plus the section's: the heading and its
    count on one band, the count flush with the card's right edge, the
    heading flush with the card's left edge."""
    W.wait(lambda: bool(W.visible("dialog_frame")), 6)
    sn = W.snap()
    c = dialog_checks(sn, "dialog_frame", (P,), viewport=VP)
    head, count, card = W.rect(f"{P}jobs_head", sn=sn), W.rect(f"{P}jobs_count", sn=sn), W.rect(f"{P}card_jobs", sn=sn)
    extra = []
    if head and card and abs(head[0] - card[0]) > 1.5:
        extra.append(f"heading x {head[0]} vs card x {card[0]}")
    if count and card and abs((count[0] + count[2]) - (card[0] + card[2])) > 1.5:
        extra.append(f"count right {count[0] + count[2]} vs card right {card[0] + card[2]}")
    if head and count and abs((head[1] + head[3] / 2) - (count[1] + count[3] / 2)) > 2.5:
        extra.append("count off the heading's band")
    ok = c["ok"] and not extra
    W.check(f"{name}: numeric checks", ok, checks_line(c) + (f"; {extra}" if extra else ""))
    return c


def open_skills(W: Walk, tag: str) -> bool:
    W.mark()
    ok = W.palette_run("ski", "/skills") and W.wait_shown(f"{P}jobs_head", 10)
    W.check(f"{tag}: the /skills palette row CLICK opens Skills with its Background jobs", ok)
    return ok


def close_dialog(W: Walk) -> None:
    W.click("dialog_close")
    W.wait_shown("dialog_frame", 5, gone=True)


def dialog_texts(W: Walk) -> list:
    return [(w.get("t") or "") for w in W.snap() if str(w.get("i", "")).startswith(P) and W.shown(w)]


def all_dialog_text(W: Walk, n: int) -> str:
    """Every text the dialog draws: each of its `n` job rows scrolled into
    view, then Installed."""
    seen = set(dialog_texts(W))
    for i in range(n):
        W.scroll_into(f"{P}job_{i}_name", VP)
        seen |= set(dialog_texts(W))
    W.scroll_into(f"{P}t_inst_head", VP)
    seen |= set(dialog_texts(W))
    W.scroll_into(f"{P}jobs_head", VP)
    return "\n".join(sorted(seen))


def seeded(W: Walk, zh: bool = False) -> None:
    tag = "zh" if zh else "seeded"
    W.note(f"== {tag} 1. /skills: the dialog reads its jobs for its Profile + Session")
    open_skills(W, tag)
    W.check(f"{tag}: the host ran the on-open job read (log)", W.logged("dialog load dialog.refresh.skill_jobs", 8))
    W.check(f"{tag}: wire: skill/action/job/list names the dialog's Profile and Session",
            W.wait(lambda: {"profile_id": PROFILE, "session_id": HOME} in list_requests(W), 6), f"{list_requests(W)}")
    W.check(f"{tag}: wire: the server sent two newer updates BEFORE the list reply (the race)", W.wait(lambda: race_order(W), 6))
    chips = ZH_CHIPS if zh else [c for _, c, _, _ in SEEDED]
    W.check(f"{tag}: the rows arrive (report.pdf first, q3-review Done)",
            W.wait(lambda: W.text(f"{P}job_0_name") == "report.pdf" and W.text(f"{P}job_2_state") == chips[2], 10),
            f"{W.text(P + 'job_0_name')!r} {W.text(P + 'job_2_state')!r}")
    time.sleep(0.6)
    head = "后台作业" if zh else "Background jobs"
    count = "1 个运行中 · 1 个排队中" if zh else "1 running · 1 queued"
    W.check(f"{tag}: the heading and the active count", W.text(f"{P}jobs_head") == head and W.text(f"{P}jobs_count") == count,
            f"{W.text(P + 'jobs_head')!r} · {W.text(P + 'jobs_count')!r}")
    if zh:
        W.check("zh: the dialog title is the web's 配置档案技能", W.text(f"{P}t_title") == "配置档案技能", repr(W.text(f"{P}t_title")))
    W.check(f"{tag}: no note (this server seeds the section)", not W.visible(f"{P}jobs_note"))
    sn = W.snap()
    card, inst = W.rect(f"{P}card_jobs", sn=sn), W.rect(f"{P}t_inst_head", sn=sn)
    W.check(f"{tag}: the section is at the top, above Installed", bool(card) and (inst is None or inst[1] > card[1] + card[3]),
            f"card={card} installed={inst}")
    numeric(W, f"{tag} open")
    W.shot(f"{'07-zh' if zh else '01-seeded'}-{MODE}")
    rows = read_rows(W, len(SEEDED))
    names = [r["name"] for r in rows]
    W.check(f"{tag}: newest first", names == [n for n, _, _, _ in SEEDED], f"{names}")
    W.check(f"{tag}: every status's chip: queued, running, succeeded, failed, cancelled, abandoned",
            [r["state"] for r in rows] == chips, f"{[r['state'] for r in rows]}")
    if not zh:
        msgs = [(r["msg"] or None, r["cause"] or None) for r in rows]
        W.check("seeded: Done shows the job's output", msgs[2] == ("Imported 14 pages", None), f"{msgs[2]}")
        W.check("seeded: Failed shows the lead, then the error as a muted cause",
                msgs[3] == ("Couldn't finish this job.", "pdftotext exited with status 1"), f"{msgs[3]}")
        W.check("seeded: abandoned reads Stopped with the restart cause",
                msgs[5] == ("The server restarted before this job finished.", None), f"{msgs[5]}")
        W.check("seeded: queued, running and cancelled rows carry no message", all(msgs[i] == (None, None) for i in (0, 1, 4)),
                f"{[msgs[i] for i in (0, 1, 4)]}")
        W.check("seeded: the skill and action line (mono)", rows[0]["skill"] == "source-skill" and rows[0]["action"] == "· source.import"
                and rows[4]["skill"] == "deck-skill" and rows[4]["action"] == "· deck.render", f"{rows[0]['skill']} {rows[0]['action']}")
        W.check("seeded: the times (since each job's last change)", [r["time"] for r in rows] == ["now", "now", "2m", "5m", "20m", "1h"],
                f"{[r['time'] for r in rows]}")
        W.check("race: the stale list (q3-review still running there) did not regress the newer update", rows[2]["state"] == "✓ Done")
        W.check("race: notes.md, announced before the reply and absent from it, survived it", rows[1]["name"] == "notes.md")
    else:
        W.check("zh: the failure lead reads Chinese", rows[3]["msg"] == "无法完成此作业。", repr(rows[3]["msg"]))
        W.check("zh: the abandoned cause reads Chinese", rows[5]["msg"] == "服务器在此作业完成前已重启。", repr(rows[5]["msg"]))
    problems = [(i, r["problems"]) for i, r in enumerate(rows) if r["problems"]]
    W.check(f"{tag}: every row's labels inside the row; name clear of the chip, chip clear of the time; no cut name",
            not problems, f"{problems}")
    chip_r = {round(r["chip_r"]) for r in rows if r["chip_r"]}
    time_r = {round(r["time_r"]) for r in rows if r["time_r"]}
    W.check(f"{tag}: the chips and the times form two right-aligned columns", len(chip_r) == 1 and len(time_r) == 1,
            f"chip right edges {sorted(chip_r)}, time right edges {sorted(time_r)}")
    text = all_dialog_text(W, len(SEEDED))
    W.check(f"{tag}: another Session's job and another Profile's never show", not any(f in text for f in FOREIGN),
            f"{[f for f in FOREIGN if f in text]}")
    if zh:
        return
    W.scroll_into(f"{P}t_inst_head", VP)
    numeric(W, "seeded scrolled to Installed")
    W.check("seeded: Installed follows with the Profile's two skills",
            W.text(f"{P}t_name3") == "source-skill" and W.text(f"{P}t_name4") == "deck-skill",
            f"{W.text(P + 't_name3')!r} {W.text(P + 't_name4')!r}")
    W.shot(f"02-seeded-installed-{MODE}")
    W.scroll_into(f"{P}jobs_head", VP)

    W.note("== seeded 2. a live update while the dialog is open")
    W.mark()
    TRIGGER.write_text("go")
    ok = W.wait(lambda: W.text(f"{P}job_0_name") == "notes.md" and W.text(f"{P}job_0_state") == "● Running"
                and W.text(f"{P}job_1_name") == "report.pdf" and W.text(f"{P}job_1_state") == "✓ Done", 10)
    W.check("live: notes.md starts and report.pdf finishes, without reopening", ok,
            f"{W.text(P + 'job_0_name')} {W.text(P + 'job_0_state')} / {W.text(P + 'job_1_name')} {W.text(P + 'job_1_state')}")
    W.check("live: the finished job shows its output", W.text(f"{P}job_1_msg") == "Imported 9 pages", repr(W.text(f"{P}job_1_msg")))
    W.check("live: the count follows (1 running)", W.text(f"{P}jobs_count") == "1 running", repr(W.text(f"{P}jobs_count")))
    numeric(W, "live")
    W.shot(f"03-live-{MODE}")
    close_dialog(W)

    W.note("== seeded 3. the Session scope: the other Session's row CLICK, /skills again")
    if MODE == "phone" and not W.visible("sb_new_chat_hit"):
        W.click("sidebar_toggle_hit")
        W.wait(lambda: bool(W.visible("sb_new_chat_hit")), 4)
    row = next((w for w in W.snap() if w.get("i") == "sb_r_title" and W.shown(w) and (w.get("t") or "").startswith("Import the Q3")), None)
    if row:
        x, y, w_, h = row["r"]
        W.note(f"CLICK sb_r_title 'Import the Q3 reports' r={row['r']}")
        W.click_xy(x + w_ / 2, y + h / 2)
    W.check("scope: the other Session's sidebar row CLICK switches to it",
            row is not None and W.wait(lambda: W.text("hd_title") == "Import the Q3 reports", 10), repr(W.text("hd_title")))
    if MODE == "phone" and W.visible("drawer_close"):
        W.click("drawer_close")
        time.sleep(0.6)
    open_skills(W, "scope")
    W.check("scope: wire: the list names the other Session",
            W.wait(lambda: {"profile_id": PROFILE, "session_id": OTHER} in list_requests(W), 6), f"{list_requests(W)}")
    W.check("scope: the other Session's dialog shows its own jobs (invoice.pdf running, statement.pdf done)",
            W.wait(lambda: W.text(f"{P}job_0_name") == "invoice.pdf" and W.text(f"{P}job_1_name") == "statement.pdf", 10)
            and W.text(f"{P}job_0_state") == "● Running" and W.text(f"{P}job_1_state") == "✓ Done",
            f"{W.text(P + 'job_0_name')!r} {W.text(P + 'job_1_name')!r}")
    text = all_dialog_text(W, 2)
    home = [n for n, _, _, _ in SEEDED] + ["secret.pdf"]
    W.check("scope: none of the first Session's jobs (nor the other Profile's) show", not any(n in text for n in home),
            f"{[n for n in home if n in text]}")
    W.check("scope: the count is this Session's (1 running)", W.text(f"{P}jobs_count") == "1 running", repr(W.text(f"{P}jobs_count")))
    numeric(W, "other Session")
    W.shot(f"04-other-session-{MODE}")
    close_dialog(W)


def announced(W: Walk) -> None:
    W.note("== announced 1. no skill.action_jobs.v1: the jobs announced since connecting, and the note")
    time.sleep(1.0)  # the announcements follow the open
    open_skills(W, "announced")
    W.check("announced: the rows are the announced jobs, newest first",
            W.wait(lambda: W.text(f"{P}job_0_name") == "report.pdf" and W.text(f"{P}job_1_name") == "scan-007.pdf", 10),
            f"{W.text(P + 'job_0_name')!r} {W.text(P + 'job_1_name')!r}")
    W.check("announced: the muted note says only announced jobs show", W.text(f"{P}jobs_note") == NOTE, repr(W.text(f"{P}jobs_note")))
    time.sleep(2.0)
    W.check("announced: wire: no skill/action/job/list is read", not list_requests(W), f"{list_requests(W)}")
    W.check("announced: chips and count", W.text(f"{P}job_0_state") == "● Running" and W.text(f"{P}job_1_state") == "✕ Failed"
            and W.text(f"{P}jobs_count") == "1 running", f"{W.text(P + 'job_0_state')} {W.text(P + 'job_1_state')} {W.text(P + 'jobs_count')}")
    text = all_dialog_text(W, 2)
    W.check("announced: another Session's job and another Profile's never show", not any(f in text for f in FOREIGN),
            f"{[f for f in FOREIGN if f in text]}")
    numeric(W, "announced")
    W.shot(f"05-announced-{MODE}")
    W.note("== announced 2. a live update still applies")
    TRIGGER.write_text("go")
    W.check("announced: live: notes.md starts, report.pdf finishes",
            W.wait(lambda: W.text(f"{P}job_0_name") == "notes.md" and W.text(f"{P}job_1_state") == "✓ Done", 10),
            f"{W.text(P + 'job_0_name')!r} {W.text(P + 'job_1_state')!r}")
    numeric(W, "announced live")
    W.shot(f"06-announced-live-{MODE}")
    close_dialog(W)


def run_phase(phase: str) -> int:
    out = pathlib.Path(OUT)
    out.mkdir(parents=True, exist_ok=True)
    trig = TRIGGER
    if trig.exists():
        trig.unlink()
    args = ["--jobs-trigger", str(trig)]
    env = {}
    if phase == "announced":
        args += ["--drop-feature", "skill.action_jobs.v1", "--drop-method", "skill/action/job/list",
                 "--drop-method", "skill/action/job/read"]
    if phase == "zh":
        prefs = (out / "display-zh.json").resolve()
        prefs.write_text(json.dumps({"version": 1, "theme": "terminal", "language": "zh", "vimMode": False}))
        env["OCTOSCODE_DISPLAY_PREFS_PATH"] = str(prefs)
    fn = {"seeded": seeded, "announced": announced, "zh": lambda W: seeded(W, zh=True)}[phase]
    sub = out / phase
    try:
        return run_session(fn, mode=MODE, outdir=str(sub), scenario="skill-jobs", replay_args=args, env=env)
    finally:
        for f in (trig, out / "display-zh.json"):
            if f.exists():
                f.unlink()


if __name__ == "__main__":
    phases = [PHASE] if PHASE else ["seeded", "announced", "zh"]
    rc = 0
    for ph in phases:
        rc |= run_phase(ph)
    sys.exit(rc)
