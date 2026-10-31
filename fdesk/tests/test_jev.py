"""Unit tests for fdesk.jev."""

from __future__ import annotations

import unittest

from fdesk import jev


class TestValidateAnswers(unittest.TestCase):
    def _choice_question(self) -> dict:
        return {
            "op": jev.choice_question("Pick", {"A": "alpha", "B": "beta"}),
        }

    def _choice_answer(self, choice: str, probs: dict | None = None) -> dict:
        return {
            "op": {
                "type": "choice",
                "choice": choice,
                "probabilities": probs or {choice: 1.0},
            },
        }

    def test_choice_key_not_in_criteria_is_invalid(self):
        result = jev.validate_answers(self._choice_question(), {"answers": self._choice_answer("C")})
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "JEV_INVALID_ANSWER")

    def test_incoherent_probabilities_are_invalid(self):
        result = jev.validate_answers(
            self._choice_question(),
            {"answers": self._choice_answer("A", {"A": 0.6, "B": 0.1})},
        )
        self.assertFalse(result["ok"])
        self.assertIn("coherent", result["error"]["message"])

    def test_valid_choice_passes(self):
        result = jev.validate_answers(
            self._choice_question(),
            {"answers": self._choice_answer("A", {"A": 0.7, "B": 0.3})},
        )
        self.assertTrue(result["ok"])
        self.assertEqual(result["data"]["op"]["choice"], "A")

    def test_noul_shape_requires_probability(self):
        question = {"done": jev.noul_question("Done?", "yes", "no")}
        result = jev.validate_answers(question, {"answers": {"done": {"type": "noul"}}})
        self.assertFalse(result["ok"])

    def test_score_shape_requires_score_and_probabilities(self):
        question = {"level": jev.score_question("Rate", ["low", "high"])}
        result = jev.validate_answers(
            question,
            {"answers": {"level": {"type": "score", "score": 1, "probabilities": {"0": 0.2, "1": 0.8}}}},
        )
        self.assertTrue(result["ok"])


class TestWire(unittest.TestCase):
    def test_emits_instructions_not_prompt(self):
        wired = jev._wire({"q": jev.choice_question("Pick", {"A": "a"})})
        self.assertIn("instructions", wired["q"])
        self.assertNotIn("prompt", wired["q"])

    def test_noul_uses_true_false_description(self):
        wired = jev._wire({"q": jev.noul_question("Done?", "yes", "no")})
        self.assertEqual(wired["q"]["type"], "noul")
        self.assertIn("true_description", wired["q"])
        self.assertIn("false_description", wired["q"])

    def test_noul_prefers_true_description_over_true(self):
        wired = jev._wire({"q": {"type": "noul", "instructions": "x", "true": "t", "false": "f",
                                   "true_description": "TD", "false_description": "FD"}})
        self.assertEqual(wired["q"]["true_description"], "TD")
        self.assertEqual(wired["q"]["false_description"], "FD")


class TestEstimateCost(unittest.TestCase):
    def test_flat_cost_when_no_usage(self):
        self.assertEqual(jev.estimate_cost(None), jev.FLAT_CALL_COST)

    def test_cost_from_input_tokens(self):
        cost = jev.estimate_cost({"input_tokens": 1_000_000})
        self.assertAlmostEqual(cost, 0.042, places=6)


if __name__ == "__main__":
    unittest.main()
