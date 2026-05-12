//! Build a sample `Report`, export it as JUnit XML, and print to stdout.
//!
//! Requires the `junit` feature:
//!
//! ```text
//! cargo run --example junit_export --features junit > report.junit.xml
//! ```
//!
//! Demonstrates the verdict-to-element mapping: pass and warn produce
//! self-closing `<testcase>`, fail produces `<failure>` children, skip
//! produces `<skipped/>` children. Durations propagate as `time`
//! attributes in seconds.

use dev_report::{CheckResult, Report, Severity};

fn main() {
    let mut r = Report::new("sample-subject", "0.9.3").with_producer("dev-bench");

    r.push(CheckResult::pass("compile").with_duration_ms(120));
    r.push(CheckResult::pass("test::math").with_duration_ms(7));

    r.push(
        CheckResult::fail("test::round_trip", Severity::Error)
            .with_duration_ms(13)
            .with_detail("expected 42, got 41"),
    );

    r.push(
        CheckResult::fail("integration::startup", Severity::Critical)
            .with_detail("service refused to start"),
    );

    r.push(
        CheckResult::warn("style::trailing_ws", Severity::Warning)
            .with_detail("3 trailing-whitespace warnings"),
    );

    r.push(CheckResult::skip("integration::network").with_detail("no network in sandbox"));

    r.finish();

    println!("{}", r.to_junit_xml());
}
