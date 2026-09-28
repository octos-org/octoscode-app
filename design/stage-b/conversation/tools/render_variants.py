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

# Responsive flags (card #18: fill the slot width, height from content).
RESPONSIVE = {
    "approval-card": {"approval_card": {"fillw": 1, "fith": 1},
                      "reason_text": {"fillw": 1},
                      "cmd_box": {"fillw": 1}, "t02": {"fillw": 1},
                      "approve_once": {"fillw": 1}, "approve_session": {"fillw": 1},
                      "deny": {"fillw": 1}},
    "question-card": {"question_card": {"fillw": 1, "fith": 1},
                      "question_text": {"fillw": 1}, "note_box": {"fillw": 1},
                      "note_input": {"fillw": 1},
                      "opt_ledger": {"fillw": 1}, "opt_memory": {"fillw": 1},
                      "opt_ask": {"fillw": 1},
                      "submit_answer": {"fillw": 1}, "skip": {"fillw": 1}},
    "edited-files-card": {"edited_files_card": {"fillw": 1, "fith": 1},
                          "files_card": {"fillw": 1},
                          "review": {"fillw": 1}},
    "plan-card": {"plan_card": {"fillw": 1, "fith": 1},
                  "plan_steps": {"fillw": 1},
                  "step_0_label": {"fillw": 1}, "step_1_label": {"fillw": 1},
                  "step_2_label": {"fillw": 1}, "step_3_label": {"fillw": 1},
                  "step_4_label": {"fillw": 1}},
    "goal-strip": {"goal_strip": {"fillw": 1, "fith": 1}, "t01": {"fillw": 1}},
    "diff-view": {"diff_view": {"fillw": 1, "fith": 1},
                  "diff_rows": {"fillw": 1},
                  "file_header": {"fillw": 1}, "scope_pill": {"fillw": 1}},
    "settings-group": {"settings_group": {"fillw": 1, "fith": 1},
                       "perm_card": {"fillw": 1}, "model_card": {"fillw": 1},
                       "perm_divider": {"fillw": 1}},
}

VARIANTS = {
    "approval-card": {
        "short": {"text": {"t02": "git push origin feat/steer-queue",
                           "reason_text": "Reason: Push the fix branch so CI can run"}},
        "long": {"text": {"t02": "cargo test -p octos-cli steer_queue -- --nocapture",
                          "reason_text": "Run the full steer-queue integration suite before "
                                         "pushing so a regression in the durable queue is caught "
                                         "locally rather than in CI."},
                 "flags": {"approval_card": {"h": 700}, "reason_text": {"h": 130}}},
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
        "short": {"text": {"t_file": "ui_protocol.rs", "t_fadd": "+9", "t_fdel": "-1",
                           "t_fold": ":88 unmodified lines"}},
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


def apply_variant(tree, comp, variant):
    spec = VARIANTS[comp][variant]
    # Merge per NODE ID so a variant flag does not replace the responsive flags.
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
                if bot > top and right > left:
                    im = im.crop((left, top, right + 1, bot + 1))
                im.save(dest / f"native-{w}.png")
            subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                           capture_output=True)
            time.sleep(1)
        finally:
            if proc.poll() is None:
                proc.kill()
    text = (dest / f"host-{w}.log").read_text(errors="ignore")
    return {"w": w, "font_warnings": text.count("not available in this build")}


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
                    r = render(dest, PORTS[(vi * len(WIDTHS) + wi) % len(PORTS)], w, 720)
                    renders.append(r)
                    if (dest / f"native-{w}.png").exists():
                        shutil.copy(dest / f"native-{w}.png", vdir / f"{variant}-{w}.png")
                panels = [str(COMPONENTS / comp / "reference.png")]
                for w in WIDTHS:
                    panels.append(str(vdir / f"{variant}-{w}.png"))
                ok = all(Path(p).is_file() for p in panels)
                review = str(assemble(comp, variant, panels)) if ok else None
                summary[comp][variant] = {
                    "render": str(Path(review).relative_to(ROOT)) if review else None,
                    "widths": list(WIDTHS),
                    "font_warnings": sum(r.get("font_warnings", 0) for r in renders),
                }
                print(json.dumps({"component": comp, "variant": variant, "review": review,
                                  "ok": ok}, ensure_ascii=False))
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
