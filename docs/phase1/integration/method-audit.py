#!/usr/bin/env python3
"""Card #13 audit: which web-used core methods does our client not reference?

The card's question: the fan-out relied on `docs/protocol-matrix.csv`'s
`native_status=handled`, but that meant handled by **appcard's** code, not our
`octoscode-client`. This script takes the methods the WEB uses (a non-zero
`web_call_sites` in the matrix) and checks whether our client references each
one anywhere in `crates/octoscode-client/src` (as a `methods::CONST` or the
literal string).

Usage:  python3 docs/phase1/integration/method-audit.py [repo-root]
Output: docs/phase1/integration/method-audit.csv  (header: method,kind,referenced,where)
Exit 0 when every web-used method is referenced; 1 otherwise (so the outer
loop can gate on it).
"""
import csv
import os
import re
import sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
MATRIX = os.path.join(ROOT, "docs", "protocol-matrix.csv")
EXT = os.path.join(ROOT, "docs", "protocol-ext-matrix.csv")
CLIENT = os.path.join(ROOT, "crates", "octoscode-client", "src")
MODULE = os.path.join(ROOT, "crates", "octoscode-module", "src")
OUT = os.path.join(ROOT, "docs", "phase1", "integration", "method-audit.csv")

# Every method the client references, as the literal string it appears with.
def client_references():
    refs = set()
    for dirpath, _dirs, files in os.walk(CLIENT):
        for f in files:
            if not f.endswith(".rs"):
                continue
            text = open(os.path.join(dirpath, f), errors="ignore").read()
            # `methods::FOO` — resolve the const's VALUE from octos-core is not
            # needed: we match the wire string directly below.
            for m in re.findall(r'"([a-z][a-z0-9_]*(?:/[a-z0-9_.]+)+)"', text):
                refs.add(m)
    return refs


def const_map():
    """method const name -> wire string, scraped from the `pub mod methods` block.

    Scoped to the module on purpose: `ui_protocol.rs` defines some names twice
    (e.g. `TOOL_PROGRESS` is both the method `tool/progress` at `:1126` and a
    v1 payload tag `tool_progress` at `:4881`), so a whole-file scan can resolve
    `methods::TOOL_PROGRESS` to the wrong string. And a method name need not
    contain a slash (`warning`, `snapshot/list`), so a slash heuristic is wrong.
    """
    src = "/Users/yuechen/home/oa.noindex/src-octos/crates/octos-core/src/ui_protocol.rs"
    out = {}
    if not os.path.exists(src):
        return out
    text = open(src, errors="ignore").read()
    start = text.find("pub mod methods {")
    if start < 0:
        return out
    # The module ends at the first line that is exactly "}" at column 0.
    end = text.find("\n}\n", start)
    block = text[start:end if end > 0 else len(text)]
    for name, value in re.findall(r'pub const ([A-Z0-9_]+): &str = "([^"]+)";', block):
        out[name] = value
    return out


# Methods reached only through the transport's TYPED commands (no string in our
# crates): `octos-app-transport/src/proto.rs:151-184` maps each to its method.
TRANSPORT_TYPED = {
    "session/open": "octos-app-transport proto.rs:151 (OpenSession)",
    "turn/start": "octos-app-transport proto.rs:161 (StartTurn)",
    "turn/interrupt": "octos-app-transport proto.rs:164 (InterruptTurn)",
    "approval/respond": "octos-app-transport proto.rs:167 (SendApprovalResponse)",
    "diff/preview/get": "octos-app-transport proto.rs:170 (FetchDiffPreview)",
    "task/output/read": "octos-app-transport proto.rs:173 (RequestTaskOutput)",
    "session/list": "octos-app-transport proto.rs:176 (ListSessions)",
    "session/hydrate": "octos-app-transport proto.rs:184 (HydrateSession)",
}


def used_by_web(path):
    rows = []
    if not os.path.exists(path):
        return rows
    for r in csv.DictReader(open(path)):
        try:
            n = int(r.get("web_call_sites") or 0)
        except ValueError:
            n = 0
        if n > 0:
            rows.append(r)
    return rows


def main():
    consts = const_map()
    refs = client_references()
    # Also resolve `methods::CONST` usages to their wire strings.
    resolved = set(refs)
    for base in (CLIENT, MODULE):
        for dirpath, _dirs, files in os.walk(base):
            for f in files:
                if not f.endswith(".rs"):
                    continue
                text = open(os.path.join(dirpath, f), errors="ignore").read()
                for name in re.findall(r"methods::([A-Z0-9_]+)", text):
                    if name in consts:
                        resolved.add(consts[name])
    # The transport's typed commands are a real production path (see the map).
    resolved |= set(TRANSPORT_TYPED)

    rows = used_by_web(MATRIX) + used_by_web(EXT)
    seen = set()
    out_rows = []
    for r in rows:
        m = r["method"]
        if m in seen:
            continue
        seen.add(m)
        ok = m in resolved
        where = ""
        if ok:
            where = TRANSPORT_TYPED.get(m) or "crates/octoscode-client/src (+ module)"
        out_rows.append(
            {
                "method": m,
                "kind": r.get("kind", ""),
                "referenced": "yes" if ok else "no",
                "where": where,
            }
        )
    out_rows.sort(key=lambda x: (x["referenced"], x["method"]))
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["method", "kind", "referenced", "where"])
        w.writeheader()
        w.writerows(out_rows)
    missing = [r["method"] for r in out_rows if r["referenced"] == "no"]
    print(f"web-used methods: {len(out_rows)}; unreferenced: {len(missing)}")
    for m in missing:
        print(f"  MISSING {m}")
    return 0 if not missing else 1


if __name__ == "__main__":
    sys.exit(main())
