#!/usr/bin/env python3
"""Validate one or more JSON documents against the dev-report schema.

Used by CI to confirm the schema file (`schema/report.schema.json`) and the
Rust types in `src/lib.rs` agree on the wire format. The Rust example
`schema_sample` emits documents exercising every variant; this script then
validates them against the schema.

Usage:

    python scripts/validate_schema.py SCHEMA DOC [DOC ...]

Exit code: 0 on success, 1 on any validation failure.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
    from jsonschema.exceptions import ValidationError
except ImportError:
    print(
        "missing dependency: pip install jsonschema",
        file=sys.stderr,
    )
    sys.exit(2)


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2

    schema_path = Path(argv[1])
    doc_paths = [Path(p) for p in argv[2:]]

    schema = load_json(schema_path)
    validator = Draft202012Validator(schema)

    # Sanity-check the schema itself before validating documents.
    Draft202012Validator.check_schema(schema)

    any_failed = False
    for doc_path in doc_paths:
        doc = load_json(doc_path)
        errors = sorted(validator.iter_errors(doc), key=lambda e: e.path)
        if errors:
            any_failed = True
            print(f"FAIL: {doc_path}", file=sys.stderr)
            for err in errors:
                location = "/".join(str(p) for p in err.absolute_path) or "<root>"
                print(f"  at {location}: {err.message}", file=sys.stderr)
        else:
            print(f"OK:   {doc_path}")

    return 1 if any_failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
