#!/usr/bin/env python3
"""Card #16b: render each L0 component's short/long variants, hidden, at 360 and 540.

For every component under `design/components/<id>/` this copies the component,
applies a variant transform to its `mapped.json` tree (text, responsive
fill/fit flags, and structural inserts for tool-cell expanded/failed and the
composer queued chip), re-compiles it with the flow's OWN compiler
(`flows/image-lib/compile.py::compile_page`), renders it hidden with
`beauty-host` at 360 and 540, and writes:

  design/components/<id>/variants/<variant>-<w>.png   the raw native render
  design/components/<id>/review-<variant>.png         atlas crop | native@360 | native@540

Why go through `compile_page` rather than hand-editing the kit: `fillw`/`fith`
are carried in the node's *style*, which the compiler lowers to the kit pack and
`design.rs` reads as `a.fillw`/`a.fith`; hand-editing the kit would bypass the
one code path production uses. Inserted nodes get matching `semantic-map.json`
entries so `preflight` stays green (it requires every id classified).

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

from PIL import Image

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]          # repo root (p0-harness)
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
COMPONENTS = ROOT / "design/components"
WORK = ROOT / "tmp/stage-b/render-variants"
ART_PORT = 8179
PORTS = [8390, 8391, 8392, 8393]
WIDTHS = (360, 540)

INTER4 = "self:resources/ux/Inter-400.ttf"
INTER5 = "self:resources/ux/Inter-500.ttf"
INTER7 = "self:resources/ux/Inter-700.ttf"
MONO = "self:resources/ux/LiberationMono-Regular.ttf"
RED = 0xFFCF222E
GREY = 0xFF6E6E73
INK = 0xFF1D1D1F
MONOBG = 0xFFF6F6F7

# Responsive flags (card #16b: fill the slot width, height from content). Applied
# to every variant. `fillw` on a text node lets it wrap to the slot; `fith` on
# the root lets it hug its content.
RESPONSIVE = {
    "thread-row": {"thread_1": {"fillw": 1, "fith": 1},
                   "thread_1_surface": {"fillw": 1, "fillh": 1},
                   "thread_1_control": {"fillw": 1, "fillh": 1},
                   "thread_1_label": {"fillw": 1}},
    "new-chat": {"new_chat": {"fillw": 1, "fith": 1},
                 "new_chat_surface": {"fillw": 1, "fillh": 1},
                 "new_chat_control": {"fillw": 1, "fillh": 1},
                 "new_chat_label": {"fillw": 1}},
    "worked-for": {"worked_row": {"fillw": 1, "fith": 1},
                   "worked_row_surface": {"fillw": 1, "fillh": 1},
                   "worked_row_control": {"fillw": 1, "fillh": 1},
                   "worked_row_label": {"fillw": 1}},
    "user-bubble": {"user_bubble": {"fillw": 1, "fith": 1},
                    "t01": {"fillw": 1}, "t02": {"fillw": 1}},
    "working-row": {"working_row": {"fillw": 1, "fith": 1}, "t03": {"fillw": 1}},
    "assistant-prose": {"answer_prose": {"fillw": 1, "fith": 1},
                        "answer_md": {"fillw": 1, "fith": 1}},
    "answer-actions": {"answer_actions": {"fillw": 1, "fith": 1},
                       "t11": {"fillw": 1, "alignx": 1}},
    "tool-cell": {"tool_1": {"fillw": 1, "fith": 1}, "t01": {"fillw": 1},
                  "t02": {"fillw": 1}},
    "composer": {"composer_idle": {"fillw": 1, "fith": 1},
                 "composer_idle_input": {"fillw": 1}},
}

# The expanded tool-cell console, from scene 04's third card (`tool_3_output`).
OUTPUT_BOX = {"t": "stack", "id": "tool_1_output", "x": 10, "y": 62, "w": 351, "h": 116,
              "variant": "surface", "bg": MONOBG, "radius": 8, "c": [
                  {"t": "text", "id": "o1", "x": 18, "y": 10, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281216815,
                   "variant": "single_line", "text": "running 12 tests"},
                  {"t": "text", "id": "o2", "x": 18, "y": 38, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281479731,
                   "variant": "single_line", "text": "test steer_queue::reconnect_ok ... ok"},
                  {"t": "text", "id": "o3", "x": 18, "y": 66, "w": 316, "h": 17, "size": 13.4,
                   "weight": 500, "font_src": MONO, "line_height": 16.6, "color": 4281362226,
                   "variant": "single_line", "text": "test result: ok. 12 passed; 0 failed"},
                  {"t": "text", "id": "o4", "x": 18, "y": 92, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281216815,
                   "variant": "single_line", "text": "Finished in 0.42s"},
              ]}
# A red status glyph, used by the failed variant (the asset check icon cannot be
# recoloured through the kit, so it is collapsed to zero and replaced by a glyph).
FAILED_X = {"t": "text", "id": "status_x", "x": 336, "y": 25, "w": 20, "h": 23, "size": 17,
            "weight": 700, "font_src": INTER7, "line_height": 21, "color": RED,
            "variant": "single_line", "text": "\u2715"}
# The queued chip, from scene 08 (`queued_row`).
QUEUED = {"t": "stack", "id": "queued_row", "x": 10, "y": 196, "w": 240, "h": 50,
          "variant": "surface", "bg": 4294440952, "radius": 12, "c": [
              {"t": "text", "id": "q1", "x": 16, "y": 13, "w": 208, "h": 24, "size": 15,
               "weight": 500, "font_src": INTER5, "line_height": 18, "color": 4280953387,
               "variant": "single_line", "text": "1 queued \u00b7 Steer now \u00b7 \u2715"}]}

VARIANTS = {
    "thread-row": {
        # scene 01: thread_2 (0xFFFDFDFD) is unselected; thread_1 (0xFFF1F1F3) is
        # the selected row (its fill is the darker token).
        "short": {"text": {"thread_1_label": "Add session fork"},
                  "flags": {"thread_1_surface": {"bg": 0xFFFDFDFD}}},
        "long": {"text": {"thread_1_label":
                          "Bump octos-core to a6ea8505 and re-verify the steer queue timeout"},
                 "flags": {"thread_1_surface": {"bg": 0xFFF1F1F3}}},
    },
    "new-chat": {"short": {"text": {}}, "long": {"text": {}}},
    "user-bubble": {
        "short": {"text": {"t01": "Retry the build"}, "drop": ["t02"]},
        "long": {"text": {"t01": "Fix the steer queue so queued",
                          "t02": "steers survive a reconnect"}},
    },
    "working-row": {"short": {"text": {"t03": "Working \u2022 3s"}},
                    "long": {"text": {"t03": "Working \u2022 12s"}}},
    "assistant-prose": {
        "short": {"text": {"answer_md": "Fixed `steer_dropped` handling."}},
        "long": {"text": {"answer_md":
                          "Queued steers now survive a reconnect.\n\n"
                          "\u2022 Updated `ui_protocol_transport.rs` to persist queued steers.\n\n"
                          "\u2022 All tests pass: `12 passed`."}},
    },
    "worked-for": {"short": {"text": {"worked_row_label": "Worked for 3s \u203a"}},
                   "long": {"text": {"worked_row_label": "Worked for 3m 4s \u203a"}}},
    "answer-actions": {"short": {"text": {"t11": "now"}},
                       "long": {"text": {"t11": "Sep 28, 9:41 PM"}}},
    "tool-cell": {
        "short": {"text": {"t01": "Read ui_protocol_transport.rs", "t02": "\u2022 412 lines"}},
        "long": {"text": {"t01": "Ran cargo test -p octos-cli", "t02": "\u2022 12 passed"},
                 "flags": {"tool_1": {"h": 190}}, "insert": [("tool_1", OUTPUT_BOX)]},
        "failed": {"text": {"t01": "Ran cargo test -p octos-cli",
                            "t02": "\u2022 exit 2 \u00b7 0 passed, 2 failed"},
                   "flags": {"icon_check1": {"w": 0, "h": 0}, "t02": {"color": RED}},
                   "insert": [("tool_1", FAILED_X)]},
    },
    "composer": {
        "short": {"text": {}},
        "long": {"text": {"composer_idle_input": "also add a test for reconnect"},
                 "flags": {"composer_idle": {"h": 250}, "send1": {"bg": 0xFFFF5F5F}},
                 "insert": [("composer_idle", QUEUED)]},
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


def apply_variant(tree, comp, variant):
    spec = VARIANTS[comp][variant]
    # Merge per NODE ID, not per top-level key: otherwise a variant flag like
    # {"tool_1": {"h": 190}} would *replace* the responsive {"fillw":1,"fith":1}
    # for that node, and the root would pin its atlas width again.
    flags = {k: dict(v) for k, v in RESPONSIVE[comp].items()}
    for nid, fl in spec.get("flags", {}).items():
        flags.setdefault(nid, {}).update(fl)
    inserts = spec.get("insert", [])
    drops = set(spec.get("drop", []))
    for n in walk(tree):
        if n["id"] in spec.get("text", {}):
            n["text"] = spec["text"][n["id"]]
        if n["id"] in flags:
            n.update(flags[n["id"]])
    if drops:
        def prune(node):
            node["c"] = [c for c in node.get("c", []) if c["id"] not in drops]
            for c in node["c"]:
                prune(c)
        prune(tree)
    for parent_id, node in inserts:
        parent = next(n for n in walk(tree) if n["id"] == parent_id)
        parent.setdefault("c", []).append(json.loads(json.dumps(node)))
    return tree, inserts, drops


def build_workspace(comp, variant):
    dest = WORK / f"{comp}-{variant}"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(COMPONENTS / comp, dest)
    for stale in ("page.card", "page.data.json", "kit", "semantic-preflight.json",
                  "semantic-audit.json", "semantic-repair.json", "mapping.json",
                  "semantic-state.json"):
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
    # A dropped node must leave the semantic map too, or preflight reports it as
    # "absent from the composition".
    semantic["elements"] = [e for e in semantic["elements"] if e["id"] not in drops]
    for parent_id, node in inserts:
        for n in walk(node):
            if n["id"] in known:
                continue
            role = ROLE_BY_KIND.get(n["t"], "layout")
            semantic["elements"].append({"id": n["id"], "role": role,
                                         "basis": f"authored {n['t']} node (card #16b variant)",
                                         "confidence": 1.0, "decision": "declared"})
            known.add(n["id"])
    semantic.pop("contract_sha256", None)
    semantic.pop("reference_sha256", None)
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    return dest


def compile_component(dest, comp):
    # `compile.py`/`semantics.py` live in `flows/image-lib`; `flow.py` (its
    # `sha`/`local` helpers) lives in `flows/image-to-card`; both import
    # `core.native_paths` from `flows/`.
    for p in (CLONE / "flows/image-lib", CLONE / "flows/image-to-card", CLONE / "flows"):
        sys.path.insert(0, str(p))
    from compile import compile_page
    from flow import sha
    # `compile_page` re-derives the two sha bindings the semantic map must match.
    semantic = json.loads((dest / "semantic-map.json").read_text())
    semantic["contract_sha256"] = sha(dest / "contract.json")
    semantic["reference_sha256"] = sha(dest / "reference.png")
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    # The default artwork origin is 8170, which another lane's process owns; point
    # the compiled SVG `src` at OUR art server, or an icon node renders 0x0.
    result = compile_page(dest, artwork_origin=f"http://127.0.0.1:{ART_PORT}/ux-images")
    # Publish this workspace's compiled assets where that server serves them
    # (`published/ux-images/<id>/assets/...`); idempotent.
    pub = PUBLISHED / "ux-images" / comp / "assets"
    pub.mkdir(parents=True, exist_ok=True)
    for f in (dest / "assets").glob("*"):
        shutil.copy(f, pub / f.name)
    return result


def render(dest, port, w, h):
    request = {
        "card": str(dest / "page.card"), "data": str(dest / "page.data.json"),
        "kit_dir": str(dest / "kit"), "format": "l0-kit", "width": w, "height": h,
        "nonce": f"var-{dest.name}-{w}", "result": str(dest / "native.json"),
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
            snap = subprocess.check_output(
                ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
            (dest / f"snap-{w}.json").write_text(snap)
            grab = json.loads(subprocess.check_output(
                ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
            png = grab.get("png")
            if png and Path(png).is_file():
                shutil.copy(png, dest / f"native-{w}.png")
            subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                           capture_output=True)
            time.sleep(1)
        finally:
            if proc.poll() is None:
                proc.kill()
    text = (dest / f"host-{w}.log").read_text(errors="ignore")
    nodes = [(e["i"], e["ty"], e["r"]) for e in json.loads(snap)["s"] if e["i"].startswith("beauty")]
    return {"w": w, "nodes": nodes, "font_warnings": text.count("not available in this build")}


def assemble(comp, variant, panels):
    """atlas crop | native@360 | native@540, top-aligned, on white."""
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
                    r = render(dest, PORTS[(vi * len(WIDTHS) + wi) % len(PORTS)], w, 640)
                    renders.append(r)
                    if r.get("nodes") is not None:
                        shutil.copy(dest / f"native-{w}.png", vdir / f"{variant}-{w}.png")
                panels = [str(COMPONENTS / comp / "reference.png")]
                for w in WIDTHS:
                    p = vdir / f"{variant}-{w}.png"
                    panels.append(str(p))
                ok = all(Path(p).is_file() for p in panels)
                review = str(assemble(comp, variant, panels)) if ok else None
                summary[comp][variant] = {
                    "render": str(Path(review).relative_to(ROOT)) if review else None,
                    "widths": list(WIDTHS),
                    "nodes": {f"w{r['w']}": r.get("nodes") for r in renders},
                    "font_warnings": sum(r.get("font_warnings", 0) for r in renders),
                }
                print(json.dumps({"component": comp, "variant": variant, "review": review,
                                  "ok": ok}, ensure_ascii=False))
    finally:
        httpd.shutdown()
    (WORK / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
