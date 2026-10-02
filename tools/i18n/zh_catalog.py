#!/usr/bin/env python3
"""A24 - generate the native Chinese catalog FROM the web's own catalogs.

The web keys its UI text by the ENGLISH SOURCE STRING (`t("Settings")`,
`features/preferences/ui-text.tsx:17-37`). Its loaded Chinese catalog is
(`loadChineseCatalog`, ui-text.tsx:42-67):

    zh.ts                 = { ...catalog, ...FLEET_ZH_COPY }      (zh.ts, last lines)
    loaded                = { ...zh, ...REASONING_ZH_COPY, ...SESSION_CONFIG_ZH_COPY }

so later tables win a collision. This script parses those four object literals
from verbatim copies of the web files (vendored under tools/i18n/web/, provenance
in tools/i18n/web/SOURCE.json) and writes crates/octoscode-module/src/i18n/zh.rs:
one Rust table per web table, in the web's merge order. The native `tr()` merges
them lazily on the first Chinese lookup, the way the web imports them lazily.

The parser is a small JS tokenizer (identifier or quoted keys, "..." / '...'
values with every JS escape, comments, trailing commas, spreads). A duplicate
key inside one literal keeps its FIRST position and its LAST value, exactly as
a JS object literal does.

Usage:
  python3 tools/i18n/zh_catalog.py                 regenerate zh.rs from the vendored copies
  python3 tools/i18n/zh_catalog.py --check         exit 1 if zh.rs is stale; when `node` is on
                                                   PATH, also prove the parse equals the web
                                                   modules' own evaluation (node imports the .ts)
  python3 tools/i18n/zh_catalog.py --refresh DIR   re-vendor from a web checkout's apps/web/src
                                                   (then regenerate)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WEB = os.path.join(ROOT, "tools", "i18n", "web")
OUT = os.path.join(ROOT, "crates", "octoscode-module", "src", "i18n", "zh.rs")

# (rust table name, vendored file, the JS binding whose object literal is the table, web doc)
TABLES = [
    ("ZH_BASE", "features/preferences/zh.ts", "catalog",
     "`features/preferences/zh.ts` `catalog` (its own literal; zh.ts then spreads the Fleet table over it)"),
    ("FLEET_ZH", "features/fleet/fleet-copy.ts", "FLEET_ZH_COPY",
     "`features/fleet/fleet-copy.ts` `FLEET_ZH_COPY` (spread LAST in zh.ts: Fleet wins a collision)"),
    ("REASONING_ZH", "features/reasoning/reasoning-copy.ts", "REASONING_ZH_COPY",
     "`features/reasoning/reasoning-copy.ts` `REASONING_ZH_COPY` (merged by ui-text.tsx's loader)"),
    ("SESSION_CONFIG_ZH", "features/session-config/session-config-copy.ts", "SESSION_CONFIG_ZH_COPY",
     "`features/session-config/session-config-copy.ts` `SESSION_CONFIG_ZH_COPY` (merged last by the loader)"),
]

# A30 - web tables the loader does NOT merge, emitted as their own statics
# (never in TABLES / MERGED_LEN): the native `tr()` consults each one AFTER
# the merged catalog (`i18n::zh_for`). `peer-copy.ts` declares itself merged
# "via { ...catalog, ...FLEET_ZH_COPY, ...PEER_ZH_COPY }", but zh.ts never
# spreads it (zh.ts carries one of its entries by hand), so the web's own
# dock copy ("Hide peers", "Approve once", "asks to run", ...) would stay
# English in Chinese without it.
UNMERGED = [
    ("PEER_ZH", "features/peers/peer-copy.ts", "PEER_ZH_COPY",
     "`features/peers/peer-copy.ts` `PEER_ZH_COPY` (the peers' own table; the web's loader does not merge it - "
     "`super::zh_for` reads it after the merged catalog)"),
]


class ParseError(Exception):
    pass


class Lexer:
    def __init__(self, text, path):
        self.s = text
        self.i = 0
        self.path = path

    def fail(self, msg):
        line = self.s.count("\n", 0, self.i) + 1
        raise ParseError(f"{self.path}:{line}: {msg}")

    def skip(self):
        s = self.s
        while self.i < len(s):
            c = s[self.i]
            if c in " \t\r\n":
                self.i += 1
            elif s.startswith("//", self.i):
                j = s.find("\n", self.i)
                self.i = len(s) if j < 0 else j + 1
            elif s.startswith("/*", self.i):
                j = s.find("*/", self.i + 2)
                if j < 0:
                    self.fail("unterminated block comment")
                self.i = j + 2
            else:
                break

    def peek(self):
        self.skip()
        return self.s[self.i] if self.i < len(self.s) else ""

    def expect(self, c):
        if self.peek() != c:
            self.fail(f"expected {c!r}, found {self.s[self.i:self.i + 20]!r}")
        self.i += 1

    def ident(self):
        self.skip()
        j = self.i
        s = self.s
        while j < len(s) and (s[j].isalnum() or s[j] in "_$"):
            j += 1
        if j == self.i:
            self.fail(f"expected an identifier, found {s[self.i:self.i + 20]!r}")
        out = s[self.i:j]
        self.i = j
        return out

    def string(self):
        self.skip()
        q = self.s[self.i]
        if q not in "\"'":
            self.fail(f"expected a string literal, found {self.s[self.i:self.i + 20]!r}")
        self.i += 1
        out = []
        s = self.s
        while True:
            if self.i >= len(s):
                self.fail("unterminated string")
            c = s[self.i]
            if c == q:
                self.i += 1
                return "".join(out)
            if c == "\n":
                self.fail("newline in a string literal")
            if c != "\\":
                out.append(c)
                self.i += 1
                continue
            e = s[self.i + 1]
            self.i += 2
            simple = {"n": "\n", "t": "\t", "r": "\r", "b": "\b", "f": "\f", "v": "\v", "0": "\0",
                      "\\": "\\", "'": "'", '"': '"'}
            if e in simple:
                out.append(simple[e])
            elif e == "\n":
                pass  # line continuation
            elif e == "x":
                out.append(chr(int(s[self.i:self.i + 2], 16)))
                self.i += 2
            elif e == "u":
                if s[self.i] == "{":
                    j = s.index("}", self.i)
                    out.append(chr(int(s[self.i + 1:j], 16)))
                    self.i = j + 1
                else:
                    cp = int(s[self.i:self.i + 4], 16)
                    self.i += 4
                    # A UTF-16 surrogate pair spelled as two escapes.
                    if 0xD800 <= cp < 0xDC00 and s.startswith("\\u", self.i):
                        lo = int(s[self.i + 2:self.i + 6], 16)
                        if 0xDC00 <= lo < 0xE000:
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                            self.i += 6
                    out.append(chr(cp))
            else:
                out.append(e)  # JS: an unknown escape is the character itself

    def value(self):
        """A string literal, or several joined with `+`."""
        parts = [self.string()]
        while self.peek() == "+":
            self.i += 1
            parts.append(self.string())
        return "".join(parts)


def parse_object_after(text, path, binding):
    """The object literal assigned to `binding` (`const <binding>... = {` or
    `= Object.freeze({`): an ordered list of (key, value) with JS semantics."""
    import re
    m = re.search(r"\bconst\s+" + re.escape(binding) + r"\b[^=]*=\s*(Object\.freeze\(\s*)?\{", text)
    if not m:
        raise ParseError(f"{path}: no `const {binding} = {{...}}` literal")
    lx = Lexer(text, path)
    lx.i = m.end()  # just after the opening brace
    order = []
    values = {}
    while True:
        c = lx.peek()
        if c == "}":
            lx.i += 1
            break
        if lx.s.startswith("...", lx.i):
            lx.fail("a spread inside a catalog literal is not supported")
        key = lx.string() if c in "\"'" else lx.ident()
        lx.expect(":")
        val = lx.value()
        if key not in values:
            order.append(key)
        values[key] = val  # last value wins, first position stays (JS)
        if lx.peek() == ",":
            lx.i += 1
    return [(k, values[k]) for k in order]


def load_tables(web_root, specs=None):
    out = []
    for name, rel, binding, doc in (TABLES if specs is None else specs):
        path = os.path.join(web_root, rel)
        with open(path, encoding="utf-8") as f:
            text = f.read()
        out.append((name, rel, doc, parse_object_after(text, rel, binding)))
    if specs is not None:
        return out
    # zh.ts must still spread the Fleet table LAST (the merge order this
    # module assumes); fail loudly if the web changed it.
    with open(os.path.join(web_root, "features/preferences/zh.ts"), encoding="utf-8") as f:
        zh_src = f.read()
    if "{ ...catalog, ...FLEET_ZH_COPY }" not in zh_src:
        raise ParseError("zh.ts no longer ends with `{ ...catalog, ...FLEET_ZH_COPY }`: re-check the merge order")
    return out


def merged(tables):
    order, values = [], {}
    for _, _, _, rows in tables:
        for k, v in rows:
            if k not in values:
                order.append(k)
            values[k] = v
    return [(k, values[k]) for k in order]


def rust_lit(s):
    out = ['"']
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        elif ord(ch) < 0x20 or ord(ch) == 0x7F:
            out.append("\\u{%x}" % ord(ch))
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def source_info():
    with open(os.path.join(WEB, "SOURCE.json"), encoding="utf-8") as f:
        return json.load(f)


def render(tables, unmerged=()):
    info = source_info()
    lines = [
        "//! GENERATED by `tools/i18n/zh_catalog.py` from the web's own Chinese",
        "//! catalogs - do not edit by hand; re-run the script (`--check` in CI).",
        "//!",
        f"//! Web source: `{info['repo']}` @ `{info['commit']}`, `{info['root']}/` (verbatim",
        "//! copies in `tools/i18n/web/`). English is the key (the web's `t()`",
        "//! convention); the value is the web's Simplified Chinese text.",
        "//!",
        "//! The tables are listed in the web's merge order (`ui-text.tsx:42-67`",
        "//! `loadChineseCatalog`: zh.ts = `{ ...catalog, ...FLEET_ZH_COPY }`, then",
        "//! `{ ...zh, ...REASONING_ZH_COPY, ...SESSION_CONFIG_ZH_COPY }`): a later",
        "//! table wins a collision. `super::catalog()` merges them lazily.",
        "",
        f"/// The web commit the tables were generated from.",
        f"pub const WEB_COMMIT: &str = {rust_lit(info['commit'])};",
        "",
    ]
    for name, rel, doc, rows in tables:
        lines.append(f"/// {doc}: {len(rows)} entries.")
        lines.append(f"pub static {name}: &[(&str, &str)] = &[")
        for k, v in rows:
            lines.append(f"    ({rust_lit(k)}, {rust_lit(v)}),")
        lines.append("];")
        lines.append("")
    names = ", ".join(t[0] for t in tables)
    lines.append("/// Every table, in the web's merge order (later wins).")
    lines.append(f"pub static TABLES: &[&[(&str, &str)]] = &[{names}];")
    lines.append("")
    lines.append(f"/// The number of distinct keys after the merge (the web's loaded catalog).")
    lines.append(f"pub const MERGED_LEN: usize = {len(merged(tables))};")
    lines.append("")
    for name, rel, doc, rows in unmerged:
        lines.append(f"/// {doc}: {len(rows)} entries.")
        lines.append(f"pub static {name}: &[(&str, &str)] = &[")
        for k, v in rows:
            lines.append(f"    ({rust_lit(k)}, {rust_lit(v)}),")
        lines.append("];")
        lines.append("")
    return "\n".join(lines)


NODE_ORACLE = r"""
const base = process.argv[2];
const zh = (await import(base + '/features/preferences/zh.ts')).default;
const { FLEET_ZH_COPY } = await import(base + '/features/fleet/fleet-copy.ts');
const { REASONING_ZH_COPY } = await import(base + '/features/reasoning/reasoning-copy.ts');
const { SESSION_CONFIG_ZH_COPY } = await import(base + '/features/session-config/session-config-copy.ts');
const { PEER_ZH_COPY } = await import(base + '/features/peers/peer-copy.ts');
const loaded = { ...zh, ...REASONING_ZH_COPY, ...SESSION_CONFIG_ZH_COPY };
process.stdout.write(JSON.stringify({ loaded: Object.entries(loaded), fleet: Object.entries(FLEET_ZH_COPY), peer: Object.entries(PEER_ZH_COPY) }));
"""


def node_check(tables, unmerged=()):
    node = shutil.which("node")
    if not node:
        print("zh_catalog: node not on PATH - skipped the web-evaluation cross-check")
        return True
    with tempfile.TemporaryDirectory() as tmp:
        oracle = os.path.join(tmp, "oracle.mjs")
        with open(oracle, "w", encoding="utf-8") as f:
            f.write(NODE_ORACLE)
        res = subprocess.run([node, oracle, "file://" + WEB], capture_output=True, text=True)
    if res.returncode != 0:
        print("zh_catalog: node could not evaluate the web catalogs:\n" + res.stderr[-2000:])
        return False
    got = json.loads(res.stdout)
    ours = merged(tables)
    if [list(p) for p in ours] != got["loaded"]:
        want = dict(map(tuple, got["loaded"]))
        have = dict(ours)
        diff = [k for k in set(want) | set(have) if want.get(k) != have.get(k)][:10]
        print(f"zh_catalog: the parse differs from node's evaluation of the web modules: {diff}")
        return False
    fleet = [t for t in tables if t[0] == "FLEET_ZH"][0][3]
    if [list(p) for p in fleet] != got["fleet"]:
        print("zh_catalog: FLEET_ZH differs from node's FLEET_ZH_COPY")
        return False
    peer = [t for t in unmerged if t[0] == "PEER_ZH"]
    if peer and [list(p) for p in peer[0][3]] != got["peer"]:
        print("zh_catalog: PEER_ZH differs from node's PEER_ZH_COPY")
        return False
    print(f"zh_catalog: node evaluation of the web modules matches the parse ({len(ours)} merged keys)")
    return True


def main(argv):
    if len(argv) >= 2 and argv[0] == "--refresh":
        src = argv[1]
        for _, rel, _, _ in TABLES + UNMERGED:
            os.makedirs(os.path.dirname(os.path.join(WEB, rel)), exist_ok=True)
            shutil.copyfile(os.path.join(src, rel), os.path.join(WEB, rel))
        print(f"zh_catalog: re-vendored {len(TABLES) + len(UNMERGED)} files from {src}; update SOURCE.json's commit")
        argv = []
    tables = load_tables(WEB)
    unmerged = load_tables(WEB, UNMERGED)
    text = render(tables, unmerged)
    if argv and argv[0] == "--check":
        ok = True
        try:
            with open(OUT, encoding="utf-8") as f:
                current = f.read()
        except FileNotFoundError:
            current = ""
        if current != text:
            print(f"zh_catalog: {os.path.relpath(OUT, ROOT)} is stale - run python3 tools/i18n/zh_catalog.py")
            ok = False
        ok = node_check(tables, unmerged) and ok
        return 0 if ok else 1
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(text)
    counts = ", ".join(f"{t[0]} {len(t[3])}" for t in tables + unmerged)
    print(f"zh_catalog: wrote {os.path.relpath(OUT, ROOT)} ({counts}; merged {len(merged(tables))})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
