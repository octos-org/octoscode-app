#!/usr/bin/env python3
"""A34 — a partial walk run is safe evidence. Tests for how `tools/walk/run.py`
writes the official table (`docs/walk/results.csv` + `results-checks.csv`),
driven through `run.main()` on a small temp table with a fake app layer (no
process starts, no port opens):

* `--only AREA` MERGES: every row outside the filter keeps its line, byte for
  byte, in both files; a row inside it is replaced only by the checks that ran
  (a native row's walk checks did not run, so they stay).
* an env-group row under the filter is RUN (each of its instances); an instance
  that cannot start leaves its checks `not-run` for that row — never dropped.
* `--native-only --walks W` re-points only the rows W maps; a row another walk
  also maps keeps that walk's checks FROM THE TABLE (not from a local cache).
* provenance: `run_sha` / `run_at` / `run_scope` per row (`run_sha` / `run_at`
  per check), set for what the run re-ran and kept for everything else.
* only a complete `--full` run regenerates every row.

  python3 tools/walk/test_run_merge.py
"""
from __future__ import annotations

import contextlib
import csv
import io
import os
import pathlib
import re
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native  # noqa: E402
import run  # noqa: E402

ROW_FIELDS = ["row_id", "area", "spec", "case", "status", "depth", "evidence", "reason",
              "run_sha", "run_at", "run_scope"]
CHECK_FIELDS = ["row_id", "area", "spec", "case", "check", "status", "evidence", "reason",
                "run_sha", "run_at"]
WALK_FIELDS = ["spec", "case", "web_src", "steps", "pass_condition", "protocol_methods",
               "natively_handled", "needs", "web_only_reason"]
PARITY_FIELDS = ["feature", "group", "capability", "web_e2e_specs", "phase4_bucket",
                 "phase4_bucket_manual"]

OLD = {"run_sha": "lane@0001111", "run_at": "2026-10-01T00:00:00Z",
       "run_scope": "full (results 0002222)"}
NEW_SHA = "lane@9998888"
UTC = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")

CONV = range(1, 21)          # rows 1-20: area `conversation`
PEER = range(21, 41)         # rows 21-40: area `peer`
NATIVE_CONV = 3              # a native row (walk w1) outside the peer filter
DEMOTABLE = 6                # a smoke pass whose capability the parity now calls C
RELABELABLE = 7              # a not-yet-implemented row the parity now calls built
PARTIAL = 22                 # native-partial (w1 + w2) with run.py's targeted check
NATIVE_PEER = 23             # native (w2) inside the peer filter
NEWLY_MAPPED = 24            # w2's spec maps it; the table has no w2 check for it yet
TEMPLATE = 40                # the env-group row: one instance per viewport width

W1 = "docs/walk/evidence/native/w1-desktop.log"
W2 = "docs/walk/evidence/native/w2-desktop.log"
AREA_PEER = "docs/walk/evidence/area-peer.snap.json"
AREA_1280 = "docs/walk/evidence/area-peer-viewport-1280.snap.json"

SPECS = {
    "w1": {"name": "w1", "modes": ["desktop"],
           "rows": {NATIVE_CONV: ["alpha"], PARTIAL: {"checks": ["beta"], "partial": "half"}}},
    "w2": {"name": "w2", "modes": ["desktop"],
           "rows": {PARTIAL: {"checks": ["gamma"], "partial": "half"}, NATIVE_PEER: ["delta"],
                    NEWLY_MAPPED: ["epsilon"]}},
}


def case_of(i: int) -> str:
    if i in CONV:
        return f"streams an answer number {i:02d}"
    if i == TEMPLATE:
        return "fleet keeps its geometry template at the viewport width"
    return f"fleet roster lane number {i:02d}"


def spec_of(i: int) -> str:
    return "e2e/conv.spec.ts" if i in CONV else "e2e/fleet.spec.ts"


def area_of(i: int) -> str:
    return "conversation" if i in CONV else "peer"


def old_row(i: int, **kw) -> dict:
    r = {"row_id": i, "area": area_of(i), "spec": spec_of(i), "case": case_of(i),
         "status": "pass", "depth": "smoke", "evidence": AREA_PEER if i in PEER else "",
         "reason": "1 checks, all pass", **OLD}
    r.update(kw)
    return r


def old_check(i: int, name: str, status="pass", evidence="", reason="ok") -> dict:
    return {"row_id": i, "area": area_of(i), "spec": spec_of(i), "case": case_of(i),
            "check": name, "status": status, "evidence": evidence, "reason": reason,
            "run_sha": OLD["run_sha"], "run_at": OLD["run_at"]}


def old_table():
    rows, checks = [], []
    for i in list(CONV) + list(PEER):
        if i == NATIVE_CONV:
            rows.append(old_row(i, depth="native", evidence=W1,
                                reason="1 native click-walk checks (w1:desktop), all pass"))
            checks.append(old_check(i, "w1: alpha [desktop]", evidence=W1 + ":3", reason=""))
        elif i == RELABELABLE:
            rows.append(old_row(i, status="not-yet-implemented", depth="",
                                reason="missing: an old reason [C]"))
        elif i == PARTIAL:
            rows.append(old_row(i, depth="native-partial", evidence=f"{W1};{W2}",
                                reason="2 native click-walk checks (w1:desktop, w2:desktop), all pass; "
                                       "not covered: half; with run.py's 1 own checks, all pass"))
            checks.append(old_check(i, "peer targeted", evidence=AREA_PEER))
            checks.append(old_check(i, "w1: beta [desktop]", evidence=W1 + ":4", reason=""))
            checks.append(old_check(i, "w2: gamma [desktop]", evidence=W2 + ":1", reason=""))
        elif i == NATIVE_PEER:
            rows.append(old_row(i, depth="native", evidence=W2,
                                reason="1 native click-walk checks (w2:desktop), all pass"))
            checks.append(old_check(i, "w2: delta [desktop]", evidence=W2 + ":2", reason=""))
        elif i == TEMPLATE:
            rows.append(old_row(i, status="fail", depth="specific", evidence=AREA_1280,
                                reason="failing checks: fake width check [viewport-1280]"))
            checks.append(old_check(i, "fake width check [viewport-1280]", "fail", AREA_1280, "size=1275"))
            checks.append(old_check(i, "fake geometry check [viewport-1280]", "pass", AREA_1280))
            checks.append(old_check(i, "fake width check [viewport-720]", "pass", AREA_1280))
        else:
            rows.append(old_row(i))
            checks.append(old_check(i, "conv smoke" if i in CONV else "peer smoke",
                                    evidence=AREA_PEER if i in PEER else ""))
    return rows, checks


def write_csv(path: pathlib.Path, fields: list, rows: list):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        w.writerows(rows)


def read_dicts(path: pathlib.Path) -> list:
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def raw_lines(path: pathlib.Path) -> dict:
    """{row_id: [the file's own lines for that row]} — byte-level, no re-parsing."""
    out: dict = {}
    for line in path.read_bytes().decode().split("\r\n")[1:]:
        if line:
            out.setdefault(int(line.split(",", 1)[0]), []).append(line)
    return out


class FakeApp:
    def __init__(self, port):
        self.port, self.env = port, {}

    def snap(self):
        return {"s": []}

    def raw(self, _path):
        return b"\x89PNG fake"


class World:
    """A temp checkout: walk rows, parity, the official table, fake checks,
    fake env groups, a fake app layer and fake native walks."""

    def __init__(self, tmp: pathlib.Path):
        self.tmp = tmp
        self.walk = tmp / "docs" / "walk"
        self.peer_ok = False         # this run's peer smoke check FAILS (it passed before)
        self.fail_env: dict | None = None
        self.launches: list = []
        self.native_results: list = []
        rows, checks = old_table()
        write_csv(self.walk / "results.csv", ROW_FIELDS, rows)
        write_csv(self.walk / "results-checks.csv", CHECK_FIELDS, checks)
        write_csv(tmp / "docs" / "walk-rows.csv", WALK_FIELDS,
                  [{"spec": spec_of(i), "case": case_of(i), "needs": "fixture"}
                   for i in list(CONV) + list(PEER)])
        write_csv(tmp / "docs" / "parity-matrix.csv", PARITY_FIELDS, [
            {"feature": "f", "group": "g", "capability": "an unbuilt capability",
             "web_e2e_specs": f'e2e/conv.spec.ts "{case_of(DEMOTABLE)}"', "phase4_bucket": "C"},
            {"feature": "f", "group": "g", "capability": "a built capability",
             "web_e2e_specs": f'e2e/conv.spec.ts "{case_of(RELABELABLE)}"', "phase4_bucket": "C",
             "phase4_bucket_manual": "A"},
        ])
        self.bin = tmp / "host" / "target" / "debug" / "octosense"
        self.bin.parent.mkdir(parents=True)
        self.bin.write_text("#!/bin/sh\n")
        self.bin.chmod(0o755)
        self.before_rows = raw_lines(self.walk / "results.csv")
        self.before_checks = raw_lines(self.walk / "results-checks.csv")

    # -- the fakes ----------------------------------------------------------
    def checks(self):
        def mk(area, name, rows=run.ALL, fn=None, specific=False):
            return {"area": area, "name": name, "rows": rows, "specific": specific,
                    "fn": fn or (lambda app: (True, "ok"))}
        return [
            mk("conversation", "conv smoke"),
            mk("peer", "peer smoke", fn=lambda app: (self.peer_ok, "peer ok" if self.peer_ok else "peer broke")),
            mk("peer", "peer targeted", rows=("lane number 22",), specific=True),
            mk("peer", "fake width check", rows=("geometry template",), specific=True,
               fn=lambda app: (app.env.get("OCTOSENSE_WINDOW_SIZE") in ("1280x900", "720x900"),
                               f"size={app.env.get('OCTOSENSE_WINDOW_SIZE')}")),
            mk("peer", "fake geometry check", rows=("geometry template",), specific=True),
        ]

    def env_groups(self):
        return {
            ("peer", "viewport-1280"): {"env": {"OCTOSENSE_WINDOW_SIZE": "1280x900"},
                                        "checks": ["fake width check", "fake geometry check"],
                                        "rows": ("geometry template",)},
            ("peer", "viewport-720"): {"env": {"OCTOSENSE_WINDOW_SIZE": "720x900"},
                                       "checks": ["fake width check"],
                                       "rows": ("geometry template",)},
        }

    def procs(self):
        world = self

        class FakeProcs:
            def __init__(self, port, shell_cwd=None):
                self.app_port, self.last_env = port, {}

            def start_server(self, scenario):
                pass

            def start_app_opts(self, scenario, first_run=False, extra_env=None):
                env = dict(extra_env or {})
                if world.fail_env and all(env.get(k) == v for k, v in world.fail_env.items()):
                    raise run.PrereqError("the fake instance refused to start")
                self.last_env = env
                world.launches.append(env)

            def stop_app(self):
                pass

            def stop_all(self):
                pass
        return FakeProcs

    def run_all(self, binary, port, fixture_base, modes=None, only=None, log=print):
        res = [r for r in self.native_results
               if (not only or r["name"] in only) and (not modes or r["mode"] in modes)]
        return res, {n: SPECS[n] for n in {r["name"] for r in res}}

    # -- one run.py invocation --------------------------------------------------
    def run(self, *argv, env=None) -> int:
        walk_files = {"WALK": self.walk, "EVIDENCE": self.walk / "evidence",
                      "WALK_ROWS": self.tmp / "docs" / "walk-rows.csv",
                      "PARITY": self.tmp / "docs" / "parity-matrix.csv", "ROOT": self.tmp}
        environ = {k: v for k, v in os.environ.items()
                   if k not in ("WALK_SCENARIO", "WALK_ONLY_ROWS", "WALK_RUN_SHA")}
        environ.update({"WALK_RUN_SHA": NEW_SHA, **(env or {})})
        with contextlib.ExitStack() as st:
            for name, value in walk_files.items():
                st.enter_context(mock.patch.object(run, name, value))
            st.enter_context(mock.patch.object(run, "CHECKS", self.checks()))
            st.enter_context(mock.patch.object(run, "ENV_GROUPS", self.env_groups()))
            st.enter_context(mock.patch.object(run, "Procs", self.procs()))
            st.enter_context(mock.patch.object(run, "App", FakeApp))
            st.enter_context(mock.patch.object(run, "BIN", self.bin))
            st.enter_context(mock.patch.object(run, "check_prereqs", lambda **kw: (None, self.bin)))
            st.enter_context(mock.patch.object(run, "shrink_png", lambda *a, **kw: None))
            st.enter_context(mock.patch.object(native, "run_all", self.run_all))
            st.enter_context(mock.patch.object(native, "discover",
                                               lambda *a, **kw: [(self.tmp / f"{n}.py", s) for n, s in SPECS.items()]))
            st.enter_context(mock.patch.object(native, "SCRATCH", self.tmp / "tmp" / "walk" / "native"))
            st.enter_context(mock.patch.dict(os.environ, environ, clear=True))
            st.enter_context(mock.patch.object(sys, "argv", ["run.py", "--port", "1", *argv]))
            st.enter_context(contextlib.redirect_stdout(io.StringIO()))
            st.enter_context(contextlib.redirect_stderr(io.StringIO()))
            return run.main()

    # -- reading the result ----------------------------------------------------
    def rows(self) -> dict:
        return {int(r["row_id"]): r for r in read_dicts(self.walk / "results.csv")}

    def checks_of(self, i: int) -> list:
        return [c for c in read_dicts(self.walk / "results-checks.csv") if int(c["row_id"]) == i]

    def unchanged(self, ids) -> list:
        """The rows among `ids` whose lines differ from the table before the run."""
        rows, checks = raw_lines(self.walk / "results.csv"), raw_lines(self.walk / "results-checks.csv")
        return [i for i in ids
                if rows.get(i) != self.before_rows.get(i) or checks.get(i) != self.before_checks.get(i)]


def native_result(name, checks):
    log = f"docs/walk/evidence/native/{name}-desktop.log"
    return {"name": name, "mode": "desktop", "error": None, "log": log,
            "checks": [(n, ok, detail, f"{log}:{k}") for k, (n, ok, detail) in enumerate(checks, start=1)]}


class Case(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.world = World(pathlib.Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()


class OnlyRunMerges(Case):
    def test_rows_outside_the_filter_keep_their_lines_byte_for_byte(self):
        self.world.run("--only", "peer")
        self.assertEqual(self.world.unchanged(CONV), [],
                         "a --only peer run changed conversation rows or their checks")

    def test_rows_inside_the_filter_are_replaced_by_the_checks_that_ran(self):
        self.world.run("--only", "peer")
        rows = self.world.rows()
        # run.py's peer smoke check passed in the table and fails now — on EVERY
        # row of the area (`--only` selects the whole area, not a round-robin cut)
        for i in [21] + list(range(NEWLY_MAPPED, TEMPLATE)):
            self.assertEqual((rows[i]["status"], rows[i]["reason"]), ("fail", "failing checks: peer smoke"), f"row {i}")
            self.assertEqual([(c["check"], c["status"]) for c in self.world.checks_of(i)],
                             [("peer smoke", "fail")], f"row {i}")
        self.assertEqual(rows[NEWLY_MAPPED]["depth"], "smoke", "w2 did not run: run.py decides row 24")

    def test_a_native_partial_row_keeps_its_walk_checks_and_takes_the_fresh_run_py_check(self):
        self.world.run("--only", "peer")
        got = [(c["check"], c["status"], c["run_sha"]) for c in self.world.checks_of(PARTIAL)]
        self.assertEqual(got, [("peer targeted", "pass", NEW_SHA),
                               ("w1: beta [desktop]", "pass", OLD["run_sha"]),
                               ("w2: gamma [desktop]", "pass", OLD["run_sha"])])
        row = self.world.rows()[PARTIAL]
        self.assertEqual((row["status"], row["depth"]), ("pass", "native-partial"))
        self.assertEqual(row["run_sha"], NEW_SHA)

    def test_a_native_row_whose_walks_did_not_run_is_kept_verbatim(self):
        self.world.run("--only", "peer")
        self.assertEqual(self.world.unchanged([NATIVE_PEER]), [],
                         "no check that decides row 23 ran, yet the row changed")

    def test_an_env_group_row_under_the_filter_is_run_at_every_width(self):
        self.world.run("--only", "peer")
        got = sorted((c["check"], c["status"], c["run_sha"]) for c in self.world.checks_of(TEMPLATE))
        self.assertEqual(got, [("fake geometry check [viewport-1280]", "pass", NEW_SHA),
                               ("fake width check [viewport-1280]", "pass", NEW_SHA),
                               ("fake width check [viewport-720]", "pass", NEW_SHA)])
        self.assertEqual(self.world.rows()[TEMPLATE]["status"], "pass")
        sizes = sorted(e.get("OCTOSENSE_WINDOW_SIZE", "") for e in self.world.launches)
        self.assertIn("1280x900", sizes)
        self.assertIn("720x900", sizes)

    def test_an_env_group_instance_that_cannot_start_marks_its_checks_not_run(self):
        self.world.fail_env = {"OCTOSENSE_WINDOW_SIZE": "720x900"}
        self.world.run("--only", "peer", "--limit", "50")
        got = {c["check"]: c for c in self.world.checks_of(TEMPLATE)}
        self.assertEqual(sorted(got), ["fake geometry check [viewport-1280]",
                                       "fake width check [viewport-1280]",
                                       "fake width check [viewport-720]"],
                         "the 1280 results and the 720 check must all be recorded")
        self.assertEqual(got["fake width check [viewport-1280]"]["status"], "pass")
        self.assertEqual(got["fake width check [viewport-720]"]["status"], "not-run")
        self.assertIn("refused to start", got["fake width check [viewport-720]"]["reason"])
        row = self.world.rows()[TEMPLATE]
        self.assertEqual(row["status"], "not-run", "a row with a check that never ran is not a pass")
        self.assertIn("fake width check [viewport-720]", row["reason"])

    def test_provenance_is_set_for_the_re_run_rows_and_kept_for_the_rest(self):
        self.world.run("--only", "peer")
        rows = self.world.rows()
        for col in ("run_sha", "run_at", "run_scope"):
            self.assertIn(col, rows[1], f"results.csv has no {col} column")
        self.assertEqual(rows[21]["run_sha"], NEW_SHA)
        self.assertRegex(rows[21]["run_at"], UTC)
        self.assertIn("only=peer", rows[21]["run_scope"])
        for i in list(CONV) + [NATIVE_PEER]:
            self.assertEqual({k: rows[i][k] for k in OLD}, OLD, f"row {i}")
        self.assertEqual({c["run_sha"] for c in self.world.checks_of(21)}, {NEW_SHA})

    def test_an_unknown_area_is_refused_and_writes_nothing(self):
        self.assertEqual(self.world.run("--only", "peers"), 2)
        self.assertEqual(self.world.unchanged(list(CONV) + list(PEER)), [])


class NativeOnlyMerges(Case):
    def setUp(self):
        super().setUp()
        self.world.native_results = [native_result("w2", [("gamma", True, ""), ("delta", True, ""),
                                                          ("epsilon", False, "broke")])]

    def test_only_the_rows_the_walk_maps_change(self):
        self.world.run("--native-only", "--walks", "w2", "--modes", "desktop")
        changed = [i for i in list(CONV) + list(PEER) if i not in (PARTIAL, NATIVE_PEER, NEWLY_MAPPED)]
        self.assertEqual(self.world.unchanged(changed), [],
                         "rows w2 does not map (and their checks) must not change")

    def test_a_row_another_walk_maps_keeps_that_walks_checks_from_the_table(self):
        self.world.run("--native-only", "--walks", "w2", "--modes", "desktop")
        got = [(c["check"], c["run_sha"]) for c in self.world.checks_of(PARTIAL)]
        self.assertEqual(got, [("peer targeted", OLD["run_sha"]),
                               ("w1: beta [desktop]", OLD["run_sha"]),
                               ("w2: gamma [desktop]", NEW_SHA)])
        row = self.world.rows()[PARTIAL]
        self.assertEqual((row["status"], row["depth"], row["run_sha"]), ("pass", "native-partial", NEW_SHA))

    def test_the_walks_rows_are_re_pointed_with_provenance(self):
        self.world.run("--native-only", "--walks", "w2", "--modes", "desktop")
        rows = self.world.rows()
        self.assertEqual((rows[NEWLY_MAPPED]["status"], rows[NEWLY_MAPPED]["depth"]), ("fail", "native"))
        self.assertEqual([(c["check"], c["status"]) for c in self.world.checks_of(NEWLY_MAPPED)],
                         [("w2: epsilon [desktop]", "fail")])
        for i in (NATIVE_PEER, NEWLY_MAPPED):
            self.assertEqual(rows[i]["run_sha"], NEW_SHA)
            self.assertIn("native-only", rows[i]["run_scope"])


class CompleteRunRegenerates(Case):
    def test_a_full_run_rewrites_every_row_with_its_provenance(self):
        self.world.native_results = [
            native_result("w1", [("alpha", True, ""), ("beta", True, "")]),
            native_result("w2", [("gamma", True, ""), ("delta", True, ""), ("epsilon", True, "")])]
        self.world.peer_ok = True
        self.world.run("--full")
        rows = self.world.rows()
        self.assertEqual(sorted(rows), list(CONV) + list(PEER))
        self.assertEqual({rows[i]["run_sha"] for i in rows}, {NEW_SHA})
        self.assertEqual({rows[i]["run_scope"] for i in rows}, {"full"})
        self.assertEqual(rows[DEMOTABLE]["status"], "not-yet-implemented", "a full run re-reads the parity")
        self.assertEqual((rows[NEWLY_MAPPED]["status"], rows[NEWLY_MAPPED]["depth"]), ("pass", "native"))
        self.assertEqual(rows[TEMPLATE]["status"], "pass")


class OtherSnapshotsNeverTouchTheTable(Case):
    def test_a_scenario_override_writes_its_own_files(self):
        self.world.run("--only", "peer", env={"WALK_SCENARIO": "peer=conversation"})
        self.assertEqual(self.world.unchanged(list(CONV) + list(PEER)), [],
                         "the negative control must never land in the official table")
        self.assertTrue((self.world.walk / "results-scenario.csv").is_file())


class BuildId(unittest.TestCase):
    def test_the_host_build_is_read_from_hostbuilds_stamp(self):
        with tempfile.TemporaryDirectory() as d:
            host = pathlib.Path(d) / "host-lane"
            binary = host / "target" / "debug" / "octosense"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            with mock.patch.dict(os.environ, {}, clear=False):
                os.environ.pop("WALK_RUN_SHA", None)
                self.assertEqual(run.build_id(binary), "unknown")
                (host / "apps" / "octoscode").mkdir(parents=True)
                (host / "apps" / "octoscode" / "BUILT_FROM").write_text("octoscode-app@9c4fb794 20:01:02\n")
                self.assertEqual(run.build_id(binary), "octoscode-app@9c4fb794")
                os.environ["WALK_RUN_SHA"] = "other@1234567"
                self.assertEqual(run.build_id(binary), "other@1234567")


if __name__ == "__main__":
    unittest.main(verbosity=2)
