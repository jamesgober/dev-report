use dev_report::{
    CheckResult, DiffOptions, Evidence, EvidenceKind, MultiReport, Report, Severity, Verdict,
};

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

#[test]
fn smoke_tags_and_evidence_roundtrip() {
    let mut r = Report::new("smoke-test", "0.2.0");
    r.push(
        CheckResult::pass("bench::parse")
            .with_tag("bench")
            .with_duration_ms(7)
            .with_evidence(Evidence::numeric("mean_ns", 1234.5))
            .with_evidence(Evidence::numeric("baseline_ns", 1100.0))
            .with_evidence(Evidence::kv("env", [("CI", "true"), ("RUST_LOG", "debug")]))
            .with_evidence(Evidence::file_ref_lines("site", "src/parse.rs", 10, 20)),
    );
    r.push(
        CheckResult::fail("chaos::recover", Severity::Critical)
            .with_tags(["chaos", "recovery"])
            .with_detail("recovery did not restore final state")
            .with_evidence(Evidence::snippet(
                "context",
                "expected=2 observed=2 final_state_ok=false",
            )),
    );
    r.finish();

    let json = r.to_json().unwrap();
    let parsed = Report::from_json(&json).unwrap();

    assert_eq!(parsed.overall_verdict(), Verdict::Fail);
    assert_eq!(parsed.checks.len(), 2);

    let bench: Vec<_> = parsed.checks_with_tag("bench").collect();
    assert_eq!(bench.len(), 1);
    assert_eq!(bench[0].name, "bench::parse");
    assert_eq!(bench[0].evidence.len(), 4);
    assert_eq!(bench[0].evidence[3].kind(), EvidenceKind::FileRef);

    let chaos: Vec<_> = parsed.checks_with_tag("chaos").collect();
    assert_eq!(chaos.len(), 1);
    assert!(chaos[0].has_tag("recovery"));
}

#[test]
fn smoke_diff_flags_regression() {
    let mut prev = Report::new("c", "0.1.0");
    prev.push(CheckResult::pass("compile"));
    prev.push(CheckResult::pass("hot_path").with_duration_ms(100));
    prev.finish();

    let mut curr = Report::new("c", "0.1.0");
    curr.push(CheckResult::fail("compile", Severity::Error));
    curr.push(CheckResult::pass("hot_path").with_duration_ms(200));
    curr.push(CheckResult::pass("new_check"));
    curr.finish();

    let diff = curr.diff_with(
        &prev,
        &DiffOptions {
            duration_regression_pct: Some(20.0),
            duration_regression_abs_ms: None,
        },
    );
    assert!(!diff.is_clean());
    assert_eq!(diff.newly_failing, vec!["compile".to_string()]);
    assert_eq!(diff.added, vec!["new_check".to_string()]);
    assert_eq!(diff.duration_regressions.len(), 1);
    assert_eq!(diff.duration_regressions[0].name, "hot_path");
}

#[test]
fn smoke_multi_report_aggregation() {
    let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
    bench.push(CheckResult::pass("hot_path").with_duration_ms(50));
    bench.finish();

    let mut chaos = Report::new("c", "0.1.0").with_producer("dev-chaos");
    chaos.push(CheckResult::fail("recover", Severity::Critical));
    chaos.finish();

    let mut multi = MultiReport::new("c", "0.1.0");
    multi.push(bench);
    multi.push(chaos);
    multi.finish();

    assert_eq!(multi.overall_verdict(), Verdict::Fail);
    assert_eq!(multi.total_check_count(), 2);

    let json = multi.to_json().unwrap();
    let parsed = MultiReport::from_json(&json).unwrap();
    assert_eq!(parsed.reports.len(), 2);
}

#[cfg(feature = "terminal")]
#[test]
fn smoke_terminal_render() {
    let mut r = Report::new("c", "0.1.0").with_producer("dev-bench");
    r.push(CheckResult::pass("a"));
    r.push(CheckResult::fail("b", Severity::Error));
    r.finish();
    let out = r.to_terminal();
    assert!(out.contains("[PASS]"));
    assert!(out.contains("[FAIL error]"));
    let colored = r.to_terminal_color();
    assert!(colored.contains('\x1b'));
}

#[cfg(feature = "markdown")]
#[test]
fn smoke_markdown_render() {
    let mut r = Report::new("c", "0.1.0").with_producer("dev-bench");
    r.push(CheckResult::pass("a"));
    r.finish();
    let md = r.to_markdown();
    assert!(md.starts_with("# Report:"));
    assert!(md.contains("**Overall verdict:** **PASS**"));
}
