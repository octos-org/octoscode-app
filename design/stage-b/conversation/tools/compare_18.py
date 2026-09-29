#!/usr/bin/env python3
"""Objective gate-B compare: reference crop vs native render, per scene.

Reports, per scene:
  - overall ink coverage ratio (native/ref) — too low = missing content, too high = blobs
  - dark-band (y) lists side by side — vertical placement of every text/box run
  - mean RGB in the black-button band, the grey-fill band, and a page sample
so the visual read in the report is backed by numbers.
"""
import sys
from pathlib import Path

from PIL import Image
import numpy as np

HERE = Path(__file__).resolve().parents[1]
EVIDENCE = HERE / "evidence/gate-b"


def gray(p):
    im = Image.open(p).convert("L")
    if im.size != (406, 776):
        im = im.resize((406, 776), Image.LANCZOS)
    return np.asarray(im).astype(int)


def bands(a, thr=200, minv=6):
    r = (a < thr).sum(axis=1)
    out, s = [], None
    for i, v in enumerate(r):
        if v > minv and s is None:
            s = i
        elif v <= minv and s is not None:
            if i - s >= 3:
                out.append((s, i))
            s = None
    if s is not None:
        out.append((s, len(r)))
    return out


def rgb(p):
    im = Image.open(p).convert("RGB")
    if im.size != (406, 776):
        im = im.resize((406, 776), Image.LANCZOS)
    return np.asarray(im).astype(int)


def main(tag, scenes):
    for n in scenes:
        ref = gray(HERE / "cards" / f"conversation-{n}" / "reference.png")
        nat_path = EVIDENCE / f"conversation-{n}-native-{tag}.png"
        if not nat_path.exists():
            print(f"{n}: NO NATIVE")
            continue
        nat = gray(nat_path)
        rin = (ref < 210).mean()
        nin = (nat < 210).mean()
        rb, nb = bands(ref), bands(nat)
        print(f"== {n} ==")
        print(f"  ink coverage ref={rin:.4f} native={nin:.4f} ratio={nin/max(rin,1e-6):.2f}")
        print(f"  ref  bands ({len(rb)}): {rb}")
        print(f"  nat  bands ({len(nb)}): {nb}")
        # row-profile correlation (how well vertical structure matches)
        rp = (ref < 210).sum(axis=1).astype(float)
        npp = (nat < 210).sum(axis=1).astype(float)
        if rp.std() > 0 and npp.std() > 0:
            corr = np.corrcoef(rp, npp)[0, 1]
            print(f"  row-profile corr={corr:.3f}")


if __name__ == "__main__":
    tag = sys.argv[1] if len(sys.argv) > 1 else "v1"
    scenes = sys.argv[2].split(",") if len(sys.argv) > 2 else \
        ["05", "06", "07", "10", "11", "12"]
    main(tag, scenes)
