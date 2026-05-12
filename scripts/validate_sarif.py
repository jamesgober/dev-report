#!/usr/bin/env python3
"""Structural validation for SARIF 2.1.0 documents emitted by dev-report.

Checks the fields dev-report actually emits, not the full SARIF spec. The
goal is to catch shape regressions in CI without depending on the heavy
SARIF JSON Schema.

Verifies:
- Top-level `version` == "2.1.0"
- Top-level `runs` is a list
- Each run has `tool.driver.name` (string)
- Each run has `results` (list)
- Each result has `ruleId` (string), `level` (one of "none", "note",
  "warning", "error"), `message.text` (string)
- Each `physicalLocation` (if present) has `artifactLocation.uri`

Usage: `python scripts/validate_sarif.py FILE [FILE ...]`
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

VALID_LEVELS = {"none", "note", "warning", "error"}


class SarifError(Exception):
    pass


def _expect(cond: bool, msg: str) -> None:
    if not cond:
        raise SarifError(msg)


def _validate_location(loc: dict, where: str) -> None:
    phys = loc.get("physicalLocation")
    if phys is None:
        return
    art = phys.get("artifactLocation") or {}
    _expect(isinstance(art.get("uri"), str), f"{where}: missing artifactLocation.uri")
    region = phys.get("region")
    if region is not None:
        for key in ("startLine", "endLine"):
            if key in region:
                _expect(
                    isinstance(region[key], int) and region[key] >= 1,
                    f"{where}: region.{key} must be a positive integer",
                )


def _validate_result(result: dict, where: str) -> None:
    _expect(isinstance(result.get("ruleId"), str), f"{where}: missing ruleId")
    level = result.get("level")
    _expect(level in VALID_LEVELS, f"{where}: level {level!r} not in {sorted(VALID_LEVELS)}")
    msg = result.get("message") or {}
    _expect(isinstance(msg.get("text"), str), f"{where}: missing message.text")
    locations = result.get("locations", [])
    _expect(isinstance(locations, list), f"{where}: locations must be a list")
    for i, loc in enumerate(locations):
        _validate_location(loc, f"{where}.locations[{i}]")


def _validate_run(run: dict, where: str) -> None:
    tool = run.get("tool") or {}
    driver = tool.get("driver") or {}
    _expect(isinstance(driver.get("name"), str), f"{where}: missing tool.driver.name")
    results = run.get("results")
    _expect(isinstance(results, list), f"{where}: results must be a list")
    for i, result in enumerate(results):
        _validate_result(result, f"{where}.results[{i}]")


def validate(doc: dict) -> None:
    _expect(doc.get("version") == "2.1.0", f"version must be \"2.1.0\", got {doc.get('version')!r}")
    runs = doc.get("runs")
    _expect(isinstance(runs, list), "runs must be a list")
    for i, run in enumerate(runs):
        _validate_run(run, f"runs[{i}]")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    any_failed = False
    for path in argv[1:]:
        try:
            doc = json.loads(Path(path).read_text(encoding="utf-8"))
            validate(doc)
            print(f"OK:   {path}")
        except (json.JSONDecodeError, SarifError) as e:
            any_failed = True
            print(f"FAIL: {path}: {e}", file=sys.stderr)
    return 1 if any_failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
