//! Terminal pretty-printer. Available with the `terminal` feature.
//!
//! Pure function over a [`Report`]. No I/O, no global state, no extra
//! dependencies. ANSI color is opt-in so the caller decides based on
//! their own TTY detection.

use std::fmt::Write as _;

use crate::{CheckResult, EvidenceData, FileRef, Report, Severity, Verdict};

/// Render a report to a TTY-friendly string. Monochrome.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report};
///
/// let mut r = Report::new("my-crate", "0.1.0");
/// r.push(CheckResult::pass("compile"));
/// r.finish();
/// let out = r.to_terminal();
/// assert!(out.contains("[PASS]"));
/// assert!(out.contains("compile"));
/// ```
pub fn to_terminal(report: &Report) -> String {
    render(report, false)
}

/// Render a report with ANSI color codes for TTY output.
///
/// Caller is responsible for checking color support before invoking
/// (e.g. `std::io::stdout().is_terminal()` and `NO_COLOR` env var).
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report};
///
/// let mut r = Report::new("my-crate", "0.1.0");
/// r.push(CheckResult::pass("compile"));
/// r.finish();
/// let out = r.to_terminal_color();
/// assert!(out.contains("\x1b["));
/// ```
pub fn to_terminal_color(report: &Report) -> String {
    render(report, true)
}

fn render(report: &Report, color: bool) -> String {
    let mut out = String::with_capacity(256);
    let _ = write_header(&mut out, report, color);
    let _ = write_summary(&mut out, report, color);
    out.push('\n');
    for check in &report.checks {
        let _ = write_check(&mut out, check, color);
    }
    out
}

fn write_header(out: &mut String, report: &Report, color: bool) -> std::fmt::Result {
    let bold = if color { "\x1b[1m" } else { "" };
    let reset = if color { "\x1b[0m" } else { "" };
    writeln!(
        out,
        "{}=== dev-report :: {} {} ==={}",
        bold, report.subject, report.subject_version, reset
    )?;
    if let Some(p) = &report.producer {
        writeln!(out, "producer: {}", p)?;
    }
    writeln!(out, "schema:   v{}", report.schema_version)?;
    Ok(())
}

fn write_summary(out: &mut String, report: &Report, color: bool) -> std::fmt::Result {
    let (mut p, mut f, mut w, mut s) = (0usize, 0usize, 0usize, 0usize);
    for c in &report.checks {
        match c.verdict {
            Verdict::Pass => p += 1,
            Verdict::Fail => f += 1,
            Verdict::Warn => w += 1,
            Verdict::Skip => s += 1,
        }
    }
    let overall = report.overall_verdict();
    let label = verdict_label(overall, color);
    writeln!(
        out,
        "verdict:  {}  ({} checks: {} fail, {} warn, {} pass, {} skip)",
        label,
        report.checks.len(),
        f,
        w,
        p,
        s
    )?;
    if let Some(end) = report.finished_at {
        let dur_ms = (end - report.started_at).num_milliseconds();
        writeln!(
            out,
            "duration: {} -> {} ({}ms)",
            report.started_at.format("%Y-%m-%d %H:%M:%S"),
            end.format("%H:%M:%S"),
            dur_ms
        )?;
    } else {
        writeln!(
            out,
            "started:  {}",
            report.started_at.format("%Y-%m-%d %H:%M:%S")
        )?;
    }
    Ok(())
}

fn write_check(out: &mut String, c: &CheckResult, color: bool) -> std::fmt::Result {
    let badge = check_badge(c, color);
    let dim = if color { "\x1b[2m" } else { "" };
    let reset = if color { "\x1b[0m" } else { "" };
    let dur = c
        .duration_ms
        .map(|ms| format!("  {dim}{ms}ms{reset}"))
        .unwrap_or_default();
    writeln!(out, "{} {}{}", badge, c.name, dur)?;

    if !c.tags.is_empty() {
        writeln!(out, "   tags: {}", c.tags.join(", "))?;
    }
    if let Some(detail) = &c.detail {
        writeln!(out, "   detail: {}", detail)?;
    }
    if !c.evidence.is_empty() {
        writeln!(out, "   evidence:")?;
        for e in &c.evidence {
            write_evidence(out, &e.label, &e.data)?;
        }
    }
    Ok(())
}

fn write_evidence(out: &mut String, label: &str, data: &EvidenceData) -> std::fmt::Result {
    match data {
        EvidenceData::Numeric(n) => {
            writeln!(out, "     - {}: {}", label, n)
        }
        EvidenceData::Snippet(s) => {
            writeln!(out, "     - {}: {:?}", label, s)
        }
        EvidenceData::FileRef(f) => {
            writeln!(out, "     - {}: {}", label, file_ref_inline(f))
        }
        EvidenceData::KeyValue(map) => {
            let pairs: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
            writeln!(out, "     - {}: {{ {} }}", label, pairs.join(", "))
        }
    }
}

fn file_ref_inline(f: &FileRef) -> String {
    match (f.line_start, f.line_end) {
        (Some(s), Some(e)) if s == e => format!("{}:{}", f.path, s),
        (Some(s), Some(e)) => format!("{}:{}-{}", f.path, s, e),
        (Some(s), None) => format!("{}:{}", f.path, s),
        _ => f.path.clone(),
    }
}

fn verdict_label(v: Verdict, color: bool) -> String {
    if !color {
        return match v {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Warn => "WARN",
            Verdict::Skip => "SKIP",
        }
        .to_string();
    }
    match v {
        Verdict::Pass => "\x1b[32mPASS\x1b[0m".to_string(),
        Verdict::Fail => "\x1b[31mFAIL\x1b[0m".to_string(),
        Verdict::Warn => "\x1b[33mWARN\x1b[0m".to_string(),
        Verdict::Skip => "\x1b[2mSKIP\x1b[0m".to_string(),
    }
}

fn check_badge(c: &CheckResult, color: bool) -> String {
    let sev = c
        .severity
        .map(|s| match s {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Critical => "critical",
        })
        .map(|s| format!(" {}", s))
        .unwrap_or_default();
    let label = match c.verdict {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Warn => "WARN",
        Verdict::Skip => "SKIP",
    };
    if !color {
        return format!("[{}{}]", label, sev);
    }
    let (open, close) = match c.verdict {
        Verdict::Pass => ("\x1b[32m", "\x1b[0m"),
        Verdict::Fail => ("\x1b[31m", "\x1b[0m"),
        Verdict::Warn => ("\x1b[33m", "\x1b[0m"),
        Verdict::Skip => ("\x1b[2m", "\x1b[0m"),
    };
    format!("[{}{}{}{}]", open, label, sev, close)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evidence, EvidenceKind};

    fn sample_report() -> Report {
        let mut r = Report::new("widget", "0.1.0").with_producer("dev-report-test");
        r.push(CheckResult::pass("compile").with_duration_ms(7));
        r.push(
            CheckResult::warn("flaky", Severity::Warning)
                .with_tag("bench")
                .with_evidence(Evidence::numeric("mean_ns", 1234.5))
                .with_evidence(Evidence::kv("env", [("CI", "true")])),
        );
        r.push(
            CheckResult::fail("chaos::recover", Severity::Critical)
                .with_tags(["chaos", "recovery"])
                .with_detail("recovery did not restore final state")
                .with_evidence(Evidence::file_ref_lines("site", "src/recover.rs", 10, 20)),
        );
        r.push(CheckResult::skip("not_applicable"));
        r.finish();
        r
    }

    #[test]
    fn monochrome_render_contains_all_checks() {
        let out = to_terminal(&sample_report());
        assert!(out.contains("compile"));
        assert!(out.contains("flaky"));
        assert!(out.contains("chaos::recover"));
        assert!(out.contains("not_applicable"));
        assert!(out.contains("[PASS]"));
        assert!(out.contains("[WARN warning]"));
        assert!(out.contains("[FAIL critical]"));
        assert!(out.contains("[SKIP]"));
    }

    #[test]
    fn monochrome_render_has_no_ansi() {
        let out = to_terminal(&sample_report());
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn color_render_has_ansi() {
        let out = to_terminal_color(&sample_report());
        assert!(out.contains('\x1b'));
        assert!(out.contains("\x1b[31m")); // fail red
        assert!(out.contains("\x1b[32m")); // pass green
        assert!(out.contains("\x1b[33m")); // warn yellow
    }

    #[test]
    fn render_includes_evidence() {
        let out = to_terminal(&sample_report());
        assert!(out.contains("mean_ns"));
        assert!(out.contains("1234.5"));
        assert!(out.contains("CI: true"));
        assert!(out.contains("src/recover.rs:10-20"));
    }

    #[test]
    fn render_includes_tags_and_detail() {
        let out = to_terminal(&sample_report());
        assert!(out.contains("tags: chaos, recovery"));
        assert!(out.contains("detail: recovery did not restore final state"));
    }

    #[test]
    fn render_includes_summary_counts() {
        let out = to_terminal(&sample_report());
        assert!(out.contains("4 checks"));
        assert!(out.contains("1 fail"));
        assert!(out.contains("1 warn"));
        assert!(out.contains("1 pass"));
        assert!(out.contains("1 skip"));
    }

    #[test]
    fn fits_under_80_columns_for_typical_report() {
        let out = to_terminal(&sample_report());
        for line in out.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds 80 cols: {:?}",
                line
            );
        }
    }

    #[test]
    fn pure_function_same_input_same_output() {
        let r = sample_report();
        let a = to_terminal(&r);
        let b = to_terminal(&r);
        assert_eq!(a, b);
    }

    #[test]
    fn empty_report_renders() {
        let r = Report::new("nothing", "0.0.0");
        let out = to_terminal(&r);
        assert!(out.contains("nothing"));
        assert!(out.contains("0 checks"));
    }

    #[test]
    fn file_ref_inline_formats() {
        let no_lines = file_ref_inline(&FileRef::new("a.rs"));
        assert_eq!(no_lines, "a.rs");
        let single = file_ref_inline(&FileRef::new("a.rs").with_line_range(5, 5));
        assert_eq!(single, "a.rs:5");
        let range = file_ref_inline(&FileRef::new("a.rs").with_line_range(5, 9));
        assert_eq!(range, "a.rs:5-9");
    }

    #[test]
    fn evidence_kind_dispatch_covers_all_variants() {
        let r = sample_report();
        let kinds: std::collections::HashSet<_> = r
            .checks
            .iter()
            .flat_map(|c| &c.evidence)
            .map(|e| e.kind())
            .collect();
        // sample_report uses Numeric, KeyValue, FileRef
        assert!(kinds.contains(&EvidenceKind::Numeric));
        assert!(kinds.contains(&EvidenceKind::KeyValue));
        assert!(kinds.contains(&EvidenceKind::FileRef));
    }
}
