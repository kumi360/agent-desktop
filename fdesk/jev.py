"""Jev SystemOne client. POST ``{base}/systemone``, 30s timeout, two retries on 429/5xx."""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
from typing import Any, Callable

from fdesk import log, redact

DEFAULT_BASE = "https://openrouter.ai/api"
DEFAULT_MODEL = "jev-1.13"
DEFAULT_TIMEOUT = 30.0
MAX_RETRIES = 2
PRICE_PER_INPUT_TOKEN = 0.042 / 1_000_000
FLAT_CALL_COST = 0.00004
_ROUNDING = 0.01

def _ok(data: Any) -> dict:
    return {"ok": True, "data": data, "error": None}

def _err(code: str, message: str, data: Any = None) -> dict:
    return {"ok": False, "data": data, "error": {"code": code, "message": message}}

def choice_question(instructions: str, criteria: dict) -> dict:
    """Build a choice question. Each criterion key maps to a description."""
    return {"type": "choice", "instructions": instructions, "criteria": dict(criteria)}

def noul_question(instructions: str, true: str, false: str) -> dict:
    """Build a noul question with true and false descriptions."""
    return {"type": "noul", "instructions": instructions, "true": true, "false": false}

def score_question(instructions: str, levels: list[str]) -> dict:
    """Build a score question over an ordered list of levels."""
    return {"type": "score", "instructions": instructions, "levels": list(levels)}


def estimate_cost(usage: dict | None) -> float:
    """Estimate dollars from a reported cost, else input tokens, else one step."""
    if not isinstance(usage, dict):
        return FLAT_CALL_COST
    cost = usage.get("cost")
    if isinstance(cost, str):
        try:
            cost = float(cost)
        except ValueError:
            cost = None
    if isinstance(cost, (int, float)) and not isinstance(cost, bool) and cost >= 0:
        return float(cost)
    tokens = usage.get("input_tokens", usage.get("inputTokens"))
    if isinstance(tokens, int) and not isinstance(tokens, bool) and tokens >= 0:
        return tokens * PRICE_PER_INPUT_TOKEN
    return FLAT_CALL_COST

def _unit(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    number = float(value)
    if number != number or number in {float("inf"), float("-inf")} or not 0.0 <= number <= 1.0:
        return None
    return number

def _probabilities(raw: Any, keys: list[str]) -> tuple[dict[str, float] | None, str | None]:
    if not isinstance(raw, dict) or set(raw) != set(keys) or not keys:
        return None, "probability keys differ from the offered options"
    parsed: dict[str, float] = {}
    for key in keys:
        number = _unit(raw.get(key))
        if number is None:
            return None, "probabilities must be numbers between 0 and 1"
        parsed[key] = number
    tolerance = max(0.02, 0.005 * len(parsed))
    if abs(sum(parsed.values()) - 1.0) > tolerance + 1e-9:
        return None, "probabilities are not a coherent distribution"
    return parsed, None

def _leaders(probs: dict[str, float]) -> list[str]:
    top = max(probs.values())
    return [key for key, value in probs.items() if value >= top - _ROUNDING - 1e-9]

def _check_question(name: str, question: Any) -> str | None:
    if not isinstance(question, dict):
        return f"{name} is not an object"
    kind = question.get("type")
    if kind == "choice":
        criteria = question.get("criteria")
        if not isinstance(criteria, dict) or not criteria:
            return f"{name} choice has no criteria"
        return None
    if kind == "noul":
        criteria = question.get("criteria") if isinstance(question.get("criteria"), dict) else {}
        if not (question.get("true") or criteria.get("true")):
            return f"{name} noul needs a true description"
        if not (question.get("false") or criteria.get("false")):
            return f"{name} noul needs a false description"
        return None
    if kind == "score":
        levels = question.get("levels") if question.get("levels") is not None else question.get("criteria")
        if not isinstance(levels, list) or not 2 <= len(levels) <= 10:
            return f"{name} score needs 2 to 10 levels"
        return None
    return f"{name} has an unknown question type"

def _validate_choice(name: str, question: dict, answer: dict) -> str | None:
    criteria = question.get("criteria") or {}
    choice = answer.get("choice")
    if not isinstance(choice, str) or choice not in criteria:
        return f"choice for {name} is not one of the offered criteria"
    parsed, problem = _probabilities(answer.get("probabilities"), list(criteria))
    if problem or parsed is None:
        return f"{problem or 'probabilities are missing'} for {name}"
    if choice not in _leaders(parsed):
        return f"top choice for {name} is not the argmax"
    if "confidence" in answer and _unit(answer.get("confidence")) is None:
        return f"confidence for {name} is not a probability"
    return None


def _validate_noul(name: str, answer: dict) -> str | None:
    raw = answer.get("probability", answer.get("noul"))
    if _unit(raw) is None:
        return f"noul probability for {name} is missing or outside [0, 1]"
    return None

def _validate_score(name: str, question: dict, answer: dict) -> str | None:
    levels = list(question.get("levels") or question.get("criteria") or [])
    index_keys = [str(i) for i in range(len(levels))]
    label_keys = [str(level) for level in levels]
    raw = answer.get("probabilities")
    if not isinstance(raw, dict):
        return f"probabilities for {name} are missing"
    if set(raw) == set(index_keys):
        scheme = index_keys
    elif set(raw) == set(label_keys):
        scheme = label_keys
    else:
        return f"probability keys differ from levels for {name}"
    parsed, problem = _probabilities(raw, scheme)
    if problem or parsed is None:
        return f"{problem or 'probabilities are missing'} for {name}"
    score = answer.get("score")
    if isinstance(score, bool) or not isinstance(score, (int, float)):
        return f"score for {name} is not a number"
    score_f = float(score)
    if score_f != score_f or not 0.0 <= score_f <= len(levels) - 1:
        return f"score for {name} is outside its levels"
    nearest = scheme[int(round(score_f))]
    if nearest not in _leaders(parsed):
        return f"top score for {name} is not the argmax"
    if "confidence" in answer and _unit(answer.get("confidence")) is None:
        return f"confidence for {name} is not a probability"
    return None

def validate_answers(questions: dict, payload: Any) -> dict:
    """Validate answers: offered choice, coherent probabilities, and an argmax top choice."""
    if not isinstance(questions, dict) or not questions:
        return _err("JEV_INVALID_ANSWER", "questions must be a non-empty object")
    for name, question in questions.items():
        problem = _check_question(name, question)
        if problem:
            return _err("JEV_INVALID_ANSWER", problem)
    if not isinstance(payload, dict):
        return _err("JEV_INVALID_ANSWER", "response was not a JSON object")
    answers = payload.get("answers")
    if answers is None and isinstance(payload.get("data"), dict):
        answers = payload["data"].get("answers")
    if not isinstance(answers, dict):
        return _err("JEV_INVALID_ANSWER", "response has no answers object")
    if set(answers) != set(questions):
        return _err("JEV_INVALID_ANSWER", "answer ids differ from question ids")
    normalized: dict[str, Any] = {}
    for name, question in questions.items():
        answer = answers.get(name)
        if not isinstance(answer, dict):
            return _err("JEV_INVALID_ANSWER", f"answer for {name} is not an object")
        kind = question["type"]
        if answer.get("type") not in (None, kind):
            return _err("JEV_INVALID_ANSWER", f"incorrect answer type for {name}")
        if kind == "choice":
            problem = _validate_choice(name, question, answer)
        elif kind == "noul":
            problem = _validate_noul(name, answer)
        else:
            problem = _validate_score(name, question, answer)
        if problem:
            return _err("JEV_INVALID_ANSWER", problem)
        normalized[name] = dict(answer)
        normalized[name]["type"] = kind
        if kind == "noul":
            normalized[name]["probability"] = _unit(answer.get("probability", answer.get("noul")))
    return _ok(normalized)


def _wire(questions: dict) -> dict:
    wired: dict[str, Any] = {}
    for name, question in questions.items():
        kind = question.get("type")
        instructions = question.get("instructions") or ""
        if kind == "noul":
            true_d = question.get("true_description") or question.get("true") or ""
            false_d = question.get("false_description") or question.get("false") or ""
            wired[name] = {"type": "noul", "instructions": instructions,
                           "true_description": true_d, "false_description": false_d}
        elif kind == "score":
            levels = list(question.get("levels") or question.get("criteria") or [])
            wired[name] = {"type": "score", "instructions": instructions, "criteria": levels}
        else:
            wired[name] = {
                "type": "choice",
                "instructions": instructions,
                "criteria": question.get("criteria") or {},
            }
    return wired


class JevClient:
    """POST decision batches to a SystemOne endpoint."""

    def __init__(
        self,
        base_url: str | None = None,
        api_key: str | None = None,
        model: str | None = None,
        timeout: float = DEFAULT_TIMEOUT,
        opener: Callable[..., tuple[int, bytes]] | None = None,
    ):
        """Read ``JEV_BASE_URL``, ``JEV_API_KEY``, and ``JEV_MODEL`` when arguments are omitted."""
        self.base_url = (base_url or os.environ.get("JEV_BASE_URL") or DEFAULT_BASE).rstrip("/")
        self.api_key = api_key if api_key is not None else (os.environ.get("JEV_API_KEY") or "")
        self.model = model or os.environ.get("JEV_MODEL") or DEFAULT_MODEL
        self.timeout = timeout
        self._opener = opener

    def endpoint(self) -> str:
        """Return ``{base}/systemone``."""
        return self.base_url + "/systemone"

    def _scrub(self, text: str) -> str:
        cleaned = redact(text)
        if self.api_key and len(self.api_key) >= 4 and self.api_key in cleaned:
            cleaned = cleaned.replace(self.api_key, "[REDACTED]")
        return cleaned

    def evaluate(self, state: dict, questions: dict) -> dict:
        """Ask one batch and return a validated Result. Transport failures stay typed."""
        try:
            return self._evaluate(state, questions)
        except Exception as exc:
            message = self._scrub(str(exc))
            log(f"jev error: {type(exc).__name__}: {message}")
            return _err("JEV_ERROR", message)

    def self_test(self) -> dict:
        """Send one noul question asking whether two is even, and return the answer."""
        return self.evaluate(
            {"task": "self-test", "prompt": "Decide whether two is an even integer."},
            {"even": noul_question("Is two an even integer?", "Two is divisible by two.", "Two is odd.")},
        )

    def _evaluate(self, state: dict, questions: dict) -> dict:
        if not isinstance(state, dict):
            return _err("JEV_INVALID_ANSWER", "state must be an object")
        problem = None if isinstance(questions, dict) else "questions must be an object"
        if problem is None:
            for name, question in questions.items():
                problem = _check_question(name, question)
                if problem:
                    break
        if problem:
            return _err("JEV_INVALID_ANSWER", problem)
        if not self.api_key:
            return _err("JEV_AUTH", "JEV_API_KEY is not set")
        if any(char in self.api_key for char in "\r\n"):
            return _err("JEV_AUTH", "JEV_API_KEY contains a newline")
        if "://" not in self.base_url:
            return _err("JEV_AUTH", "JEV_BASE_URL is not a URL")
        body = json.dumps({"model": self.model, "state": state, "questions": _wire(questions)}).encode("utf-8")
        attempts = MAX_RETRIES + 1
        for attempt in range(attempts):
            status, raw, transport = self._post(body)
            if transport == "timeout":
                return _err("JEV_TIMEOUT", f"SystemOne timed out after {self.timeout:g}s")
            if transport:
                return _err("JEV_CONNECTION", transport)
            if status == 429 or (status is not None and 500 <= status <= 599):
                if attempt + 1 < attempts:
                    log(f"jev HTTP {status}; retry {attempt + 1}")
                    time.sleep(0.2 * (attempt + 1))
                    continue
                return _err("JEV_HTTP", f"SystemOne returned HTTP {status} after {attempts} attempts")
            if status is None or status >= 400:
                detail = self._scrub(raw.decode("utf-8", "replace"))[:300]
                code = "JEV_AUTH" if status in {401, 403} else "JEV_HTTP"
                return _err(code, f"SystemOne returned HTTP {status}: {detail}")
            try:
                payload = json.loads(raw.decode("utf-8"))
            except json.JSONDecodeError:
                return _err("JEV_BAD_RESPONSE", "SystemOne returned malformed JSON")
            if isinstance(payload, dict) and payload.get("error") and "answers" not in payload:
                message = payload["error"]
                if isinstance(message, dict):
                    message = message.get("message") or message
                return _err("JEV_HTTP", self._scrub(str(message))[:300])
            checked = validate_answers(questions, payload)
            usage = payload.get("usage") if isinstance(payload, dict) else None
            cost = estimate_cost(usage if isinstance(usage, dict) else None)
            if not checked["ok"]:
                checked["data"] = {"estimated_cost": cost, "usage": usage, "requests": attempt + 1}
                return checked
            model = payload.get("model") if isinstance(payload, dict) else None
            return _ok({
                "model": model or self.model,
                "answers": checked["data"],
                "usage": usage if isinstance(usage, dict) else {},
                "estimated_cost": cost,
                "requests": attempt + 1,
            })
        return _err("JEV_HTTP", "SystemOne retries exhausted")
    def _post(self, body: bytes) -> tuple[int | None, bytes, str | None]:
        request = urllib.request.Request(
            self.endpoint(),
            data=body,
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
                "Authorization": f"Bearer {self.api_key}",
            },
            method="POST",
        )
        try:
            if self._opener is not None:
                status, raw = self._opener(request, self.timeout)
                return status, raw, None
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                return response.status, response.read(), None
        except urllib.error.HTTPError as exc:
            return exc.code, exc.read() or b"", None
        except TimeoutError:
            return None, b"", "timeout"
        except urllib.error.URLError as exc:
            reason = exc.reason
            if isinstance(reason, TimeoutError) or "timed out" in str(reason).lower():
                return None, b"", "timeout"
            return None, b"", self._scrub(str(reason) or "connection failed")
