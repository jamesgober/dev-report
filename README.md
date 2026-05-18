<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <strong>dev-report</strong>
    <br>
    <sup><sub>STRUCTURED VERIFICATION REPORTS FOR RUST</sub></sup>
</h1>
<p align="center">
    <a href="https://crates.io/crates/dev-report"><img alt="crates.io" src="https://img.shields.io/crates/v/dev-report.svg"></a>
    <a href="https://crates.io/crates/dev-report"><img alt="downloads" src="https://img.shields.io/crates/d/dev-report.svg"></a>
    <a href="https://github.com/jamesgober/dev-report/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/jamesgober/dev-report/actions/workflows/ci.yml/badge.svg"></a>
    <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.75%2B-blue.svg?style=flat-square" title="Rust Version">
    <a href="https://docs.rs/dev-report"><img alt="docs.rs" src="https://docs.rs/dev-report/badge.svg"></a>
</p>

<p align="center">
    <strong>A versioned JSON schema for Rust verification output.</strong> Produce it, consume it, diff it — no log scraping, no colored checkmarks.
</p>

<br>

<div align="center">
    <strong>Foundation of the <a href="https://crates.io/crates/dev-tools"><code>dev-*</code></a> verification collection.</strong><br>
    <sub>Every other crate in the suite emits reports in this schema. Pull <code>dev-report</code> directly to consume them, or use the <a href="https://crates.io/crates/dev-tools"><code>dev-tools</code></a> umbrella crate to drive the producers as well.</sub>
</div>

<br>

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
dev-report = "0.9.7"
```

Opt-in features:

```toml
[dependencies]
dev-report = { version = "0.9.6", features = ["terminal", "markdown"] }
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

## Tags and evidence

Tags filter checks by category. Evidence attaches typed, decision-grade
data without forcing the consumer to parse the free-form `detail` string.

```rust
use dev_report::{CheckResult, Evidence, Report};

let mut report = Report::new("my-crate", "0.2.0");

report.push(
    CheckResult::pass("bench::parse")
        .with_tag("bench")
        .with_duration_ms(7)
        .with_evidence(Evidence::numeric("mean_ns", 1234.5))
        .with_evidence(Evidence::numeric("baseline_ns", 1100.0))
        .with_evidence(Evidence::kv(
            "env",
            [("CI", "true"), ("RUST_LOG", "debug")],
        ))
        .with_evidence(Evidence::file_ref_lines("site", "src/parse.rs", 10, 20)),
);

// Filter by tag without parsing names.
let bench_checks: Vec<_> = report.checks_with_tag("bench").collect();
assert_eq!(bench_checks.len(), 1);
```

The four evidence kinds are:

| Kind       | Constructor                              | Use for                                    |
|------------|-------------------------------------------|---------------------------------------------|
| `Numeric`  | `Evidence::numeric(label, value)`         | A single labeled number (mean_ns, ops/sec). |
| `KeyValue` | `Evidence::kv(label, pairs)`              | String->string maps (env, config).          |
| `Snippet`  | `Evidence::snippet(label, text)`          | Short captured text (panic, log line).      |
| `FileRef`  | `Evidence::file_ref(label, path)` / `file_ref_lines(label, path, start, end)` | Pointer to a source location. |

Both `tags` and `evidence` are additive: v0.1.0 reports deserialize as
v0.9.x reports with empty collections, and reports with no tags or
evidence omit those keys from the JSON output.

## Diffing two reports

Compare a current run against a baseline to flag regressions:

```rust
use dev_report::{CheckResult, DiffOptions, Report, Severity};

let mut baseline = Report::new("crate", "0.1.0");
baseline.push(CheckResult::pass("hot_path").with_duration_ms(100));

let mut current = Report::new("crate", "0.1.0");
current.push(CheckResult::fail("hot_path", Severity::Error).with_duration_ms(200));
current.push(CheckResult::pass("new_check"));

let diff = current.diff_with(
    &baseline,
    &DiffOptions {
        duration_regression_pct: Some(20.0),
        duration_regression_abs_ms: None,
    },
);

assert_eq!(diff.newly_failing, vec!["hot_path".to_string()]);
assert_eq!(diff.added, vec!["new_check".to_string()]);
assert!(!diff.is_clean());
```

`Diff` exposes `newly_failing`, `newly_passing`, `severity_changes`,
`duration_regressions`, `added`, `removed`. All vectors are sorted by
check name so two diffs of the same input pair are byte-equal.

## Aggregating multiple producers

A single CI run usually invokes several producers (`dev-bench`,
`dev-fixtures`, `dev-async`, ...). `MultiReport` aggregates them while
preserving each check's `(producer, name)` identity:

```rust
use dev_report::{CheckResult, MultiReport, Report, Severity};

let mut bench = Report::new("crate", "0.1.0").with_producer("dev-bench");
bench.push(CheckResult::pass("hot_path"));

let mut chaos = Report::new("crate", "0.1.0").with_producer("dev-chaos");
chaos.push(CheckResult::fail("recover", Severity::Critical));

let mut multi = MultiReport::new("crate", "0.1.0");
multi.push(bench);
multi.push(chaos);
multi.finish();

let json = multi.to_json().unwrap();
```

## Output formats

Four opt-in features render a `Report` (and `MultiReport`) into other
formats. All are pure functions; JSON remains the only round-trippable
wire format.

- `terminal` — `to_terminal` / `to_terminal_color` (ANSI). 80-column
  friendly. No new dependencies.
- `markdown` — `to_markdown` emits a CommonMark-compatible document
  preserving every fact (verdict, severity, tags, evidence, durations).
  No new dependencies.
- `sarif` — `to_sarif` emits a SARIF 2.1.0 document. Only `Fail` and
  `Warn` checks are included (SARIF is a defect-report format).
  Severity maps to SARIF `level`: `Critical`/`Error` → `error`,
  `Warning` → `warning`, `Info` → `note`. `Evidence::FileRef` becomes
  a SARIF `physicalLocation`. No new dependencies.
- `junit` — `to_junit_xml` emits a Jenkins/Surefire JUnit XML
  document. Every check becomes a `<testcase>`; fails get a
  `<failure>` child, skips get a `<skipped/>` child, warns are emitted
  as passing testcases (JUnit has no native warn representation; use
  SARIF for warns). No new dependencies.

## Verdict rules

Computed by `Report::overall_verdict()`:

| Condition | Overall verdict |
|---|---|
| Any check is `Fail` | `Fail` |
| Else any check is `Warn` | `Warn` |
| Else any check is `Pass` | `Pass` |
| Else (all `Skip` or empty) | `Skip` |

## Wire format and JSON Schema

The canonical wire format for both `Report` and `MultiReport` is JSON. A
JSON Schema document (Draft 2020-12) describing every field is shipped in
the crate at [`schema/report.schema.json`](schema/report.schema.json).

The schema is the contract for cross-language consumers: a TypeScript /
Python / Go / `jq` user can write tooling against a `Report` without
touching Rust. The schema covers all of `Report`, `MultiReport`,
`CheckResult`, `Verdict`, `Severity`, `Evidence`, `EvidenceData`, and
`FileRef`, with field-level descriptions.

CI validates a generated sample against the schema on every run via
`scripts/validate_schema.py` and the `schema_sample` example.

## The `dev-*` collection

`dev-report` is the foundation. Every other crate in the collection
produces reports in this schema:

- [`dev-tools`](https://crates.io/crates/dev-tools) — umbrella crate with feature gates over the whole collection
- [`dev-fixtures`](https://crates.io/crates/dev-fixtures) — test environments and sample data
- [`dev-bench`](https://crates.io/crates/dev-bench) — performance measurement and regression detection
- [`dev-async`](https://crates.io/crates/dev-async) — async-specific validation
- [`dev-stress`](https://crates.io/crates/dev-stress) — high-load stress testing
- [`dev-chaos`](https://crates.io/crates/dev-chaos) — failure injection and recovery testing
- [`dev-coverage`](https://crates.io/crates/dev-coverage) — code coverage with regression gates
- [`dev-security`](https://crates.io/crates/dev-security) — CVE / license / banned-crate audit
- [`dev-deps`](https://crates.io/crates/dev-deps) — unused / outdated dependency detection
- [`dev-ci`](https://crates.io/crates/dev-ci) — GitHub Actions workflow generator (also ships a CLI binary)
- [`dev-fuzz`](https://crates.io/crates/dev-fuzz) — fuzz testing workflow over `cargo-fuzz`
- [`dev-flaky`](https://crates.io/crates/dev-flaky) — flaky-test detection across N-iteration runs
- [`dev-mutate`](https://crates.io/crates/dev-mutate) — mutation testing via `cargo-mutants`

Pick the crates you need individually, or pull
[`dev-tools`](https://crates.io/crates/dev-tools) and opt in to each
verification layer with a feature flag.

## Status

`v0.9.x` is the pre-1.0 stabilization line. The schema is at
`schema_version = 1` and stays there through this line. Minor
additions remain possible before `1.0`; the `1.0` release will pin
the schema and follow strict semver.

## Minimum supported Rust version

`1.85` — pinned in `Cargo.toml` via `rust-version` and verified by
the MSRV job in CI. (Bumped from 1.75 because transitive dependencies
in the suite require `edition2024`, stabilized in Rust 1.85.)

## License

Apache-2.0. See [LICENSE](LICENSE).




<!-- COPYRIGHT
---------------------------------->
<div align="center">
    <br>
    <h2></h2>
    Copyright &copy; 2026 James Gober.
</div>
