#!/usr/bin/env python3
"""A8 — numeric /snap checks for one captured state (the `checks` cell of a
docs/ux-scores.csv row), read from the `/snap?all=1` dump the A8 walks save
next to each capture (`<mode>-<name>.json`).

usage: a8_ux_checks.py <snap.json> <container id> [frame id] [prefix,...]

  container  the surface being scored (`b3_dialog`, `b3_strip`, `set_page`...)
  frame      what its gutters are measured against (default `b3_root`; any
             laid-out widget id, e.g. `OctoscodeView`)
  prefix     the id families of the container's subtree (default: the
             container id's first `_` segment, e.g. `b3`). The snapshot is
             the widget tree in draw order, so the subtree is the run of
             widgets after the container whose ids carry those prefixes (or
             none): what is drawn UNDER a modal never counts.

Checks, over the visible widgets inside the container:
  * gutters: the container's left/right/top/bottom distance to the frame;
  * overflow-x: widgets running past the container's left/right edge;
  * clip: labels cut by the container (outside it) — a label cut only by a
    scroll body's edge is scrolled content, counted apart as `scrolled`;
  * truncated: labels drawn with an ellipsis ("…");
  * taps: hit targets, and how many are < 28 px in either axis (a target cut
    by the scroll body's edge is not measured);
  * tap-overlap / text-overlap: partial overlaps (one rect neither inside nor
    outside the other) between two taps / two labels.

Prints a PASS/FAIL line per gate and the one-line summary. Exit 0 when no
overflow, clip, small tap or overlap was found. Never prints input values.
"""
import json
import sys

TOL = 1.0
TAPS = {"Button", "DesignNativeButton", "CheckBox", "Toggle"}
TEXTS = {"Label", "TextInput"}


def load(path, cid, prefixes):
    """(container, its laid-out subtree, every laid-out widget)."""
    raw = json.load(open(path)).get("s", [])
    def row(w):
        return {"i": str(w.get("i") or ""), "ty": w.get("ty") or "", "r": w.get("r") or [0, 0, 0, 0], "t": w.get("t") or ""}
    def shown(w):
        r = w.get("r") or [0, 0, 0, 0]
        return r[2] > 0 and r[3] > 0 and w.get("v", 1) != 0
    at = next((n for n, w in enumerate(raw) if w.get("i") == cid and shown(w)), None)
    if at is None:
        return None, [], [row(w) for w in raw if shown(w)]
    sub = []
    for w in raw[at + 1:]:
        i = str(w.get("i") or "")
        if i not in ("", "-") and not i.startswith(prefixes) and not i[:1].isdigit():
            break
        if shown(w):
            sub.append(row(w))
    return row(raw[at]), sub, [row(w) for w in raw if shown(w)]


def find(rows, wid):
    hits = [w for w in rows if w["i"] == wid]
    return hits[0] if hits else None


def inside(a, b, tol=TOL):
    return a[0] >= b[0] - tol and a[1] >= b[1] - tol and a[0] + a[2] <= b[0] + b[2] + tol and a[1] + a[3] <= b[1] + b[3] + tol


def intersects(a, b):
    return a[0] < b[0] + b[2] - TOL and b[0] < a[0] + a[2] - TOL and a[1] < b[1] + b[3] - TOL and b[1] < a[1] + a[3] - TOL


def partial(a, b):
    return intersects(a, b) and not inside(a, b) and not inside(b, a)


def on_edge(r, body):
    """`r` is cut by the scroll body's top or bottom edge."""
    return body is not None and (abs(r[1] - body[1]) <= TOL or abs((r[1] + r[3]) - (body[1] + body[3])) <= TOL)


def main():
    path, cid = sys.argv[1], sys.argv[2]
    fid = sys.argv[3] if len(sys.argv) > 3 else "b3_root"
    prefixes = tuple(sys.argv[4].split(",")) if len(sys.argv) > 4 else (cid.split("_")[0],)
    c, members, rows = load(path, cid, prefixes)
    f = find(rows, fid)
    if c is None:
        print(f"FAIL container {cid} is not laid out")
        sys.exit(2)
    cr = c["r"]
    body = (find(members, "b3_scroll") or {}).get("r")
    gut = ""
    if f is not None:
        fr = f["r"]
        gut = "gutters L{:.0f} R{:.0f} T{:.0f} B{:.0f}".format(
            cr[0] - fr[0], fr[0] + fr[2] - cr[0] - cr[2], cr[1] - fr[1], fr[1] + fr[3] - cr[1] - cr[3])
    over = [w for w in members if w["r"][0] < cr[0] - TOL or w["r"][0] + w["r"][2] > cr[0] + cr[2] + TOL]
    texts = [w for w in members if w["ty"] in TEXTS and w["t"]]
    clipped = [w for w in texts if not inside(w["r"], cr)]
    scrolled = [w for w in texts if on_edge(w["r"], body)]
    trunc = [w for w in texts if w["t"].rstrip().endswith("…") and w["ty"] == "Label"]
    taps = [w for w in members if w["ty"] in TAPS]
    small = [w for w in taps if (w["r"][2] < 28 - TOL or w["r"][3] < 28 - TOL) and not on_edge(w["r"], body)]
    tap_ov = [(a["i"], b["i"]) for n, a in enumerate(taps) for b in taps[n + 1:] if partial(a["r"], b["r"])]
    txt_ov = [(a["i"], b["i"]) for n, a in enumerate(texts) for b in texts[n + 1:]
              if partial(a["r"], b["r"]) and not on_edge(a["r"], body) and not on_edge(b["r"], body)]
    gates = [
        ("overflow-x", not over, [w["i"] for w in over][:4]),
        ("clip", not clipped, [w["i"] for w in clipped][:4]),
        ("taps >= 28", not small, [(w["i"], [round(v) for v in w["r"]]) for w in small][:4]),
        ("tap-overlap", not tap_ov, tap_ov[:3]),
        ("text-overlap", not txt_ov, txt_ov[:3]),
    ]
    for name, ok, detail in gates:
        print(("PASS " if ok else "FAIL ") + name + ("" if ok else f" {detail}"))
    if trunc:
        print("NOTE truncated: " + ", ".join(f"{w['i']}={w['t']!r}" for w in trunc[:4]))
    summary = (f"{cid} {cr[2]:.0f}x{cr[3]:.0f} {gut}; overflow-x {len(over)}; clip {len(clipped)}"
               f" (scrolled {len(scrolled)}); truncated {len(trunc)}; taps {len(taps)} (<28: {len(small)});"
               f" tap-overlap {len(tap_ov)}; text-overlap {len(txt_ov)}")
    print(summary)
    sys.exit(0 if all(ok for _, ok, _ in gates) else 1)


if __name__ == "__main__":
    main()
