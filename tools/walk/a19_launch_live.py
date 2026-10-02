#!/usr/bin/env python3
"""A19 — the connect-time launch on a REAL server: a live click walk.

Drives an app that is ALREADY running hidden (harness/headless.sh, isolated
state per brief §8 including OCTOSCODE_CONNECTION_FILE) against a PRIVATE
`octos serve` (never the operator's), and reads the app's protocol trace
(OCTOSCODE_TRACE_FILE) for the wire half. Phases:

  fresh    a server with NO profile, an app state with nothing remembered:
           the first launch sends `launch/resolve` WITHOUT a profile id,
           Core answers `no_profile`, the onboarding panel shows with the
           provider catalog loaded — then the form is filled by clicks with
           NO key typed (the submit stays disabled). Nothing is created or
           opened.
  session  an existing setup (profiles, a model), nothing remembered: the
           launch (still no profile id) opens a Session at once; one prompt
           streams to its end (ONE live model turn).
  restore  the same app state relaunched: the remembered Session is
           restored directly (no `launch/resolve`), its history on screen.
  migrate  an app state whose previous build used this server: the one-time
           migration reopens `<profile>:main`, no `launch/resolve`.

  python3 tools/walk/a19_launch_live.py <phase> <app-port> <out-dir> <trace> [desktop|phone] [prompt-text]

Captures: PNG <= 1400 px and /snap JSON; every label or field carrying a
machine path is painted over in the PNG and rewritten in the JSON
(<scratch>/<worktree>/<home>), and the raw input values are dropped.
"""
from __future__ import annotations

import json
import os
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import a10_lib  # noqa: E402
from a10_lib import checks_line, dialog_checks, inside  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
TITLE = "Create your local coding profile"
PREFIX = ("b3_onb", "b3_title", "b3_close")


# ---------------------------------------------------------------- redaction
def _path_rules() -> list[tuple[str, str]]:
    """Machine paths -> placeholders, longest first (built at run time, so
    this file carries none)."""
    home = os.path.expanduser("~")
    rules = [(str(ROOT / "tmp"), "<scratch>"), (str(ROOT), "<worktree>")]
    for p in (os.environ.get("TMPDIR", ""), "/private/tmp", "/private/var/folders"):
        if p and p != "/":
            rules.append((p.rstrip("/"), "<scratch>"))
    rules.append((home, "<home>"))
    return sorted(rules, key=lambda r: -len(r[0]))


def scrub(text: str) -> str:
    for raw, ph in _path_rules():
        text = text.replace(raw, ph)
    return text


def has_machine_path(text: str) -> bool:
    return any(raw in text for raw, _ in _path_rules() if raw)


def redact(w: a10_lib.Walk, name: str) -> list[str]:
    """Paint over every shown label/field whose text carries a machine path
    (the header's workspace path, the session pane's folder) and rewrite the
    saved /snap. Returns the painted widget ids."""
    png = w.out / f"{name}.png"
    snp = w.out / f"{name}.snap.json"
    sn = json.loads(snp.read_text())
    painted = []
    try:
        from PIL import Image, ImageDraw

        img = Image.open(png).convert("RGB")
        mod = w.module_rect(sn)
        win = next((x["r"] for x in sn if x.get("ty") == "Window" and w.shown(x)), None)
        if w.mode == "desktop" and mod:
            ox, oy, s = mod[0], max(mod[1] - 32, 0), img.width / mod[2]
        elif win:
            ox, oy, s = win[0], win[1], img.width / win[2]
        else:
            ox, oy, s = 0, 0, 1.0
        d = ImageDraw.Draw(img)
        for x in sn:
            t = str(x.get("t") or "")
            if w.shown(x) and x.get("ty") in ("Label", "TextInput") and has_machine_path(t):
                r = x["r"]
                box = ((r[0] - ox) * s - 2, (r[1] - oy) * s - 2, (r[0] + r[2] - ox) * s + 2, (r[1] + r[3] - oy) * s + 2)
                d.rectangle(box, fill=(128, 128, 128))
                painted.append(str(x.get("i")))
        img.save(png)
    except Exception as e:  # never keep an unredacted capture
        png.unlink(missing_ok=True)
        w.note(f"REDACTION FAILED for {name} ({e}): the PNG was dropped")
    for x in sn:
        x.pop("val", None)
    snp.write_text(scrub(json.dumps(sn)))
    if painted:
        w.note(f"redacted {name}: {painted}")
    return painted


def shot(w: a10_lib.Walk, name: str) -> None:
    w.shot(name)
    redact(w, name)


# -------------------------------------------------------------------- trace
def trace(path: str) -> list[dict]:
    p = pathlib.Path(path)
    if not p.exists():
        return []
    out = []
    for line in p.read_text().splitlines():
        try:
            out.append(json.loads(line))
        except ValueError:
            pass
    return out


def frames(rows: list[dict], direction: str, method: str) -> list[dict]:
    return [r.get("body") or {} for r in rows if r.get("dir") == direction and r.get("method") == method]


def keep_trace(src: str, dst: pathlib.Path) -> None:
    """The run's trace beside its captures, machine paths rewritten."""
    p = pathlib.Path(src)
    if p.exists():
        dst.write_text(scrub(p.read_text()))


# ----------------------------------------------------------------------- ux
def ux(w: a10_lib.Walk, name: str) -> dict:
    """A17's numeric checks of the open panel (a10_lib.dialog_checks): labels
    inside the frame, no overlaps, controls >= 28 px, even module gutters."""
    sn = w.snap()
    c = dialog_checks(sn, "b3_dialog", PREFIX, viewport="b3_scroll")
    lv = w.rect("b3_onb_list_scroll", sn=sn)
    if lv and c.get("under28"):
        def cut(wid):
            r = w.rect(wid, sn=sn)
            return bool(r) and (abs(r[1] + r[3] - (lv[1] + lv[3])) <= 1.5 or abs(r[1] - lv[1]) <= 1.5)
        c["under28"] = [i for i in c["under28"] if not cut(i)]
        c["ok"] = not c["outside"] and not c["overlaps"] and not c["under28"]
    mod = w.module_rect(sn)
    fr = c.get("frame")
    extra = []
    if fr and mod:
        gl, gr = fr[0] - mod[0], (mod[0] + mod[2]) - (fr[0] + fr[2])
        gt, gb = fr[1] - mod[1], (mod[1] + mod[3]) - (fr[1] + fr[3])
        extra.append(f"gutters L{gl:.0f} R{gr:.0f} T{gt:.0f} B{gb:.0f}")
        c["in_module"] = gl >= 15.5 and gr >= 15.5 and gt >= -0.5 and gb >= -0.5 and abs(gl - gr) <= 1.5
    for wid in ("b3_onb_submit", "b3_onb_submit_off", "b3_onb_disconnect", "b3_close"):
        r = w.rect(wid, sn=sn)
        if r and fr:
            extra.append(f"{wid} {r[2]:.0f}x{r[3]:.0f}{'' if inside(r, fr) else ' OUTSIDE'}")
            c["controls_in"] = c.get("controls_in", True) and inside(r, fr)
    c["line"] = checks_line(c) + ("; " + "; ".join(extra) if extra else "")
    ok = c.get("ok") and c.get("in_module", True) and c.get("controls_in", True)
    w.check(f"UX {name}: {c['line']}", bool(ok))
    with open(w.out / "ux-checks.txt", "a") as f:
        f.write(f"{name}: {c['line']}\n")
    return c


def pick(w: a10_lib.Walk, which: str, label: str) -> bool:
    """CLICK the select, then CLICK the option reading `label` (scrolled into
    view first, as a person scrolls to it)."""
    w.scroll_into(f"b3_onb_{which}", "b3_scroll")
    w.click(f"b3_onb_{which}")
    if not w.wait_shown("b3_onb_list", 5):
        return w.check(f"the {which} list unfolds", False)
    sn = w.snap()
    opts = sorted(
        (x for x in sn if str(x.get("i", "")).startswith("b3_onb_opt_") and str(x.get("i", "")).endswith("_label")),
        key=lambda x: int(str(x["i"]).split("_")[3]),
    )
    idx = next((int(str(x["i"]).split("_")[3]) for x in opts if (x.get("t") or "") == label), None)
    if idx is None:
        w.check(f"the {which} list offers {label!r}", False, repr([x.get("t") for x in opts][:12]))
        w.click("b3_onb_back" if w.mode == "phone" else f"b3_onb_{which}")
        return False
    vp = "b3_scroll" if w.mode == "phone" else "b3_onb_list_scroll"
    w.scroll_into(f"b3_onb_opt_{idx}", vp)
    w.click(f"b3_onb_opt_{idx}")
    w.wait_shown("b3_onb_list", 4, gone=True)
    w.scroll_into(f"b3_onb_{which}", "b3_scroll")
    val = w.text(f"b3_onb_{which}_value")
    return w.check(f"the {which} field shows {label!r}", val == label or (val.endswith("…") and label.startswith(val[:-1])), repr(val))


# ------------------------------------------------------------------- phases
def phase_fresh(w: a10_lib.Walk, tr: str, _prompt: str) -> None:
    w.check("the onboarding panel opens at the first launch",
            w.wait(lambda: w.text("b3_title") == TITLE, 40), repr(w.text("b3_title")))
    w.check("the provider catalog loads (the form's Provider select)", w.wait_shown("b3_onb_provider", 30))
    time.sleep(0.6)
    sn = w.snap()
    w.check("no Connect form behind the panel", not w.visible("connect_btn", sn=sn))
    w.check("no Session yet (no composer)", w.composer(sn) is None)
    rows = trace(tr)
    caps = frames(rows, "out", "config/capabilities/list")
    probe = frames(rows, "out", "launch/resolve")
    answer = frames(rows, "in", "launch/resolve")
    w.check("wire: the capability read (the web's authenticate)", len(caps) >= 1)
    w.check("wire: launch/resolve carries the folder and NO profile id",
            len(probe) == 1 and "cwd" in probe[0] and "profile_id" not in probe[0], scrub(json.dumps(probe)))
    w.check("wire: Core answered no_profile", len(answer) == 1 and answer[0].get("decision") == "no_profile", json.dumps(answer))
    w.check("wire: the catalog was read (profile/llm/catalog)", len(frames(rows, "out", "profile/llm/catalog")) >= 1)
    for m in ("profile/local/create", "session/open", "profile/llm/test", "profile/llm/upsert", "turn/start"):
        w.check(f"wire: no {m}", not frames(rows, "out", m))
    shot(w, f"01-onboarding-{w.mode}")
    ux(w, "onboarding (live, fresh server)")
    prov = w.text("b3_onb_provider_value")
    w.note(f"the live catalog's default provider: {prov!r}, model {w.text('b3_onb_model_value')!r}")
    # Fill the form by clicks — no key.
    if pick(w, "provider", "deepseek"):
        model = w.text("b3_onb_model_value")
        w.check("the provider's first model is selected", bool(model), repr(model))
        pick(w, "route", "Official API")
    w.check("no key typed: 'Test, save & open' stays disabled",
            not w.visible("b3_onb_submit") and bool(w.visible("b3_onb_submit_off")))
    time.sleep(0.4)
    shot(w, f"02-filled-no-key-{w.mode}")
    ux(w, "filled form, no key (live)")
    rows = trace(tr)
    for m in ("profile/local/create", "session/open", "profile/llm/test", "profile/llm/upsert"):
        w.check(f"wire after filling: still no {m}", not frames(rows, "out", m))


def in_timeline(w: a10_lib.Walk, needle: str) -> bool:
    """The prompt as a user bubble IN the conversation (`i<n>_userbubble_<k>`),
    not the header's or the sidebar's Session title."""
    return any(
        "_userbubble_" in str(x.get("i", "")) and needle in (x.get("t") or "")
        for x in w.snap()
        if w.shown(x)
    )


def wait_live(w: a10_lib.Walk, secs: float = 40) -> bool:
    return w.wait(lambda: w.composer() is not None and not w.visible("b3_dialog"), secs)


def answered(w: a10_lib.Walk) -> bool:
    return any("Worked for" in (x.get("t") or "") for x in w.snap() if w.shown(x))


def send_prompt(w: a10_lib.Walk, text: str) -> None:
    c = w.composer()
    x, y, cw, ch = c["r"]
    w.click_xy(x + cw / 2, y + ch / 2)
    time.sleep(0.4)
    w.type_text(text)
    time.sleep(0.4)
    if not w.click("send_hit"):
        w.key("Return")


def phase_session(w: a10_lib.Walk, tr: str, prompt: str) -> None:
    w.check("straight to a Session (composer, no panel)", wait_live(w), "")
    rows = trace(tr)
    probe = frames(rows, "out", "launch/resolve")
    answer = frames(rows, "in", "launch/resolve")
    opens = frames(rows, "out", "session/open")
    w.check("wire: launch/resolve carries NO profile id", len(probe) == 1 and "profile_id" not in probe[0], scrub(json.dumps(probe)))
    w.check("wire: Core resolved a profile at once (resume/activate)",
            len(answer) == 1 and answer[0].get("decision") in ("resume", "activate") and bool(answer[0].get("resolved_profile")),
            json.dumps(answer))
    w.check("wire: the Session opened under it",
            len(opens) >= 1 and answer and opens[0].get("profile_id") == answer[0].get("resolved_profile"), scrub(json.dumps(opens[:1])))
    w.check("wire: nothing created", not frames(rows, "out", "profile/local/create"))
    shot(w, f"01-session-{w.mode}")
    if prompt:
        send_prompt(w, prompt)
        w.check("the answer streamed to its end ('Worked for')", w.wait(lambda: answered(w), 150, 1.0))
        rows = trace(tr)
        w.check("wire: one turn/start", len(frames(rows, "out", "turn/start")) == 1)
        time.sleep(1.0)
        shot(w, f"02-answer-{w.mode}")


def phase_restore(w: a10_lib.Walk, tr: str, prompt: str) -> None:
    w.check("the relaunch goes straight back to the Session", wait_live(w))
    rows = trace(tr)
    opens = frames(rows, "out", "session/open")
    w.check("wire: no launch/resolve on a restore", not frames(rows, "out", "launch/resolve"))
    want = os.environ.get("A19_EXPECT_SESSION", "")
    w.check("wire: the remembered Session is opened with its profile",
            len(opens) >= 1 and (not want or opens[0].get("session_id") == want) and bool(opens[0].get("profile_id")),
            scrub(json.dumps(opens[:1])))
    w.check("wire: nothing created", not frames(rows, "out", "profile/local/create"))
    if prompt:
        w.check("its history is in the conversation (the earlier prompt's bubble)", w.wait(lambda: in_timeline(w, prompt[:40]), 20))
    shot(w, f"01-restored-{w.mode}")


def phase_restore_turn(w: a10_lib.Walk, tr: str, prompt: str) -> None:
    """A restore, then ONE prompt in the restored Session (its next launch's
    `restore` phase then finds it in the history)."""
    phase_restore(w, tr, "")
    send_prompt(w, prompt)
    w.check("the answer streamed to its end ('Worked for')", w.wait(lambda: answered(w), 150, 1.0))
    w.check("wire: one turn/start", len(frames(trace(tr), "out", "turn/start")) == 1)
    time.sleep(1.0)
    shot(w, f"02-restored-answer-{w.mode}")


def phase_migrate(w: a10_lib.Walk, tr: str, prompt: str) -> None:
    w.check("the first connect after the upgrade lands in a Session", wait_live(w))
    rows = trace(tr)
    opens = frames(rows, "out", "session/open")
    w.check("wire: no launch/resolve (the previous build's landing)", not frames(rows, "out", "launch/resolve"))
    w.check("wire: <profile>:main reopened",
            len(opens) >= 1 and str(opens[0].get("session_id", "")).endswith(":main")
            and opens[0].get("session_id") == f"{opens[0].get('profile_id')}:main", scrub(json.dumps(opens[:1])))
    w.check("wire: nothing created", not frames(rows, "out", "profile/local/create"))
    if prompt:
        # The Session's existing history (a turn an earlier run left in it).
        w.check("its history is in the conversation (the earlier prompt's bubble)", w.wait(lambda: in_timeline(w, prompt[:40]), 20))
    shot(w, f"01-migrated-{w.mode}")


CHROME_PREFIXES = ("hd_", "sb_", "sg_", "b3_strip", "i0_composer", "composer_", "empty_", "set_", "history_")


def transcript_scrolled(w: a10_lib.Walk, top_shot: str | None = None, steps: int = 400) -> set[str]:
    """tools/judge/live_smoke.py's transcript_scrolled: every transcript text
    from the bottom to the top of the conversation (the list lays out only
    the rows in view), then back to the bottom. Chrome texts are left out.
    `top_shot`: a capture once the top is reached. The top is where SIX
    scroll steps in a row change nothing in view: the instrument clips each
    rect to the viewport, so at phone width one paragraph taller than the
    view reads the same for two or three steps, which the judge's two-step
    test took for the top (measured: it stopped mid-answer)."""
    conv = w.composer()
    x = (conv["r"][0] + conv["r"][2] / 2) if conv else 600
    y = max(150, (conv["r"][1] - 220) if conv else 300)
    seen, quiet, last = set(), 0, None
    for _ in range(steps):
        rows = [x_ for x_ in w.snap()
                if w.shown(x_) and (x_.get("t") or "").strip()
                and not str(x_.get("i", "")).startswith(CHROME_PREFIXES)]
        seen |= {x_.get("t") or "" for x_ in rows}
        now = {(x_.get("t") or "", round(x_["r"][1])) for x_ in rows}
        quiet = quiet + 1 if now == last else 0
        if quiet >= 6:
            break
        last = now
        w.get(f"/m?k=scroll&x={x}&y={y}&dy=-350&wait=1")
        time.sleep(0.3)
    if top_shot:
        shot(w, top_shot)
    for _ in range(steps + 10):
        w.get(f"/m?k=scroll&x={x}&y={y}&dy=600&wait=1")
    time.sleep(1.0)
    return seen


def phase_upgrade(w: a10_lib.Walk, tr: str, prompt: str) -> None:
    """A19b — the operator's first launch after the upgrade: nothing
    remembered, the previous build's server in A1's last-server, an existing
    Session WITH history. It must open in its folder at once and show the
    WHOLE history (scroll-read), then the next prompt streams."""
    # The Session's distinct user prompts (a JSON list), read from its copied
    # transcript by the caller.
    expected = json.loads(pathlib.Path(os.environ["A19_EXPECT_FILE"]).read_text()) if os.environ.get("A19_EXPECT_FILE") else []
    # From the first frame the instrument serves until the history is on
    # screen: the empty welcome (an empty timeline) must never show over this
    # Session — only "Loading conversation…" while its history is read.
    welcome = loading = frames_seen = 0
    texts: list[str] = []
    end = time.time() + 60
    while time.time() < end:
        try:
            sn = w.snap()
        except Exception:
            time.sleep(0.1)
            continue
        frames_seen += 1
        welcome += bool(w.visible("empty_title", sn))
        loading += bool(w.visible("history_title", sn))
        texts = [x.get("t") for x in sn if w.shown(x) and (x.get("t") or "").strip()
                 and not str(x.get("i", "")).startswith(CHROME_PREFIXES)]
        if texts:
            break
        time.sleep(0.1)
    w.check("never the empty welcome over the Session, sampled from the first frame until its history showed",
            welcome == 0 and bool(texts), f"{frames_seen} frames: {welcome} welcome, {loading} loading")
    w.check("the first launch after the upgrade lands in a Session", wait_live(w, 60))
    rows = trace(tr)
    opens = frames(rows, "out", "session/open")
    lists = frames(rows, "out", "session/list")
    refused = frames(rows, "in", "error:session/hydrate")
    want = os.environ.get("A19_EXPECT_SESSION", "")
    w.check("wire: the per-workspace catalog read found the Session's folder (session/list {cwd, profile_id})",
            any(str(p.get("cwd", "")).endswith("/ws") and p.get("profile_id") for p in lists), scrub(json.dumps(lists[:6])))
    w.check("wire: the FIRST open is the Session, carrying that folder",
            bool(opens) and opens[0].get("session_id") == want and str(opens[0].get("cwd") or "").endswith("/ws"),
            scrub(json.dumps(opens[:1])))
    w.check("wire: no launch/resolve, nothing created",
            not frames(rows, "out", "launch/resolve") and not frames(rows, "out", "profile/local/create"))
    w.check("wire: no history read refused", not refused, scrub(json.dumps(refused[:2])))
    # The app log (the instrument's /log): the previous build's profile was
    # resolved BEFORE the socket, so the connection carried it (Core finds
    # `<profile>:main` only through the connection's X-Profile-Id).
    try:
        log = [scrub(l.split(" - ", 1)[-1]) for l in json.loads(w.get("/log?n=800")).get("l", [])]
    except Exception:
        log = []
    resolved = [l for l in log if "migration: the previous build's profile is" in l]
    w.check("the migration's profile was resolved before the socket (the connection carries it)", bool(resolved),
            "; ".join(resolved[:1]))
    w.note("app log: " + " | ".join(l for l in log if "[octoscode] migration" in l or "history" in l)[:600])
    w.check("no loading or failure state left on screen", w.wait(lambda: not w.visible("history_title"), 20))
    time.sleep(1.0)
    shot(w, f"01-first-launch-{w.mode}")
    seen = transcript_scrolled(w, top_shot=f"02-history-top-{w.mode}")
    text = "\n".join(seen)
    missing = [e for e in expected if e[:70] not in text]
    w.check(f"the full history by scrolling: all {len(expected)} distinct prompts of the Session",
            bool(expected) and not missing, f"missing {missing}")
    w.note(f"scroll-read {len(seen)} distinct transcript texts")
    if prompt:
        send_prompt(w, prompt)
        # The history already holds finished turns: the NEW turn is judged on
        # the wire — its turn/start, streamed `response` progress, and its
        # `stream_end` (live-gate.sh's verdict).
        def new_turn() -> str:
            starts = frames(trace(tr), "out", "turn/start")
            return str(starts[0].get("turn_id", "")) if starts else ""

        def progress(kind: str) -> bool:
            t = new_turn()
            return bool(t) and any(
                r.get("dir") == "in" and r.get("method") == "progress/updated"
                and (r.get("body") or {}).get("turn_id") == t
                and ((r.get("body") or {}).get("metadata") or {}).get("kind") == kind
                for r in trace(tr))

        w.check("wire: one turn/start for the next prompt", w.wait(lambda: bool(new_turn()), 20)
                and len(frames(trace(tr), "out", "turn/start")) == 1)
        w.check("the answer streams (response progress)", w.wait(lambda: progress("response"), 120, 1.0))
        w.check("the turn reaches its end (stream_end)", w.wait(lambda: progress("stream_end"), 150, 1.0))
        w.check("the composer is idle again", w.wait(lambda: bool(w.visible("composer_send_icon"))
                                                      and not w.visible("composer_stop_icon"), 30))
        time.sleep(1.0)
        shot(w, f"03-next-prompt-{w.mode}")


def open_sidebar(w: a10_lib.Walk) -> None:
    """The phone's sidebar is a drawer: open it when its rows are hidden."""
    if w.mode == "phone" and not w.visible("sb_add_hit"):
        w.wait(lambda: w.click("sidebar_toggle_hit") and w.wait_shown("sb_add_hit", 3), 12)


def phase_sidebar(w: a10_lib.Walk, tr: str, _prompt: str) -> None:
    """A19b — the retry, live: a Session whose id does not name its profile
    (`dsflash:main`) opened from the sidebar on a connection that does not
    carry its profile (a fresh launch: no profile id; launch/resolve decided).
    Core answers its history "unknown session" (measured); the retry finds
    its folder (the per-workspace catalog), re-dials CARRYING the Session's
    profile, the re-dial re-opens it in that folder, and the history shows —
    "Loading conversation…" meanwhile, never the empty welcome."""
    expected = json.loads(pathlib.Path(os.environ["A19_EXPECT_FILE"]).read_text()) if os.environ.get("A19_EXPECT_FILE") else []
    want = os.environ.get("A19_EXPECT_SESSION", "")
    title = os.environ.get("A19_EXPECT_TITLE", "")
    folder = os.environ.get("A19_FOLDER", "ws")
    w.check("the fresh launch is live", wait_live(w, 60))
    w.check("wire: a fresh launch (launch/resolve, no profile id)",
            any("profile_id" not in p for p in frames(trace(tr), "out", "launch/resolve")))
    # + Add workspace: the folder browser over the serve's folder -> pick the
    # Session's folder -> Use this folder (A2 board 1's p4-08 flow).
    open_sidebar(w)
    w.check("+ Add workspace opens the folder browser",
            w.wait(lambda: w.click("sb_add_hit") and w.wait_shown("b1_br_row_t0", 4), 20))
    idx = next((int(str(x["i"])[len("b1_br_row_t"):]) for x in w.snap()
                if str(x.get("i", "")).startswith("b1_br_row_t") and (x.get("t") or "") == folder and w.shown(x)), None)
    if not w.check(f"the browser lists {folder!r}", idx is not None):
        return
    w.click(f"b1_br_row_{idx}")
    w.check("picking it fills the path box", w.wait(lambda: w.text("b1_br_path").endswith("/" + folder), 6))
    w.click("b1_br_use")
    w.check("Use this folder closes the browser", w.wait_shown("b1_title", 10, gone=True))

    def row() -> dict | None:
        open_sidebar(w)
        return next((x for x in w.snap() if x.get("i") == "sb_r_title" and w.shown(x)
                     and (x.get("t") or "").startswith(title)), None)

    w.check("the sidebar lists the Session (the folder's catalog)", w.wait(lambda: row() is not None, 20))
    r = row()
    if r is None:
        return
    n_err = len(frames(trace(tr), "in", "error:session/hydrate"))
    n_open = len(frames(trace(tr), "out", "session/open"))
    x, y, rw, rh = r["r"]
    w.note(f"CLICK the {title!r} row")
    w.click_xy(x + rw / 2, y + rh / 2)
    # From the click until its history is on screen: never the welcome.
    welcome = loading = n = 0
    end = time.time() + 60
    while time.time() < end:
        try:
            sn = w.snap()
        except Exception:
            time.sleep(0.1)
            continue
        n += 1
        welcome += bool(w.visible("empty_title", sn))
        loading += bool(w.visible("history_title", sn))
        if any("_userbubble_" in str(x_.get("i", "")) and w.shown(x_) for x_ in sn):
            break
        time.sleep(0.1)
    w.check("never the empty welcome over it, sampled from the click until its history showed",
            welcome == 0 and n > 0, f"{n} frames: {welcome} welcome, {loading} loading")
    rows = trace(tr)
    opens = [o for o in frames(rows, "out", "session/open")[n_open:] if o.get("session_id") == want]
    refused = frames(rows, "in", "error:session/hydrate")[n_err:]
    w.check("wire: the open carries the Session's folder",
            bool(opens) and str(opens[0].get("cwd") or "").endswith("/" + folder), scrub(json.dumps(opens[:1])))
    w.check("wire: Core refused the history on the connection without its profile ('unknown session')",
            any(e.get("code") == -32100 for e in refused), json.dumps(refused[:1]))
    reopen = [o for o in opens if o.get("reconnect")]
    w.check("wire: the retry re-dialed and re-opened it in its folder",
            bool(reopen) and str(reopen[-1].get("cwd") or "").endswith("/" + folder), scrub(json.dumps(reopen[-1:])))
    w.check("wire: then its history read answered",
            any(h.get("session_id") == want for h in frames(rows, "in", "session/hydrate")))
    try:
        log = [scrub(l.split(" - ", 1)[-1]) for l in json.loads(w.get("/log?n=800")).get("l", [])]
    except Exception:
        log = []
    redial = [l for l in log if "history retry" in l and "re-dialing as" in l]
    w.check("the app re-dialed carrying the Session's profile", bool(redial), "; ".join(redial[:1]))
    w.check("no loading or failure state left on screen", w.wait(lambda: not w.visible("history_title"), 20))
    time.sleep(1.0)
    shot(w, f"01-sidebar-history-{w.mode}")
    seen = transcript_scrolled(w, top_shot=f"02-sidebar-history-top-{w.mode}")
    text = "\n".join(seen)
    missing = [e for e in expected if e[:70] not in text]
    w.check(f"the full history by scrolling: all {len(expected)} distinct prompts of the Session",
            bool(expected) and not missing, f"missing {missing}")
    w.note(f"scroll-read {len(seen)} distinct transcript texts")


PHASES = {
    "fresh": phase_fresh,
    "session": phase_session,
    "restore": phase_restore,
    "restore-turn": phase_restore_turn,
    "migrate": phase_migrate,
    "upgrade": phase_upgrade,
    "sidebar": phase_sidebar,
}


def main() -> int:
    phase, port, out, tr = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
    mode = sys.argv[5] if len(sys.argv) > 5 else "desktop"
    prompt = sys.argv[6] if len(sys.argv) > 6 else ""
    w = a10_lib.Walk(port, out, mode)
    (w.out / "ux-checks.txt").write_text("") if not (w.out / "ux-checks.txt").exists() else None
    if mode == "phone":
        w.wait(lambda: '"sz"' in w.get("/s", tolerant=True), 20)
    PHASES[phase](w, tr, prompt)
    keep_trace(tr, w.out / f"trace-{phase}-{mode}.jsonl")
    rc = w.summary()
    print(f"== A19 live {phase} ({mode}): {'PASS' if rc == 0 else 'FAIL'}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
