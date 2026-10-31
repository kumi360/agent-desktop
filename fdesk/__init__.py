"""Autonomous desktop agent: Jev decides, agent-desktop acts, code gates."""

from __future__ import annotations

import re
import sys

__version__ = "0.1.0"

_BEARER = re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._\-+/=]{6,}")
_ASSIGNED = re.compile(
    r"(?i)\b(api[_-]?key|token|secret|authorization|password)\b(\s*[=:]\s*)(\S+)"
)
_SK = re.compile(r"\bsk-[A-Za-z0-9_\-]{8,}\b")


def redact(text: str) -> str:
    """Mask bearer tokens, ``sk-`` keys, and secret-shaped assignments."""
    if not text:
        return ""
    cleaned = _BEARER.sub("Bearer [REDACTED]", text)
    cleaned = _SK.sub("sk-[REDACTED]", cleaned)
    return _ASSIGNED.sub(lambda match: f"{match.group(1)}{match.group(2)}[REDACTED]", cleaned)


def log(message: str) -> None:
    """Write one redacted diagnostic line to stderr."""
    sys.stderr.write(redact(str(message)).rstrip() + "\n")
    sys.stderr.flush()
