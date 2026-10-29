import contextlib
from concurrent.futures import ProcessPoolExecutor
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from jev_budget import Budget, BudgetError, BudgetExceeded, DEFAULT_LIMIT, WEEK_SECONDS, main


def reserve_in_process(path):
    try:
        Budget(path).reserve(300)
        return True
    except BudgetExceeded:
        return False


class BudgetTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "budget.sqlite3"
        self.now = 1_800_000_000
        self.budget = Budget(self.path, clock=lambda: self.now)

    def test_default_budget_and_persistent_controls(self):
        initial = self.budget.snapshot()
        self.assertTrue(initial["enabled"])
        self.assertEqual(initial["limit_tokens"], DEFAULT_LIMIT)
        self.budget.configure("set", 1000)
        self.budget.configure("off")
        reopened = Budget(self.path, clock=lambda: self.now).snapshot()
        self.assertEqual((reopened["enabled"], reopened["limit_tokens"]), (False, 1000))

    def test_reservations_reconcile_to_reported_usage(self):
        self.budget.configure("set", 1000)
        reservation = self.budget.reserve(800)
        with self.assertRaises(BudgetExceeded):
            self.budget.reserve(300)
        self.budget.settle(reservation, 250)
        self.budget.settle(self.budget.reserve(700), 500)
        state = self.budget.snapshot()
        self.assertEqual((state["used_tokens"], state["reserved_tokens"], state["remaining_tokens"]),
                         (750, 0, 250))

    def test_unknown_and_abandoned_requests_stay_charged(self):
        self.budget.configure("set", 1000)
        self.budget.settle(self.budget.reserve(400), None)
        self.budget.reserve(400)
        self.now += 301
        state = self.budget.snapshot()
        self.assertEqual((state["unreported_tokens"], state["reserved_tokens"]), (800, 0))
        with self.assertRaises(BudgetExceeded):
            self.budget.reserve(201)

    def test_disabled_budget_still_accounts_and_reenable_enforces(self):
        self.budget.configure("set", 100)
        self.budget.configure("off")
        self.budget.settle(self.budget.reserve(300), 200)
        state = self.budget.configure("on")
        self.assertEqual((state["used_tokens"], state["remaining_tokens"]), (200, 0))
        with self.assertRaises(BudgetExceeded):
            self.budget.reserve(1)

    def test_reset_preserves_live_reservations_and_settings(self):
        self.budget.configure("set", 1000)
        self.budget.configure("off")
        self.budget.settle(self.budget.reserve(100), 75)
        self.budget.settle(self.budget.reserve(200), None)
        pending = self.budget.reserve(300)
        self.now += 10
        state = self.budget.configure("reset")
        self.assertEqual((state["enabled"], state["limit_tokens"], state["accounted_tokens"]),
                         (False, 1000, 300))
        self.budget.settle(pending, 80)
        self.assertEqual(self.budget.snapshot()["accounted_tokens"], 80)

    def test_automatic_weekly_reset_preserves_call_spanning_boundary(self):
        self.budget.snapshot()
        self.budget.settle(self.budget.reserve(100), 75)
        self.now += WEEK_SECONDS - 1
        pending = self.budget.reserve(200)
        self.now += 2
        state = self.budget.snapshot()
        self.assertEqual((state["used_tokens"], state["reserved_tokens"]), (0, 200))
        self.budget.settle(pending, 50)
        self.now += 3 * WEEK_SECONDS
        self.assertEqual(self.budget.snapshot()["accounted_tokens"], 0)

    def test_concurrent_processes_cannot_over_admit_reservations(self):
        Budget(self.path).configure("set", 1000)
        with ProcessPoolExecutor(max_workers=4) as pool:
            admitted = list(pool.map(reserve_in_process, [str(self.path)] * 12))
        self.assertEqual(sum(admitted), 3)
        self.assertEqual(Budget(self.path).snapshot()["reserved_tokens"], 900)

    def test_corrupt_state_fails_closed(self):
        self.path.write_text("invalid database")
        with self.assertRaises(BudgetError):
            self.budget.reserve(100)

    def test_failed_transaction_rolls_back_and_releases_lock(self):
        self.budget.configure("set", 1000)
        with self.assertRaises(RuntimeError):
            with self.budget.transaction() as connection:
                connection.execute("UPDATE settings SET limit_tokens = 0")
                raise RuntimeError("interrupted")
        self.budget.reserve(500)
        self.assertEqual(self.budget.snapshot()["remaining_tokens"], 500)

    def test_cli_controls_are_available_without_provider_credentials(self):
        with patch.dict("os.environ", {"CODEX_JEV_BUDGET_FILE": str(self.path)}):
            for arguments, expected in [
                (["set", "1234", "--json"], (True, 1234)),
                (["--json", "off"], (False, 1234)),
                (["reset", "--json"], (False, 1234)),
                (["on", "--json"], (True, 1234)),
            ]:
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    self.assertEqual(main(arguments), 0)
                state = json.loads(output.getvalue())
                self.assertEqual((state["enabled"], state["limit_tokens"]), expected)

    def test_invalid_limit_does_not_change_state(self):
        for invalid in (-1, True, 1.5, 10**16):
            with self.assertRaises(BudgetError):
                self.budget.configure("set", invalid)
        self.assertEqual(self.budget.snapshot()["limit_tokens"], DEFAULT_LIMIT)


if __name__ == "__main__":
    unittest.main()
