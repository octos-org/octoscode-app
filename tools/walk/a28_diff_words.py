#!/usr/bin/env python3
"""A28 — parity row 23 by CLICKS: word-level change marks and per-file
syntax colours in the diff review, the large-preview bound, and the phone
full-screen sheet (design board 4, frames 1 / 1b / 2; web
`features/review/diff-presentation.ts:26-160`, `DiffReviewDialog.tsx`,
`app/styles.css:1615-2030` + the compact block :2155-2200).

Against `replay_serve --scenario surfaces --first-turn 3 --diff-words`
(the SYNTHETIC previews of a28-diff-words-synthetic.jsonl, built by
tools/fixtures/a28_diff_words_fixture.py):

1. a prompt CLICK + Return plays the approvals turn; Deny the command
   approval; the typed DIFF approval's "Review diff" CLICK opens the review
   on ONE diff/preview/get (preview …0f1):
   - steer_queue.rs (Rust): per-token runs `<line>_c<k>_<class>` (kw, fn,
     cn, st, pu, cm, pl; `tx` = no grammar) on context, removed and added
     lines; the two EQUAL 1:1 blocks carry word marks `<line>_w<m>` over the
     web's changed words; the UNEQUAL block (+48, +49) has none;
   - octos.conf (no grammar): plain `tx` runs only, word marks on 500/250;
   - steer.md: a pair sharing < 25 % of its words: no marks;
   - every mark lies inside its row, starts at its words' column and spans
     them (mono 0.6 em), and is drawn in the success 22 % / error 20 % fill
     (sampled from the /g capture); a keyword run is drawn in its colour;
   - every line is whole (its runs join to the fixture's text);
   desktop: the dialog is min(1080, 100%) x min(780, 100%) of the module
   (inside the web's 24 px backdrop inset), centred; phone: a FULL-SCREEN
   sheet without the preview id, each hunk scrolling sideways.
2. the HEADER Review control CLICK re-opens the same preview (one more read).
3. Approve for session -> the next diff approval (preview …0f2: 522 lines
   past the 400-line bound): its Review diff CLICK shows the note "Large
   preview shown as plain text. All lines are included.", NO run, NO mark
   (all or nothing), the line tint and red / green text only, and every
   line (the last one scrolled into view).
4. (A28_LANG=zh) the same review in Chinese: the eyebrow, Refresh and the
   note in the web's / the native catalog's zh, no CJK label clipped.

usage: OCTOSCODE_APP_BIN=<host octosense> a28_diff_words.py <desktop|phone> <outdir>
"""
import json
import os
import pathlib
import re
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)
from a10_lib import Walk, checks_line, dialog_checks, inside, run_session  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a28_diff_words",
    "title": "the diff review's word marks + per-file syntax colours, the large-preview bound and the phone sheet, by clicks",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        35: {"checks": ["words: the equal 1:1 blocks carry the web's word marks",
                        "words: keyword / function / constant runs on context, removed and added lines",
                        "words: every line is whole (its runs join to the fixture text)",
                        "words: the marks are drawn in the success / error fills"],
             "partial": "selection and the dark theme are web-only (the dialog kit is light); the mobile width is walked"},
        36: ["large: the note 'Large preview shown as plain text. All lines are included.'",
             "large: no run and no word mark anywhere (all or nothing)",
             "large: every line is included (all 520 Cargo.lock lines, the last one scrolled into view)"],
        37: ["conf: no grammar -> plain runs only, word marks on 500 / 250"],
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a28/walk-{MODE}"
LANG = os.environ.get("A28_LANG", "en")
VP = "b3_scroll"
ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURE = ROOT / "crates" / "octoscode-client" / "tests" / "fixtures" / "a28-diff-words-synthetic.jsonl"
NOTE = {"en": "Large preview shown as plain text. All lines are included.",
        "zh": "大型预览以纯文本显示，已包含所有行。"}
CW = 11.5 * 0.6  # the mono face's advance at the code size (px)
# The web's own changed words for the fixture (diff-presentation.ts run by
# node on these exact lines; see docs/ux/a28/README.md).
MARKS = {
    "b3_diff_file_0_h0_l2": ["Duration::from_millis", "500"],
    "b3_diff_file_0_h0_l3": ["self.backoff.next", "attempt"],
    "b3_diff_file_0_h0_l5": ["send"],
    "b3_diff_file_0_h0_l6": ["send_with_retry", ", &self.backoff"],
    "b3_diff_file_1_h0_l0": ["500"],
    "b3_diff_file_1_h0_l1": ["250"],
}
NO_MARKS = ["b3_diff_file_0_h0_l0", "b3_diff_file_0_h0_l1", "b3_diff_file_0_h0_l4", "b3_diff_file_0_h0_l7",
            "b3_diff_file_0_h0_l8", "b3_diff_file_0_h0_l9", "b3_diff_file_0_h0_l10", "b3_diff_file_0_h0_l11",
            "b3_diff_file_2_h0_l1", "b3_diff_file_2_h0_l2"]
ADDED_MARK, REMOVED_MARK = (0xbb, 0xea, 0xcb), (0xfa, 0xc1, 0xc1)
RUN_RE = re.compile(r"^(b3_diff_file_\d+_h\d+_l\d+)_(?:w(\d+)_)?c(\d+)_([a-z]+)$")
MARK_RE = re.compile(r"^(b3_diff_file_\d+_h\d+_l\d+)_w(\d+)$")


def fixture(name):
    for line in FIXTURE.read_text().splitlines():
        fr = json.loads(line)
        if fr.get("fixture") == name:
            return fr["body"]["preview"]
    raise SystemExit(f"fixture {name} missing")


WORDS, LARGE = fixture("words"), fixture("large")


# ------------------------------------------------------------------ helpers
def runs_of(sn, lid=None):
    """{line id: [(k, class, widget)]} for every code run in the snap (shown
    or scrolled out: a run's text is reported either way). A run inside a
    word mark is `<line>_w<m>_c<k>_<class>`; its mark is `w["mark"]`."""
    out = {}
    for w in sn:
        m = RUN_RE.match(str(w.get("i", "")))
        if m and (lid is None or m.group(1) == lid):
            w = dict(w, mark=None if m.group(2) is None else int(m.group(2)))
            out.setdefault(m.group(1), []).append((int(m.group(3)), m.group(4), w))
    for v in out.values():
        v.sort(key=lambda t: t[0])
    return out


def id_marks(runs, lid):
    """The words each mark of a line holds, from the run ids (structure)."""
    words = {}
    for _, _, w in runs.get(lid, []):
        if w["mark"] is not None:
            words[w["mark"]] = words.get(w["mark"], "") + (w.get("t") or "")
    return [words[m] for m in sorted(words)]


def reveal(W, wid, step=900, tries=40):
    """Wheel the review body until `wid` lies wholly inside it (downward
    first, then back up): the user's own gesture. Positive dy scrolls down."""
    vp = W.rect(VP)
    if not vp:
        return False
    x, y = vp[0] + 40, vp[1] + vp[3] / 2
    for direction in (1, -1):
        for _ in range(tries):
            sn = W.snap()
            r = W.rect(wid, sn=sn)
            if r and inside(r, vp, tol=0.5):
                return True
            if r:
                # Shown but cut: a small step toward it.
                dy = (r[1] + r[3] - (vp[1] + vp[3]) + 8) if r[1] + r[3] > vp[1] + vp[3] else (r[1] - vp[1] - 8)
            else:
                dy = step * direction
            before = [w["r"] for w in sn if w.get("i") == "b3_diff_file_0" and W.shown(w)]
            W.get(f"/m?k=scroll&x={x:.0f}&y={y:.0f}&dy={dy:.0f}&wait=1", tolerant=True)
            time.sleep(0.15)
            after = [w["r"] for w in W.snap() if w.get("i") == "b3_diff_file_0" and W.shown(w)]
            if r is None and before and after and before == after:
                break  # at the end of the range this way
    return bool(W.rect(wid) and inside(W.rect(wid), vp, tol=0.5))


def marks_of(sn, lid=None):
    out = {}
    for w in sn:
        m = MARK_RE.match(str(w.get("i", "")))
        if m and (lid is None or m.group(1) == lid):
            out.setdefault(m.group(1), []).append((int(m.group(2)), w))
    for v in out.values():
        v.sort(key=lambda t: t[0])
    return out


def line_text(runs):
    return "".join((w.get("t") or "") for _, _, w in runs)


def grab(W):
    """The /g PNG as a PIL image plus the logical->pixel scale."""
    from PIL import Image
    import io
    sn = W.snap()
    data = urllib.request.urlopen(W.base + "/g?raw=1", timeout=30).read()
    img = Image.open(io.BytesIO(data)).convert("RGB")
    win = next((w["r"] for w in sn if w.get("ty") == "Window" and W.shown(w)), None)
    k = img.width / win[2] if win else 2.0
    return img, k, sn


def mode_colour(img, k, r, inset=1.0):
    """The most common pixel colour in a logical rect (a fill's colour)."""
    from collections import Counter
    x0, y0 = int((r[0] + inset) * k), int((r[1] + inset) * k)
    x1, y1 = int((r[0] + r[2] - inset) * k), int((r[1] + r[3] - inset) * k)
    c = Counter()
    for y in range(y0, max(y0 + 1, y1)):
        for x in range(x0, max(x0 + 1, x1)):
            c[img.getpixel((x, y))] += 1
    return c.most_common(1)[0][0] if c else (0, 0, 0)


def darkest(img, k, r):
    x0, y0, x1, y1 = int(r[0] * k), int(r[1] * k), int((r[0] + r[2]) * k), int((r[1] + r[3]) * k)
    best = (255, 255, 255)
    for y in range(y0, max(y0 + 1, y1)):
        for x in range(x0, max(x0 + 1, x1)):
            p = img.getpixel((x, y))
            if sum(p) < sum(best):
                best = p
    return best


def shows(t, full):
    """`t` is `full` or its ellipsized head (a phone title is cut to fit)."""
    t = (t or "").strip()
    return t == full or (t.endswith("…") and len(t) > 4 and full.startswith(t[:-1].rstrip()))


def near(a, b, tol=14):
    return all(abs(x - y) <= tol for x, y in zip(a, b))


def send_prompt(W, text):
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


def close_review(W):
    W.click("b3_close")
    return W.wait_shown("b3_dialog", 6, gone=True)


def geometry(W, name, sn):
    mod = W.module_rect(sn)
    fr = W.rect("b3_dialog", sn=sn)
    if not (mod and fr):
        W.check(f"{name}: the dialog and the module are laid out", False, f"module={mod} dialog={fr}")
        return
    if MODE == "phone":
        W.check(f"{name}: phone = a full-screen sheet over the module",
                abs(fr[0] - mod[0]) <= 1.5 and abs(fr[1] - mod[1]) <= 1.5
                and abs(fr[2] - mod[2]) <= 1.5 and abs(fr[3] - mod[3]) <= 1.5, f"dialog={fr} module={mod}")
        W.check(f"{name}: phone hides the preview id", not W.visible("b3_diff_preview_id", sn),
                repr(W.text("b3_diff_preview_id", sn)))
    else:
        want_w, want_h = min(1080, mod[2] - 48), min(780, mod[3] - 48)
        cx = (fr[0] + fr[2] / 2) - (mod[0] + mod[2] / 2)
        cy = (fr[1] + fr[3] / 2) - (mod[1] + mod[3] / 2)
        W.check(f"{name}: desktop = min(1080, 100%) x min(780, 100%) inside the 24 px inset, centred",
                abs(fr[2] - want_w) <= 2 and abs(fr[3] - want_h) <= 2 and abs(cx) <= 1.5 and abs(cy) <= 1.5,
                f"dialog={fr} want={want_w}x{want_h} module={mod} centre=({cx:.1f},{cy:.1f})")
        W.check(f"{name}: desktop shows the preview id", bool(W.text("b3_diff_preview_id", sn)))
    c = dialog_checks(sn, "b3_dialog", ("b3_diff_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: dialog numeric checks (no clipped label, no overlap, controls >= 28 px)", c["ok"], checks_line(c))


# --------------------------------------------------------------- the walk
def words_checks(W, name):
    sn = W.snap()
    geometry(W, name, sn)
    runs = runs_of(sn)
    marks = marks_of(sn)
    # Every line of the preview is whole: its runs join to the fixture text.
    bad = []
    for fi, f in enumerate(WORDS["files"]):
        for hi, h in enumerate(f["hunks"]):
            for li, l in enumerate(h["lines"]):
                lid = f"b3_diff_file_{fi}_h{hi}_l{li}"
                got = line_text(runs.get(lid, []))
                if got != l["content"]:
                    bad.append(f"{lid}: {got!r} != {l['content']!r}")
    W.check("words: every line is whole (its runs join to the fixture text)", not bad, "; ".join(bad[:3]))
    # Syntax colours per file: the Rust file is tokenised on every kind of line.
    rs = {lid: v for lid, v in runs.items() if lid.startswith("b3_diff_file_0_")}
    classes = lambda lid: {c for _, c, _ in rs.get(lid, [])}
    kinds_ok = ("kw" in classes("b3_diff_file_0_h0_l0") and "fn" in classes("b3_diff_file_0_h0_l0")
                and "kw" in classes("b3_diff_file_0_h0_l2") and "fn" in classes("b3_diff_file_0_h0_l3")
                and "cn" in classes("b3_diff_file_0_h0_l10"))
    first = [(c, w.get("t")) for _, c, w in rs.get("b3_diff_file_0_h0_l0", [])][:6]
    W.check("words: keyword / function / constant runs on context, removed and added lines", kinds_ok, f"l0={first}")
    conf = {c for lid, v in runs.items() if lid.startswith("b3_diff_file_1_") for _, c, _ in v}
    W.check("conf: no grammar -> plain runs only, word marks on 500 / 250",
            conf == {"tx"} and all(id_marks(runs, lid) == MARKS[lid] for lid in ("b3_diff_file_1_h0_l0", "b3_diff_file_1_h0_l1")),
            f"classes={sorted(conf)} l0={id_marks(runs, 'b3_diff_file_1_h0_l0')} l1={id_marks(runs, 'b3_diff_file_1_h0_l1')}")
    got = {lid: id_marks(runs, lid) for lid in MARKS}
    W.check("words: the equal 1:1 blocks carry the web's word marks",
            all(got[lid] == want for lid, want in MARKS.items() if lid.startswith("b3_diff_file_0_"))
            and all(len(marks.get(lid, [])) == len(want) for lid, want in MARKS.items()),
            "; ".join(f"{lid[-6:]}={got[lid]}" for lid in MARKS))
    stray = [lid for lid in NO_MARKS if marks.get(lid) or id_marks(runs, lid)]
    W.check("words: the unequal block (+48, +49), context lines and the < 25 % pair carry no mark", not stray, f"{stray}")
    # Geometry: each drawn mark lies inside its row, starts at its words'
    # column and spans them (mono 0.6 em), and its fill covers its runs.
    geo_bad, seen = [], 0
    for lid in MARKS:
        if not reveal(W, lid):
            geo_bad.append(f"{lid}: could not scroll it into view")
            continue
        sn2 = W.snap()
        runs2, marks2 = runs_of(sn2), marks_of(sn2)
        row = W.rect(lid, sn=sn2)
        rr = runs2.get(lid, [])
        first_run = rr[0][2] if rr else None
        if not (row and first_run and W.shown(first_run)):
            continue
        code_x, text, words = first_run["r"][0], line_text(rr), id_marks(runs2, lid)
        for idx, (m, mw) in enumerate(marks2.get(lid, [])):
            if not W.shown(mw) or mw["r"][0] + mw["r"][2] > row[0] + row[2] - 1:
                continue  # clipped by the hunk's sideways scroller (a phone)
            seen += 1
            word = words[idx] if idx < len(words) else ""
            col = col_of(text, word, idx, None, words)
            want_x, want_w = code_x + col * CW, len(word) * CW
            r = mw["r"]
            covers = all(inside(w["r"], r, tol=1.6) for _, _, w in rr if w["mark"] == m and W.shown(w))
            if not (inside(r, row) and abs(r[0] - want_x) <= 1.6 and abs(r[2] - want_w) <= 1.6 and covers):
                geo_bad.append(f"{lid}_w{m} r={r} want x={want_x:.1f} w={want_w:.1f} row={row} covers={covers}")
    # Desktop draws all nine marks; a phone's hunks clip the later ones
    # until scrolled sideways (they are checked by id above).
    W.check("words: each mark lies in its row on its words' columns (x, width)",
            seen >= (9 if MODE == "desktop" else 3) and not geo_bad, f"marks drawn={seen} " + "; ".join(geo_bad[:3]))
    reveal(W, "b3_diff_file_0_h0_l0")
    # Colour, from the app's own pixels.
    try:
        img, k, sn2 = grab(W)
        marks2, runs2 = marks_of(sn2), runs_of(sn2)
        add = next((w for _, w in marks2.get("b3_diff_file_0_h0_l3", []) if W.shown(w)), None)
        rem = next((w for _, w in marks2.get("b3_diff_file_0_h0_l2", []) if W.shown(w)), None)
        ca = mode_colour(img, k, add["r"]) if add else None
        cr = mode_colour(img, k, rem["r"]) if rem else None
        W.check("words: the marks are drawn in the success / error fills",
                bool(ca and cr and near(ca, ADDED_MARK) and near(cr, REMOVED_MARK)),
                f"added={ca} want {ADDED_MARK}; removed={cr} want {REMOVED_MARK}")
        kw = next((w for _, c, w in runs2.get("b3_diff_file_0_h0_l0", []) if c == "kw" and W.shown(w)), None)
        ink = darkest(img, k, kw["r"]) if kw else None
        W.check("words: a keyword run is drawn in the keyword colour (#b4235a family)",
                bool(ink and ink[0] > ink[1] + 40 and ink[0] > 90), f"fn ink={ink}")
    except Exception as e:  # noqa: BLE001
        W.check("words: the marks are drawn in the success / error fills", False, f"capture: {e}")


def col_of(text, word, idx, ms, words):
    """The column of the idx-th mark's words: the idx-th occurrence search
    after the previous mark's words."""
    pos = 0
    for i, w in enumerate(words):
        at = text.find(w, pos)
        if at < 0:
            return -1
        if i == idx:
            return at
        pos = at + len(w)
    return -1


def mark_words(sn, runs, marks, lid):
    """The words each mark of a line covers: the runs whose rect lies inside
    the mark's rect (a mark is a fill around its runs), in order."""
    out = []
    rr = runs.get(lid, [])
    for _, mw in marks.get(lid, []):
        r = mw["r"]
        if not Walk.shown(mw):
            out.append("?")
            continue
        inner = [w for _, _, w in rr if Walk.shown(w) and inside(w["r"], r, tol=0.6)]
        out.append("".join((w.get("t") or "") for w in inner))
    return out


def phone_sideways(W):
    """Phone: a hunk wider than the sheet scrolls sideways (A13)."""
    # The instrument reports CLIPPED rects, so "wider than the sheet" reads
    # as: the long line's first run is drawn, its last run is clipped away,
    # and every drawn run sits on ONE row (no wrap).
    lid = "b3_diff_file_0_h0_l9"
    sn = W.snap()
    sv = W.rect("b3_diff_file_0_h0_scroll", sn=sn)
    rr = runs_of(sn).get(lid, [])
    shown = [w for _, _, w in rr if W.shown(w)]
    one_row = len({round(w["r"][1]) for w in shown}) == 1
    W.check("phone: the Rust hunk is wider than the sheet (its rows scroll sideways, never wrap)",
            bool(sv and rr and W.shown(rr[0][2]) and not W.shown(rr[-1][2]) and one_row),
            f"scroll={sv} runs={len(rr)} drawn={len(shown)} one_row={one_row}")
    if not (sv and shown):
        return
    run0, last = shown[0], rr[-1][2]
    x0 = run0["r"][0]
    for _ in range(6):
        W.get(f"/m?k=scroll&x={sv[0] + sv[2] / 2:.0f}&y={sv[1] + sv[3] / 2:.0f}&dx=120&wait=1", tolerant=True)
        time.sleep(0.2)
    sn = W.snap()
    by_id = {w.get("i"): w for _, _, w in runs_of(sn).get(lid, [])}
    after, last_after = by_id.get(run0.get("i")), by_id.get(last.get("i"))
    x1 = after["r"][0] if after and W.shown(after) else None
    moved = (x1 is None or x1 < x0 - 20) and bool(last_after and W.shown(last_after))
    W.check("phone: a sideways scroll moves the hunk's lines together", moved,
            f"first run x {x0} -> {x1}; last run drawn={bool(last_after and W.shown(last_after))}")
    W.shot(f"words-scrolled-{MODE}")
    for _ in range(4):
        W.get(f"/m?k=scroll&x={sv[0] + sv[2] / 2:.0f}&y={sv[1] + sv[3] / 2:.0f}&dx=-120&wait=1", tolerant=True)
        time.sleep(0.2)


def large_checks(W, name):
    sn = W.snap()
    geometry(W, name, sn)
    note = W.text("b3_diff_plain_note", sn)
    W.check("large: the note 'Large preview shown as plain text. All lines are included.'", note == NOTE[LANG], repr(note))
    runs, marks = runs_of(sn), marks_of(sn)
    W.check("large: no run and no word mark anywhere (all or nothing)", not runs and not marks,
            f"runs={len(runs)} marks={len(marks)}")
    blocks = sorted((w for w in sn if re.match(r"^b3_diff_file_0_h0_b\d+_code$", str(w.get("i", "")))),
                    key=lambda w: int(re.findall(r"_b(\d+)_", w["i"])[0]))
    text = "\n".join((w.get("t") or "") for w in blocks)
    want = "\n".join(l["content"] for l in LARGE["files"][0]["hunks"][0]["lines"])
    W.check("large: every line is included (all 520 Cargo.lock lines, the last one scrolled into view)",
            text == want and scrolled_to_last(W, blocks), f"lines={text.count(chr(10)) + 1 if text else 0} "
            f"want={want.count(chr(10)) + 1} blocks={len(blocks)}")
    try:
        img, k, sn2 = grab(W)
        rem = next((w for w in sn2 if re.match(r"^b3_diff_file_0_h0_b\d+$", str(w.get("i", "")))
                    and W.shown(w) and near(mode_colour(img, k, w["r"]), (0xfd, 0xec, 0xec), 6)), None)
        code = next((w for w in sn2 if rem and w.get("i") == rem["i"] + "_code"), None)
        ink = darkest(img, k, code["r"]) if code else None
        W.check("large: the removed line keeps its tint and red text (no syntax colour)",
                bool(rem and ink and ink[0] > ink[1] + 60), f"block={rem and rem['i']} ink={ink}")
    except Exception as e:  # noqa: BLE001
        W.check("large: the removed line keeps its tint and red text (no syntax colour)", False, f"capture: {e}")


def scrolled_to_last(W, blocks):
    if not blocks:
        return False
    last = blocks[-1]["i"]
    ok = reveal(W, last, step=1500, tries=30)
    W.shot(f"large-end-{MODE}")
    reveal(W, "b3_diff_plain_note", step=1500, tries=30)
    return ok


def walk(W: Walk) -> None:
    W.note(f"== A28 diff review ({MODE}, {LANG})")
    W.check("turn: a prompt CLICK + Return -> turn/start (the approvals turn)", send_prompt(W, "retry the steer queue"))
    W.check("approval: the command approval takes the composer over", W.wait(lambda: bool(W.visible("cv_ap_card")), 25))
    W.click("cv_ap_deny")
    W.check("approval: Deny -> the typed DIFF approval with 'Review diff'",
            W.wait(lambda: shows(W.text("cv_ap_title"), "Apply a patch to steer_queue.rs, octos.conf and steer.md")
                   and bool(W.visible("cv_ap_diff")), 10), repr(W.text("cv_ap_title")))
    W.check("words: 'Review diff' CLICK opens the review on ONE diff/preview/get",
            W.click("cv_ap_diff") and W.wait(lambda: shows(W.text("b3_title"), WORDS["title"]), 10)
            and W.replay_saw("diff/preview/get", 4) == 1, f"title={W.text('b3_title')!r}")
    W.wait(lambda: bool(runs_of(W.snap())), 6)
    time.sleep(0.8)
    W.shot(f"words-{MODE}")
    words_checks(W, "words")
    if MODE == "phone":
        phone_sideways(W)
    W.check("words: the close CLICK returns to the approval card", close_review(W)
            and W.wait(lambda: bool(W.visible("cv_ap_session")), 6))

    W.note("== the header Review control re-opens the announced preview")
    ok = W.click("review_open_hit") and W.wait(lambda: shows(W.text("b3_title"), WORDS["title"]), 10)
    W.check("header: Review CLICK re-opens the same preview (one more read)",
            ok and W.replay_saw("diff/preview/get", 0) == 2)
    W.wait(lambda: bool(marks_of(W.snap())), 6)
    sn = W.snap()
    got = id_marks(runs_of(sn), "b3_diff_file_0_h0_l3")
    W.check("header: the re-opened review carries the same marks", got == MARKS["b3_diff_file_0_h0_l3"], f"{got}")
    close_review(W)

    W.note("== the large preview: past the bound, nothing is decorated")
    W.click("cv_ap_session")
    W.check("approval: Approve for session -> the large diff approval",
            W.wait(lambda: shows(W.text("cv_ap_title"), "Apply a patch to Cargo.lock and Cargo.toml")
                   and bool(W.visible("cv_ap_diff")), 12), repr(W.text("cv_ap_title")))
    W.check("large: 'Review diff' CLICK opens the large preview",
            W.click("cv_ap_diff") and W.wait(lambda: shows(W.text("b3_title"), LARGE["title"]), 10)
            and W.replay_saw("diff/preview/get", 4) == 3, f"title={W.text('b3_title')!r}")
    W.wait(lambda: bool(W.visible("b3_diff_plain_note")), 8)
    time.sleep(0.8)
    W.shot(f"large-{MODE}")
    large_checks(W, "large")
    close_review(W)
    W.click("cv_ap_once")
    time.sleep(0.5)


if __name__ == "__main__":
    env = {}
    if LANG == "zh":
        prefs = pathlib.Path(OUT).resolve() / "display-v1.json"
        prefs.parent.mkdir(parents=True, exist_ok=True)
        prefs.write_text(json.dumps({"version": 1, "theme": "terminal", "language": "zh", "vimMode": False}))
        env["OCTOSCODE_DISPLAY_PREFS_PATH"] = str(prefs)
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="surfaces",
                         replay_args=["--first-turn", "3", "--diff-words"], env=env))
