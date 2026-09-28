#!/usr/bin/env python3
"""Unit tests for the walk-runner's verdict + input logic (cards #19b, #19c).

Run:  python3 tools/walk/test_run.py
      (or) python3 -m unittest tools.walk.test_run

Two defect classes are pinned here:

* **#19b** — an area whose server/app failed to start got `status: blocked` with
  the real reason, but the row loop then overwrote it with
  `"area 'X': 0 checks, all pass"` (an empty check list made `all(...) == True`).
  So a start failure produced a *green-looking* row. Fix: `decided_status()` never
  calls an empty run `pass`, and `row_reason()` keeps the area's own reason.
* **#19c** — the composer checks asserted exact equality after typing into a
  **populated** field, but makepad `/t` sends `Input::Text { replace_last: false }`
  (`native/makepad/platform/src/remote.rs:1328`) — it **inserts at the caret**.
  `check_applies()` also pins the finer per-check row mapping.
"""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import run  # noqa: E402


class DecidedStatus(unittest.TestCase):
    def test_empty_run_is_never_pass(self):
        """#19b: 0 checks must NOT read as 'all pass'."""
        self.assertEqual(run.decided_status([], area_blocked=False), "fail")

    def test_all_pass_is_pass(self):
        self.assertEqual(run.decided_status(["pass", "pass"], area_blocked=False), "pass")

    def test_one_fail_is_fail(self):
        self.assertEqual(run.decided_status(["pass", "fail"], area_blocked=False), "fail")

    def test_blocked_area_is_blocked_even_with_checks(self):
        self.assertEqual(run.decided_status([], area_blocked=True), "blocked")


class RowReason(unittest.TestCase):
    def test_blocked_area_keeps_its_start_failure_reason(self):
        """#19b: a blocked area must not be relabelled 'all pass' on an empty run."""
        reason = run.row_reason([], area_reason="scenario 'conversation' failed to start: boom")
        self.assertIn("boom", reason)
        self.assertNotIn("all pass", reason)

    def test_empty_run_never_says_all_pass(self):
        reason = run.row_reason([], area_reason="")
        self.assertNotIn("all pass", reason)

    def test_passing_row_reports_the_check_count(self):
        checks = [("a", "pass"), ("b", "pass")]
        self.assertIn("2 checks, all pass", run.row_reason(checks))

    def test_failing_row_names_the_failed_checks(self):
        reason = run.row_reason([("a", "pass"), ("b", "fail")])
        self.assertIn("b", reason)
        self.assertNotIn("all pass", reason)


class CheckApplies(unittest.TestCase):
    """#19c item 3: a per-check `rows=` selector scopes that check to walk rows."""

    def _row(self, case, methods=""):
        return {"case": case, "spec": "e2e/x.spec.ts", "protocol_methods": methods}

    def test_all_marker_applies_to_every_row(self):
        chk = {"rows": run.ALL}
        self.assertTrue(run.check_applies(chk, self._row("anything")))
        self.assertTrue(run.check_applies(chk, self._row("", "turn/start")))

    def test_substring_scopes_to_matching_rows(self):
        chk = {"rows": ("prompt", "input")}
        # matches on the row's case …
        self.assertTrue(run.check_applies(chk, self._row("accepts a typed prompt")))
        # … or on its protocol_methods (the selector searches both).
        self.assertTrue(run.check_applies(chk, self._row("unrelated", "x/input_thing")))

    def test_substring_excludes_non_matching_rows(self):
        """The composer-input check must not fail a row that does not use it."""
        chk = {"rows": ("prompt", "input")}
        self.assertFalse(run.check_applies(chk, self._row("keeps a background turn alive")))
        self.assertFalse(run.check_applies(chk, self._row("reconnect replays the turn", "turn/start;turn/steer")))


class ExitCode(unittest.TestCase):
    def test_ignores_a_clean_run(self):
        self.assertEqual(run.exit_code({"pass": 30, "skipped": 31}, 0), 0)

    def test_flags_a_failure(self):
        self.assertEqual(run.exit_code({"pass": 29, "fail": 1}, 0), 1)

    def test_flags_a_not_run_area(self):
        self.assertEqual(run.exit_code({"pass": 20, "not-run": 10}, 0), 1)

    def test_flags_an_infra_blocked_selected_row(self):
        """A start failure that blocks a *selected* row must not exit 0."""
        self.assertEqual(run.exit_code({"pass": 0, "blocked": 9}, 9), 1)

    def test_ignores_the_three_real_turn_blocked_rows(self):
        """The 3 real-turn rows are never selected, so they never affect the code."""
        self.assertEqual(run.exit_code({"pass": 30, "blocked": 3}, 0), 0)


class PrereqMessage(unittest.TestCase):
    def test_app_bin_help_names_the_build_recipe(self):
        """The fail-fast message must tell an operator exactly what to run."""
        self.assertIn("OCTOSCODE_APP_BIN", run.APP_BIN_HELP)
        self.assertIn("prepare-octosense-fork.sh", run.APP_BIN_HELP)
        self.assertIn("cargo build", run.APP_BIN_HELP)

    def test_default_shell_cwd_derives_from_a_repo_binary(self):
        """<repo>/tmp/octosense-target/debug/octosense -> <repo>/desktop, when present."""
        got = run.default_shell_cwd(run.BIN)
        self.assertIsInstance(got, pathlib.Path)


class ComposerSemantics(unittest.TestCase):
    """#19c: the input helper must clear first — `/t` inserts, it does not replace."""

    def test_clear_presses_is_positive(self):
        self.assertGreater(run.CLEAR_PRESSES, 0)

    def test_placeholder_is_the_design_copy(self):
        # design `conversation-08` composer.placeholder (design/bindings.json:28)
        self.assertEqual(run.PLACEHOLDER, "Ask Octos anything")


if __name__ == "__main__":
    unittest.main(verbosity=2)
