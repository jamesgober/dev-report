//! Build a sample `Report`, export it as SARIF 2.1.0, and print to stdout.
//!
//! Requires the `sarif` feature:
//!
//! ```text
//! cargo run --example sarif_export --features sarif > findings.sarif.json
//! ```
//!
//! Demonstrates the severity-to-level mapping and `Evidence::FileRef` →
//! SARIF `physicalLocation` conversion. Pass and skip checks are absent
//! from the output (SARIF reports defects, not test results).

use dev_report::{CheckResult, Evidence, Report, Severity};

fn main() {
    let mut r = Report::new("sample-subject", "0.9.3").with_producer("dev-bench");

    r.push(CheckResult::pass("compile"));
    r.push(CheckResult::skip("network"));

    r.push(
        CheckResult::fail("test::round_trip", Severity::Error)
            .with_detail("expected 42, got 41")
            .with_evidence(Evidence::file_ref_lines("site", "src/math.rs", 10, 12)),
    );

    r.push(
        CheckResult::fail("integration::startup", Severity::Critical)
            .with_detail("service refused to start")
            .with_evidence(Evidence::file_ref("source", "src/bin/server.rs")),
    );

    r.push(
        CheckResult::warn("style::trailing_ws", Severity::Warning)
            .with_detail("3 trailing-whitespace warnings"),
    );

    r.finish();

    println!("{}", r.to_sarif());
}
