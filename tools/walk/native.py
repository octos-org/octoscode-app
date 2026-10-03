#!/usr/bin/env python3
"""The native click-walk aggregator (A11) — `tools/walk/run.py` calls it, so
ONE walk run regenerates `docs/walk/results*.csv` with the per-row verdicts
of the subagents' native click walks, not only run.py's own checks.

## The convention (how a walk is picked up — no registration)

Any `tools/walk/*.py` or `tools/*/*.py` that carries a top-level literal
`WALK = {...}` (a `*_walk.py` name is the habit, not the rule: A10's walks
are `a10_<area>.py`). It is read with `ast.literal_eval` — the script is
NEVER imported (walk scripts parse argv / touch files at import time). A
`*_walk.py` without a `WALK` literal is listed as "not a native walk" and
skipped.

    WALK = {
        "name": "a2_board1",                  # unique; names the evidence
        "modes": ["desktop", "phone"],        # which windows to walk in
        # The app: "self" = the walk launches (and always stops) its own app
        # and fixtures (it gets {bin}/{port}/{fport} on its argv); else the
        # aggregator launches it hidden with ISOLATED state + this env:
        "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}"},
                "ready": ["b1_connect_pair"]},
        # Optional fixture server, started first, always stopped:
        "fixture": {"argv": ["{examples}/board1_serve", "{fport}"],
                    "fport": 8422,              # a fixed port, else allocated
                    "env": {},                  # its environment additions
                    "pidfile": "{state}/serve.pid"},  # a walk may restart it
        # One run, or several; before a later run "restart" restarts the
        # "app" (state kept), the "fixture" or "both". A run's "app_env",
        # "fixture_env", "fixture_args" and "ready" apply to the launch it
        # causes (the first run's, to the first launch):
        "runs": [{"argv": ["{mode}", "{out}"], "env": {"PORT": "{port}"}},
                 {"restart": "both", "fixture_env": {"A7_SERVE_HELD": "octos-tui"},
                  "app_env": {"OCTOSCODE_PANIC_PROBE": "surface:fleet"}, "argv": [...]}],
        "needs": ["target/debug/examples/board1_serve"],   # built if missing
        "timeout": 900,                       # seconds per run
        # walk row id -> the check-name substrings that prove it (every
        # matched check in every mode must pass); a dict adds a mode split
        # and/or the part of the row's case the walk does NOT cover:
        "rows": {87: ["p4-06", "provider saved"],
                 227: {"checks": {"phone": ["drawer"]},
                       "partial": "the composer-on-screen half is run.py's"}},
    }

Placeholders: {bin} the host binary, {mode}, {port} the app port, {fport}
the fixture port, {fixture_log} the running fixture's output, {state} the
run's isolated state dir, {out} the run's scratch output dir, {examples}
target/debug/examples, {root} the checkout, {python} this interpreter.

A walk prints one line per check — `PASS <name>` or `FAIL <name> — <detail>`
— and exits 0 iff all passed (the shape every walk already printed).

## Isolation (brief §8)

Every app the aggregator launches — and every app a "self" walk launches,
through the inherited environment — gets a fresh per-run state tree for
drafts, credentials, preferences, notifications, recents, show-thinking,
downloads and display preferences (`walk_env.isolated_env`), never the
operator's home state. The
phone window is launched straight into OctosCode
(`--test-action page:0 --test-action launch-octoscode`, 360x780).

## Evidence

Each (walk, mode) leaves `docs/walk/evidence/native/<name>-<mode>.log`: the
walk's own output (machine paths scrubbed). A check's evidence cell is that
file plus the line of its PASS/FAIL. Captures stay in the scratch `{out}`
(tmp/), the agents' UX evidence lives under docs/ux/.
"""
from __future__ import annotations

import ast
import json
import os
import pathlib
import re
import shutil
import signal
import socket
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from walk_env import ROOT, App, app_env, isolated_env, scrub  # noqa: E402

EXAMPLES = ROOT / "target" / "debug" / "examples"
EVIDENCE = ROOT / "docs" / "walk" / "evidence" / "native"
SCRATCH = ROOT / "tmp" / "walk" / "native"
LINE_RE = re.compile(r"^\s*(PASS|FAIL)\s+(.+?)(?:\s+—\s+(.*))?\s*$")
DEFAULT_READY = ["connect_btn", "b1_connect_pair", "i0_composer_0", "sidebar_toggle_hit", "b3_strip_tap"]


# ------------------------------------------------------------------ discovery

NAMED = ("tools/walk/*_walk.py", "tools/*/*_walk.py")
ANY = ("tools/walk/*.py", "tools/*/*.py")


def walk_paths(root: pathlib.Path = ROOT) -> list:
    """Every `*_walk.py`, plus any other tools script that carries a top-level
    `WALK = {` literal (A10's walks are `a10_<area>.py`)."""
    seen, out = set(), []
    for pattern in NAMED + ANY:
        for p in sorted(root.glob(pattern)):
            if p.resolve() in seen:
                continue
            if pattern in ANY:
                try:
                    if "\nWALK = {" not in p.read_text(errors="ignore"):
                        continue
                except OSError:
                    continue
            seen.add(p.resolve())
            out.append(p)
    return out


def read_spec(path: pathlib.Path):
    """The script's `WALK` literal, or None. Never imports the script."""
    try:
        tree = ast.parse(path.read_text(), filename=str(path))
    except (OSError, SyntaxError):
        return None
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
                isinstance(t, ast.Name) and t.id == "WALK" for t in node.targets):
            try:
                spec = ast.literal_eval(node.value)
            except ValueError:
                return None
            return spec if isinstance(spec, dict) and spec.get("name") else None
    return None


def discover(root: pathlib.Path = ROOT, only=None) -> list:
    """[(path, spec)] of every native walk, in name order."""
    out = []
    for p in walk_paths(root):
        spec = read_spec(p)
        if spec is None:
            continue
        if only and spec["name"] not in only:
            continue
        out.append((p, spec))
    return sorted(out, key=lambda ps: ps[1]["name"])


def skipped_scripts(root: pathlib.Path = ROOT) -> list:
    return [p for p in walk_paths(root) if read_spec(p) is None]


# ---------------------------------------------------------------- expansion

def expand(value, ctx: dict):
    """Fill {placeholders} in every string of a spec value."""
    if isinstance(value, str):
        out = value
        for k, v in ctx.items():
            out = out.replace("{" + k + "}", str(v))
        return out
    if isinstance(value, list):
        return [expand(v, ctx) for v in value]
    if isinstance(value, dict):
        return {k: expand(v, ctx) for k, v in value.items()}
    return value


def runs_of(spec: dict) -> list:
    if spec.get("runs"):
        return list(spec["runs"])
    return [{"argv": spec.get("argv", []), "env": spec.get("env", {})}]


def parse_checks(text: str) -> list:
    """[(name, ok, detail, line_no)] from a walk's output."""
    out = []
    for n, line in enumerate(text.splitlines(), start=1):
        m = LINE_RE.match(line)
        if m:
            out.append((m.group(2).strip(), m.group(1) == "PASS", (m.group(3) or "").strip(), n))
    return out


# ------------------------------------------------------------------ helpers

def port_free(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.3)
        return s.connect_ex(("127.0.0.1", port)) != 0


def ensure_built(spec: dict) -> str | None:
    """Build the examples a walk needs; None, or why it cannot run."""
    for need in spec.get("needs", []):
        p = ROOT / need
        if p.exists():
            continue
        m = re.match(r"target/debug/examples/([A-Za-z0-9_]+)$", need)
        if not m:
            return f"missing {need}"
        r = subprocess.run(["cargo", "build", "-p", "octoscode-module", "--example", m.group(1)],
                           cwd=str(ROOT), capture_output=True, text=True)
        if r.returncode != 0 or not p.exists():
            return f"could not build {need}: {(r.stderr or r.stdout)[-300:]}"
    return None


def kill_pid(pid: int):
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return
    for _ in range(25):
        try:
            os.kill(pid, 0)
        except OSError:
            return
        time.sleep(0.2)
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError:
        pass


class FixtureProc:
    def __init__(self, argv, log, pidfile=None, env=None):
        self.argv, self.log, self.pidfile, self.env = argv, pathlib.Path(log), pidfile, env
        self.proc = None

    def start(self, port: int | None, timeout=25.0):
        self.log.parent.mkdir(parents=True, exist_ok=True)
        self.proc = subprocess.Popen(self.argv, stdout=open(self.log, "w"), stderr=subprocess.STDOUT,
                                     cwd=str(ROOT), env=self.env)
        if self.pidfile:
            pathlib.Path(self.pidfile).write_text(str(self.proc.pid))
        end = time.time() + timeout
        while time.time() < end:
            text = self.log.read_text() if self.log.exists() else ""
            if "listening" in text or (port and not port_free(port)):
                time.sleep(0.3)
                return
            if self.proc.poll() is not None:
                raise RuntimeError(f"fixture exited: {text[-300:]}")
            time.sleep(0.2)
        raise RuntimeError("fixture never listened")

    def stop(self):
        # A walk may have replaced the process it was given (A8's reconnect
        # check restarts the fixture and writes the new pid): stop that one.
        if self.pidfile and pathlib.Path(self.pidfile).exists():
            try:
                pid = int(pathlib.Path(self.pidfile).read_text().strip())
                if not self.proc or pid != self.proc.pid:
                    kill_pid(pid)
            except ValueError:
                pass
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()


# --------------------------------------------------------------------- run

def crash_of(text: str) -> str:
    """The exception line of an uncaught Python traceback in a walk's output
    ('' when the walk ran to its end — FAIL lines and a non-zero exit are a
    finished walk's verdict, not a crash)."""
    if "Traceback (most recent call last)" not in text:
        return ""
    tail = text.split("Traceback (most recent call last)")[-1]
    lines = [l.strip() for l in tail.splitlines() if l.strip() and not l.startswith(" ")]
    return lines[-1] if lines else "Traceback"


def run_walk(path: pathlib.Path, spec: dict, mode: str, binary: str, port: int, fport: int,
             log=print) -> dict:
    """Run one walk in one mode; always stops what it started.

    Returns {"name", "mode", "checks": [(name, ok, detail, evidence)],
    "error": str|None, "log": evidence path (repo-relative)}."""
    name = spec["name"]
    work = SCRATCH / f"{name}-{mode}"
    shutil.rmtree(work, ignore_errors=True)
    state, out = work / "state", work / "out"
    state.mkdir(parents=True)
    out.mkdir(parents=True)
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    ev_path = EVIDENCE / f"{name}-{mode}.log"
    fx_spec = spec.get("fixture")
    if fx_spec and fx_spec.get("fport"):
        fport = int(fx_spec["fport"])
    ctx = {"bin": binary, "mode": mode, "port": port, "fport": fport, "state": state, "out": out,
           "examples": EXAMPLES, "root": ROOT, "python": sys.executable, "fixture_log": ""}
    transcript: list[str] = [f"== native walk {name} [{mode}] ({path.relative_to(ROOT)})"]
    result = {"name": name, "mode": mode, "checks": [], "error": None,
              "log": str(ev_path.relative_to(ROOT))}

    why = ensure_built(spec)
    if why:
        result["error"] = why
        transcript.append(f"BLOCKED {why}")
        ev_path.write_text("\n".join(transcript) + "\n")
        return result

    base_env = os.environ.copy()
    for k in ("OCTOS_BEARER", "OCTOS_PAIRING_LINK"):
        base_env.pop(k, None)
    base_env.update(isolated_env(state))   # inherited by a "self" walk's app
    base_env["PYTHONUNBUFFERED"] = "1"

    fixture = None
    fixtures = []
    app = None
    app_spec = spec.get("app", "self")

    def start_fixture(n: int, run: dict):
        nonlocal fixture
        if not port_free(fport):
            raise RuntimeError(f"fixture port {fport} is taken")
        fenv = dict(base_env)
        fenv.update({k: str(v) for k, v in expand(fx_spec.get("env", {}), ctx).items()})
        fenv.update({k: str(v) for k, v in expand(run.get("fixture_env", {}), ctx).items()})
        log_path = work / f"fixture-{n}.log"
        argv = expand(fx_spec["argv"], ctx) + expand(run.get("fixture_args", []), ctx)
        fixture = FixtureProc(argv, log_path,
                              expand(fx_spec.get("pidfile"), ctx) if fx_spec.get("pidfile") else None, fenv)
        fixtures.append(fixture)
        ctx["fixture_log"] = log_path
        fixture.start(fport)

    def start_app(run: dict):
        nonlocal app
        if not port_free(port):
            raise RuntimeError(f"app port {port} is taken")
        extra = dict(expand(app_spec.get("env", {}), ctx))
        extra.update(expand(run.get("app_env", {}), ctx))
        env = app_env(mode, state, extra, hs=work / "hs")
        app = App(port, env)
        ready = run.get("ready") or app_spec.get("ready", DEFAULT_READY)
        if not app.start(binary, ready_ids=tuple(ready)):
            raise RuntimeError("the app never showed " + "/".join(ready))

    try:
        runs = runs_of(spec)
        if fx_spec:
            start_fixture(1, runs[0] if runs else {})
        if app_spec != "self":
            start_app(runs[0] if runs else {})
        for i, run in enumerate(runs):
            restart = run.get("restart") if i else None
            if restart in ("fixture", "both") and fx_spec:
                transcript.append("-- fixture restart" + (f" with {run.get('fixture_env')}" if run.get("fixture_env") else "")
                                  + (f" args {run.get('fixture_args')}" if run.get("fixture_args") else ""))
                if app is not None and restart == "both":
                    app.stop()
                fixture.stop()
                start_fixture(i + 1, run)
                if app is not None and restart == "both":
                    start_app(run)
            if restart == "app" and app is not None:
                transcript.append("-- app restart (state kept)" + (f" with {run.get('app_env')}" if run.get("app_env") else ""))
                app.stop()
                start_app(run)
            argv = [sys.executable, str(path)] + [str(a) for a in expand(run.get("argv", []), ctx)]
            env = dict(base_env)
            env.update({k: str(v) for k, v in expand(run.get("env", {}), ctx).items()})
            t0 = time.time()
            try:
                r = subprocess.run(argv, cwd=str(ROOT), env=env, capture_output=True, text=True,
                                   timeout=int(spec.get("timeout", 900)))
                text = (r.stdout or "") + (("\n" + r.stderr) if r.stderr.strip() else "")
                rc = r.returncode
            except subprocess.TimeoutExpired as e:
                text = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
                text += f"\nFAIL run {i + 1} finished — timed out after {spec.get('timeout', 900)} s"
                rc = -1
                result["error"] = f"run {i + 1} timed out after {spec.get('timeout', 900)} s"
            transcript.append(f"-- run {i + 1}: exit {rc} in {time.time() - t0:.0f} s")
            transcript.extend(scrub(text).splitlines())
            # A walk that CRASHED (an uncaught exception) did not finish what
            # it maps: every row it proves fails in this mode, even where its
            # checks before the crash passed (a crash never reads green).
            why = crash_of(text)
            if why and not result["error"]:
                result["error"] = f"run {i + 1} crashed (exit {rc}): {scrub(why)[:160]}"
        log(f"  [{name}:{mode}] done")
    except Exception as e:  # noqa: BLE001 — a start failure blocks this walk only
        result["error"] = f"{e}"
        transcript.append(f"BLOCKED {e}")
    finally:
        if app is not None:
            app.stop()
        for fx in fixtures:
            fx.stop()
        if fixture is not None:
            try:
                transcript.append("-- fixture log tail:")
                transcript.extend(scrub(fixture.log.read_text()).splitlines()[-15:])
            except OSError:
                pass
    ev_path.write_text("\n".join(transcript) + "\n")
    lines = ev_path.read_text().splitlines()
    for cname, ok, detail, n in parse_checks("\n".join(lines)):
        result["checks"].append((cname, ok, detail, f"{result['log']}:{n}"))
    return result


# ------------------------------------------------------------------ verdicts

def row_mapping(spec: dict) -> dict:
    """{row_id: {"checks": {mode|"*": [substr]}, "partial": str}}"""
    out = {}
    for rid, v in (spec.get("rows") or {}).items():
        if isinstance(v, (list, tuple)):
            out[int(rid)] = {"checks": {"*": list(v)}, "partial": ""}
        else:
            checks = v.get("checks", [])
            if isinstance(checks, dict):
                checks = {m: list(c) for m, c in checks.items()}
            else:
                checks = {"*": list(checks)}
            out[int(rid)] = {"checks": checks, "partial": v.get("partial", "")}
    return out


def mapped_patterns(spec: dict, rid: int, mode: str):
    """The check-name substrings that prove row `rid` in `mode`, or None when
    the walk does not map that row in that mode."""
    m = row_mapping(spec).get(rid)
    return None if m is None else m["checks"].get(mode, m["checks"].get("*"))


def log_path(name: str, mode: str) -> str:
    """The evidence log a (walk, mode) run leaves (repo-relative)."""
    return str((EVIDENCE / f"{name}-{mode}.log").relative_to(ROOT))


def add_checks(rows: dict, rid: int, spec: dict, mode: str, checks: list, log: str):
    """Fold one (walk, mode)'s checks for row `rid` into `rows` (the shape
    `row_entries` builds). A check is (name, ok, detail, evidence[, extra]);
    `extra` (a dict) rides along to the per-check row (A34: the provenance of a
    check carried from the table)."""
    m = row_mapping(spec)[rid]
    e = rows.setdefault(rid, {"checks": [], "walks": set(), "partial": {}, "logs": set(), "full": False})
    if m["partial"]:
        e["partial"].setdefault(spec["name"], m["partial"])
    else:
        e["full"] = True  # one walk covers the whole case
    e["walks"].add(f"{spec['name']}:{mode}")
    e["logs"].add(log)
    modes = list(spec.get("modes", ["desktop"]))
    # One canonical order — by walk name, then the walk's own mode order — so
    # a verdict assembled from several runs reads like one full run's.
    key = (spec["name"], modes.index(mode) if mode in modes else len(modes))
    e["checks"].extend((key, c) for c in checks)


def row_entries(results: list, specs: dict) -> dict:
    """Every (walk, mode) result's checks, per row it maps (see row_verdicts)."""
    rows: dict = {}
    for res in results:
        spec = specs[res["name"]]
        for rid in row_mapping(spec):
            pats = mapped_patterns(spec, rid, res["mode"])
            if pats is None:
                continue  # this row is not mapped in this mode
            hit = [c for c in res["checks"] if any(p in c[0] for p in pats)]
            checks = []
            if res.get("error"):
                checks.append((f"{res['name']} [{res['mode']}]: the walk ran", False,
                               f"blocked: {res['error']}", res["log"]))
            elif not hit:
                checks.append((f"{res['name']} [{res['mode']}]: a check matching {pats}", False,
                               "no mapped check ran", res["log"]))
            checks += [(f"{res['name']}: {cname} [{res['mode']}]", ok, detail, ev)
                       for cname, ok, detail, ev in hit]
            add_checks(rows, rid, spec, res["mode"], checks, res["log"])
    return rows


def finalize(rows: dict) -> dict:
    """Per-row verdicts from `row_entries` (+ `add_checks`) entries."""
    out = {}
    for rid, e in rows.items():
        checks = [c for _key, c in sorted(e["checks"], key=lambda kc: kc[0])]
        failed = [c[0] for c in checks if not c[1]]
        status = "pass" if checks and not failed else "fail"
        walks = ", ".join(sorted(e["walks"]))
        if failed:
            reason = f"failing native checks: {'; '.join(failed[:4])}" + (" …" if len(failed) > 4 else "")
        else:
            reason = f"{len(checks)} native click-walk checks ({walks}), all pass"
        texts = []
        for name in sorted(e["partial"]):
            if e["partial"][name] not in texts:
                texts.append(e["partial"][name])
        partial = bool(texts) and not e["full"]
        if partial:
            reason += "; not covered: " + " / ".join(texts)
        out[rid] = {"status": status, "depth": "native-partial" if partial else "native",
                    "reason": reason, "evidence": ";".join(sorted(e["logs"])), "checks": checks}
    return out


def row_verdicts(results: list, specs: dict) -> dict:
    """Fold every (walk, mode) result into per-row verdicts.

    `results`: run_walk outputs; `specs`: {name: spec}. A row's checks are the
    union over every walk that maps it, in every mode that walk ran; a mode
    in which NO mapped check ran is a failing synthetic check (a walk that
    crashed early can never read green). Returns {row_id: {...}}."""
    return finalize(row_entries(results, specs))


def run_all(binary: str, port: int, fixture_base: int, modes=None, only=None, log=print) -> tuple:
    """Run every discovered walk in its modes. Returns (results, specs)."""
    found = discover(ROOT, only)
    specs = {s["name"]: s for _, s in found}
    results = []
    # Each walk gets the next FREE fixture port at or after base + its index:
    # agents hold their own port blocks on this host, and a counted port that
    # one of them holds used to BLOCK the walk ("fixture port N is taken").
    next_fport = fixture_base
    for k, (path, spec) in enumerate(found):
        fport = max(next_fport, fixture_base + k)
        for _ in range(400):
            if fport != port and port_free(fport):
                break
            fport += 1
        next_fport = fport + 1
        for mode in spec.get("modes", ["desktop"]):
            if modes and mode not in modes:
                continue
            log(f"[native] {spec['name']} [{mode}] …")
            res = run_walk(path, spec, mode, binary, port, fport, log=log)
            ok = sum(1 for c in res["checks"] if c[1])
            log(f"[native] {spec['name']} [{mode}]: {ok}/{len(res['checks'])} pass"
                + (f" — BLOCKED {res['error']}" if res["error"] else ""))
            results.append(res)
    return results, specs


def write_json(results: list, path: pathlib.Path):
    path.write_text(json.dumps([{**r, "checks": [list(c) for c in r["checks"]]} for r in results],
                               indent=1) + "\n")


def load_json(path: pathlib.Path) -> list:
    data = json.loads(path.read_text())
    for r in data:
        r["checks"] = [tuple(c) for c in r["checks"]]
    return data


if __name__ == "__main__":
    # Standalone: list the walks the convention picks up.
    for p, s in discover():
        rows = sorted(row_mapping(s))
        print(f"{s['name']:14} {str(p.relative_to(ROOT)):40} modes={s.get('modes')} rows={rows}")
    for p in skipped_scripts():
        print(f"(not a native walk: {p.relative_to(ROOT)} has no WALK literal)")
