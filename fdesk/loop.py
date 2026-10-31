"""Observe, decide, gate, act, and verify.

Mutations are never retried. When a click may have landed but the following
snapshots disagree, the loop re-observes instead of issuing the click again.
"""

from __future__ import annotations

from typing import Any

from fdesk import log, redact
from fdesk.driver import Desktop
from fdesk.gates import assess, take_text, text_queue
from fdesk.jev import JevClient
from fdesk.policy import (
    MAX_CONTROLS,
    complete_question,
    fingerprint,
    index_controls,
    interpret,
    build_questions,
    route,
    tree_fingerprint,
    window_title,
)

MAX_STEPS = 40
MAX_CALLS = 80
STALL_TURNS = 3
_ELEMENT_OPS = {"CLICK", "TYPE_TEXT", "SET_VALUE", "FOCUS_WINDOW", "SCROLL"}
_TEXT_OPS = {"TYPE_TEXT", "SET_VALUE"}
_STOP = {"DONE", "BLOCKED"}


def run(
    task: str,
    app: str,
    texts: list[str] | None = None,
    authorize: bool = False,
    max_steps: int = MAX_STEPS,
    max_calls: int = MAX_CALLS,
    window_id: str | None = None,
    driver: Any = None,
    client: Any = None,
    notes: str = "",
) -> dict:
    """Run the desktop loop until DONE, BLOCKED, confirmation, or a budget.

    Returns a Result-style dict. ``ok`` is true only when the model finishes
    on DONE. Jev calls and an estimated dollar spend are counted on the result.
    """
    desktop = driver or Desktop()
    jev = client or JevClient()
    queue = text_queue(task, texts)
    history: list[dict] = []
    ctx = {
        "task": task, "app": app, "window_id": window_id, "steps": 0, "calls": 0,
        "spend": 0.0, "history": history, "stall": 0, "latch": None, "notes": notes,
        "repeat": 0, "last_sig": None,
    }
    try:
        observed = desktop.snapshot(app, window_id=window_id)
        if not observed["ok"]:
            return _finish(ctx, "error", observed["error"])
        while ctx["steps"] < max_steps:
            if ctx["calls"] >= max_calls:
                return _finish(ctx, "budget", _budget(f"stopped at {max_calls} model calls"))
            ctx["steps"] += 1
            outcome = _step(desktop, jev, observed, queue, authorize, ctx)
            if outcome.get("stop"):
                return outcome["result"]
            observed = outcome.get("observation") or observed
            if observed is None or not observed.get("ok"):
                error = (observed or {}).get("error") or {"code": "CLI_ERROR", "message": "snapshot failed"}
                return _finish(ctx, "error", error)
        return _finish(ctx, "budget", _budget(f"stopped at {max_steps} steps"))
    except Exception as exc:
        message = redact(str(exc))
        log(f"loop error: {type(exc).__name__}: {message}")
        return _finish(ctx, "error", {"code": "INTERNAL", "message": message})


def _step(desktop: Any, jev: Any, observed: dict, queue: list[str], authorize: bool, ctx: dict) -> dict:
    controls = index_controls(observed.get("data"))
    built = build_questions(ctx["task"], controls, ctx["history"], _notes(ctx, observed.get("data"), controls))
    ctx["calls"] += 1
    asked = jev.evaluate(built["state"], built["questions"])
    ctx["spend"] += _cost(asked)
    if not asked["ok"]:
        code = asked["error"]["code"]
        log(f"step {ctx['steps']} jev {code}")
        _record(ctx, "", None, "abstain", False, False, code)
        return _stall_or_continue(ctx, observed)
    decision = interpret(asked["data"]["answers"], controls)
    operation = decision["operation"]
    element = decision["element"]
    assessment = assess(
        {"operation": operation, "key": decision["target"] if operation == "PRESS_KEY" else ""},
        element,
    )
    if assessment["destructive"]:
        current = decision["destructive"] if decision["destructive"] is not None else 0.0
        decision = {**decision, "destructive": max(current, 0.50)}
    chosen = route(decision, authorize)
    target = decision.get("target")
    log(
        f"step {ctx['steps']} op={operation or '-'} target={target or '-'} "
        f"route={chosen} confidence={float(decision['confidence']):.2f}"
    )
    if operation in _STOP and chosen == "act":
        _record(ctx, operation, target, chosen, True, None, operation.lower())
        if operation == "DONE":
            return {"stop": True, "result": _finish(ctx, "done", None)}
        return {"stop": True, "result": _finish(ctx, "blocked", {"code": "BLOCKED", "message": "the model stopped on BLOCKED"})}
    if chosen == "confirm":
        _record(ctx, operation, target, chosen, False, False, "confirm")
        message = (
            f"needs confirmation ({', '.join(assessment['reasons']) or 'confidence'}); "
            f"confidence {float(decision['confidence']):.2f}"
        )
        return {"stop": True, "result": _finish(ctx, "confirm", {"code": "GATE_REFUSED", "message": message})}
    if chosen != "act":
        _record(ctx, operation, target, "abstain", False, False, "abstain")
        return _stall_or_continue(ctx, observed)
    sig = (operation, str(target).split(":")[-1] if target else "")
    if sig == ctx["last_sig"] and operation not in _TEXT_OPS:
        ctx["repeat"] += 1
    else:
        ctx["repeat"] = 0
    ctx["last_sig"] = sig
    if ctx["repeat"] >= 2:
        _record(ctx, operation, target, "abstain", False, False,
                "same action chosen 3 times in a row; stopping to avoid a loop")
        return {"stop": True, "result": _finish(
            ctx, "blocked",
            {"code": "REPEAT_ACTION", "message": f"{operation} on {target} chosen 3 times consecutively without task completion"},
        )}
    if ctx["latch"] and ctx["latch"] == (operation, target):
        log("not retrying a mutation that may have landed")
        ctx["latch"] = None
        _record(ctx, operation, target, "abstain", False, False, "not retried; previous dispatch may have landed")
        return _stall_or_continue(ctx, observed)
    if operation == "WAIT":
        _record(ctx, operation, target, chosen, True, None, "wait")
        return {"stop": False, "observation": desktop.snapshot(ctx["app"], window_id=ctx["window_id"])}
    fresh = _fresh(desktop, ctx["app"], ctx["window_id"], observed)
    if fresh["stop"]:
        return {"stop": True, "result": _finish(ctx, "error", fresh["error"])}
    if not fresh["ok"]:
        _record(ctx, operation, target, chosen, False, None, "window changed before the action; mutation skipped")
        ctx["stall"] = 0
        return {"stop": False, "observation": fresh["observation"] or observed}
    text = None
    if operation in _TEXT_OPS:
        text = take_text(queue)
        if text is None:
            _record(ctx, operation, target, chosen, False, False, "no supplied text")
            return {"stop": True, "result": _finish(
                ctx, "blocked", {"code": "TEXT_UNAVAILABLE", "message": "no caller-supplied text remains"},
            )}
    before = tree_fingerprint(observed.get("data"))
    acted = _dispatch(desktop, operation, target, text, ctx["app"], ctx["window_id"])
    if operation in _TEXT_OPS and text is not None and not acted.get("ok") and not _may_have_landed(acted):
        queue.insert(0, text)
    verified = _verify(desktop, ctx["app"], ctx["window_id"])
    uncertain = _may_have_landed(acted) or not verified["settled"]
    changed = None
    if verified["settled"] and verified["fingerprint"] is not None:
        changed = verified["fingerprint"] != before
    effect = _effect(acted, verified["settled"], changed)
    if uncertain:
        effect = "mutation may have landed; not retried. " + effect
        ctx["latch"] = (operation, target)
    _record(ctx, operation, target, chosen, bool(acted.get("ok")), changed, effect,
            target_label=(decision.get("element") or {}).get("label"))
    ctx["stall"] = _bump(ctx["stall"], operation, changed, uncertain)
    if ctx["stall"] >= STALL_TURNS:
        return {"stop": True, "result": _stalled(ctx)}
    if changed and not uncertain and operation not in _STOP:
        done = _check_complete(jev, ctx)
        if done.get("stop"):
            return done
    return {"stop": False, "observation": verified["observation"] or observed}


def _check_complete(jev: Any, ctx: dict) -> dict:
    """Ask the standalone completion question after a successful visible action.

    Asked alone (not in the decision batch) because batched noul answers
    anchor to the operation choice and under-report completion.
    """
    question = complete_question(ctx["task"], ctx["history"])
    ctx["calls"] += 1
    asked = jev.evaluate({"task": ctx["task"], "recent_actions": ctx["history"][-6:]}, question)
    ctx["spend"] += _cost(asked)
    if not asked.get("ok"):
        return {"stop": False}
    answer = (asked["data"].get("answers") or {}).get("complete") or {}
    choice = answer.get("choice")
    probs = answer.get("probabilities") or {}
    prob_yes = probs.get("YES", 0.0) if isinstance(probs, dict) else 0.0
    log(f"step {ctx['steps']} standalone complete choice={choice} p_yes={prob_yes:.2f}")
    if choice == "YES":
        _record(ctx, "DONE", None, "act", True, None, f"complete=YES@{prob_yes:.2f}")
        return {"stop": True, "result": _finish(ctx, "done", None)}
    return {"stop": False}


def _dispatch(desktop: Any, operation: str, target: Any, text: str | None, app: str, window_id: str | None) -> dict:
    if operation == "CLICK":
        return desktop.click(target)
    if operation == "TYPE_TEXT":
        return desktop.type_text(target, text)
    if operation == "SET_VALUE":
        return desktop.set_value(target, text)
    if operation == "PRESS_KEY":
        return desktop.press(target, app=app, window_id=window_id)
    if operation == "FOCUS_WINDOW":
        return desktop.focus(target)
    if operation == "SCROLL":
        return desktop.scroll(target)
    return {"ok": False, "data": None, "error": {"code": "INVALID_ARGS", "message": f"unsupported operation {operation}"}}


def _fresh(desktop: Any, app: str, window_id: str | None, observed: dict) -> dict:
    """Re-check that the app is running and the window title is unchanged."""
    listed = desktop.list_apps(app)
    if listed.get("ok"):
        running = _app_listed(listed.get("data"), app)
        if running is False:
            return {"ok": False, "stop": True, "error": {"code": "APP_NOT_FOUND", "message": f"{app} is not running"}, "observation": None}
    elif (listed.get("error") or {}).get("code") == "APP_NOT_FOUND":
        return {"ok": False, "stop": True, "error": listed["error"], "observation": None}
    again = desktop.snapshot(app, window_id=window_id)
    if not again.get("ok"):
        code = (again.get("error") or {}).get("code")
        if code in {"APP_NOT_FOUND", "AMBIGUOUS_TARGET"}:
            return {"ok": False, "stop": True, "error": again["error"], "observation": None}
        return {"ok": False, "stop": False, "error": again.get("error"), "observation": None}
    before, after = window_title(observed.get("data")), window_title(again.get("data"))
    if before and after and before != after:
        return {"ok": False, "stop": False, "error": None, "observation": again}
    return {"ok": True, "stop": False, "error": None, "observation": again}


def _verify(desktop: Any, app: str, window_id: str | None) -> dict:
    """Take snapshots until two in a row share a fingerprint, or the check is unclear."""
    shots: list[tuple[dict, str]] = []
    for _ in range(3):
        shot = desktop.snapshot(app, window_id=window_id)
        if not shot.get("ok"):
            return {"settled": False, "observation": shot if shot.get("data") else (shots[-1][0] if shots else shot), "fingerprint": shots[-1][1] if shots else None}
        digest = tree_fingerprint(shot.get("data"))
        shots.append((shot, digest))
        if len(shots) >= 2 and shots[-1][1] == shots[-2][1]:
            return {"settled": True, "observation": shot, "fingerprint": digest}
    return {"settled": False, "observation": shots[-1][0], "fingerprint": shots[-1][1]}


def _may_have_landed(result: dict) -> bool:
    if result.get("ok"):
        return False
    disposition = (result.get("error") or {}).get("disposition") or {}
    if disposition.get("retry") == "safe" or disposition.get("delivery") == "not_delivered":
        return False
    return True


def _effect(acted: dict, settled: bool, changed: bool | None) -> str:
    if not acted.get("ok"):
        code = (acted.get("error") or {}).get("code") or "CLI_ERROR"
        base = f"action failed: {code}"
    elif not settled:
        base = "verification unclear"
    elif changed:
        base = "screen changed"
    else:
        base = "no visible change"
    if not settled:
        return base + "; mutation was not retried"
    return base


def _app_listed(data: Any, app: str) -> bool | None:
    apps = data.get("apps") if isinstance(data, dict) else data if isinstance(data, list) else None
    if not isinstance(apps, list):
        return None
    names = []
    for item in apps:
        if isinstance(item, str):
            names.append(item)
        elif isinstance(item, dict):
            names.append(str(item.get("name") or item.get("app") or ""))
    return any(name.lower() == app.lower() for name in names)


def _notes(ctx: dict, data: Any, controls: list[dict]) -> str:
    parts = [f"Application: {ctx['app']}."]
    if ctx["notes"]:
        parts.append(str(ctx["notes"]))
    if isinstance(data, dict) and (data.get("complete") is False or data.get("truncated")):
        parts.append("The accessibility tree was truncated.")
    if len(controls) >= MAX_CONTROLS:
        parts.append("The control list was capped. Prefer SCROLL over BLOCKED when the target may be off screen.")
    if ctx["latch"]:
        parts.append("The previous mutation may have landed. Do not repeat it until a settled screen shows it did not.")
    return " ".join(parts)


def _cost(result: dict) -> float:
    data = result.get("data") or {}
    value = data.get("estimated_cost") if isinstance(data, dict) else None
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return float(value)
    return 0.0


def _bump(stall: int, operation: str, changed: bool | None, uncertain: bool) -> int:
    if operation == "WAIT" or uncertain or changed is None:
        return stall
    return 0 if changed else stall + 1


def _record(ctx: dict, operation: str, target: Any, chosen: str, ok: bool, changed: bool | None, effect: str, target_label: str | None = None) -> None:
    entry = {
        "step": ctx["steps"], "operation": operation, "target": target, "route": chosen,
        "ok": ok, "changed": changed, "effect": effect,
    }
    if target_label:
        entry["target_label"] = target_label
    ctx["history"].append(entry)


def _stall_or_continue(ctx: dict, observed: dict) -> dict:
    ctx["stall"] += 1
    if ctx["stall"] >= STALL_TURNS:
        return {"stop": True, "result": _stalled(ctx)}
    return {"stop": False, "observation": observed}


def _stalled(ctx: dict) -> dict:
    return _finish(ctx, "blocked", {"code": "BLOCKED", "message": f"stopped after {STALL_TURNS} turns that changed nothing"})


def _budget(message: str) -> dict:
    return {"code": "BUDGET_EXCEEDED", "message": message}


def _finish(ctx: dict, status: str, error: dict | None) -> dict:
    """Package the run ledger. ``ok`` is true only for a DONE completion."""
    return {
        "ok": status == "done",
        "status": status,
        "error": error,
        "steps": ctx["steps"],
        "model_calls": ctx["calls"],
        "estimated_spend_usd": round(ctx["spend"], 8),
        "history": ctx["history"],
        "task": ctx["task"],
        "app": ctx["app"],
        "window_id": ctx["window_id"],
    }
