#!/usr/bin/env python3
"""Card #18: render each L0 component's short/long variants, hidden, at 360 and 540.

For every component under `design/components/<id>/` this copies the component,
applies a variant transform to its `mapped.json` tree (text + the responsive
fill flags), re-compiles it with the flow's OWN compiler
(`flows/image-lib/compile.py::compile_page`), renders it hidden with
`beauty-host` at 360 and 540, and writes:

  design/components/<id>/variants/<variant>-<w>.png   the raw native render
  design/components/<id>/review-<variant>.png         atlas crop | native@360 | native@540

`fillw`/`fith` are carried in the node's *style*, which the compiler lowers to the
kit pack and `design.rs` reads as `a.fillw`/`a.fith`; hand-editing the kit would
bypass the one code path production uses.

Run:  python3 tools/render_variants.py [component ...]
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]          # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
COMPONENTS = ROOT / "design/components"
WORK = ROOT / "tmp/stage-b/render-variants-18"
ART_PORT = 8180                                     # 8179 is another lane's art server
PORTS = [8346, 8347, 8348, 8349]                    # this card's block (8340-8349)
WIDTHS = (360, 540)
ROOT_GUTTER_X = 16   # logical page gutter each side (card #18c item 1)
ROOT_GUTTER_Y = 8

def _btn(*ids):
    """A KitButton is a `stack` wrapping `_surface` (the rounded pill), `_control`
    (the hit area) and `_label`. `#18b`: the pill stayed at its measured width
    inside a slot-filling button, so the fill stopped short at 540 and the pill
    clipped at 360. The container + surface + control fill the slot.

    The LABEL deliberately keeps its measured box: bisected on this renderer, a
    free-standing `text` node carrying ANY responsive flag (`fillw`/`fitw`/`fith`)
    paints nothing (deny label ink 301 -> 0), so filling the label would erase it.
    At the reference width the label's measured x is already exact."""
    d = {}
    for i in ids:
        d[i] = {"fillw": 1}
        d[i + "_surface"] = {"fillw": 1}
        d[i + "_control"] = {"fillw": 1, "fillh": 1}
    return d


# Responsive flags (card #18/#18b: fill the slot width).
# RULE (bisected, not guessed): `fillw` is set on CONTAINERS and on a KitButton's
# `_surface`/`_control`/`_label`. It is NOT set on a free-standing `text` node: a
# responsive text emitted with `margin` + `width: Fill` inside an Overlay parent
# paints nothing on this renderer, whereas a non-fillw text keeps `abs_pos` and
# paints. Free text therefore keeps its authored box and its natural wrapping.
RESPONSIVE = {
    "approval-card": {"approval_card": {"fillw": 1, "fith": 1},
                      "cmd_box": {"fillw": 1},
                      **_btn("approve_once", "approve_session", "deny")},
    "question-card": {"question_card": {"fillw": 1, "fith": 1},
                      "note_box": {"fillw": 1}, "note_input": {"fillw": 1},
                      "opt_ledger": {"fillw": 1}, "opt_memory": {"fillw": 1},
                      "opt_ask": {"fillw": 1},
                      **_btn("submit_answer", "skip")},
    "edited-files-card": {"edited_files_card": {"fillw": 1, "fith": 1},
                          "files_card": {"fillw": 1},
                          "div_1": {"fillw": 1}, "div_2": {"fillw": 1},
                          **_btn("review")},
    "plan-card": {"plan_card": {"fillw": 1, "fith": 1},
                  "plan_steps": {"fillw": 1}},
    # The strip is a ROW: the goal text fills and the two trailing controls are
    # pushed to the slot's right edge (they were pinned at their measured x, so at
    # 540 they sat in the middle). In a flow container the children are emitted
    # `in_flow` (no `abs_pos`/`margin`), so a fillw text paints here.
    "goal-strip": {"goal_strip": {"fillw": 1, "fith": 1, "variant": "row"},
                   "t01": {"fillw": 1}},
    "diff-view": {"diff_view": {"fillw": 1, "fith": 1},
                  "diff_rows": {"fillw": 1},
                  "file_header": {"fillw": 1}, "scope_pill": {"fillw": 1},
                  "row_1": {"fillw": 1}, "row_2": {"fillw": 1},
                  "row_3": {"fillw": 1}, "row_4": {"fillw": 1},
                  "row_5": {"fillw": 1}, "row_6": {"fillw": 1},
                  "folded": {"fillw": 1}},
    "settings-group": {"settings_group": {"fillw": 1, "fith": 1},
                       "perm_card": {"fillw": 1}, "model_card": {"fillw": 1},
                       "perm_divider": {"fillw": 1},
                       # t01 is the section title — FIXED CHROME, not a fill region.
                       # A `text` node with `fillw` is emitted with a `margin`
                       # (design.rs), and makepad pins the glyphs to the box bottom,
                       # clipping the title to a 9px band (probe: `fillw` 9px vs
                       # 27px un-flagged). Give it a WIDER MEASURED box instead: it
                       # keeps `abs_pos` (correct vertical anchor, full glyph height)
                       # and has room for the long variant "Permissions and defaults".
                       "t01": {"w": 340}},
}

VARIANTS = {
    "approval-card": {
        "short": {"text": {"t02": "git push origin feat/steer-queue",
                           "reason_text": "Reason: Push the fix branch so CI can run"}},
        "long": {
            # Card #18c item 4: (a) keep the "Reason:" prefix; (b) make the long
            # command WRAP in the mono box (drop `single_line` + give the box room);
            # (c) grow the card AND push every action down by the reason's extra
            # height, so the taller runtime reason never runs under the Approve
            # button (measured overlap: reason bottom 331 vs button top 300).
            "text": {"t02": "cargo test -p octos-cli steer_queue -- --nocapture",
                     "reason_text": "Reason: Run the full steer-queue integration suite "
                                    "before pushing so a regression in the durable queue "
                                    "is caught locally rather than in CI."},
            "flags": {"approval_card": {"h": 704},
                      "t02": {"variant": None, "h": 56},
                      "cmd_box": {"h": 112},
                      "reason_text": {"y": 210, "h": 130},
                      "approve_once": {"y": 352},
                      "approve_session": {"y": 435},
                      "deny": {"y": 518},
                      "t_hint": {"y": 618}}},
    },
    "question-card": {
        "short": {"text": {"question_text": "Where should queued steers be persisted?"}},
        "long": {"text": {"question_text": "Where should queued steers be persisted so they "
                                           "survive both a reconnect and an app restart without "
                                           "losing ordering?"},
                 "flags": {"question_card": {"h": 720}, "question_text": {"h": 110}}},
    },
    "edited-files-card": {
        "short": {"text": {"t01": "Edited 1 file", "t02": "+12 -2"}},
        "long": {"text": {"t01": "Edited 3 files", "t02": "+62 -5"}},
    },
    "plan-card": {
        "short": {"text": {"t02": "Plan \u00b7 1 of 2",
                           "step_0_label": "Reproduce reconnect drop",
                           "step_1_label": "Implement durable queue"},
                  "drop": ["step_2_label", "step_3_label", "step_4_label",
                           "icon_step2", "icon_step3", "icon_step4"],
                  "flags": {"plan_card": {"h": 300}}},
        "long": {"text": {"t02": "Plan \u00b7 3 of 5"}},
    },
    "goal-strip": {
        "short": {"text": {"t01": "Goal \u00b7 Fix steer queue \u00b7 2m"}},
        "long": {"text": {"t01": "Goal \u00b7 Fix steer queue on reconnect \u00b7 18m"}},
    },
    "diff-view": {
        # Card #18c item 5: the atlas folded row leads AND trails with a vertical
        # ellipsis (OCR read the leading one as a colon); the short variant carried
        # the stale ":88" string.
        "short": {"text": {"t_file": "ui_protocol.rs", "t_fadd": "+9", "t_fdel": "-1",
                           "t_fold": "⋮ 88 unmodified lines ⋮"}},
        "long": {"text": {"t_file": "ui_protocol_transport.rs", "t_fadd": "+31", "t_fdel": "-4",
                          "t_fold": ":412 unmodified lines"}},
    },
    "settings-group": {
        "short": {"text": {"t01": "Permissions"}},
        "long": {"text": {"t01": "Permissions and defaults"}},
    },
}

ROLE_BY_KIND = {"stack": "layout", "text": "text", "svg": "icon", "button": "button",
                "input": "input"}


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def walk(node):
    yield node
    for c in node.get("c", []):
        yield from walk(c)


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False



def relativize(tree):
    """Card #18c: give every fill-width node the geometry a responsive parent needs.

    `design.rs` emits a responsive node's inset as a MARGIN and makepad insets a
    `Fill` extent by it. Two things must therefore be parent-relative:

    * `x`/`y` — the offset FROM the parent (a nested fill would otherwise apply the
      outer inset twice).
    * `padright` — the RIGHT inset = parent_w - (x + w), i.e. the node's authored right
      gap. Using `x` on the right (symmetric) collapsed right-anchored fills such as
      edited-files' Review pill (left 296 + right 296 > its parent width). The gap
      is right for every node: the box then spans x .. x+authored_w, which is the
      atlas geometry, at any parent width.

    The ROOT has no `padright`: its parent is the render slot, and the atlas card is
    wider than a 360 slot once the page gutters are restored, so it must inset
    symmetrically (left = x, right = x) to shrink into the slot.
    """
    def rec(n, ox, oy, pw):
        for c in n.get("c", []) or []:
            filled = any(c.get(k) == 1 for k in ("fillw", "fith", "fillh", "fitw"))
            cx, cy, cw = c.get("x", 0), c.get("y", 0), c.get("w")
            if filled:
                c["x"] = round(cx - ox, 2)
                c["y"] = round(cy - oy, 2)
                if pw is not None and cw is not None:
                    # Use the RELATIVE x: pw is the parent's width, so the gap must
                    # be measured from the parent's own origin. With the absolute x
                    # a nested fill got a hugely negative gap (review_surface:
                    # 79 - (296 + 79) = -296), which mis-placed and clipped it.
                    c["padright"] = round(pw - ((cx - ox) + cw), 2)
                rec(c, cx, cy, cw)
            else:
                rec(c, ox, oy, pw)
    rec(tree, 0, 0, None)
    return tree


def right_edge_ok(png_path, tol=6):
    """Card #18c acceptance check: the rightmost 4px column of a variant must be
    background (near-white page ground, or the host's #4c4c4c), i.e. no card
    border, radius or tint may be cut off at the render's right edge."""
    a = np.asarray(Image.open(png_path).convert("RGB")).astype(int)
    strip = a[:, -4:, :]
    white = (strip.min(axis=2) > 238).all()
    ground = (np.abs(strip - 76).max(axis=2) <= 8).all()
    from collections import Counter
    top = Counter(map(tuple, strip.reshape(-1, 3))).most_common(1)[0][0]
    return bool(white or ground), [int(v) for v in top]


def apply_variant(tree, comp, variant):
    spec = VARIANTS[comp][variant]
    # Merge per NODE ID so a variant flag does not replace the responsive flags.
    flags = {k: dict(v) for k, v in RESPONSIVE[comp].items()}
    for nid, fl in spec.get("flags", {}).items():
        flags.setdefault(nid, {}).update(fl)
    inserts = spec.get("insert", [])
    drops = set(spec.get("drop", []))
    moved = []
    for n in walk(tree):
        if n["id"] in spec.get("text", {}):
            n["text"] = spec["text"][n["id"]]
        if n["id"] in flags:
            before = (n.get("x"), n.get("y"))
            n.update(flags[n["id"]])
            if (n.get("x"), n.get("y")) != before:
                moved.append((n, before[0], before[1]))
    # Card #18c item 4: a positional flag moves a node AND its subtree. Without
    # this a moved KitButton left its `_surface`/`_label` at the old y, so
    # `relativize` computed a stale offset (button surface rendered 60px above its
    # wrapper). Shift every descendant by the same delta.
    for n, bx, by in moved:
        dx = (n.get("x") or 0) - (bx or 0)
        dy = (n.get("y") or 0) - (by or 0)
        for c in walk(n):
            if c is n:
                continue
            if c.get("x") is not None:
                c["x"] = round(c["x"] + dx, 2)
            if c.get("y") is not None:
                c["y"] = round(c["y"] + dy, 2)
    if drops:
        def prune(node):
            node["c"] = [c for c in node.get("c", []) if c["id"] not in drops]
            for c in node["c"]:
                prune(c)
        prune(tree)
    for parent_id, node in inserts:
        parent = next(n for n in walk(tree) if n["id"] == parent_id)
        parent.setdefault("c", []).append(json.loads(json.dumps(node)))
    relativize(tree)
    # Card #18c item 1: the atlas crop is card-tight, so the extracted root sits
    # at x=0 and a `Fill` root would run edge-to-edge with its right border/radius
    # at the window's last pixel. Give it the page gutter the atlas normalised
    # away, so all four rounded corners are inside the render at every width.
    # Applied AFTER relativize: children keep their offsets relative to the card's
    # own box, and makepad insets them by the root's margin.
    tree["x"] = round(tree.get("x", 0) + ROOT_GUTTER_X, 2)
    tree["y"] = round(tree.get("y", 0) + ROOT_GUTTER_Y, 2)
    tree.pop("padright", None)          # root insets symmetrically (see relativize)
    return tree, inserts, drops


def build_workspace(comp, variant):
    dest = WORK / f"{comp}-{variant}"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(COMPONENTS / comp, dest)
    for stale in ("page.card", "page.data.json", "kit", "semantic-preflight.json",
                  "semantic-audit.json", "semantic-repair.json", "mapping.json",
                  "semantic-state.json", "variants"):
        p = dest / stale
        if p.is_dir():
            shutil.rmtree(p)
        elif p.exists():
            p.unlink()
    mapped = json.loads((dest / "mapped.json").read_text())
    tree, inserts, drops = apply_variant(mapped["tree"], comp, variant)
    mapped["tree"] = tree
    (dest / "mapped.json").write_text(json.dumps(mapped, indent=2, ensure_ascii=False) + "\n")
    semantic = json.loads((dest / "semantic-map.json").read_text())
    known = {e["id"] for e in semantic["elements"]}
    semantic["elements"] = [e for e in semantic["elements"] if e["id"] not in drops]
    for parent_id, node in inserts:
        for n in walk(node):
            if n["id"] in known:
                continue
            semantic["elements"].append({"id": n["id"], "role": ROLE_BY_KIND.get(n["t"], "layout"),
                                         "basis": f"authored {n['t']} node (card #18 variant)",
                                         "confidence": 1.0, "decision": "declared"})
            known.add(n["id"])
    semantic.pop("contract_sha256", None)
    semantic.pop("reference_sha256", None)
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    return dest


def compile_component(dest, comp):
    for p in (CLONE / "flows/image-lib", CLONE / "flows/image-to-card", CLONE / "flows"):
        sys.path.insert(0, str(p))
    from compile import compile_page
    from flow import sha
    semantic = json.loads((dest / "semantic-map.json").read_text())
    semantic["contract_sha256"] = sha(dest / "contract.json")
    semantic["reference_sha256"] = sha(dest / "reference.png")
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    result = compile_page(dest, artwork_origin=f"http://127.0.0.1:{ART_PORT}/ux-images")
    pub = PUBLISHED / "ux-images" / comp / "assets"
    pub.mkdir(parents=True, exist_ok=True)
    for f in (dest / "assets").glob("*"):
        shutil.copy(f, pub / f.name)
    return result


def render(dest, port, w, h):
    request = {
        "card": str(dest / "page.card"), "data": str(dest / "page.data.json"),
        "kit_dir": str(dest / "kit"), "format": "l0-kit", "width": w, "height": h,
        "nonce": f"var18-{dest.name}-{w}", "result": str(dest / "native.json"),
        "layout": str(dest / "layout.json"), "actions": str(dest / "actions.json"),
    }
    (dest / "request.json").write_text(json.dumps(request))
    with (dest / f"host-{w}.log").open("w") as log:
        env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
               "BEAUTY_REQUEST": str(dest / "request.json")}
        proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                                stdout=log, stderr=subprocess.STDOUT)
        try:
            if not wait_port(port):
                return {"error": "no port"}
            time.sleep(6)
            try:
                snap = subprocess.check_output(
                    ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
                grab = json.loads(subprocess.check_output(
                    ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
            except subprocess.CalledProcessError as exc:
                # a flaky /snap or /g must not abort the whole batch
                return {"error": f"curl: {exc.returncode}"}
            (dest / f"snap-{w}.json").write_text(snap)
            png = grab.get("png")
            if png and Path(png).is_file():
                # `beauty-host` never sets a light canvas ground (unlike the
                # production `card-host`, host.rs:23 `clear_color: #fff`), so a
                # render is a canvas with the component at the TOP and the host's
                # dark `#4c4c4c` ground filling the rest. Trim that ground from the
                # edges (card #18; same finding as #16b). Keyed on the host ground
                # colour, NOT a corner pixel: a tall component's top-left is white,
                # so a corner-keyed bbox kept the whole dark band.
                im = Image.open(png).convert("RGB")
                a = np.asarray(im).astype(int)
                ground = np.array([76, 76, 76])
                near = np.abs(a - ground).max(axis=2) <= 8
                top, bot, left, right = 0, a.shape[0] - 1, 0, a.shape[1] - 1
                while bot > top and near[bot].all():
                    bot -= 1
                while top < bot and near[top].all():
                    top += 1
                while right > left and near[top:bot + 1, right].all():
                    right -= 1
                while left < right and near[top:bot + 1, left].all():
                    left += 1
                # Card #18c item 1: KEEP the full window width and crop only
                # vertically. Displaying a card-tight crop would hide the very
                # defect the card asks about; keeping the window makes the
                # rightmost-4px background check meaningful rather than vacuous.
                if bot > top:
                    im = im.crop((0, top, a.shape[1], bot + 1))
                im.save(dest / f"native-{w}.png")
            subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                           capture_output=True)
            time.sleep(1)
        finally:
            if proc.poll() is None:
                proc.kill()
    text = (dest / f"host-{w}.log").read_text(errors="ignore")
    clip = {}
    cf = dest / f"clip-{w}.json"
    if cf.exists():
        clip = json.loads(cf.read_text())
    return {"w": w, "clipped": clip, "font_warnings": text.count("not available in this build")}


def assemble(comp, variant, panels):
    imgs = [Image.open(p).convert("RGB") for p in panels]
    gap, pad = 12, 16
    h = max(i.height for i in imgs)
    w = sum(i.width for i in imgs) + gap * (len(imgs) - 1) + pad * 2
    canvas = Image.new("RGB", (w, h + pad * 2), (255, 255, 255))
    x = pad
    for i in imgs:
        canvas.paste(i, (x, pad))
        x += i.width + gap
    out = COMPONENTS / comp / f"review-{variant}.png"
    canvas.save(out)
    return out


def main():
    only = sys.argv[1:]
    WORK.mkdir(parents=True, exist_ok=True)
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), partial(Quiet, directory=str(PUBLISHED)))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    summary = {}
    try:
        for comp in sorted(VARIANTS):
            if only and comp not in only:
                continue
            summary[comp] = {}
            vdir = COMPONENTS / comp / "variants"
            vdir.mkdir(exist_ok=True)
            for vi, variant in enumerate(VARIANTS[comp]):
                dest = build_workspace(comp, variant)
                try:
                    compile_component(dest, comp)
                except Exception as exc:            # noqa: BLE001 - surface the real error
                    print(json.dumps({"component": comp, "variant": variant,
                                      "compile_error": str(exc)[:400]}))
                    continue
                renders = []
                for wi, w in enumerate(WIDTHS):
                    # Card #18c: delete the destination first. A failed width must
                    # NOT pass the acceptance check on a stale PNG from an earlier
                    # run (settings-group-short had no native-360 yet reported ok).
                    stale = vdir / f"{variant}-{w}.png"
                    if stale.exists():
                        stale.unlink()
                    r = render(dest, PORTS[(vi * len(WIDTHS) + wi) % len(PORTS)], w, 720)
                    renders.append(r)
                    if (dest / f"native-{w}.png").exists():
                        shutil.copy(dest / f"native-{w}.png", stale)
                panels = [str(COMPONENTS / comp / "reference.png")]
                for w in WIDTHS:
                    panels.append(str(vdir / f"{variant}-{w}.png"))
                # Card #18c item 1 acceptance check: the rightmost 4px column of
                # every variant must be background, i.e. no card border/radius/tint
                # is cut off at the render's right edge.
                edge = {}
                for w in WIDTHS:
                    f = vdir / f"{variant}-{w}.png"
                    if f.exists():
                        ok_e, col = right_edge_ok(f)
                        edge[w] = {"ok": ok_e, "colour": col}
                edge_ok = all(v["ok"] for v in edge.values()) if edge else False
                widths_ok = len(edge) == len(WIDTHS)
                ok = all(Path(p).is_file() for p in panels) and edge_ok and widths_ok
                review = str(assemble(comp, variant, panels)) if ok else None
                summary[comp][variant] = {
                    "render": str(Path(review).relative_to(ROOT)) if review else None,
                    "widths": list(WIDTHS),
                    "right_edge": edge,
                    "font_warnings": sum(r.get("font_warnings", 0) for r in renders),
                }
                print(json.dumps({"component": comp, "variant": variant, "review": review,
                                  "ok": ok, "right_edge": edge}, ensure_ascii=False))
    finally:
        httpd.shutdown()
    (WORK / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    sync_index_render_paths()


def sync_index_render_paths():
    """Rewrite each component's `render` list in design/components/index.json to
    the artwork that actually exists on disk (reference crop, the two variant
    reviews, and the four per-width PNGs) — card #18 asks the index to carry
    render paths."""
    idx_path = COMPONENTS / "index.json"
    if not idx_path.exists():
        return
    idx = json.loads(idx_path.read_text())
    for entry in idx.get("components", []):
        d = COMPONENTS / entry["id"]
        paths = []
        for rel in ("reference.png", "review-short.png", "review-long.png"):
            if (d / rel).exists():
                paths.append(str((d / rel).relative_to(ROOT)))
        for v in ("short", "long"):
            for w in WIDTHS:
                p = d / "variants" / f"{v}-{w}.png"
                if p.exists():
                    paths.append(str(p.relative_to(ROOT)))
        if paths:
            entry["render"] = paths
    idx_path.write_text(json.dumps(idx, indent=2) + "\n")


if __name__ == "__main__":
    main()
