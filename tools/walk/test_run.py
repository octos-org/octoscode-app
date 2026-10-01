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


class EnvGroups(unittest.TestCase):
    """#43b: the launch env a row needs is DATA (`ENV_GROUPS`), and a row that is
    a TEMPLATE (row 183 runs at every viewport width) needs MORE THAN ONE
    instance — `env_group_for` alone would launch only the first width."""

    def _row(self, case):
        return {"case": case, "spec": "e2e/x.spec.ts", "protocol_methods": ""}

    def test_first_run_row_gets_the_no_connect_instance(self):
        row = self._row("starts a first workspace with the keyboard and focuses "
                        "the useful field when adding another")
        self.assertEqual(run.env_group_for(row), ("keyboard", "first-run"))
        self.assertTrue(run.ENV_GROUPS[("keyboard", "first-run")]["first_run"])

    def test_template_row_gets_every_viewport_width(self):
        # The area is `peer` — AREA_PATTERNS["peer"]'s `lease` matches
        # "re-LEASE-readiness" (run.py:177), the same verdict phase4-gaps.csv:35
        # records. The keys must follow `area_of`, not the row's wording, and
        # `area_of` reads case + SPEC, so the fixture needs the real spec.
        row = {"case": "settings preserves its geometry while model management "
                       "loads at ${viewport.width}px",
               "spec": "e2e/release-readiness.spec.ts", "protocol_methods": ""}
        self.assertEqual(run.area_of(row), "peer")
        self.assertEqual(run.env_groups_for(row),
                         [("peer", "viewport-1280"), ("peer", "viewport-720")])
        # The representative key is the first; the row's verdict is the AND.
        self.assertEqual(run.env_group_for(row), ("peer", "viewport-1280"))

    def test_theme_row_gets_the_light_instance(self):
        row = self._row("manual light preserves readable conversation and "
                        "settings colors on a dark OS")
        self.assertEqual(run.env_group_for(row), ("settings", "theme-light"))
        self.assertEqual(
            run.ENV_GROUPS[("settings", "theme-light")]["env"],
            {"OCTOSCODE_THEME": "light"})

    def test_ordinary_row_keeps_the_default_connected_env(self):
        """A row with no entry must run on the pre-#43b connected instance."""
        for case in ("accepts a typed prompt and streams an answer",
                     "renders the review badge with the live counts",
                     "opens the palette by '/' and runs a command"):
            self.assertIsNone(run.env_group_for(self._row(case)), case)

    def test_every_group_names_checks_that_exist(self):
        """A typo in `checks=` would silently run ZERO checks — and an empty run
        is never `pass` (decided_status), so the row would go red for a wrong
        reason. Pin the names against the registry."""
        known = {c["name"] for c in run.CHECKS}
        for key, spec in run.ENV_GROUPS.items():
            self.assertTrue(spec.get("checks"), f"{key} declares no checks")
            for name in spec["checks"]:
                self.assertIn(name, known, f"{key} names an unregistered check")
            self.assertTrue(spec.get("rows"), f"{key} matches no row")

    def test_every_group_check_is_registered_under_its_own_area(self):
        """A check named by group A but registered under area B would never run
        against A's instance (main() filters by area first)."""
        by_name = {}
        for c in run.CHECKS:
            by_name.setdefault(c["name"], set()).add(c["area"])
        for key, spec in run.ENV_GROUPS.items():
            for name in spec["checks"]:
                self.assertIn(key[0], by_name[name],
                              f"{key} names a check not registered under {key[0]}")


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
