#!/usr/bin/env python3
"""#28c step 2: place the five extracted setup components at design/components/
and render their short/long variants at 360 and 540 with the rightmost-4px check.

Placement mirrors conversation/tools/extract_components.py::main (staging ->
design/components/<id>/, index.json MERGED — the conversation entries must
survive). Variants re-compile with the flow's own compiler
(flows/image-lib/compile.py::compile_page) and render hidden with beauty-host on
this card's ad-hoc ports (8391..8394, GUIDE.md "manual / ad-hoc" block).
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

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/setup
ROOT = Path(__file__).resolve().parents[4]
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
STAGE = HERE / "pipeline-output/service-cards"
OUT = ROOT / "design/components"
WORK = ROOT / "tmp/stage-b/render-variants-setup"
ART_PORT = 8182
PORTS = [8391, 8392, 8393, 8394]
WIDTHS = (360, 540)

sys.path.insert(0, str(CLONE / "flows/image-lib"))
from compile import compile_page   # noqa: E402

# text/geometry-level variant data per component (node ids come from the setup
# scenes; structural variants are out of scope here and none is required).
VARIANTS = {
    "labeled-input": {
        "short": {"server_field": {"text": "http://127.0.0.1:50190"}},
        "long": {"server_field": {"text": "••••••••••"}},
    },
    "provider-row": {
        "short": {"t_prov_deepseek": {"text": "DeepSeek • Official API"}},
        "long": {"t_prov_deepseek": {"text": "Kimi Coding Plan"}},
    },
    "ws-group": {
        "short": {"t_ws0_name": {"text": "octos"}, "t_ws0_path": {"text": "~/home/octos"}},
        "long": {"t_ws0_name": {"text": "p0-proto2"}, "t_ws0_path": {"text": "~/home/octos/p0-proto2"}},
    },
    "seg-control": {
        "short": {"seg_sel": {"x": None}},   # x=None -> keep authored (first segment)
        "long": {"seg_sel": {"x": "second"}},  # move the selection to segment 2
    },
    "settings-toggle-row": {
        "short": {"toggle1": {"bg": "blue", "knob_dx": 0}},
        "long": {"toggle1": {"bg": "hair", "knob_dx": -26}},
    },
}
BINDINGS = {
    "labeled-input": ["label", "value", "action:submit"],
    "provider-row": ["label", "selected", "action:pick"],
    "ws-group": ["rows", "action:open"],
    "seg-control": ["options", "selected", "action:pick"],
    "settings-toggle-row": ["label", "on", "action:toggle"],
}


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def place():
    catalogue = json.loads((STAGE / "catalogue.json").read_text())
    index_path = OUT / "index.json"
    index = json.loads(index_path.read_text()) if index_path.exists() else {
        "schema_version": 1, "kind": "octoscode-l0-components", "note": "", "components": []}
    keep = {c["id"]: c for c in index["components"]}
    placed = []
    for card in catalogue["cards"]:
        cid = card["id"]
        dest = OUT / cid
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(STAGE / card["folder"], dest)
        entry = {"id": cid, "source_scene": card["scene"].zfill(2),
                 "extracted_from": f"scene {card['scene']} root '{card['root']}'",
                 "owner": card["owner"], "bindings": BINDINGS[cid],
                 "artboard": card["artboard"], "nodes": card["nodes"],
                 "native_controls": card["native_controls"],
                 "card": str((dest / "page.card").relative_to(ROOT)),
                 "data": str((dest / "page.data.json").relative_to(ROOT)),
                 "render": [str((dest / "reference.png").relative_to(ROOT))]}
        keep[cid] = entry
        placed.append(cid)
    index["components"] = [keep[cid] for cid in keep]
    index["note"] = ("Reusable per-item components compiled from the approved conversation (card #18) "
                     "and setup (card #28c) scenes. Width-responsive: fill the slot width, height from content.")
    index_path.write_text(json.dumps(index, indent=2) + "\n")
    print("placed:", ", ".join(placed))


def apply_variant(comp_id, variant, tree):
    """Apply text/geometry edits to the mapped tree.

    Returns [(prev_text, new_text)] for data-file replacement: input VALUES are
    mounted from page.data.json (the data binding), so a tree-only text edit on
    an input does not reach the render.
    """
    edits = VARIANTS[comp_id][variant]
    data_repl = []
    for n in walk(tree):
        rule = edits.get(n.get("id"))
        if not rule:
            continue
        if "text" in rule:
            prev = n.get("text")
            n["text"] = rule["text"]
            if prev and prev != rule["text"]:
                data_repl.append((prev, rule["text"]))
        if "bg" in rule:
            n["bg"] = 0xFF2F6FEB if rule["bg"] == "blue" else 0xFFE5E5E7
        if "knob_dx" in rule:
            for c in n.get("c", []):
                c["x"] = c["x"] + rule["knob_dx"]
    if comp_id == "seg-control":
        sel = next((n for n in walk(tree) if n["id"] == "seg_sel"), None)
        labels = sorted((n for n in walk(tree) if n["id"].startswith("t_seg")),
                        key=lambda n: n["x"])
        if sel is not None and len(labels) >= 2:
            mid = labels[1]
            sel["x"], sel["w"] = mid["x"] - 10, mid["w"] + 20
    return data_repl


def right_edge_ok(png):
    im = Image.open(png).convert("RGB")
    col = im.crop((im.width - 4, 0, im.width, im.height))
    px = list(col.getdata())
    mean = [sum(p[i] for p in px) / len(px) for i in range(3)]
    spread = max(max(abs(p[i] - mean[i]) for p in px) for i in range(3))
    return spread <= 8, int(spread)


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render_card(card, data, kit, port, w, h, out_png):
    reqdir = out_png.parent
    request = {"card": str(card), "data": str(data), "kit_dir": str(kit),
               "format": "l0-kit", "width": w, "height": h,
               "nonce": f"setup-variant-{port}", "result": str(reqdir / "native.json"),
               "layout": str(reqdir / "layout.json"), "actions": str(reqdir / "actions.json")}
    (reqdir / "request.json").write_text(json.dumps(request))
    log = (reqdir / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(reqdir / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        if not wait_port(port):
            return False
        time.sleep(5)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, out_png)
            return True
        return False
    finally:
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"], capture_output=True)
        time.sleep(1)
        if proc.poll() is None:
            proc.kill()
        log.close()


def main():
    place()
    EVID = HERE / "evidence/gate-b"
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(parents=True)
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    summary = []
    try:
        for i, comp_id in enumerate(VARIANTS):
            src = OUT / comp_id
            port = PORTS[i % len(PORTS)]
            for variant in ("short", "long"):
                work = WORK / f"{comp_id}-{variant}"
                shutil.copytree(src, work)
                tree = json.loads((work / "mapped.json").read_text())
                data_repl = apply_variant(comp_id, variant, tree["tree"])
                (work / "mapped.json").write_text(json.dumps(tree, indent=2) + "\n")
                data = (work / "page.data.json").read_text()
                for prev, new in data_repl:
                    data = data.replace(prev, new)
                (work / "page.data.json").write_text(data)
                compile_page(work)
                # compile_page REGENERATES page.data.json/page.card from the tree
                # and bindings, so the variant's data-level edits must land AFTER
                # it (input values are mounted from data, not the tree alone).
                data = (work / "page.data.json").read_text()
                card_txt = (work / "page.card").read_text()
                for prev, new in data_repl:
                    data = data.replace(prev, new)
                    card_txt = card_txt.replace(prev, new)
                (work / "page.data.json").write_text(data)
                (work / "page.card").write_text(card_txt)
                shots = []
                ok_edge = True
                for j, w in enumerate(WIDTHS):
                    out = work / "variants" / f"{variant}-{w}.png"
                    out.parent.mkdir(exist_ok=True)
                    (work / "page.data.json").write_text(
                        (work / "page.data.json").read_text().replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
                    good = render_card(work / "page.card", work / "page.data.json",
                                       work / "kit", port, w, 776, out)
                    edge, spread = (right_edge_ok(out) if good else (False, -1))
                    ok_edge = ok_edge and edge
                    shots.append((out, w, spread))
                ref = Image.open(src / "reference.png").convert("RGB").resize((406, 776))
                panels = [ref] + [Image.open(p).convert("RGB") for p, _, _ in shots]
                total_w = sum(min(p.width, 540) for p in panels) + 20 * (len(panels) + 1)
                H = 776 + 60
                sheet = Image.new("RGB", (total_w, H), (255, 255, 255))
                d = ImageDraw.Draw(sheet)
                x = 20
                labels = ["atlas crop"] + [f"{variant}@{w}" for _, w, _ in shots]
                for panel, label in zip(panels, labels):
                    sheet.paste(panel, (x, 40))
                    d.text((x, 15), label, fill=(0, 0, 0))
                    x += panel.width + 20
                review = src / f"review-{variant}.png"
                sheet.save(review)
                summary.append({"component": comp_id, "variant": variant,
                                "right_edge_ok": ok_edge,
                                "spreads": {str(w): s for _, w, s in shots},
                                "review": str(review.relative_to(ROOT))})
                print(json.dumps(summary[-1]), flush=True)
    finally:
        httpd.shutdown()
    (EVID / "variants-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("VARIANTS_DONE")


if __name__ == "__main__":
    main()
