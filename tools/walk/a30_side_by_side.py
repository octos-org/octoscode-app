#!/usr/bin/env python3
"""A30 — the UX side-by-side sheets: board 4 regions 6 ("Peer dock ·
expanded") and 7 ("Peer dock · collapsed") next to the native sidebar crops
the click walk captured (tools/walk/a30_peer_dock.py), each panel scaled to
one height and labelled. Writes docs/ux/a30/side-by-side-*.png (<= 1400 px).

usage: python3 tools/walk/a30_side_by_side.py
"""
import pathlib

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
ATLAS = ROOT / "design" / "stage-a" / "phase4-new4" / "atlas.png"
OUT = ROOT / "docs" / "ux" / "a30"
# Board 4 README region boxes (x0, y0, x1, y1), the title row trimmed.
REGION6 = (1298, 1150, 1722, 2092)
REGION7 = (1791, 1150, 2191, 2051)
H = 900


def font(size: int):
    for f in ("/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Helvetica.ttc"):
        try:
            return ImageFont.truetype(f, size)
        except OSError:
            continue
    return ImageFont.load_default()


def panel(img: Image.Image, label: str) -> Image.Image:
    k = H / img.height
    img = img.convert("RGB").resize((max(1, int(img.width * k)), H), Image.LANCZOS)
    out = Image.new("RGB", (img.width, H + 34), "white")
    out.paste(img, (0, 34))
    d = ImageDraw.Draw(out)
    d.text((6, 8), label, fill=(29, 29, 31), font=font(18))
    return out


def sheet(name: str, panels: list) -> pathlib.Path:
    gap = 18
    w = sum(p.width for p in panels) + gap * (len(panels) + 1)
    h = max(p.height for p in panels) + 2 * gap
    canvas = Image.new("RGB", (w, h), (236, 236, 240))
    x = gap
    for p in panels:
        canvas.paste(p, (x, gap))
        x += p.width + gap
    if canvas.width > 1400:
        k = 1400 / canvas.width
        canvas = canvas.resize((1400, int(canvas.height * k)), Image.LANCZOS)
    path = OUT / f"side-by-side-{name}.png"
    canvas.save(path)
    return path


def main() -> None:
    atlas = Image.open(ATLAS)
    r6 = atlas.crop(REGION6)
    r7 = atlas.crop(REGION7)
    load = lambda rel: Image.open(OUT / rel)  # noqa: E731
    made = [
        sheet("expanded", [
            panel(r6, "board 4 · region 6 (expanded)"),
            panel(load("phone/phone-02-expanded-sidebar.png"), "native · phone 360x780 drawer"),
            panel(load("desktop/desktop-02-expanded-sidebar.png"), "native · desktop 990x603 column"),
        ]),
        sheet("collapsed", [
            panel(r7, "board 4 · region 7 (collapsed)"),
            panel(load("phone/phone-01-folded-start-sidebar.png"), "native · phone (starts folded)"),
            panel(load("desktop/desktop-03-collapsed-sidebar.png"), "native · desktop (Alt+P)"),
        ]),
        sheet("zh", [
            panel(load("phone/phone-04-zh-expanded-sidebar.png"), "native zh · phone expanded"),
            panel(load("phone/phone-05-zh-collapsed-sidebar.png"), "native zh · phone collapsed"),
            panel(load("desktop/desktop-04-zh-expanded-sidebar.png"), "native zh · desktop expanded"),
        ]),
    ]
    for m in made:
        print(m.relative_to(ROOT))


if __name__ == "__main__":
    main()
