"""Run the fdesk unit-test suite."""

from __future__ import annotations

import sys
import unittest


def main() -> int:
    loader = unittest.TestLoader()
    suite = loader.discover("fdesk.tests", pattern="test_*.py")
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
