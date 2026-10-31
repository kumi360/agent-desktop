"""Unit tests for fdesk.loop with stubbed dependencies."""

from __future__ import annotations

import unittest

from fdesk import loop


class StubDriver:
    def __init__(self, app="App", click_ok=True, uncertain=False):
        self.app = app
        self.click_ok = click_ok
        self.uncertain = uncertain
        self._tree = {
            "role": "window",
            "name": "Win",
            "children": [
                {"role": "button", "name": "OK", "ref_id": "@1", "available_actions": ["press"]},
                {"role": "text", "name": "status: idle"},
            ],
        }
        self._version = 0
        self.calls: list[tuple] = []

    def snapshot(self, app, window_id=None, timeout=None):
        self.calls.append(("snapshot", app, window_id))
        return {"ok": True, "data": {"tree": self._tree}}

    def list_apps(self, app=None, timeout=None):
        return {"ok": True, "data": {"apps": [app or self.app]}}

    def click(self, ref, timeout=None):
        self.calls.append(("click", ref))
        if not self.click_ok:
            error = {"code": "STALE_REF", "message": "ref gone"}
            if self.uncertain:
                error["disposition"] = {"retry": "unsafe"}
            return {"ok": False, "data": None, "error": error}
        self._tree["children"][1]["name"] = f"status: v{self._version}"
        self._version += 1
        return {"ok": True, "data": None}

    def type_text(self, ref, text, timeout=None):
        self.calls.append(("type_text", ref, text))
        return {"ok": True, "data": None}

    def set_value(self, ref, text, timeout=None):
        self.calls.append(("set_value", ref, text))
        return {"ok": True, "data": None}

    def press(self, key, app=None, window_id=None, timeout=None):
        return {"ok": True, "data": None}

    def focus(self, ref, timeout=None):
        return {"ok": True, "data": None}

    def scroll(self, ref, timeout=None):
        return {"ok": True, "data": None}


class StubJev:
    def __init__(self, operation="CLICK", target="@1", confidence=0.8, present=0.9, destructive=0.1, complete="NO"):
        self.operation = operation
        self.target = target
        self.confidence = confidence
        self.present = present
        self.destructive = destructive
        self.complete = complete
        self.calls: list[tuple] = []

    def evaluate(self, state, questions):
        self.calls.append((state, questions))
        if set(questions) == {"complete"}:
            return self._complete_answer()
        return self._decision_answer(state, questions)

    def _complete_answer(self):
        if self.complete == "YES":
            probs = {"YES": 0.95, "NO": 0.05}
        else:
            probs = {"YES": 0.05, "NO": 0.95}
        return {
            "ok": True,
            "data": {
                "answers": {"complete": {"type": "choice", "choice": self.complete, "probabilities": probs}},
                "estimated_cost": 0.0,
            },
        }

    def _decision_answer(self, state, questions):
        op_keys = list(questions["operation"]["criteria"].keys())
        remainder = (1.0 - self.confidence) / (len(op_keys) - 1) if len(op_keys) > 1 else 0.0
        op_probs = {k: remainder for k in op_keys}
        if self.operation in op_probs:
            op_probs[self.operation] = self.confidence
        else:
            op_probs[op_keys[0]] = self.confidence
        answers: dict = {
            "operation": {"type": "choice", "choice": self.operation, "probabilities": op_probs},
            "present": {"type": "noul", "probability": self.present},
            "destructive": {"type": "noul", "probability": self.destructive},
            "needs_text": {"type": "noul", "probability": 0.0},
        }
        target_key = f"{self.operation.lower()}_target"
        if target_key in questions:
            ids = list(questions[target_key]["criteria"].keys())
            target_probs = {i: 0.0 for i in ids}
            target_probs[self.target if self.target in target_probs else ids[0]] = 1.0
            answers[target_key] = {"type": "choice", "choice": self.target, "probabilities": target_probs}
        return {"ok": True, "data": {"answers": answers, "estimated_cost": 0.0}}


class TestRepeatActionBreaker(unittest.TestCase):
    def test_blocks_on_third_identical_action(self):
        driver = StubDriver()
        jev = StubJev(operation="CLICK", target="@1", complete="NO")
        result = loop.run("Click OK", "App", driver=driver, client=jev, max_steps=5)
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["error"]["code"], "REPEAT_ACTION")
        click_calls = [c for c in driver.calls if c[0] == "click"]
        self.assertEqual(len(click_calls), 2)


class TestStallDetector(unittest.TestCase):
    def test_stops_after_three_no_change_turns(self):
        driver = StubDriver()
        jev = StubJev(operation="DONE", confidence=0.5)
        result = loop.run("Do nothing", "App", driver=driver, client=jev, max_steps=10)
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["error"]["code"], "BLOCKED")
        self.assertIn("3 turns", result["error"]["message"])


class TestMutationRetry(unittest.TestCase):
    def test_uncertain_mutation_is_not_retried(self):
        driver = StubDriver(click_ok=False, uncertain=True)
        jev = StubJev(operation="CLICK", target="@1", complete="NO")
        result = loop.run("Click OK", "App", driver=driver, client=jev, max_steps=2)
        click_calls = [c for c in driver.calls if c[0] == "click"]
        self.assertEqual(len(click_calls), 1)


class TestCheckComplete(unittest.TestCase):
    def test_stops_on_choice_yes(self):
        jev = StubJev(complete="YES")
        ctx = {
            "task": "Click OK",
            "app": "App",
            "window_id": None,
            "history": [{"operation": "CLICK", "target_label": "OK", "effect": "screen changed"}],
            "steps": 1,
            "calls": 0,
            "spend": 0.0,
        }
        outcome = loop._check_complete(jev, ctx)
        self.assertTrue(outcome["stop"])
        self.assertTrue(outcome["result"]["ok"])


if __name__ == "__main__":
    unittest.main()
