"""Subprocess wrapper around the agent-desktop CLI.

Every invocation is attempted with ``--json`` and a timeout. Current
agent-desktop 2.x prints a JSON envelope without that flag and rejects it
as ``INVALID_ARGS`` before doing any work; that rejection is not a delivery,
so the command is issued once more without the flag and the flag stays off.
Envelopes may use ``success`` or ``ok``. Callers receive a Result dict and
never a bare exception.
"""

from __future__ import annotations

import json
import os
import subprocess
from typing import Any

from fdesk import log, redact

DEFAULT_TIMEOUT = 30.0
_SECRET_ENV = ("JEV_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "TYPESAFE_API_KEY")


def _ok(data: Any) -> dict:
    return {"ok": True, "data": data, "error": None}


def _err(code: str, message: str, **extra: Any) -> dict:
    error: dict[str, Any] = {"code": code, "message": message}
    error.update(extra)
    return {"ok": False, "data": None, "error": error}


def _window_ids(error: dict) -> list[str]:
    """Collect window ids from an AMBIGUOUS_TARGET payload."""
    details = error.get("details") if isinstance(error.get("details"), dict) else {}
    sources = (
        error.get("window_ids"),
        details.get("window_ids"),
        details.get("windows"),
        details.get("candidates"),
    )
    found: list[str] = []
    for source in sources:
        if not isinstance(source, list):
            continue
        for item in source:
            if isinstance(item, str) and item:
                found.append(item)
            elif isinstance(item, dict):
                ident = item.get("id") or item.get("window_id") or item.get("windowId")
                if ident:
                    found.append(str(ident))
    unique: list[str] = []
    for item in found:
        if item not in unique:
            unique.append(item)
    return unique


def _annotate(error: dict) -> dict:
    """Attach recovery hints and window ids for the codes the loop branches on."""
    code = str(error.get("code") or "CLI_ERROR")
    message = str(error.get("message") or "agent-desktop command failed")
    extra: dict[str, Any] = {}
    suggestion = error.get("suggestion")
    if code == "STALE_REF":
        hint = suggestion or "Take a fresh snapshot and choose a new ref. Do not repeat the mutation."
        recovery = error.get("recovery")
        if isinstance(recovery, dict) and recovery.get("strategy"):
            hint = f"{hint} Recovery: {recovery['strategy']}."
            extra["recovery"] = recovery
        extra["hint"] = hint
    elif code == "AMBIGUOUS_TARGET":
        extra["window_ids"] = _window_ids(error)
        extra["hint"] = suggestion or "Pass --window-id with one of the candidate window ids."
    elif suggestion:
        extra["hint"] = str(suggestion)
    if isinstance(error.get("disposition"), dict):
        extra["disposition"] = error["disposition"]
    if isinstance(error.get("details"), dict):
        extra["details"] = error["details"]
    return _err(code, message, **extra)


def _parse(stdout: str) -> dict:
    text = (stdout or "").strip()
    if not text:
        return _err("INVALID_RESPONSE", "agent-desktop returned an empty envelope")
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        start, end = text.find("{"), text.rfind("}")
        if start < 0 or end <= start:
            return _err("INVALID_RESPONSE", "agent-desktop stdout was not JSON")
        try:
            payload = json.loads(text[start : end + 1])
        except json.JSONDecodeError:
            return _err("INVALID_RESPONSE", "agent-desktop stdout was not JSON")
    if not isinstance(payload, dict):
        return _err("INVALID_RESPONSE", "agent-desktop envelope was not an object")
    if "success" in payload:
        flag = payload.get("success")
    elif "ok" in payload:
        flag = payload.get("ok")
    else:
        return _err("INVALID_RESPONSE", "envelope has no success flag")
    if flag is True:
        return _ok(payload.get("data"))
    error = payload.get("error")
    if not isinstance(error, dict):
        error = {"code": "CLI_ERROR", "message": str(error or "agent-desktop command failed")}
    return _annotate(error)


def _json_flag_rejected(result: dict) -> bool:
    """True when ``--json`` was refused before the command could do any work."""
    if result.get("ok"):
        return False
    error = result.get("error") or {}
    if error.get("code") != "INVALID_ARGS" or "--json" not in str(error.get("message") or ""):
        return False
    disposition = error.get("disposition") or {}
    if not disposition:
        return True
    return disposition.get("delivery") == "not_delivered" or disposition.get("retry") == "safe"


class Desktop:
    """Run agent-desktop commands and return Result dicts."""

    def __init__(self, binary: str = "agent-desktop", timeout: float = DEFAULT_TIMEOUT, json_flag: bool = True):
        """Wrap ``binary``. ``timeout`` bounds every subprocess. ``json_flag`` appends ``--json``."""
        self.binary = binary
        self.timeout = timeout
        self.json_flag = json_flag
        self._last_stderr = ""

    def call(self, args: list[str], timeout: float | None = None) -> dict:
        """Run one command. Retries once without ``--json`` if that flag is rejected unused."""
        limit = self.timeout if timeout is None else timeout
        result = self._exec(args, limit, json_flag=self.json_flag)
        if self.json_flag and _json_flag_rejected(result):
            self.json_flag = False
            log("agent-desktop rejected --json before delivery; retrying without it")
            result = self._exec(args, limit, json_flag=False)
        if not result["ok"]:
            command = args[0] if args else "?"
            log(f"agent-desktop {command} failed: {result['error']['code']}")
            if self._last_stderr:
                log(self._last_stderr[:240])
        return result

    def _exec(self, args: list[str], timeout: float, json_flag: bool) -> dict:
        argv = [self.binary, *args]
        if json_flag:
            argv.append("--json")
        env = os.environ.copy()
        for name in _SECRET_ENV:
            env.pop(name, None)
        try:
            completed = subprocess.run(
                argv,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
                env=env,
            )
        except subprocess.TimeoutExpired:
            return _err("TIMEOUT", f"agent-desktop timed out after {timeout:g}s")
        except FileNotFoundError:
            return _err("DRIVER_NOT_FOUND", f"could not run {self.binary}")
        except OSError as exc:
            return _err("DRIVER_ERROR", redact(str(exc)))
        self._last_stderr = completed.stderr or ""
        return _parse(completed.stdout or "")

    def list_apps(self, app: str | None = None, timeout: float | None = None) -> dict:
        """List running GUI applications, optionally filtered to one name."""
        args = ["list-apps"]
        if app:
            args.extend(["--app", app])
        return self.call(args, timeout=timeout)

    def snapshot(self, app: str, window_id: str | None = None, timeout: float | None = None) -> dict:
        """Capture the accessibility tree for ``app`` (``snapshot --app``, optional ``--window-id``)."""
        if not app:
            return _err("INVALID_ARGS", "snapshot requires an application name")
        args = ["snapshot", "--app", app]
        if window_id:
            args.extend(["--window-id", window_id])
        return self.call(args, timeout=timeout)

    def click(self, ref: str, timeout: float | None = None) -> dict:
        """Click ``ref`` via the accessibility press action."""
        refused = self._require_ref(ref, "click")
        return refused or self.call(["click", ref], timeout=timeout)

    def type_text(self, ref: str, text: str, timeout: float | None = None) -> dict:
        """Insert ``text`` into the field named by ``ref`` (``type``)."""
        refused = self._require_ref(ref, "type") or self._require_text(text)
        return refused or self.call(["type", ref, text], timeout=timeout)

    def set_value(self, ref: str, text: str, timeout: float | None = None) -> dict:
        """Set the accessibility value of ``ref`` to ``text``."""
        refused = self._require_ref(ref, "set-value") or self._require_text(text)
        return refused or self.call(["set-value", ref, text], timeout=timeout)

    def press(self, key: str, app: str | None = None, window_id: str | None = None, timeout: float | None = None) -> dict:
        """Send one key combo, optionally aimed at an application or window."""
        if not isinstance(key, str) or not key.strip():
            return _err("INVALID_ARGS", "press requires a key")
        args = ["press", key]
        if app:
            args.extend(["--app", app])
        if window_id:
            args.extend(["--window-id", window_id])
        return self.call(args, timeout=timeout)

    def focus(self, ref: str, timeout: float | None = None) -> dict:
        """Move keyboard focus to ``ref``."""
        refused = self._require_ref(ref, "focus")
        return refused or self.call(["focus", ref], timeout=timeout)

    def scroll(self, ref: str, direction: str = "down", amount: int = 3, timeout: float | None = None) -> dict:
        """Scroll the pane named by ``ref``."""
        refused = self._require_ref(ref, "scroll")
        if refused:
            return refused
        if direction not in {"up", "down", "left", "right"}:
            return _err("INVALID_ARGS", "scroll direction must be up, down, left, or right")
        return self.call(
            ["scroll", ref, "--direction", direction, "--amount", str(int(amount))],
            timeout=timeout,
        )

    def _require_ref(self, ref: str, command: str) -> dict | None:
        if not isinstance(ref, str) or not ref.startswith("@"):
            return _err("INVALID_ARGS", f"{command} requires a qualified @ref")
        return None

    def _require_text(self, text: str) -> dict | None:
        if not isinstance(text, str):
            return _err("INVALID_ARGS", "text must be a string supplied by the caller")
        return None
