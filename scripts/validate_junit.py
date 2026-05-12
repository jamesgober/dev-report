#!/usr/bin/env python3
"""Round-trip a JUnit XML document through a parser and verify structure.

Parses the file with `xml.etree.ElementTree` and asserts:

- Root element is `testsuites`
- Every `testsuite` has a `name` attribute
- Every `testcase` has both `name` and `classname` attributes
- Reported `tests`/`failures`/`skipped` counts on `testsuite` agree with
  the child `<failure>` and `<skipped>` element counts
- `<failure>` elements declare a `type` attribute

Usage: `python scripts/validate_junit.py FILE [FILE ...]`
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path


class JUnitError(Exception):
    pass


def _expect(cond: bool, msg: str) -> None:
    if not cond:
        raise JUnitError(msg)


def _check_testcase(tc: ET.Element, where: str) -> tuple[int, int]:
    _expect("name" in tc.attrib, f"{where}: testcase missing name")
    _expect("classname" in tc.attrib, f"{where}: testcase missing classname")
    failures = 0
    skips = 0
    for child in tc:
        if child.tag == "failure":
            _expect("type" in child.attrib, f"{where}: failure missing type")
            failures += 1
        elif child.tag == "skipped":
            skips += 1
    return failures, skips


def _check_testsuite(ts: ET.Element, where: str) -> None:
    _expect("name" in ts.attrib, f"{where}: testsuite missing name")
    declared_tests = int(ts.attrib.get("tests", "0"))
    declared_failures = int(ts.attrib.get("failures", "0"))
    declared_skipped = int(ts.attrib.get("skipped", "0"))

    actual_tests = 0
    actual_failures = 0
    actual_skips = 0
    for i, tc in enumerate(ts.findall("testcase")):
        actual_tests += 1
        f, s = _check_testcase(tc, f"{where}.testcase[{i}]")
        actual_failures += f
        actual_skips += s

    _expect(
        actual_tests == declared_tests,
        f"{where}: declared tests={declared_tests} but found {actual_tests} testcases",
    )
    _expect(
        actual_failures == declared_failures,
        f"{where}: declared failures={declared_failures} but found {actual_failures} failure elements",
    )
    _expect(
        actual_skips == declared_skipped,
        f"{where}: declared skipped={declared_skipped} but found {actual_skips} skipped elements",
    )


def validate(root: ET.Element) -> None:
    _expect(root.tag == "testsuites", f"root must be <testsuites>, got <{root.tag}>")
    for i, ts in enumerate(root.findall("testsuite")):
        _check_testsuite(ts, f"testsuites.testsuite[{i}]")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    any_failed = False
    for path in argv[1:]:
        try:
            tree = ET.parse(Path(path))
            validate(tree.getroot())
            print(f"OK:   {path}")
        except (ET.ParseError, JUnitError) as e:
            any_failed = True
            print(f"FAIL: {path}: {e}", file=sys.stderr)
    return 1 if any_failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
