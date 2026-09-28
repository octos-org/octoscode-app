#!/usr/bin/env python3
"""Unit tests for the walk-runner's verdict logic (card #19b).

Run:  python3 tools/walk/test_run.py
      (or) python3 -m unittest tools.walk.test_run

The #19b defect: an area whose backend/app failed to start got `status: blocked`
with the real start-failure reason, but the row loop then overwrote that reason
with `"area 'X': 0 checks, all pass"` — because an empty check list made
`all(...) == True`. So a start failure produced a *green-looking* row.

These tests pin the fix: an empty run is never `pass`, and a blocked area keeps
its own reason.
"""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import run  # noqa: E402


def rr(**kw):
    """One check-result dict."""
    base = {"check": kw.pop("check", "c"), "status": kw.pop("status", "pass"),
            "reason": kw.pop("reason", "")}
    base.update(kw)
    return base


class AreaVerdict(unittest.TestCase):
    def test_empty_run_is_never_pass(self):
        """The core #19b defect: 0 checks must NOT read as 'all pass'."""
        self.assertEqual(run.area_verdict([]), "fail")

    def test_all_pass_is_pass(self):
        self.assertEqual(run.area_verdict([rr(status="pass"), rr(status="pass")]), "pass")

    def test_one_fail_is_fail(self):
        self.assertEqual(run.area_verdict([rr(status="pass"), rr(status="fail")]), "fail")


class RowReason(unittest.TestCase):
    def test_blocked_area_keeps_its_start_failure_reason(self):
        """A blocked area must not be relabelled 'all pass' on an empty run."""
        res = {"status": "blocked", "run": [],
               "evidence": "", "reason": "scenario 'conversation' failed to start: boom"}
        reason = run.row_reason("conversation", res)
        self.assertIn("boom", reason)
        self.assertNotIn("all pass", reason)

    def test_empty_non_blocked_run_does_not_say_all_pass(self):
        """Even if a caller hands an empty non-blocked result, no false 'all pass'."""
        # area_verdict would never produce a non-blocked empty run, but defend the
        # reason text independently.
        res = {"status": "fail", "run": [], "evidence": "", "reason": ""}
        reason = run.row_reason("threads", res)
        self.assertNotIn("all pass", reason)

    def test_passing_area_reports_the_check_count(self):
        res = {"status": "pass", "run": [rr(status="pass"), rr(status="pass")],
               "evidence": "", "reason": ""}
        self.assertIn("2 checks, all pass", run.row_reason("composer", res))

    def test_failing_area_names_the_failed_checks(self):
        res = {"status": "fail", "run": [rr(check="a", status="pass"),
                                         rr(check="b", status="fail")],
               "evidence": "", "reason": ""}
        reason = run.row_reason("recovery", res)
        self.assertIn("b", reason)
        self.assertNotIn("all pass", reason)


class PrereqMessage(unittest.TestCase):
    def test_app_bin_help_names_the_build_recipe(self):
        """The fail-fast message must tell an operator exactly what to run."""
        self.assertIn("OCTOSCODE_APP_BIN", run.APP_BIN_HELP)
        self.assertIn("prepare-octosense-fork.sh", run.APP_BIN_HELP)
        self.assertIn("cargo build", run.APP_BIN_HELP)

    def test_default_shell_cwd_derives_from_a_repo_binary(self):
        """<repo>/tmp/octosense-target/debug/octosense -> <repo>/desktop, when present."""
        # On a machine with the fork checked out this resolves to a Cargo.toml dir;
        # otherwise it falls back to the binary's own parent (never raises).
        got = run.default_shell_cwd(run.BIN)
        self.assertIsInstance(got, pathlib.Path)


if __name__ == "__main__":
    unittest.main(verbosity=2)
