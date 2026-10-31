"""Code-owned destructive taxonomy and the caller-supplied text rule.

The model never invents a string. Typed text comes from ``--text`` or from
quoted spans of the task, consumed in order.
"""

from __future__ import annotations

import re
from typing import Any

_DELETE = re.compile(
    r"\b(delete|trash|erase|shred|empty trash|move to trash|remove permanently|delete permanently)\b",
    re.I,
)
_SETTINGS_LABEL = re.compile(r"\b(reset|restore defaults|factory|erase all|change password|clear data)\b", re.I)
_SETTINGS_PLACE = re.compile(r"\b(settings|preferences|privacy|permissions)\b", re.I)
_SEND = re.compile(r"\b(send|post|publish|share|reply|tweet|submit)\b", re.I)
_QUIT_LABEL = re.compile(r"\b(quit|exit|close)\b", re.I)
_DISCARD = re.compile(r"(don't save|dont save|discard|quit anyway|close without saving)", re.I)
_UNSAVED = re.compile(r"\b(unsaved|edited|dirty|modified|without saving)\b", re.I)
_SECRET = re.compile(
    r"(password|passcode|secret|api key|credit card|card number|\bcvv\b|\bcvc\b|"
    r"payment|billing|social security|\bssn\b|\bpin\b)",
    re.I,
)
_SECRET_ROLES = {"securetextfield", "password", "passwordfield", "passwordtext"}
_TOGGLE_ROLES = {"switch", "checkbox", "toggle", "slider", "radiobutton", "radio", "togglebutton"}
_QUIT_KEYS = {"cmd+q", "command+q", "ctrl+q", "control+q", "alt+f4", "meta+q"}
_FILE_WINDOW = re.compile(r"\b(finder|files|folder|trash|directory)\b", re.I)
_QUOTED = re.compile(r'"([^"\n]{1,500})"|\'([^\'\n]{1,500})\'')


def _norm_role(role: str) -> str:
    text = re.sub(r"[^a-z0-9]", "", (role or "").lower())
    return text[2:] if text.startswith("ax") else text


def _blob(action: Any, element: Any) -> str:
    parts: list[str] = []
    if isinstance(action, str):
        parts.append(action)
    elif isinstance(action, dict):
        for key in ("operation", "key", "label"):
            value = action.get(key)
            if value:
                parts.append(str(value))
    if isinstance(element, dict):
        for key in ("role", "label", "value", "context", "window"):
            value = element.get(key)
            if value:
                parts.append(str(value))
        for state in element.get("states") or []:
            parts.append(str(state))
    return " ".join(parts)


def text_queue(task: str, texts: list[str] | None) -> list[str]:
    """Return the only strings the agent may type, in consumption order.

    Repeated ``--text`` values are used as given. When that list is empty,
    quoted spans are taken from the task from left to right.
    """
    if texts:
        return [str(item) for item in texts if item is not None]
    found: list[str] = []
    for match in _QUOTED.finditer(task or ""):
        found.append(match.group(1) if match.group(1) is not None else match.group(2))
    return found


def take_text(queue: list[str]) -> str | None:
    """Pop the next supplied string, or None when nothing supplied remains."""
    if not queue:
        return None
    return queue.pop(0)


def assess(action: Any, element: Any) -> dict:
    """Classify a step against the desktop destructive taxonomy.

    The result is ``{"destructive": bool, "reasons": [...]}``. Reasons cover
    file delete or trash, settings changes, sending or posting, quitting with
    unsaved work, and credential or payment fields.
    """
    blob = _blob(action, element)
    role = _norm_role(str(element.get("role") or "")) if isinstance(element, dict) else ""
    key = str(action.get("key") or "").lower() if isinstance(action, dict) else ""
    reasons: list[str] = []
    if _DELETE.search(blob) or (key in {"delete", "backspace"} and _FILE_WINDOW.search(blob)):
        reasons.append("file_delete")
    if _SETTINGS_LABEL.search(blob) or (role in _TOGGLE_ROLES and _SETTINGS_PLACE.search(blob)):
        reasons.append("settings_change")
    if _SEND.search(blob):
        reasons.append("send_or_post")
    if role in _SECRET_ROLES or _SECRET.search(blob):
        reasons.append("credential_or_payment")
    quit_unsaved = key in _QUIT_KEYS or bool(_DISCARD.search(blob))
    if not quit_unsaved and _QUIT_LABEL.search(blob) and _UNSAVED.search(blob):
        quit_unsaved = True
    if quit_unsaved:
        reasons.append("quit_unsaved")
    return {"destructive": bool(reasons), "reasons": reasons}
