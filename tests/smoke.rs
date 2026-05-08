use dev_report::{CheckResult, Report, Severity, Verdict};

#[test]
fn smoke_pass_path() {
    let mut r = Report::new("smoke-test", "0.1.0").with_producer("dev-report-smoke");
    r.push(CheckResult::pass("a"));
    r.push(CheckResult::pass("b").with_duration_ms(10));
    r.finish();

    assert_eq!(r.overall_verdict(), Verdict::Pass);
    assert_eq!(r.checks.len(), 2);
}

#[test]
fn smoke_fail_path() {
    let mut r = Report::new("smoke-test", "0.1.0");
    r.push(CheckResult::pass("a"));
    r.push(CheckResult::fail("b", Severity::Error).with_detail("bad output"));
    r.finish();

    assert_eq!(r.overall_verdict(), Verdict::Fail);
}

#[test]
fn smoke_json_roundtrip() {
    let mut r = Report::new("smoke-test", "0.1.0");
    r.push(CheckResult::warn("flaky", Severity::Warning));
    r.push(CheckResult::skip("not_applicable"));
    r.finish();

    let json = r.to_json().unwrap();
    let parsed = Report::from_json(&json).unwrap();
    assert_eq!(parsed.subject, "smoke-test");
    assert_eq!(parsed.checks.len(), 2);
    assert_eq!(parsed.overall_verdict(), Verdict::Warn);
}
