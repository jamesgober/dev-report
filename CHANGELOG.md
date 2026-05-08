# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0] - 2026-05-08

### Added

#### CheckResult: tags and structured evidence

- `tags: Vec<String>` on `CheckResult` for category filtering. Defaults to empty.
- `evidence: Vec<Evidence>` on `CheckResult` for structured backing data. Defaults to empty.
- `Evidence` struct with four payload variants via `EvidenceData`:
  - `Numeric(f64)` for single labeled measurements.
  - `KeyValue(BTreeMap<String, String>)` for environment/config dumps (deterministic JSON ordering).
  - `Snippet(String)` for short text captures.
  - `FileRef(FileRef)` for source pointers, optionally with a `[line_start, line_end]` range.
- `EvidenceKind` enum returned by `Evidence::kind` for matching on payload shape without inspecting `data`.
- `FileRef` struct with `new` and `with_line_range` builders.
- `Evidence` constructors: `numeric`, `kv`, `snippet`, `file_ref`, `file_ref_lines`.
- `CheckResult` builders: `with_severity`, `with_tag`, `with_tags`, `with_evidence`, `with_evidences`.
- `CheckResult::has_tag(&str) -> bool`.
- `Report::checks_with_tag(&str)` iterator filter.

#### Output formats (opt-in features)

- `terminal` feature: `Report::to_terminal` (monochrome) and `Report::to_terminal_color` (ANSI-colored) pretty-printers. 80-column friendly. No new dependencies.
- `markdown` feature: `Report::to_markdown` exports a CommonMark-compatible Markdown document preserving every fact in the report. No new dependencies.

#### Aggregation and diffing

- `Report::diff(&baseline) -> Diff` for default-options diffing (20% duration regression threshold).
- `Report::diff_with(&baseline, &DiffOptions) -> Diff` for custom diffing thresholds.
- `Diff` struct surfacing: `newly_failing`, `newly_passing`, `severity_changes`, `duration_regressions`, `added`, `removed`. All vectors sorted alphabetically for determinism.
- `Diff::is_clean()` shortcut.
- `DiffOptions` with `duration_regression_pct` and `duration_regression_abs_ms` thresholds (either or both).
- `SeverityChange` and `DurationRegression` as serde-roundtrippable detail types.
- `MultiReport` aggregating multiple reports from different producers in a single CI run. Check identity is `(producer, name)`; same name across producers is kept separate. `MultiReport::overall_verdict` follows the same precedence as `Report::overall_verdict`.

#### Tests

- 14 explicit verdict-precedence transition-edge tests (DIRECTIVES § 7).
- v0.1.0 JSON deserialization regression test confirming backward compat.
- Round-trip tests for tags + evidence + diff.
- Pure-function tests for terminal and markdown renderers (same input → same output).
- 80-column-fit assertion on the terminal renderer for a typical report.

### Documentation

- REPS.md expanded: `CheckResult` fields, the full `Evidence` / `EvidenceData` / `EvidenceKind` / `FileRef` contract, the `terminal` and `markdown` feature contracts, and the `Diff` / `MultiReport` contracts.
- All public items now carry rustdoc examples (DIRECTIVES § 4 SHOULD-rule).

### Schema compatibility

- `schema_version` stays at `1`. All additions are purely additive.
- v0.1.0 JSON deserializes as a v0.9.0 `Report` with empty `tags` and `evidence` on every check.
- Empty `tags` and `evidence` are omitted on serialization, so a v0.9.0 report carrying neither serializes byte-equivalent to a v0.1.0-shaped JSON document.

[0.9.0]: https://github.com/jamesgober/dev-report/releases/tag/v0.9.0

## [0.1.0] - 2026-05-07

### Added

- Initial crate skeleton.
- `Verdict` enum: `Pass`, `Fail`, `Warn`, `Skip`.
- `Severity` enum: `Info`, `Warning`, `Error`, `Critical`.
- `CheckResult` struct with builder-style helpers (`pass`, `fail`, `warn`, `skip`, `with_detail`, `with_duration_ms`).
- `Report` struct with `push`, `finish`, `overall_verdict`, JSON round-trip.
- `Producer` trait for harness integration.
- Schema version field (`schema_version: u32`) on the report. Currently `1`.
- Round-trip and empty-report unit tests.

### Note

This is a name-claim release. The schema MAY change in subsequent `0.x` versions
as the rest of the `dev-*` suite is built and the contract gets exercised.

[Unreleased]: https://github.com/jamesgober/dev-report/compare/v0.9.0...HEAD
[0.1.0]: https://github.com/jamesgober/dev-report/releases/tag/v0.1.0
