<h1 align="center">
    <strong>dev-report</strong>
    <br>
    <sup><sub>STRUCTURED REPORTS FOR AI-ASSISTED RUST DEVELOPMENT</sub></sup>
</h1>

<p align="center">
    <a href="https://crates.io/crates/dev-report"><img alt="crates.io" src="https://img.shields.io/crates/v/dev-report.svg"></a>
    <a href="https://docs.rs/dev-report"><img alt="docs.rs" src="https://docs.rs/dev-report/badge.svg"></a>
    <a href="https://github.com/jamesgober/dev-report/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
</p>

<p align="center">
    Foundation schema of the <code>dev-*</code> verification suite.
</p>

---

## What it does

`dev-report` defines the report format every other crate in the `dev-*`
suite emits. AI agents need machine-readable evidence of what passed,
what failed, and why. This crate provides that schema.

## Why it exists

A test runner that prints colored checkmarks to a TTY is unreadable to an
AI agent. The agent needs:

- A stable, versioned schema
- Verdicts separated from logs
- Enough evidence to decide accept / reject / retry / escalate

`dev-report` is that schema.

## Quick start

Add to `Cargo.toml`:

```toml
[dependencies]
dev-report = "0.1"
```

Build a report:

```rust
use dev_report::{Report, Verdict, Severity, CheckResult};

let mut report = Report::new("my-crate", "0.1.0")
    .with_producer("my-harness");

report.push(CheckResult::pass("compile"));
report.push(CheckResult::pass("test::unit").with_duration_ms(42));
report.push(
    CheckResult::fail("test::round_trip", Severity::Error)
        .with_detail("expected 42, got 41")
);

report.finish();

let verdict = report.overall_verdict();   // Verdict::Fail
let json = report.to_json().unwrap();     // ready to write to disk or stdout
```

## Verdict rules

Computed by `Report::overall_verdict()`:

| Condition | Overall verdict |
|---|---|
| Any check is `Fail` | `Fail` |
| Else any check is `Warn` | `Warn` |
| Else any check is `Pass` | `Pass` |
| Else (all `Skip` or empty) | `Skip` |

## The `dev-*` suite

`dev-report` is the foundation. The other crates produce reports in this
schema:

- [`dev-fixtures`](https://github.com/jamesgober/dev-fixtures) - test environments and sample data
- [`dev-bench`](https://github.com/jamesgober/dev-bench) - performance measurement and regression detection
- [`dev-async`](https://github.com/jamesgober/dev-async) - async-specific validation
- [`dev-stress`](https://github.com/jamesgober/dev-stress) - high-load stress testing
- [`dev-chaos`](https://github.com/jamesgober/dev-chaos) - failure injection and recovery testing
- [`dev-tools`](https://github.com/jamesgober/dev-tools) - umbrella crate with feature gates

## Status

Pre-release. The schema MAY change between `0.x` versions. The `1.0`
release will pin the schema and follow strict semver.

## License

Apache-2.0. See [LICENSE](LICENSE).
