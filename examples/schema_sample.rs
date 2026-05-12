//! Emit a sample `Report` and `MultiReport` as JSON for schema validation.
//!
//! Exercises every variant the schema is supposed to accept: all four
//! `Verdict`s, every `Severity`, every `EvidenceData` variant, a check with
//! tags, a check with no tags, a `FileRef` with and without a line range,
//! a `MultiReport` aggregating two producers, and the empty-report case.
//!
//! Usage:
//!
//! ```text
//! cargo run --example schema_sample > target/sample_report.json
//! cargo run --example schema_sample -- multi > target/sample_multi.json
//! ```
//!
//! The output JSON validates against `schema/report.schema.json`. CI runs
//! the validator script in `scripts/validate_schema.py` against this output.

use chrono::TimeZone;
use dev_report::{
    CheckResult, Evidence, FileRef, MultiReport, Report, Severity,
};

fn build_report(producer: &str) -> Report {
    let frozen_start = chrono::Utc.with_ymd_and_hms(2026, 5, 11, 12, 0, 0).unwrap();
    let frozen_end = chrono::Utc.with_ymd_and_hms(2026, 5, 11, 12, 0, 5).unwrap();

    let mut r = Report::new("sample-subject", "0.9.3").with_producer(producer);
    r.set_started_at(frozen_start);

    // Pass with no detail, no severity, no tags, no evidence.
    let mut c1 = CheckResult::pass("compile");
    c1.at = frozen_start;
    r.push(c1);

    // Pass with a duration and a single numeric evidence.
    let mut c2 = CheckResult::pass("bench::parse")
        .with_duration_ms(7)
        .with_tag("bench")
        .with_evidence(Evidence::numeric("mean_ns", 1234.5))
        .with_evidence(Evidence::numeric_int("iterations", 1_000_000));
    c2.at = frozen_start;
    r.push(c2);

    // Warn with key-value evidence.
    let mut c3 = CheckResult::warn("env::leaked", Severity::Warning)
        .with_detail("RUST_LOG was set during test")
        .with_evidence(Evidence::kv(
            "env",
            [("CI", "true"), ("RUST_LOG", "debug")],
        ));
    c3.at = frozen_start;
    r.push(c3);

    // Fail with snippet + file_ref (no line range) + file_ref (with line range).
    let mut c4 = CheckResult::fail("test::round_trip", Severity::Error)
        .with_detail("expected 42, got 41")
        .with_duration_ms(13)
        .with_tags(["unit", "flaky"])
        .with_evidence(Evidence::snippet("panic", "assertion `left == right` failed"))
        .with_evidence(Evidence::file_ref("source", "src/math.rs"))
        .with_evidence(Evidence::file_ref_lines("call_site", "tests/smoke.rs", 42, 47));
    c4.at = frozen_start;
    r.push(c4);

    // Fail with Critical severity.
    let mut c5 = CheckResult::fail("integration::startup", Severity::Critical)
        .with_detail("service refused to start");
    c5.at = frozen_start;
    r.push(c5);

    // Warn with Info severity (lowest severity, still warn verdict).
    let mut c6 = CheckResult::warn("style::trailing_ws", Severity::Info)
        .with_detail("3 trailing-whitespace warnings");
    c6.at = frozen_start;
    r.push(c6);

    // Skip with no severity.
    let mut c7 = CheckResult::skip("integration::network")
        .with_detail("no network in sandbox");
    c7.at = frozen_start;
    r.push(c7);

    // Manually-constructed FileRef via Evidence::file_ref (covered above);
    // also exercise a standalone FileRef with line_start but no line_end —
    // valid per the schema (both are optional independently).
    let mut c8 = CheckResult::pass("doc::link_check");
    c8.at = frozen_start;
    let standalone = FileRef::new("docs/index.md").with_line_range(10, 10);
    c8 = c8.with_evidence(Evidence {
        label: "anchor".into(),
        data: dev_report::EvidenceData::FileRef(standalone),
    });
    r.push(c8);

    r.set_finished_at(Some(frozen_end));
    r
}

fn build_multi() -> MultiReport {
    let frozen_start = chrono::Utc.with_ymd_and_hms(2026, 5, 11, 12, 0, 0).unwrap();
    let frozen_end = chrono::Utc.with_ymd_and_hms(2026, 5, 11, 12, 0, 10).unwrap();
    let mut m = MultiReport::new("sample-subject", "0.9.3");
    m.started_at = frozen_start;
    m.push(build_report("dev-bench"));
    m.push(build_report("dev-chaos"));
    m.finished_at = Some(frozen_end);
    m
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "report".to_string());
    let json = match arg.as_str() {
        "multi" => build_multi().to_json().expect("serialize MultiReport"),
        _ => build_report("dev-bench").to_json().expect("serialize Report"),
    };
    println!("{}", json);
}
