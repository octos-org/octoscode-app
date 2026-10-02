#!/usr/bin/env python3
"""Unit tests for the native click-walk aggregator (tools/walk/native.py) and
run.py's merge of its verdicts (A11). No app, no server:

  python3 tools/walk/test_native.py
"""
from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native  # noqa: E402
import run  # noqa: E402


class SpecDiscovery(unittest.TestCase):
    def test_the_walk_literal_is_read_without_importing_the_script(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "x_walk.py"
            # Importing this would raise: the reader must not execute it.
            p.write_text('raise SystemExit("imported!")\nWALK = {"name": "x", "rows": {1: ["a"]}}\n')
            self.assertEqual(native.read_spec(p), {"name": "x", "rows": {1: ["a"]}})
            q = pathlib.Path(d) / "y_walk.py"
            q.write_text("print('no spec')\n")
            self.assertIsNone(native.read_spec(q))

    def test_every_committed_walk_with_a_spec_maps_real_rows(self):
        rows = run.load_rows()
        found = native.discover()
        self.assertGreaterEqual(len(found), 9)
        for _, spec in found:
            for rid in native.row_mapping(spec):
                self.assertTrue(1 <= rid <= len(rows), f"{spec['name']}: row {rid}")


class Parsing(unittest.TestCase):
    def test_pass_fail_lines(self):
        text = "noise\nPASS a check — detail\nFAIL b — why it failed\n  PASS indented one\nPASSAGE no\n"
        got = native.parse_checks(text)
        self.assertEqual([(n, ok) for n, ok, _, _ in got],
                         [("a check", True), ("b", False), ("indented one", True)])
        self.assertEqual(got[1][2], "why it failed")
        self.assertEqual(got[0][3], 2)

    def test_placeholders(self):
        ctx = {"port": 8421, "mode": "phone"}
        self.assertEqual(native.expand(["{port}", {"k": "x{mode}"}], ctx), ["8421", {"k": "xphone"}])


def result(name, mode, checks, error=None):
    return {"name": name, "mode": mode, "checks": [(n, ok, "", f"log:{i}") for i, (n, ok) in enumerate(checks)],
            "error": error, "log": f"docs/walk/evidence/native/{name}-{mode}.log"}


class Verdicts(unittest.TestCase):
    specs = {
        "w": {"name": "w", "rows": {1: ["alpha"], 2: {"checks": ["beta"], "partial": "the gamma half"},
                                    3: {"checks": {"phone": ["drawer"]}}}},
        "v": {"name": "v", "rows": {2: ["delta"]}},
    }

    def test_a_row_passes_only_when_every_mapped_check_passes_in_every_mode(self):
        res = [result("w", "desktop", [("alpha one", True), ("beta", True)]),
               result("w", "phone", [("alpha one", True), ("beta", True), ("drawer opens", True)])]
        v = native.row_verdicts(res, {"w": self.specs["w"]})
        self.assertEqual(v[1]["status"], "pass")
        self.assertEqual(v[1]["depth"], "native")
        self.assertEqual(len(v[1]["checks"]), 2)
        self.assertEqual(v[2]["depth"], "native-partial")
        self.assertIn("the gamma half", v[2]["reason"])
        self.assertEqual(len(v[3]["checks"]), 1, "a phone-only mapping ignores the desktop run")
        res[1]["checks"][0] = ("alpha one", False, "broke", "log:0")
        self.assertEqual(native.row_verdicts(res, {"w": self.specs["w"]})[1]["status"], "fail")

    def test_a_mode_where_no_mapped_check_ran_fails(self):
        res = [result("w", "desktop", [("alpha", True)]), result("w", "phone", [("something else", True)])]
        v = native.row_verdicts(res, {"w": self.specs["w"]})
        self.assertEqual(v[1]["status"], "fail")
        self.assertIn("no mapped check ran", " ".join(c[2] for c in v[1]["checks"]))

    def test_a_walk_that_could_not_start_fails_its_rows(self):
        v = native.row_verdicts([result("w", "desktop", [], error="fixture never listened")], {"w": self.specs["w"]})
        self.assertEqual(v[1]["status"], "fail")

    def test_one_full_mapping_makes_the_row_native(self):
        res = [result("w", "desktop", [("beta", True)]), result("v", "desktop", [("delta", True)])]
        v = native.row_verdicts(res, self.specs)
        self.assertEqual(v[2]["depth"], "native", "v covers row 2 fully")
        self.assertEqual(v[2]["status"], "pass")


class Merge(unittest.TestCase):
    def test_native_rows_replace_the_phase3_checks_and_keep_the_rest(self):
        out_rows = [{"row_id": 1, "area": "a", "spec": "s", "case": "c1", "status": "pass", "depth": "smoke",
                     "evidence": "", "reason": "generic"},
                    {"row_id": 2, "area": "a", "spec": "s", "case": "c2", "status": "pass", "depth": "smoke",
                     "evidence": "", "reason": "generic"}]
        checks = [{"row_id": 1, "check": "generic one", "status": "pass"},
                  {"row_id": 2, "check": "generic two", "status": "pass"}]
        native_rows = {1: {"status": "fail", "depth": "native", "evidence": "e.log", "reason": "r",
                           "checks": [("w: x [desktop]", False, "why", "e.log:3")]}}
        rows, kept = run.merge_native(out_rows, checks, native_rows)
        self.assertEqual(rows[0]["status"], "fail")
        self.assertEqual(rows[0]["depth"], "native")
        self.assertEqual([c["check"] for c in kept], ["w: x [desktop]", "generic two"])
        self.assertEqual(kept[0]["evidence"], "e.log:3")


class ParityReasons(unittest.TestCase):
    parity = [
        {"capability": "built thing", "web_e2e_specs": 'src-web/e2e/demo.spec.ts:1 "built case here"',
         "phase4_bucket": "C", "phase4_bucket_manual": "A"},
        {"capability": "missing thing", "web_e2e_specs": 'src-web/e2e/demo.spec.ts:9 "missing case here"',
         "phase4_bucket": "C", "phase4_bucket_manual": ""},
    ]

    def test_built_but_unwalked_is_not_walked_and_missing_is_named(self):
        built = {"spec": "e2e/demo.spec.ts", "case": "built case here"}
        missing = {"spec": "e2e/demo.spec.ts", "case": "missing case here"}
        self.assertEqual(run.parity_reason(built, self.parity)[0], "not-walked")
        st, reason = run.parity_reason(missing, self.parity)
        self.assertEqual(st, "not-yet-implemented")
        self.assertIn("missing thing [C]", reason)
        self.assertIsNone(run.parity_reason({"spec": "e2e/other.spec.ts", "case": "x"}, self.parity))


if __name__ == "__main__":
    unittest.main()
