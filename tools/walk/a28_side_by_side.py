#!/usr/bin/env python3
"""A28 — board 4 (design/stage-a/phase4-new4/atlas.png, APPROVED) next to the
native captures of the a28_diff_words walk, for the UX score (brief §2).

  python3 tools/walk/a28_side_by_side.py <walk dir prefix> <out dir>

Reads `<prefix>-desktop/`, `<prefix>-phone/`, `<prefix>-zh-desktop/`,
`<prefix>-zh-phone/` (the walk's outdirs) and writes, at most 1400 px wide:

  sbs-1-words-desktop.png     region 1  (30,117-1229,1035)  | words-desktop
  sbs-1b-large-desktop.png    region 1b (30,1051-1229,1257) over large-desktop
  sbs-2-words-phone.png       region 2  (1298,121-1722,1071) | words-phone | words-scrolled-phone
  sbs-2-zh-phone.png          region 2 | zh words-phone | zh large-phone
  sbs-1-zh-desktop.png        region 1 | zh large-desktop
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ATLAS = os.path.join(ROOT, "design", "stage-a", "phase4-new4", "atlas.png")
REGIONS = {"1": (30, 117, 1229, 1035), "1b": (30, 1051, 1229, 1257), "2": (1298, 121, 1722, 1071)}
GAP, LABEL_H, BG = 16, 30, (246, 246, 248)


def font():
    for f in ("/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Helvetica.ttc"):
        if os.path.exists(f):
            return ImageFont.truetype(f, 18)
    return ImageFont.load_default()


def labelled(img, text):
    out = Image.new("RGB", (img.width, img.height + LABEL_H), BG)
    out.paste(img, (0, LABEL_H))
    ImageDraw.Draw(out).text((6, 6), text, fill=(60, 60, 67), font=font())
    return out


def scale_h(img, h):
    return img.resize((max(1, round(img.width * h / img.height)), h), Image.LANCZOS)


def scale_w(img, w):
    return img.resize((w, max(1, round(img.height * w / img.width))), Image.LANCZOS)


def row(panels, h):
    ps = [labelled(scale_h(p, h), t) for p, t in panels]
    w = sum(p.width for p in ps) + GAP * (len(ps) - 1)
    out = Image.new("RGB", (w, ps[0].height), BG)
    x = 0
    for p in ps:
        out.paste(p, (x, 0))
        x += p.width + GAP
    return out


def column(panels, w):
    ps = [labelled(scale_w(p, w), t) for p, t in panels]
    h = sum(p.height for p in ps) + GAP * (len(ps) - 1)
    out = Image.new("RGB", (w, h), BG)
    y = 0
    for p in ps:
        out.paste(p, (0, y))
        y += p.height + GAP
    return out


def save(img, path):
    if img.width > 1400:
        img = scale_w(img, 1400)
    img.save(path)
    print("wrote", os.path.relpath(path, ROOT), img.size)


def main():
    prefix, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    atlas = Image.open(ATLAS).convert("RGB")
    board = {k: atlas.crop(v) for k, v in REGIONS.items()}
    cap = lambda run, name: Image.open(os.path.join(f"{prefix}-{run}", f"{name}.png")).convert("RGB")
    save(row([(board["1"], "board 4 · frame 1 (approved)"), (cap("desktop", "words-desktop"), "native · desktop 990x603")], 900),
         os.path.join(out, "sbs-1-words-desktop.png"))
    save(column([(board["1b"], "board 4 · frame 1b (approved)"), (cap("desktop", "large-desktop"), "native · desktop 990x603")], 1400),
         os.path.join(out, "sbs-1b-large-desktop.png"))
    save(row([(board["2"], "board 4 · frame 2 (approved)"), (cap("phone", "words-phone"), "native · phone 360x780"),
              (cap("phone", "words-scrolled-phone"), "native · scrolled sideways")], 1200),
         os.path.join(out, "sbs-2-words-phone.png"))
    save(row([(board["2"], "board 4 · frame 2 (approved)"), (cap("zh-phone", "words-phone"), "native · zh"),
              (cap("zh-phone", "large-phone"), "native · zh · large")], 1200),
         os.path.join(out, "sbs-2-zh-phone.png"))
    save(row([(board["1"], "board 4 · frame 1 (approved)"), (cap("zh-desktop", "large-desktop"), "native · zh · large")], 900),
         os.path.join(out, "sbs-1-zh-desktop.png"))


if __name__ == "__main__":
    main()
