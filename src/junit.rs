//! JUnit XML export for [`Report`] and [`MultiReport`].
//!
//! Conforms to the common Jenkins/Surefire JUnit XML schema. Every
//! [`CheckResult`] becomes a `<testcase>`; fail verdicts get a
//! `<failure>` child, skip verdicts get a `<skipped/>` child.
//!
//! ### Verdict → element mapping
//!
//! | [`Verdict`]       | XML form                                  |
//! |-------------------|-------------------------------------------|
//! | `Pass`            | `<testcase ... />`                        |
//! | `Warn`            | `<testcase ... />` (no child)             |
//! | `Fail`            | `<testcase ...><failure .../></testcase>` |
//! | `Skip`            | `<testcase ...><skipped/></testcase>`     |
//!
//! `Warn` has no native JUnit representation; it is emitted as a passing
//! testcase. Producers that want warns surfaced as findings should use
//! the SARIF exporter (see [`crate::sarif`]) instead. `<failure>` carries
//! a `type` attribute derived from [`Severity`] (`Error`, `Critical`).
//!
//! For a [`MultiReport`], each constituent [`Report`] becomes one
//! `<testsuite>`; the producer's name becomes the testsuite name and the
//! testcase `classname`.
//!
//! Available with the `junit` feature.
//!
//! [`Verdict`]: crate::Verdict
//! [`Severity`]: crate::Severity
//! [`CheckResult`]: crate::CheckResult

use std::fmt::Write;

use crate::{CheckResult, MultiReport, Report, Severity, Verdict};

/// Render `report` as a JUnit XML document.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report};
///
/// let mut r = Report::new("crate", "0.1.0").with_producer("dev-bench");
/// r.push(CheckResult::pass("compile"));
///
/// let xml = dev_report::junit::to_junit_xml(&r);
/// assert!(xml.starts_with("<?xml"));
/// assert!(xml.contains("<testsuite"));
/// assert!(xml.contains("name=\"compile\""));
/// ```
pub fn to_junit_xml(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let reports = std::slice::from_ref(report);
    write_root_open(&mut out, &report.subject, reports);
    write_testsuite(&mut out, report);
    out.push_str("</testsuites>\n");
    out
}

/// Render `multi` as a JUnit XML document with one `<testsuite>` per
/// constituent [`Report`].
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, MultiReport, Report};
///
/// let mut bench = Report::new("crate", "0.1.0").with_producer("dev-bench");
/// bench.push(CheckResult::pass("hot"));
/// let mut multi = MultiReport::new("crate", "0.1.0");
/// multi.push(bench);
///
/// let xml = dev_report::junit::multi_to_junit_xml(&multi);
/// assert!(xml.contains("<testsuite"));
/// ```
pub fn multi_to_junit_xml(multi: &MultiReport) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_root_open(&mut out, &multi.subject, &multi.reports);
    for r in &multi.reports {
        write_testsuite(&mut out, r);
    }
    out.push_str("</testsuites>\n");
    out
}

fn write_root_open(out: &mut String, suite_name: &str, reports: &[Report]) {
    let (pass, fail, warn, skip) = aggregate_counts(reports);
    let total = pass + fail + warn + skip;
    out.push_str("<testsuites");
    write_attr(out, "name", suite_name);
    write_attr(out, "tests", &total.to_string());
    write_attr(out, "failures", &fail.to_string());
    write_attr(out, "skipped", &skip.to_string());
    out.push_str(">\n");
}

fn aggregate_counts(reports: &[Report]) -> (usize, usize, usize, usize) {
    let (mut p, mut f, mut w, mut s) = (0usize, 0usize, 0usize, 0usize);
    for r in reports {
        let (rp, rf, rw, rs) = r.verdict_counts();
        p += rp;
        f += rf;
        w += rw;
        s += rs;
    }
    (p, f, w, s)
}

fn write_testsuite(out: &mut String, report: &Report) {
    let producer = report.producer.as_deref().unwrap_or("unknown");
    let (pass, fail, warn, skip) = report.verdict_counts();
    let total = pass + fail + warn + skip;
    let total_ms: u64 = report.checks.iter().filter_map(|c| c.duration_ms).sum();

    out.push_str("  <testsuite");
    write_attr(out, "name", producer);
    write_attr(out, "tests", &total.to_string());
    write_attr(out, "failures", &fail.to_string());
    write_attr(out, "skipped", &skip.to_string());
    write_attr(out, "time", &format_seconds(total_ms));
    out.push_str(">\n");

    for c in &report.checks {
        write_testcase(out, producer, c);
    }
    out.push_str("  </testsuite>\n");
}

fn write_testcase(out: &mut String, classname: &str, c: &CheckResult) {
    let time = c.duration_ms.unwrap_or(0);
    out.push_str("    <testcase");
    write_attr(out, "name", &c.name);
    write_attr(out, "classname", classname);
    write_attr(out, "time", &format_seconds(time));
    match c.verdict {
        Verdict::Pass | Verdict::Warn => {
            out.push_str("/>\n");
        }
        Verdict::Skip => {
            out.push_str(">\n      <skipped");
            if let Some(d) = &c.detail {
                write_attr(out, "message", d);
            }
            out.push_str("/>\n    </testcase>\n");
        }
        Verdict::Fail => {
            let kind = match c.severity {
                Some(Severity::Critical) => "Critical",
                Some(Severity::Error) => "Error",
                Some(Severity::Warning) => "Warning",
                Some(Severity::Info) => "Info",
                None => "Failure",
            };
            out.push_str(">\n      <failure");
            write_attr(out, "type", kind);
            let message = c.detail.as_deref().unwrap_or(&c.name);
            write_attr(out, "message", message);
            out.push('>');
            write_text(out, message);
            out.push_str("</failure>\n    </testcase>\n");
        }
    }
}

fn format_seconds(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

fn write_attr(out: &mut String, name: &str, value: &str) {
    write!(out, " {}=\"", name).expect("write to String never fails");
    escape_attr(out, value);
    out.push('"');
}

fn escape_attr(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if (c as u32) < 0x20 => {} // strip other control chars
            c => out.push(c),
        }
    }
}

fn write_text(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\r' && c != '\t' => {}
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_emits_well_formed_envelope() {
        let r = Report::new("c", "0.1.0").with_producer("p");
        let xml = to_junit_xml(&r);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<testsuites"));
        assert!(xml.contains("<testsuite"));
        assert!(xml.contains("tests=\"0\""));
        assert!(xml.ends_with("</testsuites>\n"));
    }

    #[test]
    fn verdict_counts_appear_on_testsuite() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::pass("a"));
        r.push(CheckResult::pass("b"));
        r.push(CheckResult::fail("c", Severity::Error));
        r.push(CheckResult::warn("d", Severity::Warning));
        r.push(CheckResult::skip("e"));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("tests=\"5\""));
        assert!(xml.contains("failures=\"1\""));
        assert!(xml.contains("skipped=\"1\""));
    }

    #[test]
    fn pass_emits_self_closing_testcase() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::pass("compile"));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("<testcase name=\"compile\" classname=\"p\" time=\"0.000\"/>"));
    }

    #[test]
    fn fail_emits_failure_child_with_type_and_message() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("oops", Severity::Critical).with_detail("the reason"));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("<failure type=\"Critical\" message=\"the reason\">"));
        assert!(xml.contains("the reason</failure>"));
    }

    #[test]
    fn skip_emits_skipped_child() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::skip("network").with_detail("no net"));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("<skipped message=\"no net\"/>"));
    }

    #[test]
    fn warn_emits_passing_testcase() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::warn("flaky", Severity::Warning));
        let xml = to_junit_xml(&r);
        // No <failure> element; warn is a passing testcase in JUnit.
        assert!(!xml.contains("<failure"));
        assert!(xml.contains("<testcase name=\"flaky\""));
    }

    #[test]
    fn xml_escapes_special_chars_in_attribute_values() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(
            CheckResult::fail("name<>&\"'", Severity::Error)
                .with_detail("oh no: <bad> & \"quotes\""),
        );
        let xml = to_junit_xml(&r);
        assert!(xml.contains("name=\"name&lt;&gt;&amp;&quot;&apos;\""));
        assert!(xml.contains("message=\"oh no: &lt;bad&gt; &amp; &quot;quotes&quot;\""));
        // Inside the text body, &, <, > are escaped; " is not.
        assert!(xml.contains("oh no: &lt;bad&gt; &amp; \"quotes\"</failure>"));
    }

    #[test]
    fn duration_ms_becomes_seconds_with_three_decimals() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::pass("a").with_duration_ms(1500));
        r.push(CheckResult::pass("b").with_duration_ms(7));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("time=\"1.500\""));
        assert!(xml.contains("time=\"0.007\""));
    }

    #[test]
    fn multi_emits_one_testsuite_per_producer() {
        let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
        bench.push(CheckResult::pass("hot"));
        let mut chaos = Report::new("c", "0.1.0").with_producer("dev-chaos");
        chaos.push(CheckResult::fail("recover", Severity::Error));
        let mut multi = MultiReport::new("c", "0.1.0");
        multi.push(bench);
        multi.push(chaos);

        let xml = multi_to_junit_xml(&multi);
        let n_suites = xml.matches("<testsuite ").count();
        assert_eq!(n_suites, 2);
        assert!(xml.contains("name=\"dev-bench\""));
        assert!(xml.contains("name=\"dev-chaos\""));
    }

    #[test]
    fn output_is_deterministic() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::pass("a"));
        r.push(CheckResult::fail("b", Severity::Error).with_detail("bad"));
        let x1 = to_junit_xml(&r);
        let x2 = to_junit_xml(&r);
        assert_eq!(x1, x2);
    }

    #[test]
    fn report_without_producer_uses_unknown_name() {
        let mut r = Report::new("c", "0.1.0");
        r.push(CheckResult::pass("a"));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("<testsuite name=\"unknown\""));
        assert!(xml.contains("classname=\"unknown\""));
    }

    #[test]
    fn fail_without_detail_uses_name_as_message() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("the_check", Severity::Error));
        let xml = to_junit_xml(&r);
        assert!(xml.contains("message=\"the_check\""));
        assert!(xml.contains(">the_check</failure>"));
    }
}
