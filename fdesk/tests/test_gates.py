"""Unit tests for fdesk.gates."""

from __future__ import annotations

import unittest

from fdesk import gates


class TestAssess(unittest.TestCase):
    def test_flags_file_delete(self):
        result = gates.assess(
            {"operation": "CLICK"},
            {"role": "button", "label": "Delete File"},
        )
        self.assertIn("file_delete", result["reasons"])
        self.assertTrue(result["destructive"])

    def test_flags_settings_change(self):
        result = gates.assess(
            {"operation": "CLICK"},
            {"role": "checkbox", "label": "Enable", "context": "Settings"},
        )
        self.assertIn("settings_change", result["reasons"])

    def test_flags_send_or_post(self):
        result = gates.assess(
            {"operation": "CLICK"},
            {"role": "button", "label": "Send Email"},
        )
        self.assertIn("send_or_post", result["reasons"])

    def test_flags_credential_or_payment(self):
        result = gates.assess(
            {"operation": "CLICK"},
            {"role": "securetextfield", "label": "Password"},
        )
        self.assertIn("credential_or_payment", result["reasons"])

    def test_flags_quit_unsaved(self):
        result = gates.assess(
            {"operation": "PRESS_KEY", "key": "cmd+q"},
            {"role": "window", "label": "Doc", "window": "Doc"},
        )
        self.assertIn("quit_unsaved", result["reasons"])

    def test_quit_anyway_also_flags(self):
        result = gates.assess(
            {"operation": "CLICK"},
            {"role": "button", "label": "Quit Anyway"},
        )
        self.assertIn("quit_unsaved", result["reasons"])


class TestTextQueue(unittest.TestCase):
    def test_prefers_text_list(self):
        self.assertEqual(gates.text_queue('type "x"', ["a", "b"]), ["a", "b"])

    def test_falls_back_to_quoted_spans_in_order(self):
        self.assertEqual(
            gates.text_queue('Type "hello" then "world"', None),
            ["hello", "world"],
        )

    def test_empty_text_list_is_not_preferred(self):
        self.assertEqual(gates.text_queue('Type "fallback"', []), ["fallback"])


class TestTakeText(unittest.TestCase):
    def test_pops_in_order(self):
        queue = ["a", "b"]
        self.assertEqual(gates.take_text(queue), "a")
        self.assertEqual(gates.take_text(queue), "b")

    def test_returns_none_when_empty(self):
        self.assertIsNone(gates.take_text([]))


if __name__ == "__main__":
    unittest.main()
