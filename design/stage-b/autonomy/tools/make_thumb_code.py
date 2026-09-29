#!/usr/bin/env python3
"""Deterministic dark code-screenshot fill for autonomy-09 attachment thumbs.

Card #28b3: the atlas thumbs are dark code screenshots (mean luminance ~34);
the previous asset (thumb_code.png) was near-white (mean 239.8), which made the
white progress arc and the white "68%" invisible. This draws a stylized editor
window (dark bg, gutter, coloured mono code lines) at 732x480.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

OUT = Path(__file__).resolve().parents[1] / "cards/autonomy-09/assets/thumb_code.png"
W, H = 732, 480
BG, GUTTER, GUTTER_TXT = (20, 22, 27), (16, 18, 22), (92, 99, 112)
FG = {"kw": (198, 120, 221), "fn": (97, 175, 239), "str": (152, 195, 121),
      "cm": (92, 99, 112), "tx": (171, 178, 191), "num": (209, 154, 102)}
FONT = "/System/Library/Fonts/Menlo.ttc"

LINES = [
    [("cm", "// ui_protocol.rs — notification envelope")],
    [("kw", "pub enum"), ("tx", " UiNotification {")],
    [("tx", "    UserQuestion(UserQuestion),")],
    [("tx", "    Approval(ApprovalRequest),")],
    [("tx", "    TaskProgress(TaskProgress),")],
    [("tx", "    Steer("), ("kw", "SteerMetrics"), ("tx", "),")],
    [("tx", "}")],
    [],
    [("kw", "impl"), ("tx", " UiNotification {")],
    [("kw", "    pub fn"), ("fn", " fold_into_store"), ("tx", "(&self, s: &"), ("kw", "mut"), ("tx", " Store) {")],
    [("cm", "        // drained via projection/envelope")],
    [("tx", "        s.apply(self);")],
    [("tx", "    }")],
    [("tx", "}")],
]

def main():
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, W, 44], fill=(28, 30, 36))
    for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
        d.ellipse([18 + i * 26, 16, 32 + i * 26, 30], fill=c)
    font = ImageFont.truetype(FONT, 20)
    y = 66
    for n, line in enumerate(LINES, 1):
        d.text((14, y + 2), f"{n:>2}", font=font, fill=GUTTER_TXT)
        x = 64
        for kind, text in line:
            d.text((x, y), text, font=font, fill=FG[kind])
            x += d.textlength(text, font=font)
        y += 30
    d.rectangle([0, 44, 3, H], fill=(60, 90, 190))
    img.save(OUT)
    px = list(img.convert("L").get_flattened_data()) if hasattr(img.convert("L"), "get_flattened_data") else list(img.convert("L").getdata())
    print(f"wrote {OUT} mean={sum(px)/len(px):.1f}")

if __name__ == "__main__":
    main()
