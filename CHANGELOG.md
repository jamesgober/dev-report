# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.4] - 2026-05-12

Documentation and SEO pass. No code changes.

### Changed

- README header standardized to match the collection-wide template: Rust logo image, MSRV badge positioned between CI and docs.rs, copyright block at the bottom.
- Subtitle now reads `STRUCTURED VERIFICATION REPORTS FOR RUST` (was `STRUCTURED REPORTS FOR AI-ASSISTED RUST DEVELOPMENT`). The crate targets every Rust crate maintainer, not only AI tooling.
- Tagline rewritten to lead with what the schema *does* (produce / consume / diff) rather than its internal role.
- `## The dev-* suite` section retitled to `The dev-* collection` and expanded from 6 to 14 sibling crates (the full current suite). Crate links now point at crates.io.
- `Cargo.toml` `description` rewritten: leads with the actual feature set (JSON, versioned, with SARIF/JUnit/markdown/terminal output) instead of the AI-assisted framing.
- `Cargo.toml` `keywords` retuned: `testing`, `reporting`, `sarif`, `junit`, `ci` (was `testing`, `verification`, `reporting`, `ci`, `ai-tools`).

### Added

- "Foundation of the `dev-*` verification collection" content block on the README, sized so the suite relationship is visible at a glance without crowding the intro.

[0.9.4]: https://github.com/jamesgober/dev-report/releases/tag/v0.9.4

## [0.9.3] - 2026-05-12

### Added

- JSON Schema document at `schema/report.schema.json` (Draft 2020-12) describing the wire format for `Report` and `MultiReport`. Field-level descriptions cover `Verdict`, `Severity`, `Evidence`, `EvidenceData`, `FileRef`, `CheckResult`, `Report`, and `MultiReport`. Lets TypeScript, Python, Go, and `jq` consumers validate or generate dev-report documents without touching Rust.
- `examples/schema_sample.rs` emits a JSON document exercising every schema variant. CI uses it as the validation sample.
- `scripts/validate_schema.py` validates JSON documents against `schema/report.schema.json` using the `jsonschema` Python package.
- New `sarif` feature: `Report::to_sarif()` and `MultiReport::to_sarif()` emit SARIF 2.1.0 documents. Only `Fail` and `Warn` checks are emitted; pass and skip are omitted (SARIF is a defect-report format). Severity maps to SARIF `level` (`Critical`/`Error` → `error`, `Warning` → `warning`, `Info` → `note`). `Evidence::FileRef` becomes a SARIF `physicalLocation` with optional line range. `MultiReport` emits one SARIF `run` per constituent producer. No new runtime dependencies; uses the existing `serde_json`.
- New `junit` feature: `Report::to_junit_xml()` and `MultiReport::to_junit_xml()` emit Jenkins/Surefire JUnit XML documents. Pass/Warn become self-closing `<testcase>`; Fail produces `<failure>` with a severity-derived `type` attribute; Skip produces `<skipped/>`. Durations propagate as `time` attributes in seconds. XML attribute and text values are escaped. No external dependencies.
- `examples/sarif_export.rs` and `examples/junit_export.rs` demonstrate each exporter.
- `scripts/validate_sarif.py` performs structural validation of the SARIF subset dev-report emits.
- `scripts/validate_junit.py` round-trips JUnit XML through `xml.etree.ElementTree` and cross-checks declared counts against child element counts.

### Changed

- CI: `actions/checkout` bumped to `v5` (was `v4`); removes Node 20 deprecation warnings.
- CI: new `wire-format` job generates samples for every output format (JSON, SARIF, JUnit) and validates each.
- REPS.md § 7 now binds the JSON wire format to `schema/report.schema.json` as the canonical contract.
- No schema changes. `schema_version` stays at `1`. SARIF and JUnit exporters do not alter the JSON wire format.

[0.9.3]: https://github.com/jamesgober/dev-report/releases/tag/v0.9.3

## [0.9.2] - 2026-05-10

### Added

- `Diff::summary()` returns a one-line human-readable summary, e.g. `"clean"` or `"2 newly failing, 1 added"`. Useful for log output and CI status lines without parsing the full structure.
- `Report::set_started_at(ts)` and `Report::set_finished_at(ts)` setters for replay/import scenarios where the real timestamps are known but `Utc::now()` would be wrong.
- `Report::verdict_counts() -> (pass, fail, warn, skip)` for one-shot count aggregation.
- `MultiReport::iter_reports()` iterator over constituent reports.
- `MultiReport::report_from(producer_name)` lookup by producer.
- `MultiReport::verdict_counts()` aggregate across all constituent reports.

### Changed

- No schema changes. `schema_version` stays at `1`.

[0.9.2]: https://github.com/jamesgober/dev-report/releases/tag/v0.9.2

## [0.9.1] - 2026-05-09

### Added

- `Report::passed()` / `failed()` / `warned()` / `skipped()` shortcut methods (and the same set on `MultiReport`).
- `Report::checks_with_severity(severity)` and `MultiReport::checks_with_severity(severity)` filter iterators.
- `Evidence::numeric_int(label, i64)` constructor for integer counters that may exceed `f64`'s 53-bit exact range.
- `Diff::to_terminal()` / `to_terminal_color()` / `to_markdown()` rendering methods (gated by the `terminal` and `markdown` features respectively).
- `MultiReport::to_terminal()` / `to_terminal_color()` / `to_markdown()` rendering methods.
- New free functions in the `terminal` and `markdown` modules: `diff_to_terminal`, `diff_to_terminal_color`, `multi_to_terminal`, `multi_to_terminal_color`, `diff_to_markdown`, `multi_to_markdown`.

### Changed

- MSRV remains `1.85`. No schema changes; `schema_version` stays at `1`.

### Fixed

- Broken intra-doc link `[`alloc`]` referenced from the crate-level docstring of dev-bench would warn under `cargo doc` when the `alloc-tracking` feature is disabled. The link is now a plain code span. Mirrors the same fix in dev-async, dev-stress, dev-chaos.

[0.9.1]: https://github.com/jamesgober/dev-report/releases/tag/v0.9.1

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

[Unreleased]: https://github.com/jamesgober/dev-report/compare/v0.9.3...HEAD
[0.1.0]: https://github.com/jamesgober/dev-report/releases/tag/v0.1.0
