#!/usr/bin/env python3
"""A23 — parity rows 281 (model provider management) and 282 (the model
management projection) by CLICK on the real app, launched hidden with
isolated state against the replay server's providers simulator
(tools/walk/a10_lib.py `run_session`, scenario a10: r2's recorded
configuration and catalog, the A10 faithful test / fetch replies, r29a's
recorded 401 for the dummy key `sk-test-rejected`). Every step CLICKS a
control at its laid-out rect (or types into a field the user would) and
asserts the app's own effect (/snap) and what reached the wire (the replay
server's log, its key masked as `sk-t…(N)`). Only dummy keys.

Walks:
  main      the providers dialog (Models dialog's "Manage providers"): rows
            with the credential indicator; an EDIT-BLOCKED row visible, its
            reason, no Edit, still deletable; "Edit" on a FALLBACK opens the
            board-1 editor for it — a label-only Save carries its configured
            inference values (null included) and returns with the web's line;
            the catalog-driven "Add provider": the Provider select, a model,
            the key typed only after every selection was read back, "Test
            connection" (the test ALONE), "Fetch available models", Save; a
            refused key on "Test connection" (p4-07, the draft kept, nothing
            saved); "Delete" behind the typed phrase.
  loading   the first configuration read in flight (replay --slow): the
            loading line, then the rows.
  unread    the configuration read refused (replay --fail-config-file):
            the cause and "Try again", never the empty state; Try again lists.
  readonly  no profile/llm/upsert (replay --drop-method): the web's read-only
            notice, no Add provider, no Edit; Delete on its own method.
  empty     no provider configured yet (replay --providers-empty): the empty
            state; Add provider's first model becomes the primary "(default)".

usage: OCTOSCODE_APP_BIN=<host octosense> A10_PORT=<app port> A10_REPLAY_PORT=<replay port> \\
         a23_providers.py <main|loading|unread|readonly|empty> <desktop|phone> [outdir]
"""
import json
import os
import pathlib
import re
import sys
import time
import urllib.parse

from a10_lib import Walk, checks_line, dialog_checks, run_session

WHICH = sys.argv[1] if len(sys.argv) > 1 else "main"
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
OUT = sys.argv[3] if len(sys.argv) > 3 else f"docs/ux/a23/{WHICH}/{MODE}"
VP = "b3_scroll"
EVP = "b1_prov_scroll"
DUMMY = "sk-test-dummy"
REJECTED = "sk-test-rejected"
EDIT_BLOCKED = "This entry contains settings the editor cannot preserve. Edit it through Core configuration instead."
SAVED_EDIT = "Provider saved. Restart Octos before relying on this route or credential change."
# The replay refuses profile-config reads while this file exists (unread).
FAIL_FILE = pathlib.Path(__file__).resolve().parent.parent.parent / "tmp" / "hs" / f"a23-fail-config-{os.environ.get('A10_REPLAY_PORT', '8432')}"
INFERENCE = {"temperature": 0, "top_p": None, "context_window": 131072, "reasoning_effort": "max",
             "model_hints": {"fixed_temperature": False, "reasoning_style": "effort_low_high_max"}}


def masked(key: str) -> str:
    """How the replay log prints a key (`onboarding::masked`)."""
    return f"{key[:4]}…({len(key)})"


# ----------------------------------------------------------------- helpers

def wire(W: Walk, method: str) -> list[dict]:
    """The params of every `method` request the providers simulator answered."""
    out = []
    if not W.replay_log or not W.replay_log.exists():
        return out
    pat = re.compile(r"-> " + re.escape(method) + r" \(seat simulator\) (\{.*\})$")
    for line in W.replay_log.read_text().splitlines():
        m = pat.search(line)
        if m:
            try:
                out.append(json.loads(m.group(1)))
            except json.JSONDecodeError:
                pass
    return out


def config_reads(W: Walk) -> int:
    if not W.replay_log or not W.replay_log.exists():
        return 0
    return sum(1 for l in W.replay_log.read_text().splitlines()
               if "-> profile/llm/list (seat simulator, profile config)" in l or "profile config: injected ERROR" in l)


def _inside(r, vp, tol=1.5) -> bool:
    return (r[0] >= vp[0] - tol and r[0] + r[2] <= vp[0] + vp[2] + tol
            and r[1] >= vp[1] - tol and r[1] + r[3] <= vp[1] + vp[3] + tol)


def scroll_into(W: Walk, wid: str, vp: str, tries: int = 30) -> bool:
    """`Walk.scroll_into`, with the wheel in the editor's scroll GUTTER (its
    right 10 px, outside every row): over a field or a nested list (the
    Models list is the web's own capped `suggestionList`) the wheel is that
    widget's, not the form's — a person scrolls the form beside them."""
    if vp != EVP:
        return W.scroll_into(wid, vp)
    sign, direction, stuck = 1.0, "down", 0
    for _ in range(tries):
        sn = W.snap()
        v, r = W.rect(vp, sn=sn), W.rect(wid, sn=sn)
        if v is None:
            return False
        if r is not None and _inside(r, v):
            # The instrument reports a partly scrolled-out child's CLIPPED
            # rect, which looks inside: a target on an edge gets one more
            # step, so it is drawn whole.
            edge = 1.5
            at_bottom = r[1] + r[3] >= v[1] + v[3] - edge
            at_top = r[1] <= v[1] + edge
            if at_bottom != at_top:
                W.get(f"/m?k=scroll&x={v[0] + v[2] - 5:.0f}&y={v[1] + v[3] / 2:.0f}&dy={60 * sign if at_bottom else -60 * sign:.0f}&wait=1", tolerant=True)
                time.sleep(0.25)
            return True
        want_down = (r[1] + r[3] > v[1] + v[3]) if r is not None else direction == "down"
        # A reference must be ONE widget: board 1's unnamed nodes share the
        # id "-", so only a uniquely named b1_ widget can show the move.
        names = [str(w.get("i", "")) for w in sn]
        refs = [w for w in sn if W.shown(w) and w.get("i") != vp and _inside(w["r"], v) and w["r"][3] < v[3] * 0.8
                and str(w.get("i", "")).startswith("b1_") and names.count(str(w.get("i"))) == 1]
        ref = refs[len(refs) // 2] if refs else None
        dy = (120.0 if want_down else -120.0) * sign
        W.get(f"/m?k=scroll&x={v[0] + v[2] - 5:.0f}&y={v[1] + v[3] / 2:.0f}&dy={dy:.0f}&wait=1", tolerant=True)
        time.sleep(0.25)
        if ref is None:
            continue
        after = [w["r"] for w in W.snap() if w.get("i") == ref["i"] and W.shown(w)]
        if not after:
            stuck = 0
            continue
        moved = after[0][1] - ref["r"][1]
        if abs(moved) < 0.5:
            stuck += 1
            if r is None and stuck >= 2:
                direction = "up" if direction == "down" else "down"
                stuck = 0
            continue
        stuck = 0
        if (moved < 0) != want_down:
            sign = -sign
    r, v = W.rect(wid), W.rect(vp)
    return bool(r and v and _inside(r, v))


def scroll_end(W: Walk, vp: str, up: bool = True, tries: int = 12) -> None:
    """Wheel `vp` to its top (or bottom) — a capture frames its state from
    the scroll's natural start, never mid-row."""
    for _ in range(tries):
        v = W.rect(vp)
        if not v:
            return
        before = [(w.get("i"), w["r"][1]) for w in W.snap() if W.shown(w) and str(w.get("i", "")).startswith(("b1_", "b3_routes_"))]
        x = v[0] + v[2] - 5 if vp == EVP else v[0] + v[2] / 2
        W.get(f"/m?k=scroll&x={x:.0f}&y={v[1] + v[3] / 2:.0f}&dy={-600 if up else 600}&wait=1", tolerant=True)
        time.sleep(0.3)
        after = [(w.get("i"), w["r"][1]) for w in W.snap() if W.shown(w) and str(w.get("i", "")).startswith(("b1_", "b3_routes_"))]
        if before == after:
            return


def seen(W: Walk, wid: str, vp: str = VP) -> bool:
    return bool(W.visible(wid)) or scroll_into(W, wid, vp)


def click_in(W: Walk, wid: str, vp: str, expect, secs: float = 8.0) -> bool:
    time.sleep(0.4)

    def once() -> bool:
        if not scroll_into(W, wid, vp):
            W.note(f"SCROLL {wid} into {vp} — failed")
        return W.click(wid)

    ok = once() and W.wait(expect, secs)
    if not ok:
        W.note(f"RETRY {wid}")
        ok = once() and W.wait(expect, secs)
    return ok


def val(W: Walk, wid: str) -> str:
    hits = W.visible(wid)
    return (hits[0].get("val") or hits[0].get("t") or "") if hits else ""


def type_secret(W: Walk, text: str) -> None:
    """`Walk.type_text` without the text in the transcript: a typed key never
    reaches walk.log (the log names only its length)."""
    W.note(f"TYPE <key, {len(text)} chars>")
    W.get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}), tolerant=True)
    if W.mode == "phone":
        W.kb_up = True
    time.sleep(0.2)


def type_into(W: Walk, wid: str, vp: str, text: str, clear: int = 0, secret: bool = False) -> bool:
    """Click a field (scrolled into view), optionally clear `clear` chars
    from its end, type `text` (a `secret` is never logged)."""
    if not scroll_into(W, wid, vp):
        return False
    r = W.rect(wid)
    if not r:
        return False
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    if clear:
        W.key("End")
        W.clear_field(clear)
    if secret:
        type_secret(W, text)
    else:
        W.type_text(text)
    return True


def key_fragments() -> list:
    """Every typed key whole, its first 8 and its last 8 characters."""
    out = []
    for k in (DUMMY, REJECTED):
        out += [k, k[:8], k[-8:]]
    return out


def leaks(text: str) -> list:
    """The typed-key fragments in `text`. The last 8 of the refused key is the
    English word "rejected", so it counts only glued to a key-like left side
    ("test-rejected", "…rejected", "*rejected"), never in the sentence "The
    provider rejected …" or a capture name ("10-rejected-desktop")."""
    found = []
    for f in key_fragments():
        if f == REJECTED[-8:]:
            if re.search(r"(?:[A-Za-z]-|[…*•])rejected", text):
                found.append(f)
        elif f in text:
            found.append(f)
    return sorted(set(found))


def shot(W: Walk, name: str) -> None:
    """A capture whose snap never keeps a typed key: the instrument reports a
    masked field's buffer as `val`; the editor's key field is blanked (the
    shared scrub only knows the secret-like ids), then the whole capture is
    checked for the dummy keys."""
    W.shot(name)
    p = pathlib.Path(W.out) / f"{name}.snap.json"
    tree = json.loads(p.read_text())

    def walk(x):
        if isinstance(x, dict):
            if str(x.get("i") or "") == "b1_prov_key" and isinstance(x.get("val"), str):
                x["val"] = ""
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)
    walk(tree)
    text = json.dumps(tree)
    p.write_text(text)
    found = leaks(text)
    W.check(f"{name}: the capture carries no typed key (whole, first 8, last 8)", not found, ", ".join(found))


def dialog_numeric(W: Walk, name: str) -> None:
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_routes_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))


def card_numeric(W: Walk, name: str) -> None:
    """The board-1 editor's numeric checks (A2's walk): the card inside the
    module view, centred with the web's 460 px width on a desktop / the full
    sheet on a phone; every b1_ widget inside it; controls >= 28 px; no
    overlapping leaves; equal content padding."""
    sn = W.snap()
    view = W.module_rect(sn)
    card = W.rect("b1_card", sn=sn)
    if not view or not card:
        W.check(f"{name}: board-1 card numeric checks", False, "no card")
        return
    b1 = [w for w in sn if str(w.get("i", "")).startswith("b1_") and Walk.shown(w)
          and w.get("i") not in ("b1_card", "b1_sheet", "b1_backdrop") and not str(w.get("i")).startswith("b1_set")]
    # A row the editor's scroll edge cuts is clipped by design (more above or
    # below): the instrument reports its CLIPPED rect — not judged (A10's
    # dialog checker does the same).
    vp = W.rect(EVP, sn=sn)
    if vp:
        top, bot = vp[1], vp[1] + vp[3]

        def cut(r):
            return abs(r[1] - top) <= 1.5 or abs(r[1] + r[3] - bot) <= 1.5 or r[1] < top - 0.5 < r[1] + r[3] or r[1] < bot - 0.5 < r[1] + r[3] - 0.5

        b1 = [w for w in b1 if w.get("i") == EVP or not (_inside(w["r"], [vp[0] - 1, vp[1] - 1, vp[2] + 2, vp[3] + 2]) and cut(w["r"]))]
    left, right = card[0] - view[0], (view[0] + view[2]) - (card[0] + card[2])
    width_ok = card[2] == 460 if MODE == "desktop" else abs(card[2] - view[2]) <= 1
    outside = [w["i"] for w in b1 if not (w["r"][0] >= card[0] - 0.5 and w["r"][1] >= card[1] - 0.5
                                           and w["r"][0] + w["r"][2] <= card[0] + card[2] + 0.5
                                           and w["r"][1] + w["r"][3] <= card[1] + card[3] + 0.5)]
    small = [w["i"] for w in b1 if w.get("ty") in ("Button", "TextInput") and w["r"][3] < 28 - 0.5]
    leaves = [w for w in b1 if w.get("ty") in ("Button", "TextInput", "Label", "Svg")]
    over = []
    for i, a in enumerate(leaves):
        for b in leaves[i + 1:]:
            ax, ay, aw, ah = a["r"]
            bx, by, bw, bh = b["r"]
            if min(ax + aw, bx + bw) - max(ax, bx) > 1 and min(ay + ah, by + bh) - max(ay, by) > 1:
                inside = (ax >= bx - 0.5 and ay >= by - 0.5 and ax + aw <= bx + bw + 0.5 and ay + ah <= by + bh + 0.5) or \
                         (bx >= ax - 0.5 and by >= ay - 0.5 and bx + bw <= ax + aw + 0.5 and by + bh <= ay + ah + 0.5)
                if not inside:
                    over.append((a["i"], b["i"]))
    pads = (round(min(w["r"][0] for w in leaves) - card[0], 1), round(card[0] + card[2] - max(w["r"][0] + w["r"][2] for w in leaves), 1))
    ok = abs(left - right) <= 1 and width_ok and not outside and not small and not over and abs(pads[0] - pads[1]) <= 1.5
    W.check(f"{name}: board-1 card numeric checks", ok,
            f"card={card} margins=[{left:.0f}, {right:.0f}] pads={list(pads)} widgets={len(b1)} outside={outside} under28={small} overlaps={over[:4]}")


def open_providers(W: Walk) -> bool:
    ok = W.palette_run("mo", "/model") and W.wait_shown("dlg_models_t_title", 10)
    W.check("models: /model palette CLICK opens the Models dialog", ok)
    W.scroll_into("dlg_models_manage_providers_control", "dialog_scroll")
    ok = W.click("dlg_models_manage_providers_control") and W.wait(lambda: W.text("b3_title") == "Model providers", 10)
    return W.check("providers: 'Manage providers' CLICK opens the providers dialog", ok)


def row_index(W: Walk, title: str) -> int | None:
    for i in range(8):
        if seen(W, f"b3_routes_row_{i}_name") and W.text(f"b3_routes_row_{i}_name") == title:
            return i
    return None


def back_to_dialog(W: Walk, secs: float = 10) -> bool:
    return W.wait(lambda: W.text("b3_title") == "Model providers" and not W.visible("b1_card"), secs)


# ---------------------------------------------------------------- the walks

def main_walk(W: Walk) -> None:
    W.note("== 1. the directory")
    if not open_providers(W):
        return
    W.check("providers: the recorded rows and the web unit tests' two",
            W.wait(lambda: seen(W, "b3_routes_row_3_name"), 8)
            and [W.text(f"b3_routes_row_{i}_name") for i in range(4) if seen(W, f"b3_routes_row_{i}_name")]
            == ["Deepseek V4 Flash", "Deepseek V4 Flash", "Kimi K2", "GLM-5.3-Flash"],
            str([W.text(f"b3_routes_row_{i}_name") for i in range(4)]))
    scroll_end(W, VP)
    W.check("providers: 'Add provider', the runtime warning and the Primary tag",
            bool(W.visible("b3_routes_add_provider")) and seen(W, "b3_routes_warning_head")
            and seen(W, "b3_routes_row_0_primary_chip"))
    W.check("providers: the credential indicator (a value-safe boolean)",
            W.text("b3_routes_row_0_credential") == "Credential configured")
    W.check("wire: the configuration and the catalog read side by side",
            config_reads(W) >= 1 and W.replay_saw("profile/llm/catalog", 2) >= 1)
    scroll_end(W, VP)
    dialog_numeric(W, "providers")
    shot(W, f"01-providers-{MODE}")
    W.check("blocked: the Kimi K2 row shows why and offers no Edit, still Delete",
            seen(W, "b3_routes_row_2_reason") and W.text("b3_routes_row_2_reason") == EDIT_BLOCKED
            and not W.visible("b3_routes_row_2_edit") and seen(W, "b3_routes_row_2_delete"))
    dialog_numeric(W, "blocked row")
    shot(W, f"02-blocked-row-{MODE}")

    W.note("== 2. Edit a FALLBACK (the GLM row with configured inference): a label-only save")
    ups0 = len(wire(W, "profile/llm/upsert"))
    W.check("edit: 'Edit' (GLM-5.3-Flash) CLICK opens the board-1 editor for that row",
            click_in(W, "b3_routes_row_3_edit", VP, lambda: W.text("b1_title") == "Edit provider"))
    W.wait(lambda: val(W, "b1_prov_name") == "Z.AI · Official API", 4)
    name0, url0 = val(W, "b1_prov_name"), val(W, "b1_prov_url")
    W.check("edit: the row's own values (name, the official URL)",
            name0 == "Z.AI · Official API" and url0 == "https://api.z.ai/api/paas/v4", f"{name0!r} {url0!r}")
    W.check("edit: the identity is fixed (Provider / Route rows + the web's hints)",
            seen(W, "b1_prov_identity_hint", EVP) and W.text("b1_prov_identity_hint").startswith("Provider identity is fixed."))
    scroll_end(W, EVP)
    card_numeric(W, "edit")
    shot(W, f"03-edit-{MODE}")
    W.check("edit: GLM-5.3-Flash shows the provider's read-only guidance",
            seen(W, "b1_prov_glm_title", EVP) and W.text("b1_prov_glm_title") == "GLM-5.3-Flash recommended settings"
            and W.text("b1_prov_glm_sub") == "Read-only provider guidance")
    scroll_end(W, EVP, up=False)
    card_numeric(W, "edit guidance")
    shot(W, f"03b-edit-guidance-{MODE}")
    W.check("edit: rename the route ('Official API' -> 'Coding route') in the Name field",
            type_into(W, "b1_prov_name", EVP, "Coding route", clear=len("Official API"))
            and W.wait(lambda: val(W, "b1_prov_name") == "Z.AI · Coding route", 4), repr(val(W, "b1_prov_name")))
    W.dismiss_keyboard("b1_title")
    W.check("edit: Save CLICK -> test then upsert -> back to the providers with the web's line",
            click_in(W, "b1_prov_save", EVP, lambda: back_to_dialog(W, 1) and W.text("b3_routes_notice") == SAVED_EDIT, 12))
    ups = wire(W, "profile/llm/upsert")
    sel = ups[-1]["selection"] if len(ups) > ups0 else {}
    W.check("wire: the label-only upsert carries every configured inference value (null kept), set_primary false, no key",
            len(ups) == ups0 + 1 and all(k in sel and sel[k] == v for k, v in INFERENCE.items())
            and sel.get("route", {}).get("label") == "Coding route" and "max_output_tokens" not in sel
            and ups[-1].get("set_primary") is False and "api_key" not in ups[-1], json.dumps(ups[-1] if ups else {}))
    W.check("edit: the row now reads 'Z.AI · Coding route'",
            seen(W, "b3_routes_row_3_meta") and W.text("b3_routes_row_3_meta") == "Z.AI · Coding route")
    scroll_end(W, VP)
    dialog_numeric(W, "saved edit")
    shot(W, f"04-saved-edit-{MODE}")

    W.note("== 3. Add provider (catalog-driven) -> Test connection -> Fetch -> Save")
    W.check("add: 'Add provider' CLICK opens the editor on the first catalog model not configured",
            click_in(W, "b3_routes_add_provider", VP, lambda: W.text("b1_title") == "Add provider")
            and W.wait(lambda: W.text("b1_prov_family_value") == "Anthropic", 4)
            and val(W, "b1_prov_model_id") == "claude-3-5-haiku-20241022", f"{W.text('b1_prov_family_value')!r}")
    W.check("add: the Provider select CLICK lists the catalog's families",
            click_in(W, "b1_prov_family", EVP, lambda: bool(W.visible("b1_prov_opt_t2"))))
    opt = next((i for i in range(8) if W.text(f"b1_prov_opt_t{i}") == "DeepSeek"), None)
    card_numeric(W, "family select")
    shot(W, f"05-add-family-{MODE}")
    W.check("add: 'DeepSeek' CLICK -> its first model on its Official API",
            opt is not None and click_in(W, f"b1_prov_opt_{opt}", EVP, lambda: W.text("b1_prov_family_value") == "DeepSeek")
            and W.wait(lambda: val(W, "b1_prov_model_id") == "deepseek-v4-flash", 4)
            and val(W, "b1_prov_name") == "DeepSeek · Official API")
    W.check("add: the model 'deepseek-v4-pro' CLICK fills the Model ID",
            click_in(W, "b1_prov_model_1", EVP, lambda: val(W, "b1_prov_model_id") == "deepseek-v4-pro"))
    # Read every selection back before any key is typed (LESSONS): each
    # field scrolled into view, then read.
    back = {}
    for wid in ("b1_prov_family_value", "b1_prov_model_id", "b1_prov_name", "b1_prov_url", "b1_prov_env", "b1_prov_route_id"):
        back[wid] = (W.text(wid) if wid.endswith("_value") else val(W, wid)) if seen(W, wid, EVP) else None
    W.check("add: read back — family, model, route name, the official URL, the credential reference, the route id",
            back == {"b1_prov_family_value": "DeepSeek", "b1_prov_model_id": "deepseek-v4-pro",
                     "b1_prov_name": "DeepSeek · Official API", "b1_prov_url": "https://api.deepseek.com/v1",
                     "b1_prov_env": "DEEPSEEK_API_KEY", "b1_prov_route_id": "deepseek"}, json.dumps(back))
    scroll_end(W, EVP)
    card_numeric(W, "add")
    shot(W, f"06-add-{MODE}")
    tests0, ups0, fetch0 = len(wire(W, "profile/llm/test")), len(wire(W, "profile/llm/upsert")), len(wire(W, "profile/llm/fetch_models"))
    W.check("add: the dummy key typed into the masked API key field", type_into(W, "b1_prov_key", EVP, DUMMY, secret=True))
    W.dismiss_keyboard("b1_title")
    W.check("add: 'Test connection' CLICK -> 'Connection succeeded.'",
            click_in(W, "b1_prov_test", EVP, lambda: seen(W, "b1_prov_feedback", EVP) and W.text("b1_prov_feedback") == "Connection succeeded.", 10))
    tests = wire(W, "profile/llm/test")
    W.check("wire: Test sends profile/llm/test ALONE, the key as its argument (never in the selection)",
            len(tests) == tests0 + 1 and len(wire(W, "profile/llm/upsert")) == ups0
            and tests[-1].get("api_key") == masked(DUMMY)
            and tests[-1]["selection"] == {"family_id": "deepseek", "model_id": "deepseek-v4-pro",
                                           "route": {"route_id": "deepseek", "label": "Official API",
                                                     "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"}},
            json.dumps(tests[-1] if tests else {}))
    scroll_into(W, "b1_prov_test", EVP)
    card_numeric(W, "tested")
    shot(W, f"07-tested-{MODE}")
    W.check("add: 'Fetch available models' CLICK -> the endpoint's models",
            click_in(W, "b1_prov_fetch", EVP, lambda: seen(W, "b1_prov_fetch_feedback", EVP)
                     and W.text("b1_prov_fetch_feedback") == "3 available models found.", 10)
            and seen(W, "b1_prov_fetched_label", EVP) and W.text("b1_prov_fetched_count") == "3")
    fetches = wire(W, "profile/llm/fetch_models")
    W.check("wire: one profile/llm/fetch_models, the unsaved endpoint (family + route), the key as its argument",
            len(fetches) == fetch0 + 1 and fetches[-1]["selection"].get("family_id") == "deepseek"
            and "model_id" not in fetches[-1]["selection"] and fetches[-1].get("api_key") == masked(DUMMY),
            json.dumps(fetches[-1] if fetches else {}))
    scroll_into(W, "b1_prov_fetched_label", EVP)
    card_numeric(W, "fetched")
    shot(W, f"08-fetched-{MODE}")
    W.check("add: Save CLICK -> test then upsert -> back with 'Provider saved.' and the new row",
            click_in(W, "b1_prov_save", EVP, lambda: back_to_dialog(W, 1) and W.text("b3_routes_notice") == "Provider saved.", 12)
            and W.wait(lambda: row_index(W, "Deepseek V4 Pro") is not None, 6))
    ups = wire(W, "profile/llm/upsert")
    W.check("wire: the save tested then upserted ONE provision, a fallback, the key as its argument",
            len(wire(W, "profile/llm/test")) == tests0 + 2 and len(ups) == ups0 + 1
            and ups[-1]["selection"]["model_id"] == "deepseek-v4-pro" and ups[-1].get("set_primary") is False
            and ups[-1].get("api_key") == masked(DUMMY), json.dumps(ups[-1] if ups else {}))
    scroll_end(W, VP)
    dialog_numeric(W, "added")
    shot(W, f"09-added-{MODE}")

    W.note("== 4. a refused key on Test connection (p4-07): the draft kept, nothing saved")
    ups0 = len(wire(W, "profile/llm/upsert"))
    W.check("rejected: 'Edit' (the r2-route fallback) CLICK opens its editor",
            click_in(W, "b3_routes_row_1_edit", VP, lambda: W.text("b1_title") == "Edit provider")
            and W.wait(lambda: val(W, "b1_prov_url") == "http://127.0.0.1:9/v1", 4))
    name0, url0 = val(W, "b1_prov_name"), val(W, "b1_prov_url")
    W.check("rejected: the row read back before the key is typed", name0 == "DeepSeek · R2 Route", repr(name0))
    W.check("rejected: the refused dummy key typed", type_into(W, "b1_prov_key", EVP, REJECTED, secret=True))
    W.dismiss_keyboard("b1_title")
    W.check("rejected: 'Test connection' CLICK -> the provider rejected this key (401), read where Test was clicked",
            click_in(W, "b1_prov_test", EVP, lambda: seen(W, "b1_prov_feedback", EVP)
                     and W.text("b1_prov_feedback") == "The provider rejected this key (401). Your draft is kept.", 10))
    W.check("rejected: the editor stayed where Test was clicked (the remount keeps the body's scroll)",
            bool(W.visible("b1_prov_test")))
    kept = {wid: (val(W, wid) if seen(W, wid, EVP) else None) for wid in ("b1_prov_name", "b1_prov_url")}
    W.check("rejected: the draft is kept (name, URL) and Save stays Save",
            kept == {"b1_prov_name": name0, "b1_prov_url": url0} and W.text("b1_prov_save") != "Try again", json.dumps(kept))
    W.check("rejected: no server prose, nothing saved",
            not W.has_text("Authentication Fails") and len(wire(W, "profile/llm/upsert")) == ups0)
    scroll_into(W, "b1_prov_feedback", EVP)
    card_numeric(W, "rejected")
    shot(W, f"10-rejected-{MODE}")
    W.check("rejected: Cancel CLICK -> back to the providers, nothing saved",
            click_in(W, "b1_prov_cancel", EVP, lambda: back_to_dialog(W, 1), 10) and not W.visible("b3_routes_notice"))

    W.note("== 5. Delete the edit-blocked row behind the typed phrase")
    phrase = "DELETE moonshot/kimi-k2"
    W.check("delete: 'Delete' (Kimi K2) CLICK -> the phrase to type",
            click_in(W, "b3_routes_row_2_delete", VP, lambda: seen(W, "b3_routes_delete_prompt"))
            and phrase in W.text("b3_routes_delete_prompt")
            and "Kimi K2" in W.text("b3_routes_delete_body"))
    W.check("delete: 'Delete provider' is inert before the phrase", bool(W.visible("b3_routes_delete_off")))
    W.check("delete: the exact phrase typed", type_into(W, "b3_routes_phrase", VP, phrase))
    W.dismiss_keyboard()
    W.check("delete: the exact phrase arms 'Delete provider' (live, no remount)",
            W.wait(lambda: seen(W, "b3_routes_delete_go") and bool(W.visible("b3_routes_delete_on")), 6))
    W.scroll_into("b3_routes_delete_title", VP)
    dialog_numeric(W, "delete")
    shot(W, f"11-delete-{MODE}")
    W.check("delete: CLICK -> profile/llm/delete -> 'Provider deleted. …'; the row gone",
            click_in(W, "b3_routes_delete_go", VP, lambda: W.text("b3_routes_notice").startswith("Provider deleted.")
                     and row_index(W, "Kimi K2") is None, 10))
    dels = wire(W, "profile/llm/delete")
    W.check("wire: the delete names the row's identity",
            dels and {k: dels[-1].get(k) for k in ("family_id", "model_id", "route_id")}
            == {"family_id": "moonshot", "model_id": "kimi-k2", "route_id": "moonshot"}, json.dumps(dels[-1] if dels else {}))
    W.check("providers: the close glyph CLICK closes the dialog", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


def past_startup(W: Walk) -> None:
    """The replay injects its slow / refused configuration reads only from
    5 s into the connection (the app's start-up reads it too): let that
    window pass before the dialog opens."""
    W.note("WAIT 7 s (past the replay's start-up window)")
    time.sleep(7)


def loading_walk(W: Walk) -> None:
    past_startup(W)
    if not open_providers(W):
        return
    W.check("loading: the first read in flight shows the loading line (no rows, no empty state)",
            W.wait(lambda: W.text("b3_routes_loading") == "Loading model providers…", 4)
            and not W.visible("b3_routes_empty") and not W.visible("b3_routes_row_0_name"))
    dialog_numeric(W, "loading")
    shot(W, f"01-loading-{MODE}")
    W.check("loading: the read lands -> the rows", W.wait(lambda: seen(W, "b3_routes_row_1_name"), 12)
            and not W.visible("b3_routes_loading"))


def unread_walk(W: Walk) -> None:
    """The configuration read refused: the replay refuses profile-config
    reads while FAIL_FILE exists — created once the Models dialog (which
    reads the configuration too) is up, right before "Manage providers",
    removed before "Try again"."""
    ok = W.palette_run("mo", "/model") and W.wait_shown("dlg_models_t_title", 10)
    W.check("models: /model palette CLICK opens the Models dialog", ok)
    time.sleep(1.5)
    FAIL_FILE.parent.mkdir(parents=True, exist_ok=True)
    FAIL_FILE.write_text("refuse")
    W.scroll_into("dlg_models_manage_providers_control", "dialog_scroll")
    W.check("providers: 'Manage providers' CLICK opens the providers dialog",
            W.click("dlg_models_manage_providers_control") and W.wait(lambda: W.text("b3_title") == "Model providers", 10))
    W.check("unread: the refused read is the cause and 'Try again' — never 'No model providers configured'",
            W.wait(lambda: W.text("b3_routes_error") == "profile store unavailable", 8)
            and bool(W.visible("b3_routes_retry")) and not W.visible("b3_routes_empty")
            and not W.visible("b3_routes_row_0_name") and not W.visible("b3_routes_add_provider"))
    dialog_numeric(W, "unread")
    shot(W, f"01-unread-{MODE}")
    FAIL_FILE.unlink(missing_ok=True)
    reads = config_reads(W)
    W.check("unread: 'Try again' CLICK reads again -> the rows",
            click_in(W, "b3_routes_retry", VP, lambda: seen(W, "b3_routes_row_1_name"), 10)
            and not W.visible("b3_routes_error") and config_reads(W) == reads + 1)
    dialog_numeric(W, "unread retried")
    shot(W, f"02-retried-{MODE}")


def readonly_walk(W: Walk) -> None:
    if not open_providers(W):
        return
    W.check("readonly: the web's notice, no Add provider, no Edit, no route add",
            W.wait(lambda: W.text("b3_routes_readonly_note_text") == "Provider configuration is read-only on this server.", 8)
            and not W.visible("b3_routes_add_provider") and seen(W, "b3_routes_row_1_name")
            and not W.visible("b3_routes_row_0_edit") and not W.visible("b3_routes_row_1_edit")
            and not W.visible("b3_routes_row_1_add"))
    W.check("readonly: Delete rides its own advertised method", seen(W, "b3_routes_row_1_delete"))
    scroll_end(W, VP)
    dialog_numeric(W, "readonly")
    shot(W, f"01-readonly-{MODE}")


def empty_walk(W: Walk) -> None:
    if not open_providers(W):
        return
    W.check("empty: a configuration READ as empty is the empty state with Add provider",
            W.wait(lambda: W.text("b3_routes_empty") == "No model providers configured", 8)
            and W.text("b3_routes_empty_hint") == "Add a provider route to make a model available to Core."
            and bool(W.visible("b3_routes_add_provider")))
    dialog_numeric(W, "empty")
    shot(W, f"01-empty-{MODE}")
    W.check("empty: 'Add provider' CLICK -> the first catalog model becomes the primary '(default)'",
            click_in(W, "b3_routes_add_provider", VP, lambda: W.text("b1_title") == "Add provider")
            and W.wait(lambda: W.text("b1_prov_model_t0").endswith("(default)"), 4), W.text("b1_prov_model_t0"))
    card_numeric(W, "empty add")
    shot(W, f"02-empty-add-{MODE}")
    W.check("empty: the back chevron CLICK returns to the providers",
            click_in(W, "b1_prov_back", EVP, lambda: back_to_dialog(W, 1), 10))


WALKS = {
    "main": (main_walk, ["--providers-extra"]),
    "loading": (loading_walk, ["--slow", "profile/llm/list@profile=4000"]),
    "unread": (unread_walk, ["--fail-config-file", str(FAIL_FILE)]),
    "readonly": (readonly_walk, ["--drop-method", "profile/llm/upsert"]),
    "empty": (empty_walk, ["--providers-empty"]),
}

if __name__ == "__main__":
    fn, replay_args = WALKS[WHICH]
    FAIL_FILE.unlink(missing_ok=True)
    rc = run_session(fn, mode=MODE, outdir=OUT, scenario="a10", replay_args=replay_args)
    FAIL_FILE.unlink(missing_ok=True)
    # The committed evidence (walk.log, every snap, the wire log — the
    # simulator already masks keys): no typed key, whole, first 8 or last 8.
    for p in sorted(pathlib.Path(OUT).glob("*")):
        if p.suffix in (".log", ".json") and p.is_file():
            found = leaks(p.read_text(errors="replace"))
            if found:
                print(f"FAIL {p.name} carries a typed key fragment: {', '.join(found)}")
                rc = 1
    sys.exit(rc)
