"""Command line for the fdesk prototype.

The final Result JSON goes to stdout. Step messages go to stderr through
``log``. The process exits 0 only when the loop finishes on DONE.
"""

from __future__ import annotations

import argparse
import json
import sys

from fdesk.jev import JevClient
from fdesk.loop import MAX_STEPS, run


def parser() -> argparse.ArgumentParser:
    """Build the ``python -m fdesk`` argument parser."""
    cli = argparse.ArgumentParser(prog="fdesk", description="Jev-driven desktop agent loop over agent-desktop.")
    cli.add_argument("--app", help="Application name, for example TextEdit")
    cli.add_argument("--task", help="Task the loop should finish")
    cli.add_argument("--text", action="append", default=[], help="Caller-supplied text, consumed in order. Repeatable.")
    cli.add_argument("--authorize", action="store_true", help="Lift ordinary confidence gates. Destructive steps stay gated.")
    cli.add_argument("--max-steps", type=int, default=MAX_STEPS, help="Step budget (default 40)")
    cli.add_argument("--window-id", help="Pin one window when the app has several")
    cli.add_argument("--self-test", action="store_true", help="Send one noul question to Jev and print the answer")
    return cli


def main(argv: list[str] | None = None) -> int:
    """Parse arguments, run one task or the Jev self-test, and return an exit code."""
    args = parser().parse_args(argv)
    if args.self_test:
        outcome = JevClient().self_test()
        if outcome.get("ok") and isinstance(outcome.get("data"), dict):
            shown = outcome["data"].get("answers", outcome["data"])
        else:
            shown = outcome
        sys.stdout.write(json.dumps(shown, indent=2) + "\n")
        return 0 if outcome.get("ok") else 1
    if not args.app or not args.task:
        parser().error("--app and --task are required")
    result = run(
        args.task,
        args.app,
        texts=args.text or None,
        authorize=args.authorize,
        max_steps=args.max_steps,
        window_id=args.window_id,
    )
    sys.stdout.write(json.dumps(result, indent=2) + "\n")
    return 0 if result.get("ok") and result.get("status") == "done" else 1
