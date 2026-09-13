#!/usr/bin/env python3

import argparse
import json
import sys


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()

    summary = json.load(sys.stdin)
    total = summary.get("totalTestCount")
    passed = summary.get("passedTests")
    failed = summary.get("failedTests")
    skipped = summary.get("skippedTests")
    result = summary.get("result")

    counts = (total, passed, failed, skipped)
    if not all(isinstance(count, int) and not isinstance(count, bool) for count in counts):
        raise SystemExit("the xcresult test summary has invalid counts")
    if total < 1 or passed < 1:
        raise SystemExit("the selected iOS test did not execute")
    if failed != 0 or result != "Passed":
        raise SystemExit("the iOS XCTest result did not pass")
    if args.live and skipped != 0:
        raise SystemExit("a Wcash iOS live integration test was skipped")

    print(f"Verified XCTest result: {passed} passed, {skipped} skipped, {failed} failed")


if __name__ == "__main__":
    main()
