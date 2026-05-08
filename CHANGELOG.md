# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/jamesgober/dev-report/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jamesgober/dev-report/releases/tag/v0.1.0
