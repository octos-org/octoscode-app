#!/usr/bin/env python3
"""A18 — WCAG contrast of every visible text node, measured on the app's OWN pixels.

    python3 tools/judge/contrast.py live  <port> <outdir> <name> [--within ID] [--clip ID] [--exclude ID]
    python3 tools/judge/contrast.py files <snap.json> <png> [--within ID] [--clip ID] [--exclude ID] [--before F]
                                          [--exempt FG[:BG]]

The web's row (e2e/theme.spec.ts:72-112) runs axe's color-contrast rule on the conversation and on Settings
and expects ZERO violations. `/snap` carries no colour channel, so the colours come from the framebuffer
(`/g?raw=1`, the shell at its display scale) and the rects from `/snap?all=1` (logical px):

* every laid-out, visible node with text (`t`) is a candidate; a node whose rect is not fully on screen, or inside
  `--exclude`, is skipped; with `--within ID` only the container's own content counts: inside its rect AND later
  in the snap's draw order (a dialog's rect also covers the conversation drawn beneath it); a node that overlaps a
  `--clip` list (the transcript's `timeline_list`) but is not wholly inside it is a row scrolled under the header:
  skipped;
* BACKGROUND = the rect's modal colour (the surface the text is drawn on);
* TEXT = the glyph core: of the pixels that differ from the background (contrast >= 1.2), the 4% with the highest
  contrast, median per channel — antialiased edges are left out, so a 1-2 px stem still reads its own ink. Blending
  only ever moves a pixel TOWARD the background, so the measured ratio is a lower bound of the drawn colour's
  (a small face at 1x reads lighter than its token; the 2x grab reads within a few hundredths of it);
* ratio = WCAG 2.x `(L1 + 0.05) / (L2 + 0.05)` with the sRGB linearisation;
* a text node with NO ink (every pixel within 1.2:1 of the background — dark text on a dark list) is invisible:
  it FAILS with the best ratio its pixels reach.

Thresholds (WCAG 1.4.3, what axe applies): every text node is held to 4.5:1 (no height heuristic: a field's rect
includes its padding and a wrapped paragraph is tall, so "large text" cannot be told from a rect; the app's large
titles are primary ink, far above 4.5 anyway); a single-glyph label (an icon drawn as text, e.g. `+`, `›`) is a UI
glyph, 3:1 (WCAG 1.4.11). A disabled node (`enabled: false`) is exempt, as axe exempts disabled controls.

Output: `<outdir>/<name>.contrast.tsv` (one row per node), `<name>.png` (<= 1400 px wide) and `<name>.snap.json`
in `live` mode; the summary line `name: N nodes, min R:1 (text), F below threshold` on stdout.
Stdlib only (the walk interpreter has no Pillow): the PNG is decoded by tools/walk/run.py's `_decode_png`.
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.request
from collections import Counter
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../walk"))  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "walk"))
from run import _decode_png  # noqa: E402  (stdlib PNG decoder shared with the walk runner)

INK_MIN = 1.2      # a pixel this far from the background is ink (antialias fringe included)
CORE_SHARE = 0.04  # the glyph core: the highest-contrast share of the ink


def _lin(v: int) -> float:
    c = v / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


_LUM: dict = {}


def lum(rgb) -> float:
    v = _LUM.get(rgb)
    if v is None:
        v = 0.2126 * _lin(rgb[0]) + 0.7152 * _lin(rgb[1]) + 0.0722 * _lin(rgb[2])
        _LUM[rgb] = v
    return v


def ratio(a, b) -> float:
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hexc(rgb) -> str:
    return "#%02x%02x%02x" % tuple(rgb)


def shown(n: dict) -> bool:
    r = n.get("r") or [0, 0, 0, 0]
    return n.get("v", 1) != 0 and r[2] > 1 and r[3] > 1


def first_rect(nodes: list, wid: str):
    for n in nodes:
        if n.get("i") == wid and shown(n):
            return n["r"]
    return None


def first_at(nodes: list, wid: str):
    """(snap index, rect) of the first shown node `wid` — the index orders it in the draw."""
    for k, n in enumerate(nodes):
        if n.get("i") == wid and shown(n):
            return k, n["r"]
    return None


def inside(inner, outer, tol: float = 1.0) -> bool:
    return (inner[0] >= outer[0] - tol and inner[1] >= outer[1] - tol
            and inner[0] + inner[2] <= outer[0] + outer[2] + tol
            and inner[1] + inner[3] <= outer[1] + outer[3] + tol)


def overlaps(a, b) -> bool:
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def largest_run(ink_at: list, gap: int) -> list:
    """The ink of the column run holding the most ink pixels (runs are split by `gap` empty columns)."""
    cols = sorted({x for x, _ in ink_at})
    runs, start, last = [], cols[0], cols[0]
    for x in cols[1:]:
        if x - last > gap:
            runs.append((start, last))
            start = x
        last = x
    runs.append((start, last))
    best = max(runs, key=lambda ab: sum(1 for x, _ in ink_at if ab[0] <= x <= ab[1]))
    return [(x, p) for x, p in ink_at if best[0] <= x <= best[1]]


def kind_of(n: dict) -> str:
    t = (n.get("t") or "").strip()
    if len(t) == 1 and not t.isalnum():
        return "glyph"
    return "body"


def need_of(kind: str) -> float:
    return 4.5 if kind == "body" else 3.0


def node_key(n: dict) -> tuple:
    """A node's identity across two snaps: id and text (not the rect: an overlay still settling moves a pixel)."""
    return (n.get("i"), (n.get("t") or "").strip())


def measure(nodes: list, decoded, png_w: int, png_h: int, within=(), clip=(), exclude=(), before=None,
            exempt=()) -> list[dict]:
    """One dict per measured node (see the module doc for the method). `before` (the `node_key`s shown before the
    click that opened an overlay) limits the measurement to the overlay's own content: what was not there before.
    `exempt` lists `(fg, bg)` hex pairs (`bg` None = any) drawn only by DISABLED controls (the kit's disabled ink,
    `screens::theme::EXEMPT_INKS`): such a node is reported `exempt`, as axe skips a disabled control."""
    buf, stride, nch = decoded
    win = next((n["r"] for n in nodes if n.get("ty") == "Window" and shown(n)), None) or [0, 0, png_w, png_h]
    k = png_w / float(win[2])
    boxes_in = [b for b in (first_at(nodes, w) for w in within) if b]
    boxes_clip = [r for r in (first_rect(nodes, c) for c in clip) if r]
    boxes_out = [r for r in (first_rect(nodes, e) for e in exclude) if r]
    out = []
    for idx, n in enumerate(nodes):
        t = (n.get("t") or "").strip()
        if not t or not shown(n) or n.get("ty") in ("Window", "Root"):
            continue
        r = n["r"]
        if not inside(r, win, 0.5):
            continue
        if before is not None and node_key(n) in before:
            continue
        if within and not any(idx > at and inside(r, b) for at, b in boxes_in):
            continue
        if any(overlaps(r, b) and not inside(r, b) for b in boxes_clip):
            continue
        if exclude and any(inside(r, b) for b in boxes_out):
            continue
        x0, y0 = int(round(r[0] * k)), int(round(r[1] * k))
        x1, y1 = int(round((r[0] + r[2]) * k)), int(round((r[1] + r[3]) * k))
        px, xs = [], []
        for y in range(max(0, y0), min(png_h, y1)):
            row = y * stride
            for x in range(max(0, x0), min(png_w, x1)):
                o = row + x * nch
                px.append((buf[o], buf[o + 1], buf[o + 2]))
                xs.append(x)
        if not px:
            continue
        bg = Counter(px).most_common(1)[0][0]
        ink_at = [(x, p) for x, p in zip(xs, px) if ratio(p, bg) >= INK_MIN]
        if n.get("ty") == "TextInput" and ink_at:
            # A field's rect also holds its border and its icon (the sidebar
            # search's magnifier sits in the left padding): past a 2 px inset
            # (the border), the text is the column run with the most clear
            # ink (>= 1.5:1, so the border's 1.26:1 hairline cannot join the
            # runs), runs split by a gap of >= 5 logical px.
            lo, hi = x0 + int(2 * k), x1 - int(2 * k)
            clear = [(x, p) for x, p in ink_at if lo <= x < hi and ratio(p, bg) >= 1.5]
            if clear:
                ink_at = largest_run(clear, max(2, int(5 * k)))
        ink = [(ratio(p, bg), p) for _, p in ink_at]
        kind = kind_of(n)
        rec = {"id": n.get("i", ""), "ty": n.get("ty", ""), "text": t, "rect": r, "bg": hexc(bg), "kind": kind,
               "need": need_of(kind), "enabled": n.get("enabled", True) is not False}
        if len(ink) < 4:
            # Invisible text: its glyphs are within 1.2:1 of the surface.
            best = max(px, key=lambda p: ratio(p, bg))
            rec.update(fg=hexc(best), ratio=round(ratio(best, bg), 2),
                       verdict="exempt" if not rec["enabled"] else "FAIL")
            out.append(rec)
            continue
        ink.sort(key=lambda c: c[0], reverse=True)
        core = [p for _, p in ink[:max(3, int(len(ink) * CORE_SHARE))]]
        fg = tuple(sorted(c[i] for c in core)[len(core) // 2] for i in range(3))
        cr = ratio(fg, bg)
        disabled_style = any(hexc(fg) == f and (b is None or hexc(bg) == b) for f, b in exempt)
        if not rec["enabled"] or disabled_style:
            verdict = "exempt"
        else:
            verdict = "pass" if cr + 1e-9 >= rec["need"] else "FAIL"
        rec.update(fg=hexc(fg), ratio=round(cr, 2), verdict=verdict)
        out.append(rec)
    return out


def write_tsv(path: pathlib.Path, rows: list[dict]) -> None:
    lines = ["id\tty\tkind\ttext\trect\tbg\tfg\tratio\tneed\tverdict"]
    for m in rows:
        lines.append("\t".join([m["id"], m["ty"], m["kind"], " ".join(m["text"].split())[:60],
                                ",".join(str(round(v)) for v in m["rect"]), m["bg"], m["fg"],
                                f"{m['ratio']:.2f}", f"{m['need']:.1f}", m["verdict"]]))
    path.write_text("\n".join(lines) + "\n")


def summary(name: str, rows: list[dict]) -> str:
    scored = [m for m in rows if m["verdict"] in ("pass", "FAIL")]
    bad = [m for m in scored if m["verdict"] == "FAIL"]
    if not scored:
        return f"{name}: no measurable text"
    worst = min(scored, key=lambda m: m["ratio"] / m["need"])
    return (f"{name}: {len(scored)} nodes, min {worst['ratio']:.2f}:1 ({worst['text'][:28]!r} {worst['fg']} on "
            f"{worst['bg']}), {len(bad)} below threshold"
            + ("" if not bad else " — " + "; ".join(f"{m['text'][:22]!r} {m['ratio']:.2f}" for m in bad[:8])))


def grab(port: int) -> tuple[list, bytes]:
    base = f"http://127.0.0.1:{port}"
    data = b""
    for _ in range(6):  # the grab can miss a frame: retry
        try:
            with urllib.request.urlopen(base + "/g?raw=1", timeout=30) as r:
                data = r.read()
            if data[:4] == b"\x89PNG":
                break
        except Exception:  # noqa: BLE001
            pass
        time.sleep(0.6)
    with urllib.request.urlopen(base + "/snap?all=1", timeout=30) as r:
        nodes = json.loads(r.read()).get("s", [])
    return nodes, data


def measure_live(port: int, outdir, name: str, within=(), clip=(), exclude=(), before=None,
                 exempt=()) -> list[dict]:
    """Grab, measure, and keep the evidence: TSV, PNG (<= 1400 px) and the snap."""
    out = pathlib.Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    nodes, png = grab(port)
    w, h, decoded = _decode_png(png)
    rows = measure(nodes, decoded, w, h, within, clip, exclude, before, exempt)
    write_tsv(out / f"{name}.contrast.tsv", rows)
    (out / f"{name}.snap.json").write_text(json.dumps({"s": nodes}))
    if before is not None:  # an overlay: what was on screen before it opened (re-measurable with --before)
        (out / f"{name}.before.json").write_text(json.dumps(sorted([list(k) for k in before])))
    # The full-resolution grab (re-measurable with `files`), and the evidence copy <= 1400 px wide.
    (out / f"{name}.full.png").write_bytes(png)
    p = out / f"{name}.png"
    p.write_bytes(png)
    subprocess.run(["sips", "-Z", "1400", str(p)], capture_output=True)
    print(summary(name, rows), flush=True)
    return rows


def _opts(argv: list) -> tuple[list, list, list, set | None, list]:
    within, clip, exclude, before, exempt = [], [], [], None, []
    it = iter(argv)
    for a in it:
        if a == "--within":
            within.append(next(it))
        elif a == "--clip":
            clip.append(next(it))
        elif a == "--exclude":
            exclude.append(next(it))
        elif a == "--before":
            before = {tuple(k) for k in json.loads(pathlib.Path(next(it)).read_text())}
        elif a == "--exempt":  # FG or FG:BG (#rrggbb), a disabled control's style
            fg, _, bg = next(it).lower().partition(":")
            exempt.append((fg, bg or None))
    return within, clip, exclude, before, exempt


def main(argv: list) -> int:
    if len(argv) >= 4 and argv[0] == "live":
        within, clip, exclude, before, exempt = _opts(argv[4:])
        rows = measure_live(int(argv[1]), argv[2], argv[3], within, clip, exclude, before, exempt)
    elif len(argv) >= 3 and argv[0] == "files":
        within, clip, exclude, before, exempt = _opts(argv[3:])
        snap = json.loads(pathlib.Path(argv[1]).read_text())
        nodes = snap.get("s", snap) if isinstance(snap, dict) else snap
        w, h, decoded = _decode_png(pathlib.Path(argv[2]).read_bytes())
        rows = measure(nodes, decoded, w, h, within, clip, exclude, before, exempt)
        for m in rows:
            print(f"{m['verdict']:6} {m['ratio']:5.2f}/{m['need']:.1f} {m['fg']} on {m['bg']}  {m['id']}  {m['text'][:50]!r}")
        print(summary(os.path.basename(argv[2]), rows))
    else:
        print(__doc__)
        return 2
    return 1 if any(m["verdict"] == "FAIL" for m in rows) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
