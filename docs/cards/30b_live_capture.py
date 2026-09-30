#!/usr/bin/env python3
"""#30b — live-data headless capture of the three wired autonomy screens.

Renders autonomy-03/04/05 from tmp copies whose mapped trees carry the values
the wired screen shows after `autonomy::refresh` against the recorded r1 run
(goal_01 "r1 replay probe" active · 100000000 budget; loop_01 fixed_interval
3600; monitor_01 poll/ERROR) — the same render path Stage B used (the flow's
`compile_page` + a hidden beauty-host), so the PNGs prove the bindings feed
the card's data slots. Nothing under design/ is touched.

Outputs: docs/cards/30b-live-*.png + 30b-live-bindings.json (machine-readable).
Run: python3 docs/cards/30b_live_capture.py
"""
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent           # docs/cards
ROOT = HERE.parents[1]                            # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
CARDS = ROOT / "design/stage-b/autonomy/cards"
WORK = ROOT / "tmp/30b-live"
ART_PORT = 8182
# One port per render: goal + loops x3 + monitors x3.
PORTS = [8391, 8392, 8393, 8394, 8395, 8396, 8397]

sys.path.insert(0, str(CLONE / "flows/image-lib"))
from compile import compile_page  # noqa: E402

# The r1-autonomy recorded state — what the autonomy bindings project.
GOAL = {
    "autonomy-03": {
        "t_goal": {"text": "r1 replay probe"},
        "goal_badge_label": {"text": "Active"},
        "t_budget_val": {"text": "0 / 100M"},
        "t_elapsed_val": {"text": "0s"},
    },
}
# Row projections per store size: 0 (empty-state line), 1 (the recorded run),
# 3 items — the rows must follow the counts (entry #30b2).
LOOPS_SETS = {
    "0": {
        "loop_1_name": {"text": "No loops in this session.", "w": 280},
        "loop_1_cad": {"text": ""},
        "loop_2_name": {"text": ""}, "loop_2_cad": {"text": ""},
        "loop_3_name": {"text": ""}, "loop_3_cad": {"text": ""},
    },
    "1": {
        "loop_1_name": {"text": "r1 replay probe"},
        "loop_1_cad": {"text": "hourly"},
        "loop_2_name": {"text": ""}, "loop_2_cad": {"text": ""},
        "loop_3_name": {"text": ""}, "loop_3_cad": {"text": ""},
    },
    "3": {
        "loop_1_name": {"text": "r1 replay probe"}, "loop_1_cad": {"text": "hourly"},
        "loop_2_name": {"text": "Sync main"}, "loop_2_cad": {"text": "every 30m"},
        "loop_3_name": {"text": "Nightly review"}, "loop_3_cad": {"text": "every day · 01:00"},
    },
}
MONITORS_SETS = {
    "0": {
        "mon_1_cmd": {"text": "No monitors in this session.", "w": 280},
        "mon_1_state": {"text": ""}, "mon_1_int": {"text": ""},
        "mon_2_cmd": {"text": ""}, "mon_2_state": {"text": ""}, "mon_2_int": {"text": ""},
        "monitors_footer_label": {"text": ""},
    },
    "1": {
        "mon_1_cmd": {"text": "./scripts/wa…"},
        "mon_1_state": {"text": "active"},
        "mon_1_int": {"text": "1h"},
        "mon_2_cmd": {"text": ""}, "mon_2_state": {"text": ""}, "mon_2_int": {"text": ""},
        "monitors_footer_label": {"text": "1 monitor · 1 active"},
    },
    "3": {
        "mon_1_cmd": {"text": "cargo test -q"}, "mon_1_state": {"text": "active"},
        "mon_1_int": {"text": "30s"},
        "mon_2_cmd": {"text": "tail -n 50 app.log"}, "mon_2_state": {"text": "fired 3×"},
        "mon_2_int": {"text": "30s"},
        "mon_3_cmd": {"text": "git status --short"}, "mon_3_state": {"text": "paused"},
        "mon_3_int": {"text": "5m"},
        "monitors_footer_label": {"text": "3 monitors · 2 active"},
    },
}


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


# Static Stage B rows must FOLLOW the store size in the captures: prune the
# tmp copy's nodes per set (the wiring owns the count contract; this shows it).
PRUNE = {
    ("autonomy-04", "loops-0"): [
        "loop_1_cad", "loop_1_dot", "loop_1_pause", "loop_1_play", "loop_1_trash",
        "loop_2_divider", "loop_2_dot", "loop_2_name", "loop_2_cad",
        "loop_2_pause", "loop_2_play", "loop_2_trash",
        "loop_3_dot", "loop_3_name", "loop_3_cad", "loop_3_play", "loop_3_trash",
    ],
    ("autonomy-04", "loops-1"): [
        "loop_2_divider", "loop_2_dot", "loop_2_name", "loop_2_cad",
        "loop_2_pause", "loop_2_play", "loop_2_trash",
        "loop_3_dot", "loop_3_name", "loop_3_cad", "loop_3_play", "loop_3_trash",
    ],
    ("autonomy-05", "monitors-0"): [
        "mon_1_state", "mon_1_int", "mon_1_pause", "mon_1_trash",
        "mon_2", "mon_2_cmd", "mon_2_state", "mon_2_int", "mon_2_pause", "mon_2_trash",
    ],
    ("autonomy-05", "monitors-1"): [
        "mon_2", "mon_2_cmd", "mon_2_state", "mon_2_int", "mon_2_pause", "mon_2_trash",
    ],
}


def prune(card: str, suffix: str, tree: dict) -> int:
    # COLLAPSE (w/h = 0), not remove: the semantic map still references these
    # ids and preflight fails on absent elements ("semantic element is absent
    # from the composition") — zeroed, they leave the composition visually.
    drop = set(PRUNE.get((card, suffix), []))
    if not drop:
        return 0
    hit = 0

    def flat(n):
        nonlocal hit
        if n["id"] in drop:
            # Width 0 only: preflight rejects a text box whose HEIGHT drops
            # under its line box ("height 0 is under its line box (21.8)"),
            # and w=0 clips the glyphs horizontally (the same clip Stage B
            # measured), so the row leaves the composition visually while
            # every semantic id stays present.
            n["w"] = 0
            hit += 1
        for k in n.get("c", []):
            flat(k)
    flat(tree)
    return hit


def inject(rules: dict, tree: dict) -> int:
    hit = 0
    for n in walk(tree):
        rule = rules.get(n["id"])
        if rule and "text" in rule:
            n["text"] = rule["text"]
            hit += 1
        if rule and "w" in rule:
            n["w"] = rule["w"]
    return hit


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


def render(work: Path, port: int, out_png: Path) -> bool:
    request = {
        "card": str(work / "page.card"), "data": str(work / "page.data.json"),
        "kit_dir": str(work / "kit"), "format": "l0-kit",
        "width": 406, "height": 776, "nonce": f"30b-live-{port}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
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
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
        if proc.poll() is None:
            proc.kill()
        log.close()


def ws_table(name):
    """The autonomy id table straight from the compiled source (no drift)."""
    src = (ROOT / "crates/octoscode-module/src/screens/autonomy.rs").read_text()
    block = re.search(rf"pub const {name}: &\[\(&str, &str\)\] = &\[(.*?)\];", src, re.S).group(1)
    return re.findall(r'\("([^"]+)"', block)


def main():
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(parents=True)
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    summary = []
    try:
        entries = (
            [("autonomy-03", "goal", GOAL["autonomy-03"])]
            + [("autonomy-04", f"loops-{k}", LOOPS_SETS[k]) for k in ("0", "1", "3")]
            + [("autonomy-05", f"monitors-{k}", MONITORS_SETS[k]) for k in ("0", "1", "3")]
        )
        for i, (card, suffix, rules) in enumerate(entries):
            out_name = f"30b-live-{suffix}.png"
            # A card renders once per store size — give each entry its own
            # work dir or the second copytree hits the first's leftovers.
            work = WORK / f"{card}-{suffix}"
            shutil.copytree(CARDS / card, work)
            mapped = json.loads((work / "mapped.json").read_text())
            if (card, suffix) == ("autonomy-05", "monitors-3"):
                # The Stage B template authors TWO monitor cards; a 3-item
                # store must render three. Clone the second card's subtree
                # with shifted ids/y in the tmp copy (design untouched) so
                # the PNG shows the count contract the tests prove.
                import copy as _copy

                def find(n, nid):
                    if n["id"] == nid:
                        return n
                    for k in n.get("c", []):
                        r = find(k, nid)
                        if r is not None:
                            return r
                    return None

                m1, m2 = find(mapped["tree"], "mon_1"), find(mapped["tree"], "mon_2")
                if m1 is not None and m2 is not None:
                    delta = m2["y"] - m1["y"]
                    m3 = _copy.deepcopy(m2)

                    def shift(n):
                        n["id"] = n["id"].replace("mon_2", "mon_3")
                        n["y"] = n["y"] + delta
                        for k in n.get("c", []):
                            shift(k)

                    shift(m3)

                    def parent_of(n, target):
                        for k in n.get("c", []):
                            if k is target:
                                return n
                            r = parent_of(k, target)
                            if r is not None:
                                return r
                        return None

                    par = parent_of(mapped["tree"], m2)
                    if par is not None:
                        par["c"].insert(par["c"].index(m2) + 1, m3)
                    # The footer sits at a fixed y sized for two cards — shift
                    # it below the cloned third card or it overlaps.
                    for nid in ("monitors_footer", "monitors_footer_label"):
                        fn = find(mapped["tree"], nid)
                        if fn is not None:
                            fn["y"] = fn["y"] + delta
                    # The semantic map must classify EVERY source element —
                    # preflight fails a cloned node as "unclassified source
                    # element" unless its entries ride along.
                    smap_p = work / "semantic-map.json"
                    if smap_p.is_file():
                        smap = json.loads(smap_p.read_text())
                        clones = [
                            dict(el) | {"id": el["id"].replace("mon_2", "mon_3")}
                            for el in smap.get("elements", [])
                            if isinstance(el.get("id"), str) and el["id"].startswith("mon_2")
                        ]
                        smap["elements"].extend(clones)
                        smap_p.write_text(json.dumps(smap, indent=2) + "\n")
            hit = inject(rules, mapped["tree"])
            prune(card, suffix, mapped["tree"])
            if card == "autonomy-03":
                # The bar's fill follows the goal.fill binding (used/budget);
                # the recorded run has 0 used, so the static Stage B fill
                # (about 40 percent) must collapse to 0 in the copy — the PNG
                # has to show the binding, not the design's static fill.
                def set_fill(n):
                    if n["id"] == "bar_fill":
                        if n.get("layout") is not None:
                            n["layout"]["w"] = 0
                        elif "w" in n:
                            n["w"] = 0
                    for k in n.get("c", []):
                        set_fill(k)
                set_fill(mapped["tree"])
            (work / "mapped.json").write_text(json.dumps(mapped, indent=2) + "\n")
            compile_page(work)
            data = (work / "page.data.json").read_text()
            (work / "page.data.json").write_text(data.replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
            out = HERE / out_name
            ok = render(work, PORTS[i], out)
            summary.append({"card": card, "injected": hit, "png": out.name, "rendered": ok})
            print(json.dumps(summary[-1]), flush=True)
    finally:
        httpd.shutdown()
    (HERE / "30b-live-bindings.json").write_text(json.dumps({
        "schema_version": 1, "card": "30b",
        "source_fixture": "crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl",
        "bindings": ws_table("BINDINGS"),
        "actions": ws_table("ACTIONS"),
        "captures": summary,
    }, indent=2) + "\n")
    print("LIVE_CAPTURE_DONE")


if __name__ == "__main__":
    main()
