#!/usr/bin/env python3
"""Card #21b step 2 — the placeholder gate.

Runs over a headless capture (the app's own `/g` PNG + the `/snap` JSON beside
it) and decides, per item, whether the *real* #16 component rendered or the #17
placeholder did.

What it checks, and why each signal is (or is not) trustworthy:

1. `placeholder_dsl` — the lowered `item_splash` DSL root. #17's placeholder is
   `DesignSurface { width: 360 height: 40 … draw_bg.radius: 0 }` for every id
   (#16's real components carry their measured sizes, e.g. user-bubble
   `284.01 x 85.09 radius 16`, and worked-for/thread-row lower to `KitButton`).
   A `width: 360 height: 40` root IS the placeholder. This is the reliable
   signal, and it is read from the DSL, not from pixels.

2. `kind_label` — a widget whose `t` is exactly a component kind name. Card #21b
   suggested grepping `/g` text for "the kind names used as labels". **This is a
   false positive by construction**: `lib.rs:474`
   (`item.label(cx, ids!(item_kind)).set_text(cx, row.kind.id())`) writes the
   kind id into a native debug `Label` on EVERY row, real or placeholder. The
   count is reported for information and must NOT be used to fail a capture.

3. `item_ink` — dark pixels inside the item's own Splash rect. Card #21b asked to
   "sample pixels: the placeholder's grey gradient". **This cannot discriminate
   here**: measured on the same binary, a real-component run and a forced
   placeholder run differ by 0.08 % of pixels (see the report) — neither paints
   item content, so the pixel signal is blind. Reported for information only.

Exit code is 1 iff any item lowered the placeholder root (signal 1).
"""
import csv
import json
import sys

KINDS = [
    "thread-row", "user-bubble", "assistant-prose", "tool-cell", "working-row",
    "worked-for", "answer-actions", "composer", "new-chat",
]
PLACEHOLDER_ROOT = "width: 360 height: 40"


def analyse(png, snap_json, window, components_root, out_rows):
    import numpy as np
    from PIL import Image

    snap = json.load(open(snap_json))
    widgets = snap.get("s", [])

    # --- signal 1: the lowered DSL roots (reliable) -------------------------
    splashes = [w for w in widgets if w.get("ty") == "Splash" and w.get("t")]
    placeholder_roots = sum(PLACEHOLDER_ROOT in (w.get("t") or "") for w in splashes)

    # --- signal 2: kind-name labels (informational; always present) ---------
    kind_labels = sum(
        (w.get("t") or "").strip() in KINDS for w in widgets
    )

    # --- signal 3: dark ink inside each item Splash rect (informational) ----
    im = np.asarray(Image.open(png).convert("L")).astype(int)
    scale = im.shape[1] / 1280.0  # the grab is 2x DPI; the window is `window` wide
    printed = 0
    for w in splashes:
        if str(w.get("i")) != "item_splash":
            continue
        x, y, ww, hh = [int(v) for v in w["r"]]
        if ww <= 0 or hh <= 0:
            continue
        reg = im[int(y * scale):int((y + hh) * scale), int(x * scale):int((x + ww) * scale)]
        ink = int((reg < 120).sum())
        dsl = (w.get("t") or "")
        printed += 1
        out_rows.append({
            "window": window,
            "components_root": components_root,
            "item_index": printed - 1,
            "component": dsl.split(" ", 1)[0],
            "placeholder_dsl": int(PLACEHOLDER_ROOT in dsl),
            "kind_label_hits_on_screen": kind_labels,
            "item_splash_dark_px": ink,
            "verdict": "placeholder" if PLACEHOLDER_ROOT in dsl else "real-dsl",
        })

    if not splashes:
        out_rows.append({
            "window": window, "components_root": components_root,
            "item_index": -1, "component": "(no splash)",
            "placeholder_dsl": 0, "kind_label_hits_on_screen": kind_labels,
            "item_splash_dark_px": -1, "verdict": "no-splash",
        })
    return placeholder_roots


def main():
    # capture basename -> (window, components root read from the app log)
    caps = json.load(open(sys.argv[1]))
    out = sys.argv[2]
    rows = []
    failed = False
    for png, snap, window, root in caps:
        n = analyse(png, snap, window, root, rows)
        if n:
            failed = True
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {out} ({len(rows)} rows)")
    for r in rows:
        print("  ", r["window"], r["component"], r["verdict"],
              "placeholder_dsl=%d" % r["placeholder_dsl"],
              "kind_labels=%d" % r["kind_label_hits_on_screen"],
              "ink=%d" % r["item_splash_dark_px"])
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
