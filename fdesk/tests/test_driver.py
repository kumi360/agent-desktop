"""Unit tests for fdesk.driver."""

from __future__ import annotations

import json
import unittest
from unittest.mock import patch

from fdesk import driver


class TestParseFailure(unittest.TestCase):
    def _completed(self, stdout: str, stderr: str = "", returncode: int = 0):
        class Completed:
            pass
        c = Completed()
        c.stdout = stdout
        c.stderr = stderr
        c.returncode = returncode
        return c

    def test_result_dict_shape_on_cli_failure(self):
        envelope = json.dumps({"ok": False, "error": {"code": "APP_NOT_FOUND", "message": "not running"}})
        with patch("subprocess.run", return_value=self._completed(envelope)):
            result = driver.Desktop().snapshot("NoSuch")
        self.assertFalse(result["ok"])
        self.assertIsNone(result["data"])
        self.assertEqual(result["error"]["code"], "APP_NOT_FOUND")
        self.assertIn("not running", result["error"]["message"])


class TestStaleRef(unittest.TestCase):
    def _completed(self, stdout: str):
        class Completed:
            pass
        c = Completed()
        c.stdout = stdout
        c.stderr = ""
        c.returncode = 0
        return c

    def test_recovery_hint_attached(self):
        envelope = json.dumps({
            "ok": False,
            "error": {
                "code": "STALE_REF",
                "message": "ref vanished",
                "suggestion": "Pick a new ref.",
                "recovery": {"strategy": "reobserve"},
            },
        })
        with patch("subprocess.run", return_value=self._completed(envelope)):
            result = driver.Desktop().click("@1")
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "STALE_REF")
        self.assertIn("Pick a new ref", result["error"]["hint"])
        self.assertEqual(result["error"]["recovery"]["strategy"], "reobserve")


class TestJsonFlagFallback(unittest.TestCase):
    def _completed(self, stdout: str):
        class Completed:
            pass
        c = Completed()
        c.stdout = stdout
        c.stderr = ""
        c.returncode = 0
        return c

    def test_retries_without_json_when_flag_rejected(self):
        reject = json.dumps({
            "ok": False,
            "error": {"code": "INVALID_ARGS", "message": "unknown flag --json"},
        })
        success = json.dumps({"ok": True, "data": {"windows": []}})
        responses = [self._completed(reject), self._completed(success)]
        with patch("subprocess.run", side_effect=responses) as mock_run:
            desktop = driver.Desktop()
            result = desktop.list_apps()
        self.assertTrue(result["ok"])
        self.assertFalse(desktop.json_flag)
        self.assertEqual(mock_run.call_count, 2)
        first_args, second_args = mock_run.call_args_list[0][0][0], mock_run.call_args_list[1][0][0]
        self.assertIn("--json", first_args)
        self.assertNotIn("--json", second_args)


if __name__ == "__main__":
    unittest.main()
