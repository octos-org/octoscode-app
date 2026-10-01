#!/usr/bin/env python3
"""Apple Vision OCR via pyobjc (the swift interpreter crashes under Xcode 26.2).
Emits the same schema observe() expects: {width,height,observations:[{bounds,confidence,text}]}
+ image_sha256 cache key. bounds are [x,y,w,h] pixels, origin top-left."""
import json, sys, hashlib
from pathlib import Path
import Vision
from Foundation import NSURL

def ocr(path):
    p = Path(path)
    url = NSURL.fileURLWithPath_(str(p.resolve()))
    handler = Vision.VNImageRequestHandler.alloc().initWithURL_options_(url, None)
    req = Vision.VNRecognizeTextRequest.alloc().init()
    req.setRecognitionLevel_(Vision.VNRequestTextRecognitionLevelAccurate)
    req.setRecognitionLanguages_(["en-US"])
    ok, err = handler.performRequests_error_([req], None)
    if not ok:
        raise RuntimeError(str(err))
    from AppKit import NSImage
    img = NSImage.alloc().initWithContentsOfFile_(str(p.resolve()))
    W, H = img.size().width, img.size().height
    obs = []
    for r in req.results():
        cand = r.topCandidates_(1)[0]
        bb = r.boundingBox()  # normalized, origin BOTTOM-left
        x = bb.origin.x * W
        y = (1 - bb.origin.y - bb.size.height) * H
        obs.append({"bounds": [round(x,4), round(y,4), round(bb.size.width*W,4), round(bb.size.height*H,4)],
                    "confidence": round(float(cand.confidence()),4),
                    "text": str(cand.string())})
    obs.sort(key=lambda o: (o["bounds"][1], o["bounds"][0]))
    return {"width": W, "height": H, "observations": obs,
            "image_sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
            "engine": "Apple Vision VNRecognizeTextRequest accurate, en-US (pyobjc)"}

if __name__ == "__main__":
    for a in sys.argv[1:]:
        p = Path(a)
        out = ocr(p)
        dest = p.with_suffix(".ocr.json")
        dest.write_text(json.dumps(out, indent=2) + "\n")
        print(f"{dest}: {len(out['observations'])} observations {out['width']}x{out['height']}")
