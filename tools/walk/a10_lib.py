"""A10 — the shared click-walk helpers for tools/walk/a10_*.py.

Drives the real app (launched hidden by harness/headless.sh) through its own
Makepad instrument: `/snap?all=1` (rects in logical px), `/click`, `/t`, `/k`,
`/log`, `/g`. Every step CLICKS a control at its laid-out rect (never an env
opener) and asserts the app's own effect (a /snap widget or text, and/or the
routed log line). The dialog checker turns a /snap into the numeric UX checks
the brief asks for (no clipped text, equal margins, controls >= 28 px, no
overlaps, the frame inside the module view).
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import time
import urllib.parse
import urllib.request

TOL = 1.5


class Walk:
    def __init__(self, port: int, outdir: str, mode: str = "desktop"):
        self.port = port
        self.base = f"http://127.0.0.1:{port}"
        self.out = pathlib.Path(outdir)
        self.out.mkdir(parents=True, exist_ok=True)
        self.mode = mode
        self.results: list[tuple[str, bool, str]] = []
        self.log_seq = 0
        self.transcript: list[str] = []

    # ------------------------------------------------------------ transport
    def get(self, path: str, timeout: float = 20, tolerant: bool = False) -> str:
        try:
            with urllib.request.urlopen(self.base + path, timeout=timeout) as r:
                return r.read().decode()
        except Exception:
            if tolerant:
                return ""
            raise

    def note(self, line: str) -> None:
        print(line, flush=True)
        self.transcript.append(line)

    # ----------------------------------------------------------------- snap
    def snap(self) -> list[dict]:
        d = json.loads(self.get("/snap?all=1"))
        return d.get("s", d) if isinstance(d, dict) else d

    @staticmethod
    def shown(w: dict) -> bool:
        r = w.get("r") or [0, 0, 0, 0]
        return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0

    def visible(self, wid: str, sn: list[dict] | None = None) -> list[dict]:
        sn = sn if sn is not None else self.snap()
        hits = [w for w in sn if w.get("i") == wid and self.shown(w)]
        return sorted(hits, key=lambda w: (w["r"][1], w["r"][0]))

    def prefixed(self, prefix: str, sn: list[dict] | None = None) -> list[dict]:
        sn = sn if sn is not None else self.snap()
        return [w for w in sn if str(w.get("i", "")).startswith(prefix) and self.shown(w)]

    def rect(self, wid: str, nth: int = 0, sn=None):
        hits = self.visible(wid, sn)
        return hits[nth]["r"] if len(hits) > nth else None

    def text(self, wid: str, sn=None) -> str:
        hits = self.visible(wid, sn)
        return (hits[0].get("t") or "") if hits else ""

    def has_text(self, needle: str, sn=None) -> bool:
        sn = sn if sn is not None else self.snap()
        return any(needle in (w.get("t") or "") for w in sn if self.shown(w))

    def wait(self, pred, secs: float = 10.0, period: float = 0.2) -> bool:
        end = time.time() + secs
        while time.time() < end:
            try:
                if pred():
                    return True
            except Exception:
                pass
            time.sleep(period)
        return False

    def wait_shown(self, wid: str, secs: float = 10.0, gone: bool = False) -> bool:
        if gone:
            return self.wait(lambda: not self.visible(wid), secs)
        return self.wait(lambda: bool(self.visible(wid)), secs)

    # ---------------------------------------------------------------- input
    def click_xy(self, x: float, y: float) -> None:
        self.get(f"/click?x={x:.1f}&y={y:.1f}&wait=1", tolerant=True)
        time.sleep(0.2)

    def click(self, wid: str, nth: int = 0) -> bool:
        r = self.rect(wid, nth)
        if r is None:
            self.note(f"CLICK {wid}[{nth}] — not visible")
            return False
        x, y, w, h = r
        self.note(f"CLICK {wid} r={r} at ({x + w / 2:.0f},{y + h / 2:.0f})")
        self.click_xy(x + w / 2, y + h / 2)
        return True

    def type_text(self, text: str) -> None:
        self.note(f"TYPE {text!r}")
        self.get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}), tolerant=True)
        time.sleep(0.2)

    def key(self, code: str) -> None:
        self.note(f"KEY {code}")
        self.get(f"/k?c={code}&wait=1", tolerant=True)
        time.sleep(0.2)

    def clear_field(self, presses: int = 40) -> None:
        for _ in range(presses):
            self.get("/k?c=Backspace", tolerant=True)
        time.sleep(0.2)

    # ------------------------------------------------------------------ log
    def log_since(self) -> list[str]:
        d = json.loads(self.get(f"/log?since={self.log_seq}"))
        self.log_seq = d.get("n", self.log_seq)
        lines = d.get("l", [])
        for l in lines:
            if "[octoscode]" in l or "[a10]" in l:
                self.transcript.append("    log: " + l.strip())
        return lines

    def mark(self) -> None:
        self.log_since()

    def logged(self, needle: str, secs: float = 6.0) -> bool:
        seen: list[str] = []

        def has() -> bool:
            seen.extend(self.log_since())
            return any(needle in l for l in seen)

        return self.wait(has, secs)

    # --------------------------------------------------------------- checks
    def check(self, name: str, ok: bool, detail: str = "") -> bool:
        self.results.append((name, bool(ok), detail))
        self.note(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
        return bool(ok)

    def shot(self, name: str) -> pathlib.Path:
        """`/g` PNG of the app. Desktop: cropped to the OctosCode window (the
        module view plus its 32 px title bar), like the A5 evidence; phone:
        the whole phone shell. Downscaled to <= 1400 px."""
        png = self.out / f"{name}.png"
        sn = self.snap()
        data = urllib.request.urlopen(self.base + "/g?raw=1", timeout=30).read()
        png.write_bytes(data)
        if self.mode == "desktop":
            try:
                from PIL import Image

                mod = self.module_rect(sn)
                win = next((w["r"] for w in sn if w.get("ty") == "Window" and self.shown(w)), None)
                img = Image.open(png)
                if mod and win:
                    k = img.width / win[2]
                    x, y, w, h = mod[0], max(mod[1] - 32, 0), mod[2], mod[3] + 32
                    img.crop((int(x * k), int(y * k), int((x + w) * k), int((y + h) * k))).save(png)
            except Exception as e:  # keep the full capture
                self.note(f"crop skipped: {e}")
        subprocess.run(["sips", "-Z", "1400", str(png)], capture_output=True)
        (self.out / f"{name}.snap.json").write_text(json.dumps(sn))
        self.note(f"SHOT {name}")
        return png

    def summary(self) -> int:
        ok = sum(1 for _, p, _ in self.results if p)
        self.note(f"== {ok}/{len(self.results)} checks passed")
        (self.out / "walk.log").write_text("\n".join(self.transcript) + "\n")
        return 0 if ok == len(self.results) else 1

    # ---------------------------------------------------------- app helpers
    def module_rect(self, sn=None):
        sn = sn if sn is not None else self.snap()
        for w in sn:
            if w.get("ty") == "OctoscodeView" and self.shown(w):
                return w["r"]
        return None

    def composer(self, sn=None):
        sn = sn if sn is not None else self.snap()
        for w in sn:
            i = str(w.get("i", ""))
            if i.startswith("i") and i.endswith("_composer_0") and self.shown(w):
                return w
        return None

    def open_phone_app(self) -> None:
        """The shell's phone page: open OctosCode from the phone home once."""
        if self.composer() is None:
            self.note("CLICK OctosCode icon on the phone home at (153,363)")
            self.click_xy(153, 363)
            self.wait(lambda: self.composer() is not None, 20)

    def palette_run(self, query: str, row_text: str) -> bool:
        """Focus the composer, type `/query`, click the palette row whose
        name is `row_text` — the user's path to every dialog."""
        c = self.composer()
        if c is None:
            self.note("no composer")
            return False
        x, y, w, h = c["r"]
        self.note(f"CLICK composer r={c['r']}")
        self.click_xy(x + w / 2, y + h / 2)
        self.clear_field()
        # "/" alone opens the palette (a key-typed slash on an empty draft),
        # then the filter text narrows it — the user's own two steps.
        self.type_text("/")
        self.wait(lambda: bool(self.visible("palette_row_name")), 6)
        self.type_text(query)
        ok = self.wait(lambda: any((w.get("t") or "") == row_text for w in self.visible("palette_row_name")), 6)
        if not ok:
            self.note(f"palette row {row_text!r} not shown")
            return False
        rows = [w for w in self.visible("palette_row_name") if (w.get("t") or "") == row_text]
        r = rows[0]["r"]
        self.note(f"CLICK palette_row_name {row_text!r} r={r}")
        self.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        return True


# ------------------------------------------------------------- UX checker
def _right(r):
    return r[0] + r[2]


def _bottom(r):
    return r[1] + r[3]


def inside(inner, outer, tol=TOL) -> bool:
    return (inner[0] >= outer[0] - tol and _right(inner) <= _right(outer) + tol
            and inner[1] >= outer[1] - tol and _bottom(inner) <= _bottom(outer) + tol)


def overlap(a, b, tol=1.0) -> bool:
    return (a[0] + tol < _right(b) and b[0] + tol < _right(a)
            and a[1] + tol < _bottom(b) and b[1] + tol < _bottom(a))


def dialog_checks(sn: list[dict], frame_id: str, prefix: str, module=None) -> dict:
    """The dialog's numeric UX checks from one /snap: the frame rect, its
    centring in the module view, the content margins, labels clipped by the
    frame, overlapping labels, the controls' hit sizes."""
    shown = [w for w in sn if Walk.shown(w)]
    frames = [w for w in shown if w.get("i") == frame_id]
    if not frames:
        return {"ok": False, "why": f"no {frame_id}"}
    fr = frames[0]["r"]
    mod = module or next((w["r"] for w in shown if w.get("ty") == "OctoscodeView"), None)
    mine = [w for w in shown if str(w.get("i", "")).startswith(prefix)]
    labels = [w for w in mine if w.get("ty") == "Label" and (w.get("t") or "").strip()]
    buttons = [w for w in mine if w.get("ty") in ("Button", "DesignNativeButton")]
    outside = [w["i"] for w in labels if not inside(w["r"], fr)]
    over = []
    for i, a in enumerate(labels):
        for b in labels[i + 1:]:
            if overlap(a["r"], b["r"]):
                over.append((a["i"], b["i"]))
    under = [w["i"] for w in buttons if w["r"][2] < 28 - 0.5 or w["r"][3] < 28 - 0.5]
    drawn = [w for w in mine if w.get("ty") in ("Label", "Svg", "TextInput") and inside(w["r"], fr)]
    left = min((w["r"][0] for w in drawn), default=fr[0]) - fr[0]
    right = _right(fr) - max((_right(w["r"]) for w in drawn), default=_right(fr))
    centre_dx = None
    if mod:
        centre_dx = round((fr[0] + fr[2] / 2) - (mod[0] + mod[2] / 2), 1)
    return {
        "ok": not outside and not over and not under,
        "frame": fr,
        "centre_dx": centre_dx,
        "margins": (round(left, 1), round(right, 1)),
        "labels": len(labels),
        "outside": outside,
        "overlaps": over,
        "controls": len(buttons),
        "under28": under,
    }


def checks_line(c: dict) -> str:
    if "frame" not in c:
        return c.get("why", "no frame")
    fr = c["frame"]
    return (f"frame={fr[2]:.0f}x{fr[3]:.0f} centre_dx={c['centre_dx']} margins L={c['margins'][0]:.0f} "
            f"R={c['margins'][1]:.0f} labels={c['labels']} outside={len(c['outside'])} "
            f"overlaps={len(c['overlaps'])} controls={c['controls']} under28={len(c['under28'])}")


def env_port() -> int:
    return int(os.environ.get("A10_PORT", "8420"))
