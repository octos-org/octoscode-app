#!/usr/bin/env python3
"""Promote reusable setup-board elements to design/components/<id>/ (entry #28d).

Per element: author a component contract (subtree root, reference-sampled fills),
write mapped.json + minimal semantic-map, compile with the flow's own compiler,
render short+long variants at 360 and 540 with beauty-host (hidden), and run the
card #18c acceptance check: the rightmost 4px of every variant must be the page
background (white), i.e. nothing may bleed off the right edge.
"""
import hashlib
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

HERE = Path(__file__).resolve().parent            # design/stage-b/setup/tools
SETUP = HERE.parent                               # design/stage-b/setup
CONV = SETUP.parent / "conversation" / "tools"
sys.path.insert(0, str(CONV))
os.environ.setdefault("PYTHONDONTWRITEBYTECODE", "1")
import author_v2 as A                             # node helpers (board-1 verbatim)
from PIL import Image

import numpy as np

ROOT = Path(__file__).resolve().parents[4]
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
WORK = ROOT / "tmp/stage-b/render-components-setup"
COMPONENTS = ROOT / "design/components"
sys.path.insert(0, str(CLONE / "flows/image-lib"))
from compile import compile_page                   # flow's own compiler

W, H = (360, 540)
VW = (360, 540)


def r(v):
    return round(float(v), 2)


def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def text(id, s, x, y, w, h, *, weight=400, color="ink", size=14):
    return A.text(id, s, x, y, w, h, weight=weight, color=color, size=size)


# ---------------------------------------------------------------- components
def build_usage_bar(long=False):
    usage = "128k of 200k tokens - compact soon" if long else "124k of 200k tokens"
    pct = "64%" if long else "62%"
    fill_w = 213 if long else 205
    kids = [
        text("t_usage", usage, 12.9, 9.3, 200, 21),
        text("t_pct", pct, 294.8, 9.3, 40, 20, color="muted", size=13),
        A.surface("bar_track", 15, 52.5, 317, 15.5, bg="s9_track", radius=8),
        A.surface("bar_fill", 15, 52.5, fill_w, 15.5, bg="s9_blue", radius=8),
    ]
    return "usage-bar", 360, 80, kids, {"s9_track": "#E8E7EA", "s9_blue": "#266BEF"}


def build_segmented(long=False):
    kids = [
        A.surface("seg_box", 0, 0, 207.5, 58, bg="white", radius=29,
                  border=1, bordercolor="s9_segborder"),
        A.surface("seg_div", 105.5, 6, 1.5, 46, bg="s9_segdiv", radius=0),
        text("t_llm", "Auto (LLM)" if long else "LLM", 30.1, 21.7, 90, 20, weight=500),
        text("t_heur", "Heuristic", 122.5, 21.6, 70, 21, color="muted"),
    ]
    return "segmented-control", 207.5, 58, kids, {"s9_segborder": "#D8D9D9", "s9_segdiv": "#D0D1D0"}


def build_skeleton(long=False):
    rows = [(0, 0, 70, 50), (70, 6, 255, 44), (0, 82, 70, 56), (70, 86, 255, 52)]
    if long:
        rows += [(0, 172, 70, 49), (70, 177, 176, 44)]
    kids = [A.surface(f"skel{i}", x, y, w, h, bg="s12_skel", radius=8)
            for i, (x, y, w, h) in enumerate(rows)]
    return "skeleton-list", 340, 228 if long else 158, kids, {"s12_skel": "#F0F1F2"}


BUILDERS = {"usage-bar": build_usage_bar, "segmented-control": build_segmented,
            "skeleton-list": build_skeleton}

# Colours consumed by the component builders. They MUST be registered before
# BUILDERS[comp](...) is first called (same failure class as the screen-side
# build_09 KeyError): the builders look names up in author_v2.C at call time.
COMPONENT_COLORS = {"s9_track": "#E8E7EA", "s9_blue": "#266BEF",
                    "s9_segborder": "#D8D9D9", "s9_segdiv": "#D0D1D0",
                    "s12_skel": "#F0F1F2"}
for _n, _hx in COMPONENT_COLORS.items():
    A.C[_n] = 0xFF000000 | int(_hx[1:3], 16) << 16 | int(_hx[3:5], 16) << 8 | int(_hx[5:7], 16)
    A.C_HEX.setdefault(_n, _hx)
assert all(_n in A.C for _n in COMPONENT_COLORS), "component colour registration failed"
SOURCE_REF = {"usage-bar": ("setup-09", (30, 88, 405, 168)),
              "segmented-control": ("setup-09", (150, 400, 380, 472)),
              "skeleton-list": ("setup-12", (24, 92, 364, 326))}


def semantic_map(d, tree, extra):
    els = []
    for n in walk(tree):
        role = {"text": "text", "svg": "icon"}.get(n.get("t"), "layout")
        els.append({"id": n["id"], "role": role,
                    "basis": f"authored {n.get('t')} node", "confidence": 1.0,
                    "decision": "declared"})
    doc = {"schema_version": 1, "policy_version": "1.1.0",
           "contract_sha256": sha(d / "contract.json"),
           "reference_sha256": sha(d / "reference.png"),
           "classification_scope": "Authored component subtree; no automatic recognition.",
           "elements": els}
    (d / "semantic-map.json").write_text(json.dumps(doc, indent=2) + "\n")


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def compile_component(d):
    # preflight compares semantic-map's contract_sha256 against the CURRENT
    # contract.json; the variant loop rewrites the contract, so the map must be
    # refreshed from disk on every compile or the hash goes stale.
    semantic_map(d, json.loads((d / "contract.json").read_text())["tree"], None)
    compile_page(d, "http://127.0.0.1:8170/ux-images/")


def free_port(cands):
    for c in cands:
        t = socket.socket()
        try:
            t.bind(("127.0.0.1", c))
            return c
        except OSError:
            continue
        finally:
            t.close()
    raise SystemExit("no free port")


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render(dest, port, w, h):
    """One beauty-host on `port`; returns the grab png path or None."""
    request = {"card": str(dest / "page.card"), "data": str(dest / "page.data.json"),
               "kit_dir": str(dest / "kit"), "format": "l0-kit", "width": w, "height": h,
               "nonce": f"setup-comp-{dest.name}-{w}", "result": str(dest / "native.json"),
               "layout": str(dest / "layout.json"), "actions": str(dest / "actions.json")}
    (dest / "request.json").write_text(json.dumps(request))
    log = (dest / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
           "BEAUTY_REQUEST": str(dest / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return None
        time.sleep(5)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(0.5)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    if png and not Path(png).resolve().is_relative_to(ROOT):
        return None          # a foreign host answered: reject
    return png


def rightmost_ok(png, w, h):
    """Card #18c acceptance: rightmost 4px column must be the page background."""
    a = np.array(Image.open(png).convert("RGB")).astype(int)
    scale = a.shape[1] / w
    band = a[:, int(a.shape[1] - 4 * scale):, :]
    return bool((np.abs(band - 255).max() <= 6).all()), tuple(int(v) for v in band.reshape(-1, 3).mean(axis=0))


if __name__ == "__main__":
    art_port = None
    for cand in (8188, 8189, 8190):
        s = socket.socket()
        try:
            s.bind(("127.0.0.1", cand))
            art_port = cand
            break
        finally:
            s.close()
    class Quiet(SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    httpd = ThreadingHTTPServer(("127.0.0.1", art_port),
                                partial(Quiet, directory=str(PUBLISHED)))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(0.5)
    ports = []
    cands = list(range(8390, 8400))
    for _ in range(2):
        ports.append(free_port(cands))
        cands.remove(ports[-1])
    results = {}
    try:
        for comp in sorted(BUILDERS):
            name, cw, ch, kids, colors = BUILDERS[comp](long=False)
            name_l, cw_l, ch_l, kids_l, _ = BUILDERS[comp](long=True)
            # colours are pre-registered above (COMPONENT_COLORS)
            d = COMPONENTS / comp
            d.mkdir(parents=True, exist_ok=True)
            # component contract (short variant is the base)
            doc = {"schema_version": 1, "id": comp, "app": "octoscode", "number": 0,
                   "title": comp, "structure": "Native component reconstructed from the approved board-2 atlas",
                   "artboard": [cw, ch], "font_family": "Inter",
                   "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                               "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
                   "content_source": "Approved stage-a board-2 atlas (Gate A 2026-09-29) + Apple Vision OCR",
                   "graphics": {},
                   "tree": A.surface(comp.replace("-", "_") + "_root", 0, 0, cw, ch,
                                  bg="white", radius=0, kids=kids)}
            (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
            (d / "mapped.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
            (d / "service-actions.json").write_text(json.dumps({"frame_id": 0, "controls": {}, "source": "setup board"}, indent=2) + "\n")
            sref, box = SOURCE_REF[comp]
            ref = Image.open(SETUP / "cards" / sref / "reference.png")
            ref.crop((box[0] * 2, box[1] * 2, box[2] * 2, box[3] * 2)).save(d / "reference.png")
            semantic_map(d, doc["tree"], colors)
            for iid, (n, c) in A.ICON_REG.items():
                pass  # no icons in these three components
            compile_component(d)
            # variants: short/long x 360/540
            variants = {}
            for vname, tree_kw, cwv, chv, kidsv in (
                    ("short", False, cw, ch, kids), ("long", True, cw_l, ch_l, kids_l)):
                nm, w_, h_, ks, _ = BUILDERS[comp](long=tree_kw)
                vdoc = json.loads(json.dumps(doc))
                vdoc["tree"] = A.surface(comp.replace("-", "_") + "_root", 0, 0, w_,
                                         chv if vname else h_, bg="white", radius=0, kids=ks)
                vdoc["artboard"] = [max(w_, 360), (chv if vname else h_) + 16]
                # compile_page consumes mapped.json - both files must carry the
                # variant tree, or the long variant renders identical to short.
                (d / "contract.json").write_text(json.dumps(vdoc, indent=2) + "\n")
                (d / "mapped.json").write_text(json.dumps(vdoc, indent=2) + "\n")
                compile_component(d)
                vd = WORK / f"{comp}-{vname}"
                if vd.exists():
                    shutil.rmtree(vd)
                vd.mkdir(parents=True)
                for f in ("page.card", "page.data.json"):
                    shutil.copy(d / f, vd / f)
                shutil.copytree(d / "kit", vd / "kit")
                data = vd / "page.data.json"
                data.write_text(data.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{art_port}"))
                vh = int(chv if vname else h_) + 16
                for vw in VW:
                    png = render(vd, ports[vw == 540], vw, vh)
                    ok, mean = (False, None)
                    if png and Path(png).is_file():
                        out = d / "variants" / f"{vname}-{vw}.png"
                        out.parent.mkdir(exist_ok=True)
                        shutil.copy(png, out)
                        ok, mean = rightmost_ok(out, vw, vh)
                    variants[f"{vname}-{vw}"] = {"png": str(out) if png else None,
                                                 "rightmost4px_ok": ok, "mean": mean}
                # restore base compile state for the next variant build
                (d / "contract.json").write_text(json.dumps(doc, indent=2) + "\n")
                (d / "mapped.json").write_text(json.dumps(doc, indent=2) + "\n")
                compile_component(d)
            # review sheets: ref crop | native 360 | native 540 (long variant)
            ref_img = Image.open(d / "reference.png")
            row = [ref_img]
            for vw in VW:
                p = d / "variants" / f"long-{vw}.png"
                row.append(Image.open(p).convert("RGB"))
            hh = max(i.height for i in row)
            sheet = Image.new("RGB", (sum(i.width for i in row) + 20, hh), (255, 0, 255))
            x = 0
            for i in row:
                sheet.paste(i, (x, 0))
                x += i.width + 10
            sheet.save(d / "review-long.png")
            results[comp] = variants
            print(comp, json.dumps(variants, indent=1))
    finally:
        httpd.shutdown()
    json.dump(results, open(SETUP / "evidence" / "gate-b" / "components-rightmost.json", "w"), indent=1)
    print("rightmost-4px results ->", SETUP / "evidence/gate-b/components-rightmost.json")
