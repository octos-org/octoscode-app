#!/usr/bin/env python3
"""A17 — the solo onboarding click walk (parity rows 93-98).

Drives the real app, launched hidden by harness/headless.sh against the
replay server's `onboarding` scenario (the recorded a6ea8505 handshake +
r29a's recorded onboarding reply shapes, no model), through the user's own
path: the sidebar's "+ Add workspace" -> the folder browser's "Use this
folder" -> `launch/resolve` answers `no_profile` -> the onboarding panel.
Every step CLICKS a control at its /snap rect (or types into a clicked
field) and asserts the app's effect (/snap) AND what reached the wire (the
replay server's log: each onboarding request is printed with its params,
the key masked `sk-t…(16)`).

Three phases, each a fresh server + a fresh app (brief §8 isolation:
`a10_lib.run_session` keeps every piece of app state in a per-run dir):

  main       the catalog fails once (Retry), the default checkbox, the three
             selects, a catalog ENDPOINT + a rejected key -> the redacted
             error and the retained profile; the retry on the DERIVED Official
             API route -> create once, test twice, save once, the Session opens
             in the folder under Core's profile id. Then: the key is in no
             label, no app log line and no file of the run's state.
  supersede  a slow catalog: the panel is left while loading and launched
             again, the first reply comes back late and is dropped; a slow
             provider test: the panel is left while testing, nothing follows
             (no save, no open, no error); the next launch resolves the
             profile Core now has.
  fallback   the server withdraws `profile/llm/fetch_models`: the canonical
             `octoscode onboard` fallback, no catalog read, Disconnect.

  OCTOSCODE_APP_BIN=<host> python3 tools/walk/a17_onboarding.py desktop|phone [phase ...]

Ports: $A10_PORT (default 8452) for the app, $A10_REPLAY_PORT (default 8454)
for the replay server. Captures (PNG <= 1400 px, /snap JSON with every
input's raw `val` stripped — the masked key field reports it) and the logs go
to docs/ux/a17/<phase>-<mode>/.
"""
from __future__ import annotations

import json
import os
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import a10_lib  # noqa: E402
from a10_lib import checks_line, dialog_checks, inside  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FOLDER = "/srv/work/no-profile"
REJECTED = "sk-test-rejected"  # obvious dummies, never a real key
GOOD = "sk-test-dummy"
PREFIX = ("b3_onb", "b3_title", "b3_close")

os.environ.setdefault("A10_PORT", "8452")
os.environ.setdefault("A10_REPLAY_PORT", "8454")


def recorded_catalog() -> dict:
    """r29a line 6: the families in the order the app lists them (sorted)."""
    path = ROOT / "crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl"
    for line in path.read_text().splitlines():
        v = json.loads(line)
        if v.get("dir") == "in" and v.get("method") == "profile/llm/catalog":
            return v["body"]["families"]
    raise SystemExit("r29a has no catalog")


CATALOG = recorded_catalog()
FAMILIES = sorted(CATALOG)


def model_ids(family: str) -> list[str]:
    return [m["id"] for m in CATALOG[family]["models"]]


def route_labels(family: str, model: str) -> list[str]:
    m = next(x for x in CATALOG[family]["models"] if x["id"] == model)
    return ["Official API"] + [e.get("label") or e["id"] for e in m.get("endpoints", [])]


# ------------------------------------------------------------------ helpers
def shot(w, name: str) -> None:
    """a10_lib's capture, then the saved /snap without any input's raw value."""
    w.shot(name)
    p = w.out / f"{name}.snap.json"
    sn = json.loads(p.read_text())
    for x in sn:
        x.pop("val", None)
    p.write_text(json.dumps(sn))


def wire(w, method: str) -> list[dict]:
    """The params of every `method` request the onboarding scenario answered
    (`[replay-serve] -> <method> (onboarding) {params}`)."""
    if not w.replay_log or not w.replay_log.exists():
        return []
    tag = f"-> {method} (onboarding) "
    out = []
    for line in w.replay_log.read_text().splitlines():
        if tag in line:
            try:
                out.append(json.loads(line.split(tag, 1)[1]))
            except ValueError:
                pass
    return out


def replies_sent(w, method: str) -> int:
    if not w.replay_log or not w.replay_log.exists():
        return 0
    return sum(1 for l in w.replay_log.read_text().splitlines() if f"=> {method} reply sent" in l)


def labels_text(w) -> str:
    return "\n".join((x.get("t") or "") for x in w.snap() if w.shown(x) and x.get("ty") == "Label")


def ux(w, name: str) -> dict:
    """The numeric UX checks of the open panel (a10_lib.dialog_checks) plus:
    the card inside the module with even side gutters, the footer's actions
    inside the card, the controls' hit sizes."""
    sn = w.snap()
    c = dialog_checks(sn, "b3_dialog", PREFIX, viewport="b3_scroll")
    # A dropdown row cut by the dropdown's OWN scroll edge is clipped by
    # design (more rows below), like the body's (a10_lib's viewport rule).
    lv = w.rect("b3_onb_list_scroll", sn=sn)
    if lv and c.get("under28"):
        def cut(wid):
            r = w.rect(wid, sn=sn)
            return bool(r) and (abs(r[1] + r[3] - (lv[1] + lv[3])) <= 1.5 or abs(r[1] - lv[1]) <= 1.5)
        c["under28"] = [i for i in c["under28"] if not cut(i)]
        c["ok"] = not c["outside"] and not c["overlaps"] and not c["under28"]
    mod = w.module_rect(sn)
    fr = c.get("frame")
    extra = []
    if fr and mod:
        gl, gr = fr[0] - mod[0], (mod[0] + mod[2]) - (fr[0] + fr[2])
        gt, gb = fr[1] - mod[1], (mod[1] + mod[3]) - (fr[1] + fr[3])
        extra.append(f"gutters L{gl:.0f} R{gr:.0f} T{gt:.0f} B{gb:.0f}")
        c["in_module"] = gl >= 15.5 and gr >= 15.5 and gt >= -0.5 and gb >= -0.5 and abs(gl - gr) <= 1.5
    for wid in ("b3_onb_submit", "b3_onb_disconnect", "b3_onb_retry", "b3_close"):
        r = w.rect(wid, sn=sn)
        if r and fr:
            extra.append(f"{wid} {r[2]:.0f}x{r[3]:.0f}{'' if inside(r, fr) else ' OUTSIDE'}")
            c.setdefault("controls_in", True)
            c["controls_in"] = c["controls_in"] and inside(r, fr)
    c["line"] = checks_line(c) + ("; " + "; ".join(extra) if extra else "")
    ok = c.get("ok") and c.get("in_module", True) and c.get("controls_in", True)
    w.check(f"UX {name}: {c['line']}", bool(ok))
    with open(w.out / "ux-checks.txt", "a") as f:
        f.write(f"{name}: {c['line']}\n")
    return c


def open_browser(w) -> None:
    if w.mode == "phone" and not w.visible("sb_add_hit"):
        w.click("sidebar_toggle_hit")
        w.wait_shown("sb_add_hit", 6)
    w.click("sb_add_hit")
    w.check("+ Add workspace opens the folder browser", w.wait_shown("b1_br_use", 8))


def launch(w) -> None:
    """The user's folder launch: + Add workspace -> Use this folder."""
    n = len(wire(w, "launch/resolve"))
    open_browser(w)
    w.click("b1_br_use")
    w.check("wire: the launch asked launch/resolve for the folder",
            w.wait(lambda: len(wire(w, "launch/resolve")) > n, 8)
            and wire(w, "launch/resolve")[-1].get("cwd") == FOLDER, str(wire(w, "launch/resolve")[-1:]))


def keyboard_down(w) -> None:
    w.dismiss_keyboard()


def pick(w, which: str, index: int, label: str) -> None:
    """CLICK the select (scrolled into the body's view first, as a person
    scrolls to it), then CLICK its option (scrolled into view too)."""
    if not w.scroll_into(f"b3_onb_{which}", "b3_scroll"):
        w.note(f"scroll b3_onb_{which} into b3_scroll: not wholly inside")
    w.click(f"b3_onb_{which}")
    w.check(f"the {which} list unfolds", w.wait_shown("b3_onb_list", 5))
    vp = "b3_scroll" if w.mode == "phone" else "b3_onb_list_scroll"
    if not w.scroll_into(f"b3_onb_opt_{index}", vp):
        w.note(f"scroll b3_onb_opt_{index} into {vp}: not wholly inside")
    got = w.text(f"b3_onb_opt_{index}_label")
    w.check(f"the {which} option #{index} reads {label!r}", got == label, repr(got))
    w.click(f"b3_onb_opt_{index}")
    w.check(f"the {which} list folds", w.wait_shown("b3_onb_list", 4, gone=True))
    w.scroll_into(f"b3_onb_{which}", "b3_scroll")
    val = w.text(f"b3_onb_{which}_value")
    w.check(f"the {which} field shows {label!r}", val == label or (val.endswith("…") and label.startswith(val[:-1])), repr(val))


def type_key(w, key: str, clear: bool = False) -> None:
    vp = "b3_scroll"
    if not w.scroll_into("b3_onb_apikey", vp):
        w.note("the key field is not wholly inside the body")
    # CLICK the field at its right end: the caret lands after the text, so
    # Backspace clears it whole.
    r = w.rect("b3_onb_apikey")
    w.note(f"CLICK b3_onb_apikey r={r} (right end)")
    w.click_xy(r[0] + r[2] - 6, r[1] + r[3] / 2)
    if clear:
        w.clear_field(30)
    w.type_text(key)
    keyboard_down(w)


def submit_armed(w) -> bool:
    return bool(w.visible("b3_onb_submit"))


# -------------------------------------------------------------------- phases
def phase_main(w) -> None:
    w.mark()
    launch(w)
    w.check("the panel opens: 'Create your local coding profile'", w.wait(lambda: w.text("b3_title") == "Create your local coding profile", 8))
    w.check("loading: 'Loading providers from Octos…'", w.wait_shown("b3_onb_loading", 4))
    shot(w, f"01-loading-{w.mode}")
    ux(w, "loading")
    # The recorded server is not ready on the first read (--fail-once).
    w.check("the catalog failure card", w.wait_shown("b3_onb_failure", 10))
    body = w.text("b3_onb_failure_body")
    w.check("it says what failed", w.text("b3_onb_failure_head") == "Provider catalog unavailable" and "still loading" in body, repr(body))
    shot(w, f"02-catalog-failed-{w.mode}")
    ux(w, "catalog failed")
    w.click("b3_onb_retry")
    w.check("Retry reads the catalog again (wire: a 2nd profile/llm/catalog)", w.wait(lambda: len(wire(w, "profile/llm/catalog")) == 2, 6))
    w.check("the form", w.wait_shown("b3_onb_provider", 10))
    time.sleep(0.4)
    w.check("defaults: Profile ID 'coding', name 'Coding'",
            w.text("b3_onb_profile_id") == "coding" and w.text("b3_onb_profile_name") == "Coding",
            f"{w.text('b3_onb_profile_id')!r} {w.text('b3_onb_profile_name')!r}")
    w.check(f"defaults: the catalog's first family {FAMILIES[0]!r}, its first model, the Official API",
            w.text("b3_onb_provider_value") == FAMILIES[0] and w.text("b3_onb_route_value") == "Official API",
            f"{w.text('b3_onb_provider_value')!r} {w.text('b3_onb_route_value')!r}")
    w.check("no key yet: 'Test, save & open' is disabled (the live variant swap)",
            not submit_armed(w) and bool(w.visible("b3_onb_submit_off")))
    shot(w, f"03-form-{w.mode}")
    ux(w, "form")

    # The default checkbox (on -> off): the create must carry it.
    w.click("b3_onb_default")
    time.sleep(0.4)
    # Provider: deepseek; the model goes to its first one, the route back to
    # the Official API (OnboardingPanel.tsx:172-180).
    if w.mode == "phone":
        w.click("b3_onb_provider")
        w.check("phone: the provider picker sheet", w.wait_shown("b3_onb_back", 5))
        shot(w, f"04-provider-list-{w.mode}")
        ux(w, "provider picker")
        w.click("b3_onb_back")
        w.check("phone: back to the form", w.wait_shown("b3_onb_back", 4, gone=True) and bool(w.visible("b3_onb_provider")))
    else:
        w.click("b3_onb_provider")
        w.wait_shown("b3_onb_list", 5)
        shot(w, f"04-provider-list-{w.mode}")
        ux(w, "provider list")
        w.click("b3_onb_provider")
        w.check("a second click folds the list", w.wait_shown("b3_onb_list", 4, gone=True))
    pick(w, "provider", FAMILIES.index("deepseek"), "deepseek")
    models = model_ids("deepseek")
    w.check("a new provider takes its first model", w.text("b3_onb_model_value") == models[0], w.text("b3_onb_model_value"))
    pick(w, "model", models.index("deepseek-v4-pro"), "deepseek-v4-pro")
    routes = route_labels("deepseek", "deepseek-v4-pro")
    w.click("b3_onb_route")
    w.wait_shown("b3_onb_list", 5)
    shot(w, f"05-route-list-{w.mode}")
    ux(w, "route list")
    # Out again without a choice: a second click on the field (desktop), the
    # sheet's back control (phone).
    w.click("b3_onb_back" if w.mode == "phone" else "b3_onb_route")
    w.check("the route list folds without a choice", w.wait_shown("b3_onb_list", 4, gone=True))
    pick(w, "route", routes.index("AutoDL"), "AutoDL")

    # A rejected key: the test fails, the profile stays created.
    type_key(w, REJECTED)
    w.check("a typed key arms 'Test, save & open'", w.wait(lambda: submit_armed(w), 4))
    shot(w, f"06-key-typed-{w.mode}")
    w.click("b3_onb_submit")
    w.check("busy: 'Testing provider' + 'Working…'",
            w.wait(lambda: w.text("b3_onb_status") == "Testing provider", 6) and "Working…" in labels_text(w),
            repr(w.text("b3_onb_status")))
    shot(w, f"07-testing-{w.mode}")
    ux(w, "testing")
    w.check("the failure", w.wait_shown("b3_onb_error", 10))
    time.sleep(0.4)
    lead, detail = w.text("b3_onb_error_text"), w.text("b3_onb_error_detail")
    w.check("the error leads with the server's own message", lead == "Provider connection failed", repr(lead))
    w.check("the raw cause shows, the key REDACTED", "authentication failed" in detail and "[redacted]" in detail, repr(detail[:120]))
    w.check("no label anywhere carries the key", REJECTED not in labels_text(w))
    w.check("the retained profile: 'Profile coding-2 exists. A retry only repeats provider test and save.'",
            w.text("b3_onb_recovery_text") == "Profile coding-2 exists. A retry only repeats provider test and save.",
            repr(w.text("b3_onb_recovery_text")))
    w.check("the identity is locked once created (no Profile ID input)",
            not any(x.get("ty") == "TextInput" and x.get("i") == "b3_onb_profile_id" for x in w.snap() if w.shown(x)))
    shot(w, f"08-failed-{w.mode}")
    ux(w, "failed")
    creates, tests = wire(w, "profile/local/create"), wire(w, "profile/llm/test")
    w.check("wire: ONE profile/local/create {requested_id coding, name Coding, make_default false (the click)}",
            len(creates) == 1 and creates[0].get("requested_id") == "coding" and creates[0].get("name") == "Coding"
            and creates[0].get("make_default") is False, json.dumps(creates))
    route = (tests[0]["selection"]["route"] if tests else {})
    w.check("wire: the test carries the catalog ENDPOINT as advertised (autodl, AutoDL, its base URL and env)",
            len(tests) == 1 and tests[0]["profile_id"] == "coding-2" and tests[0]["selection"]["model_id"] == "deepseek-v4-pro"
            and route == {"route_id": "autodl", "label": "AutoDL", "base_url": "https://www.autodl.art/api/v1",
                          "api_key_env": "AUTODL_API_KEY", "api_type": "openai"}, json.dumps(tests[-1:]))
    w.check("wire: the key reached the server (masked in its log)", bool(tests) and tests[0].get("api_key") == "sk-t…(16)", json.dumps(tests[-1:]))
    w.check("wire: no save after a failed test", not wire(w, "profile/llm/upsert"))

    # The retry on the DERIVED Official API route with a good key.
    pick(w, "route", 0, "Official API")
    type_key(w, GOOD, clear=True)
    w.check("the retry is armed", w.wait(lambda: submit_armed(w), 4))
    n_open = len(wire(w, "session/open"))
    w.click("b3_onb_submit")
    w.check("the panel closes once the Session opened", w.wait(lambda: not w.visible("b3_dialog"), 15))
    creates, tests, saves = wire(w, "profile/local/create"), wire(w, "profile/llm/test"), wire(w, "profile/llm/upsert")
    w.check("wire: the retry never re-creates (create x1, test x2, save x1)",
            (len(creates), len(tests), len(saves)) == (1, 2, 1), f"{len(creates)}/{len(tests)}/{len(saves)}")
    sel = tests[-1]["selection"] if len(tests) > 1 else {}
    w.check("wire: the Official API route DERIVED from the family (deepseek, 'Official API', DEEPSEEK_API_KEY, openai)",
            sel.get("route") == {"route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"}
            and sel.get("family_id") == "deepseek" and tests[-1]["profile_id"] == "coding-2", json.dumps(tests[-1:]))
    w.check("wire: saved exactly as tested, as the primary",
            bool(saves) and saves[0]["selection"] == sel and saves[0].get("set_primary") is True and saves[0]["profile_id"] == "coding-2",
            json.dumps(saves))
    opens = wire(w, "session/open")[n_open:]
    w.check("wire: the coding Session opens in the folder under Core's profile (coding-2:api:…)",
            any(o.get("cwd") == FOLDER and o.get("profile_id") == "coding-2" and str(o.get("session_id", "")).startswith("coding-2:api:")
                for o in opens), json.dumps(opens))
    time.sleep(1.0)
    shot(w, f"09-opened-{w.mode}")
    log = "\n".join(w.log_since())
    w.check("the app log never carries the key", REJECTED not in log and GOOD not in log)


def phase_supersede(w) -> None:
    w.mark()
    launch(w)
    w.check("loading", w.wait_shown("b3_onb_loading", 6))
    first_sent = replies_sent(w, "profile/llm/catalog")
    # Leave while the catalog is in flight (the close glyph), launch again.
    w.click("b3_close")
    w.check("the close glyph leaves the panel", w.wait(lambda: not w.visible("b3_dialog"), 4))
    launch(w)
    w.check("the new launch loads again", w.wait_shown("b3_onb_loading", 6))
    w.check("wire: two catalog reads", w.wait(lambda: len(wire(w, "profile/llm/catalog")) == 2, 6))
    w.check("the FIRST reply comes back late", w.wait(lambda: replies_sent(w, "profile/llm/catalog") >= first_sent + 1, 8))
    time.sleep(0.6)
    w.check("…and is dropped: the panel still loads (latest request wins)",
            bool(w.visible("b3_onb_loading")) and not w.visible("b3_onb_provider"))
    w.check("the app says it dropped a superseded catalog reply", w.logged("superseded profile/llm/catalog reply was dropped", 4))
    shot(w, f"10-stale-dropped-{w.mode}")
    w.check("the current reply fills the form", w.wait_shown("b3_onb_provider", 10))
    # A keyless family (no key field, the keyless note).
    pick(w, "provider", FAMILIES.index("ollama"), "ollama")
    w.check("keyless: 'No API key required'", w.text("b3_onb_keyless_head") == "No API key required" and not w.visible("b3_onb_apikey"))
    shot(w, f"11-keyless-{w.mode}")
    ux(w, "keyless")
    n_open = len(wire(w, "session/open"))
    w.click("b3_onb_submit")
    w.check("busy: testing the provider", w.wait(lambda: w.text("b3_onb_status") == "Testing provider", 8), repr(w.text("b3_onb_status")))
    tests_before = replies_sent(w, "profile/llm/test")
    w.click("b3_close")
    w.check("leaving mid-test closes the panel", w.wait(lambda: not w.visible("b3_dialog"), 4))
    w.check("the test reply comes back late", w.wait(lambda: replies_sent(w, "profile/llm/test") > tests_before, 10))
    time.sleep(1.0)
    w.check("…and nothing follows: no save", not wire(w, "profile/llm/upsert"))
    w.check("…no Session opened", len(wire(w, "session/open")) == n_open)
    w.check("…no panel, no error", not w.visible("b3_dialog") and not w.visible("b3_onb_error"))
    w.check("the app says it dropped a superseded submission", w.logged("a superseded submission was dropped", 4))
    tests = wire(w, "profile/llm/test")
    w.check("wire: the keyless test carried the probe value", bool(tests) and tests[-1].get("api_key") == "octoscode-web-keyless-probe",
            json.dumps(tests[-1:]))
    # Core now has the created profile: the next launch resolves it.
    launch(w)
    w.check("the next launch opens with the profile Core resolved (no panel)",
            w.wait(lambda: any(o.get("profile_id") == "coding-2" and o.get("cwd") == FOLDER for o in wire(w, "session/open")[n_open:]), 10)
            and not w.visible("b3_onb_provider"))


def phase_fallback(w) -> None:
    w.mark()
    launch(w)
    w.check("the fallback: 'This server cannot onboard from this app'",
            w.wait(lambda: w.text("b3_onb_fallback_head") == "This server cannot onboard from this app", 8))
    w.check("…with the canonical 'octoscode onboard'", w.text("b3_onb_fallback_cmd") == "octoscode onboard")
    w.check("…and no way to start (no 'Test, save & open')", not w.visible("b3_onb_submit") and not w.visible("b3_onb_submit_disabled"))
    time.sleep(0.6)
    w.check("wire: no catalog read", not wire(w, "profile/llm/catalog"))
    shot(w, f"12-fallback-{w.mode}")
    ux(w, "fallback")
    w.click("b3_onb_disconnect")
    w.check("Disconnect leaves the panel", w.wait(lambda: not w.visible("b3_dialog"), 4))
    w.check("wire: nothing was created", not wire(w, "profile/local/create") and not wire(w, "profile/llm/catalog"))


PHASES = {
    "main": (phase_main, ["--fail-once", "profile/llm/catalog", "--slow", "profile/llm/catalog=1500", "--slow", "profile/llm/test=1500"]),
    "supersede": (phase_supersede, ["--slow", "profile/llm/catalog=2500", "--slow", "profile/llm/test=2500"]),
    "fallback": (phase_fallback, ["--drop-method", "profile/llm/fetch_models"]),
}


def post_state(outdir: pathlib.Path) -> list[tuple[str, bool]]:
    """After the app exited: the key in no file of the run's state and no
    line of the app's own log."""
    state = ROOT / "tmp" / "hs"
    iso = sorted(state.glob(f"iso-{os.environ['A10_PORT']}-*"), key=lambda p: p.stat().st_mtime)
    hits = []
    if iso:
        for f in iso[-1].rglob("*"):
            if f.is_file():
                data = f.read_bytes()
                if REJECTED.encode() in data or GOOD.encode() in data:
                    hits.append(f.name)
    app_log = state / f"port-{os.environ['A10_PORT']}.log"
    in_log = app_log.exists() and (REJECTED in app_log.read_text(errors="replace") or GOOD in app_log.read_text(errors="replace"))
    out = [
        (f"stored state: the key is in no file of the run ({len(list(iso[-1].rglob('*'))) if iso else 0} entries checked)", bool(iso) and not hits),
        ("the app's own log file never carries the key", not in_log),
    ]
    with open(outdir / "post-state.txt", "w") as f:
        for name, ok in out:
            line = ("PASS " if ok else "FAIL ") + name
            print(line)
            f.write(line + "\n")
    return out


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "desktop"
    phases = sys.argv[2:] or list(PHASES)
    rc = 0
    for phase in phases:
        fn, args = PHASES[phase]
        outdir = ROOT / "docs" / "ux" / "a17" / f"{phase}-{mode}"
        outdir.mkdir(parents=True, exist_ok=True)
        (outdir / "ux-checks.txt").write_text("")
        r = a10_lib.run_session(fn, mode=mode, outdir=str(outdir), scenario="onboarding", replay_args=args,
                                env={"OCTOS_WORKSPACE_CWD": "/srv/work/octos"})
        post = post_state(outdir) if phase == "main" else []
        ok = r == 0 and all(p for _, p in post)
        print(f"== phase {phase} ({mode}): {'PASS' if ok else 'FAIL'}")
        rc |= 0 if ok else 1
    return rc


if __name__ == "__main__":
    sys.exit(main())
