#!/usr/bin/env python3
"""Regenerate the three matrix pairs (#31b).

Extended from the phase-0 heuristic generator: the native leg now scans THIS
repo's `crates/**` (the octoscode port), not the stale vendored AppCard base,
and the contract source is the octos rev THIS repo pins (`octos-core = rev
a6ea8505…` in the workspace Cargo.toml — resolved via $OCTOS_CORE_SRC or the
shared cargo checkout; no machine paths committed).

Native match = the phase-0 intent, PLUS the `methods::CONST` form (the blind
spot the supervisor's wire-literal grep missed: the client handlers match on
typed variants + `methods::*` constants, `f31b_notifications.rs` pins them).

Outputs (regenerated in place):
  docs/protocol-matrix.{csv,md}   — committed 11-column schema (DictReader-safe)
  docs/parity-matrix.{csv,md}     — docs/parity/*.csv with conservative
                                    native refresh (missing -> partial ONLY,
                                    with an explicit note; never downgraded)
  docs/walk-rows.{csv,md}         — natively_handled re-derived from the
                                    refreshed protocol matrix (the original
                                    derivation, commit 70d1cd5)
"""
import csv
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
W = os.environ.get("SRC_WEB") or os.path.expanduser("~/home/oa.noindex/src-web")
NATIVE_ROOT = os.path.join(ROOT, "crates")


def pinned_core():
    """The ui_protocol.rs of the octos rev THIS repo pins (workspace
    Cargo.toml), located through the shared cargo checkout — no
    machine-specific absolute paths committed (the #31b hygiene gate)."""
    env = os.environ.get("OCTOS_CORE_SRC")
    if env:
        return env
    rev = ""
    with open(os.path.join(ROOT, "Cargo.toml"), errors="ignore") as f:
        for line in f:
            m = re.search(r'octos-core.*rev = "([0-9a-f]{40})"', line)
            if m:
                rev = m.group(1)
                break
    home = os.environ.get("CARGO_HOME") or os.path.expanduser("~/.cargo")
    if rev:
        hits = glob.glob(os.path.join(
            home, "git", "checkouts", "octos-*", rev[:7] + "*",
            "crates", "octos-core", "src", "ui_protocol.rs"))
        if hits:
            return hits[0]
    hits = glob.glob(os.path.join(
        home, "git", "checkouts", "octos-*", "*",
        "crates", "octos-core", "src", "ui_protocol.rs"))
    if not hits:
        sys.exit("cannot locate the pinned octos-core checkout; set OCTOS_CORE_SRC")
    return max(hits, key=os.path.getmtime)


PINNED = pinned_core()
DOCS = os.path.join(ROOT, "docs")


def files(root, exts, skip=("node_modules", "target", "generated", "fixtures")):
    for d, ds, fs in os.walk(root):
        ds[:] = [x for x in ds if x not in skip and not x.startswith(".")]
        for f in fs:
            if f.endswith(exts):
                yield os.path.join(d, f)


def block(src, name):
    # The contract mixes shapes: CORE_UI_METHODS is an object `{…} as const`,
    # the SERVER/NOTIFICATION sets are arrays `[…] as const` (the phase-0
    # script matched both with [\[{] … [\]}] — the curly-only first cut
    # emptied the kind sets and degraded every row's `kind` to "?").
    m = re.search(r"export const " + name + r"\b[^=]*=\s*[\[{](.*?)[\]}]\s*as const", src, re.S)
    return m.group(1) if m else ""


def contract():
    src = open(PINNED, errors="ignore").read()
    consts = dict(re.findall(r"([A-Z_]+):\s*\"([^\"]+)\"", block(
        open(f"{W}/packages/client/src/generated/core-contract.ts").read(), "CORE_UI_METHODS")))
    notif = set(re.findall(r"\"([^\"]+)\"", block(
        open(f"{W}/packages/client/src/generated/core-contract.ts").read(), "CORE_UI_NOTIFICATION_METHODS")))
    server = set(re.findall(r"\"([^\"]+)\"", block(
        open(f"{W}/packages/client/src/generated/core-contract.ts").read(), "CORE_UI_SERVER_METHODS")))
    return consts, server, notif, src


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def regenerate_protocol(consts, server, notif, core_src):
    web = [(p, open(p, errors="ignore").read()) for p in files(f"{W}/apps", (".ts", ".tsx"))]
    web += [(p, open(p, errors="ignore").read()) for p in files(f"{W}/packages", (".ts", ".tsx"))]
    native = [(p, open(p, errors="ignore").read()) for p in files(NATIVE_ROOT, (".rs",))]

    # contract_src: the `pub CONST: "method"` line in the pinned core.
    core_lines = core_src.splitlines()
    const_line = {}
    for i, l in enumerate(core_lines):
        m = re.match(r"\s*pub const ([A-Z_]+): &str = \"([^\"]+)\"", l)
        if m:
            const_line[m.group(2)] = i + 1

    rows = []
    for c, m in sorted(consts.items(), key=lambda x: x[1]):
        pat_w = re.compile(r"\b" + c + r"\b|[\"']" + re.escape(m) + r"[\"']")
        wsrc = [p for p, t in web if pat_w.search(t) and not re.search(r"\.(test|spec)\.", p)]
        wtest = [p for p, t in web if pat_w.search(t) and re.search(r"\.(test|spec)\.", p)]
        ev = "".join(w.capitalize() for w in re.split(r"[/_.]", m)) + "Event"
        pat_n = re.compile(r"[\"']" + re.escape(m) + r"[\"']|methods::" + c + r"\b|\b" + ev + r"\b")
        all_np = [p for p, t in native if pat_n.search(t)]
        # Production path only (RULES: a function only a test calls counts as
        # missing): `handled` requires a match under a crate's src/, never a
        # tests/ or examples/ file. Secondary evidence stays in the note.
        np_ = [p for p in all_np if "/src/" in p]
        tests = [p for p in all_np if p not in np_]
        kind = "notification" if m in notif else ("request" if m in server else "?")
        # types: <Camel>Params / <Camel>Result in the pinned core.
        camel = "".join(w.capitalize() for w in re.split(r"[/_.]", m))
        ptype = camel + "Params" if re.search(r"pub struct " + camel + "Params\b", core_src) else ""
        rtype = camel + "Result" if re.search(r"pub struct " + camel + "Result\b", core_src) else ""
        if kind == "notification":
            ptype = camel + "Event" if re.search(r"pub struct " + camel + "Event\b", core_src) else ""
            rtype = ""
        # native_status: handled = a PRODUCTION consumer (a domain handler /
        # store fold / screen binding under a crate's src/); decoded-only =
        # only the transport/enum layer names it; absent = nothing (or only
        # tests/examples — recorded as the note).
        decode_only = any("octos-core" in os.path.relpath(p, ROOT) for p in np_)
        has_handler = any(
            ("octoscode-client" in p or "octoscode-module" in p or "octoscode-store" in p)
            for p in np_)
        status = "handled" if has_handler else ("decoded-only" if np_ else "absent")
        where = ";".join(sorted({os.path.relpath(p, NATIVE_ROOT) for p in np_})[:3])
        note = ("decoder pinned octos a6ea8505" if decode_only and status == "decoded-only" else "")
        if not np_ and tests:
            note = (note + "; " if note else "") + f"test-only matches: {len(tests)}"
        rows.append(dict(
            method=m, kind=kind, params_type=ptype, result_type=rtype, feature_flag="",
            contract_src=f"octos-core/src/ui_protocol.rs:{const_line.get(m, 0)}",
            web_call_sites=";".join(sorted(os.path.relpath(p, W) for p in wsrc)[:3]),
            web_tested=str(len(wtest)),
            native_status=status, native_src=where, notes=note,
        ))
    return rows


def statuses(path):
    with open(path, newline="") as f:
        return {r["method"]: r["native_status"] for r in csv.DictReader(f)}


def regenerate_parity(proto_status):
    """Refresh docs/parity/*.csv rows that NAME protocol methods, then rebuild
    the combined pair. Conservative: missing -> partial ONLY, with a note;
    human-curated rows without protocol_methods stay verbatim."""
    header = None
    combined = []
    upgraded = 0
    for g in sorted(os.listdir(os.path.join(DOCS, "parity"))):
        if not g.endswith(".csv"):
            continue
        with open(os.path.join(DOCS, "parity", g), newline="") as f:
            rows = list(csv.DictReader(f))
            header = header or list(rows[0].keys())
        for r in rows:
            names = [x.strip() for x in (r.get("protocol_methods") or "").split(";") if x.strip()]
            if names and r.get("native_status") == "missing":
                handled = [m for m in names if proto_status.get(m) == "handled"]
                if handled:
                    r["native_status"] = "partial"
                    add = f"octoscode #31b: protocol paths now handled ({';'.join(handled[:4])})"
                    r["notes"] = (r.get("notes") + "; " if r.get("notes") else "") + add
                    upgraded += 1
            combined.append(r)
    return header, combined, upgraded


def regenerate_walk(proto_status):
    path = os.path.join(DOCS, "walk-rows.csv")
    with open(path, newline="") as f:
        reader = csv.DictReader(f)
        header = reader.fieldnames
        rows = list(reader)
    for r in rows:
        names = [x.strip() for x in (r.get("protocol_methods") or "").split(";") if x.strip()]
        if not names:
            continue
        st = {proto_status.get(m, "absent") for m in names}
        r["natively_handled"] = "all" if st <= {"handled"} else ("none" if st == {"absent"} else "some")
    return header, rows


def write_csv(path, header, rows):
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=header)
        w.writeheader()
        w.writerows(rows)


def main():
    consts, server, notif, core_src = contract()
    rows = regenerate_protocol(consts, server, notif, core_src)
    write_csv(os.path.join(DOCS, "protocol-matrix.csv"),
              ["method", "kind", "params_type", "result_type", "feature_flag", "contract_src",
               "web_call_sites", "web_tested", "native_status", "native_src", "notes"], rows)

    used = [r for r in rows if int(r["web_tested"]) > 0 or r["web_call_sites"]]
    print("contract methods", len(rows), "| requests", sum(r["kind"] == "request" for r in rows),
          "| notifications", sum(r["kind"] == "notification" for r in rows))
    print("web-used", len(used), "| handled", sum(r["native_status"] == "handled" for r in used),
          "| decoded-only", sum(r["native_status"] == "decoded-only" for r in used),
          "| absent", sum(r["native_status"] == "absent" for r in used))

    proto_status = {r["method"]: r["native_status"] for r in rows}
    ph, prow, upgraded = regenerate_parity(proto_status)
    write_csv(os.path.join(DOCS, "parity-matrix.csv"), ph, prow)
    wh, wrow = regenerate_walk(proto_status)
    write_csv(os.path.join(DOCS, "walk-rows.csv"), wh, wrow)
    print("parity rows", len(prow), "| missing->partial upgraded", upgraded,
          "| exists", sum(r["native_status"] == "exists" for r in prow),
          "partial", sum(r["native_status"] == "partial" for r in prow),
          "missing", sum(r["native_status"] == "missing" for r in prow))
    print("walk rows", len(wrow), "| all", sum(r["natively_handled"] == "all" for r in wrow),
          "some", sum(r["natively_handled"] == "some" for r in wrow),
          "none", sum(r["natively_handled"] == "none" for r in wrow))

    six = ["message/reasoning_delta", "approval/auto_resolved", "approval/cancelled",
           "context/compaction_started", "context/compaction_completed", "monitor/expired"]
    for m in six:
        r = next(r for r in rows if r["method"] == m)
        print("31b", m, "->", r["native_status"], "|", r["native_src"][:70])


if __name__ == "__main__":
    main()
