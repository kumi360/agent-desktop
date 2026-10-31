"""Unit tests for fdesk.policy."""

from __future__ import annotations

import unittest

from fdesk import policy


class TestIndexControls(unittest.TestCase):
    def _tree(self, **extra) -> dict:
        return {"role": "window", "name": "Win", "children": [], **extra}

    def test_skips_scrollbar_menubar_unknown(self):
        root = self._tree(children=[
            {"role": "button", "name": "OK", "ref_id": "@1"},
            {"role": "scrollbar", "name": "", "ref_id": "@2"},
            {"role": "menubar", "name": "File", "ref_id": "@3"},
            {"role": "unknown", "name": "x", "ref_id": "@4"},
        ])
        controls = policy.index_controls({"ok": True, "data": {"tree": root}})
        ids = [c["id"] for c in controls]
        self.assertEqual(ids, ["@1"])
        self.assertNotIn("@2", ids)
        self.assertNotIn("@3", ids)
        self.assertNotIn("@4", ids)

    def test_keeps_refless_labels_out_of_control_table(self):
        root = self._tree(children=[
            {"role": "statictext", "name": "Status: idle", "ref_id": "@1"},
            {"role": "text", "name": "No ref label"},
            {"role": "button", "name": "Refresh", "ref_id": "@2"},
        ])
        controls = policy.index_controls({"data": {"tree": root}})
        ids = [c["id"] for c in controls]
        self.assertIn("@1", ids)
        self.assertIn("@2", ids)
        self.assertNotIn("No ref label", ids)
        self.assertEqual(len(controls), 2)

    def test_caps_at_max_controls(self):
        children = [{"role": "button", "name": f"b{i}", "ref_id": f"@{i}"} for i in range(400)]
        controls = policy.index_controls({"data": {"tree": self._tree(children=children)}})
        self.assertEqual(len(controls), policy.MAX_CONTROLS)

    def test_refless_label_stays_in_tree(self):
        root = self._tree(children=[
            {"role": "text", "name": "No ref label"},
        ])
        digest = policy.tree_fingerprint({"data": {"tree": root}})
        self.assertTrue(digest)


class TestFingerprints(unittest.TestCase):
    def _snapshot(self, label_text: str) -> dict:
        return {"data": {"tree": {"role": "window", "name": "Win", "children": [
            {"role": "button", "name": "OK", "ref_id": "@1"},
            {"role": "text", "name": label_text},
        ]}}}

    def test_tree_fingerprint_changes_when_refless_label_changes(self):
        before = policy.tree_fingerprint(self._snapshot("Status: old"))
        after = policy.tree_fingerprint(self._snapshot("Status: new"))
        self.assertNotEqual(before, after)

    def test_control_fingerprint_ignores_refless_label(self):
        controls_before = policy.index_controls(self._snapshot("Status: old"))
        controls_after = policy.index_controls(self._snapshot("Status: new"))
        self.assertEqual(policy.fingerprint(controls_before), policy.fingerprint(controls_after))


class TestRoute(unittest.TestCase):
    def _decision(self, **kwargs) -> dict:
        base = {
            "operation": "CLICK",
            "target": "@1",
            "element": {"id": "@1"},
            "confidence": 0.8,
            "destructive": 0.1,
            "present": 0.9,
        }
        base.update(kwargs)
        return base

    def test_ordinary_at_bar_acts(self):
        self.assertEqual(policy.route(self._decision(confidence=0.70), authorized=False), "act")

    def test_ordinary_below_bar_confirms(self):
        self.assertEqual(policy.route(self._decision(confidence=0.69), authorized=False), "confirm")

    def test_authorize_lifts_ordinary_only(self):
        self.assertEqual(policy.route(self._decision(confidence=0.60), authorized=True), "act")

    def test_destructive_requires_ninety(self):
        self.assertEqual(policy.route(self._decision(destructive=0.55, confidence=0.89), authorized=False), "confirm")
        self.assertEqual(policy.route(self._decision(destructive=0.55, confidence=0.90), authorized=False), "act")

    def test_below_floor_abstains(self):
        self.assertEqual(policy.route(self._decision(confidence=0.54), authorized=False), "abstain")

    def test_authorize_does_not_lift_destructive(self):
        self.assertEqual(policy.route(self._decision(destructive=0.55, confidence=0.80), authorized=True), "confirm")


class TestInterpret(unittest.TestCase):
    def test_known_choice_offered(self):
        elements = [{"id": "@1", "role": "button", "label": "OK"}]
        answers = {
            "operation": {"choice": "CLICK", "probabilities": {"CLICK": 0.8, "DONE": 0.2}},
            "click_target": {"choice": "@1", "probabilities": {"@1": 1.0}},
        }
        decision = policy.interpret(answers, elements)
        self.assertEqual(decision["operation"], "CLICK")
        self.assertEqual(decision["target"], "@1")
        self.assertEqual(decision["element"]["id"], "@1")

    def test_unknown_choice_falls_back_safe(self):
        elements = [{"id": "@1", "role": "button", "label": "OK"}]
        answers = {
            "operation": {"choice": "UNKNOWN", "probabilities": {"UNKNOWN": 1.0}},
        }
        decision = policy.interpret(answers, elements)
        self.assertEqual(decision["operation"], "UNKNOWN")
        self.assertIsNone(decision["target"])
        self.assertIsNone(decision["element"])


class TestCompleteQuestion(unittest.TestCase):
    def test_returns_choice_with_yes_no_criteria(self):
        question = policy.complete_question("Click OK", [{"operation": "CLICK", "target_label": "OK"}])
        self.assertEqual(question["complete"]["type"], "choice")
        self.assertIn("YES", question["complete"]["criteria"])
        self.assertIn("NO", question["complete"]["criteria"])
        self.assertIn("OK", question["complete"]["instructions"])


if __name__ == "__main__":
    unittest.main()
