#!/usr/bin/env python3
"""Live onboarding: a FRESH octos serve (empty data dir) -> the app's first launch shows the onboarding panel (A19: no
profile id, launch/resolve answers no_profile) -> complete it with a real provider key -> the coding Session opens -> the
next prompt streams.

    LIVE_DIR=<dir holding a mode-600 `token` and ws/>  KEY_FILE=<mode-600 file holding ONLY the provider key> \\
      ONB_PROVIDER=minimax python3 tools/judge/live_onboarding.py <host-bin> <app-port> <serve-url> <outdir> [desktop|phone]

Secrets: the key is read from KEY_FILE and sent to the app by one direct instrument call; it is never printed, logged by
this script, put in a check line, or saved. No /snap JSON is written. The app's protocol trace (<outdir>/trace.jsonl) is
scrubbed by the app (trace::register_secret). After the run this script scans every file it and the app wrote (outdir,
the app's isolated state, the harness logs) for the key and its first/last 8 characters and prints COUNTS only.
"""
import json, os, pathlib, subprocess, sys, time, urllib.parse, urllib.request

BIN, PORT, SERVE, OUT = sys.argv[1], int(sys.argv[2]), sys.argv[3], pathlib.Path(sys.argv[4])
MODE = sys.argv[5] if len(sys.argv) > 5 else "desktop"
LIVE, KEY_FILE = os.environ["LIVE_DIR"], os.environ["KEY_FILE"]
PROVIDER = os.environ.get("ONB_PROVIDER", "minimax")
ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE = f"http://127.0.0.1:{PORT}"
STATE, HS = OUT / "state", OUT / "hs"
OUT.mkdir(parents=True, exist_ok=True)
RESULTS = []


def get(path, timeout=15):
    for attempt in (0, 1):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if attempt:
                raise
            time.sleep(1.5)


def snap():
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid, sn=None):
    return next((w for w in (sn if sn is not None else snap()) if w.get("i") == wid and shown(w)), None)


def text(wid, sn=None):
    w = find(wid, sn)
    return (w.get("t") or "") if w else ""


def click_w(w, dx=None):
    x, y, ww, hh = w["r"]
    cx = x + (ww - 6 if dx == "right" else ww / 2)
    get(f"/click?x={cx}&y={y + hh / 2}&wait=1")
    time.sleep(0.5)


def click(wid, dx=None):
    w = find(wid)
    if w:
        click_w(w, dx)
    return bool(w)


def wait(pred, secs, period=0.5):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(period)
    return False


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    line = f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "")
    print(line, flush=True)
    with open(OUT / "checks.txt", "a") as f:
        f.write(line + "\n")
    return ok


N = [0]


def capture(name):
    N[0] += 1
    png = OUT / f"{N[0]:02d}-{name}.png"
    png.write_bytes(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", str(png), "--out", str(png)], capture_output=True)


def env():
    tok = open(os.path.join(LIVE, "token")).read().strip()
    st = str(STATE)
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(st, d), exist_ok=True)
    e = dict(os.environ)
    e.pop("KEY_FILE", None)
    e.update({
        "OCTOSCODE_DRAFTS_FILE": f"{st}/drafts.json", "OCTOSCODE_CREDENTIALS_DIR": f"{st}/cred",
        "OCTOSCODE_PREF_PATH": f"{st}/prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": f"{st}/notifications.json",
        "OCTOSCODE_SHOW_THINKING_FILE": f"{st}/show-thinking.json", "OCTOSCODE_DOWNLOAD_DIR": f"{st}/downloads",
        "OCTOSCODE_RECENTS_DIR": f"{st}/recents", "OCTOSCODE_DISPLAY_PREFS_PATH": f"{st}/display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": f"{st}/pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": f"{st}/driver-id",
        "OCTOSCODE_CONNECTION_FILE": f"{st}/connection-v1.json",
        "OCTOS_BASE_URL": SERVE, "OCTOS_BEARER": tok, "OCTOS_WORKSPACE_CWD": os.path.join(LIVE, "ws"),
        "OCTOSCODE_TRACE_FILE": str(OUT / "trace.jsonl"),
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"), "MAKEPAD_WM_TEST_APP": "octoscode", "HEADLESS_STATE": str(HS),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if MODE == "phone" else ""),
    })
    e.pop("OCTOS_PROFILE_ID", None)
    e.pop("OCTOS_CREATE_PROFILE", None)
    if MODE == "phone":
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    return e


def harness(cmd):
    subprocess.run(["bash", "harness/headless.sh", cmd, BIN, str(PORT)] if cmd == "start"
                   else ["bash", "harness/headless.sh", cmd, str(PORT)], cwd=ROOT, env=env(),
                   stdout=open(OUT / "app-start.log", "a"), stderr=subprocess.STDOUT)


def options():
    return [(w, (w.get("t") or "").strip()) for w in snap() if str(w.get("i", "")).startswith("b3_onb_opt_")
            and str(w.get("i", "")).endswith("_label") and shown(w)]


def scroll_list(dy):
    lst = find("b3_onb_list_scroll") or find("b3_onb_list")
    if lst:
        x, y, w, h = lst["r"]
        get(f"/m?k=scroll&x={x + w / 2}&y={y + h / 2}&dy={dy}&wait=1")
        time.sleep(0.4)


def pick(which, want):
    """CLICK the select, then CLICK the first option whose label satisfies `want` (scrolling the list as a person would)."""
    for _ in range(3):  # a loaded host can miss the first tap while the catalog settles
        click(f"b3_onb_{which}")
        if wait(lambda: find("b3_onb_list") is not None, 6):
            break
    else:
        return check(f"{which}: the list unfolds", False)
    seen = []
    for _ in range(12):
        opts = options()
        seen += [t for _, t in opts if t not in seen]
        hit = next(((w, t) for w, t in opts if want(t)), None)
        if hit:
            row = find(hit[0]["i"][: -len("_label")]) or hit[0]
            click_w(row)
            wait(lambda: find("b3_onb_list") is None, 4)
            return check(f"{which}: picked {hit[1]!r}", text(f"b3_onb_{which}_value").rstrip("…") in hit[1]
                         or hit[1].startswith(text(f"b3_onb_{which}_value").rstrip("…")), text(f"b3_onb_{which}_value"))
        scroll_list(160)  # positive dy scrolls the list down (later options)
    return check(f"{which}: an option for it", False, f"seen {seen[:20]}")


def type_key():
    key = open(KEY_FILE).read().strip()
    click("b3_onb_apikey", dx="right")
    # One direct call: the walk helpers log typed text, this does not.
    urllib.request.urlopen(BASE + "/t?" + urllib.parse.urlencode({"t": key, "wait": 1}), timeout=15).read()
    time.sleep(0.5)
    return key


def idle(sn=None):
    sn = sn if sn is not None else snap()
    return find("composer_send_icon", sn) is not None and find("composer_stop_icon", sn) is None \
        and find("composer_stop_busy", sn) is None


def prose():
    return " ".join((w.get("t") or "") for w in snap() if shown(w) and "assistantprose" in str(w.get("i", "")))


def scan(key):
    parts = {"whole": key, "head8": key[:8], "tail8": key[-8:]}
    hits = {k: 0 for k in parts}
    files = 0
    for base in (OUT, HS):
        for p in base.rglob("*"):
            if not p.is_file() or p.suffix == ".png":
                continue
            files += 1
            data = p.read_bytes()
            for k, v in parts.items():
                if v.encode() in data:
                    hits[k] += 1
                    print(f"KEY FOUND ({k}) in {p.relative_to(OUT.parent) if OUT.parent in p.parents else p.name}", flush=True)
    return files, hits


def main():
    open(OUT / "checks.txt", "w").close()
    harness("start")
    check("app up against the fresh serve", wait(lambda: get("/s") is not None, 90, 1.0))
    ok = wait(lambda: find("b3_onb_provider") is not None, 90, 1.0)
    check("first launch: the onboarding panel shows with the provider catalog (no_profile)", ok)
    capture("onboarding")
    if not ok:
        return
    # Every selection must land before any key is typed: a wrong provider would send the key to the
    # wrong vendor's API in the server's provider test.
    if not (pick("provider", lambda t: t == PROVIDER)
            and pick("model", (lambda t: t == os.environ["ONB_MODEL"]) if os.environ.get("ONB_MODEL")
                     else (lambda t: "M3" in t or "MiniMax" in t))
            and pick("route", lambda t: "Official" in t or "official" in t or t != "")
            and text("b3_onb_provider_value").rstrip("…") == PROVIDER[:len(text("b3_onb_provider_value").rstrip("…"))]):
        check("all selections landed — aborting before the key is typed", False, text("b3_onb_provider_value"))
        harness("stop")
        return
    if os.environ.get("DRY_RUN"):
        click("b3_onb_model")
        wait(lambda: find("b3_onb_list") is not None, 6)
        allm = []
        for _ in range(15):
            for _, t in options():
                if t not in allm:
                    allm.append(t)
            scroll_list(160)
        print("INFO model options:", allm, flush=True)
        click("b3_onb_model")
        capture("dry-run-selected")
        check("dry run: selections landed, stopping before any key", True,
              f"{text('b3_onb_provider_value')} / {text('b3_onb_model_value')} / {text('b3_onb_route_value')}")
        harness("stop")
        return
    key = type_key()
    sn = snap()
    field = text("b3_onb_apikey", sn)
    check("the key field renders masked", bool(field) and set(field) <= {"•", "*", "●", " "}, f"{len(field)} chars, masked")
    check("no laid-out text carries the key", not any(key[:8] in (w.get("t") or "") for w in sn))
    # The dev instrument reports a TextInput's raw buffer as `val` (a tooling property, not the product UI):
    # recorded, never saved (this script writes no /snap JSON).
    print(f"INFO instrument val exposes the typed key: {any(key[:8] in str(w.get('val') or '') for w in sn)}", flush=True)
    capture("filled-masked")
    click("b3_onb_submit")
    opened = wait(lambda: find("b3_onb_provider") is None and find("i0_composer_0") is not None
                  and find("hd_tab_chat_hit") is not None, 150, 1.0)
    if not opened:
        capture("not-opened")
        check("submit: test, save and open the coding Session", False,
              f"{text('b3_onb_error_text')!r} {text('b3_onb_failure_head')!r} {text('b3_onb_status')!r}")
    else:
        check("submit: tested, saved, the coding Session opened", True, f"title {text('hd_title')!r}")
        time.sleep(2)
        capture("session")
        click("i0_composer_0")
        get("/t?" + urllib.parse.urlencode({"t": "Reply with one short sentence: what is 2 + 3?", "wait": 1}))
        get("/k?c=return&wait=1")
        started = wait(lambda: not idle(), 30, 0.5)
        done = wait(idle, 150, 1.0)
        check("the next prompt streams and completes", started and done)
        check("the answer renders", "5" in prose(), prose()[:80])
        capture("answer")
    harness("stop")
    time.sleep(1)
    files, hits = scan(key)
    check("post-run scan: the key (whole / first 8 / last 8) is in none of the run's text files",
          not any(hits.values()), f"{files} files scanned, hits {hits}")
    passed = sum(1 for _, ok in RESULTS if ok)
    print(f"== {passed}/{len(RESULTS)} live onboarding checks passed ({MODE})")


if __name__ == "__main__":
    try:
        main()
    finally:
        harness("stop")
