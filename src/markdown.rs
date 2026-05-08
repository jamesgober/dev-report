//! Markdown exporter. Available with the `markdown` feature.
//!
//! Pure function over a [`Report`] producing a CommonMark-compatible
//! string. Every fact in the report (verdict, severity, tags, evidence,
//! durations) is preserved in the output. No external dependencies.

use std::fmt::Write as _;

use crate::{CheckResult, EvidenceData, FileRef, Report, Severity, Verdict};

/// Render a report to a CommonMark-compatible Markdown string.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report};
///
/// let mut r = Report::new("my-crate", "0.1.0");
/// r.push(CheckResult::pass("compile"));
/// r.finish();
/// let md = r.to_markdown();
/// assert!(md.starts_with("# Report"));
/// assert!(md.contains("compile"));
/// ```
pub fn to_markdown(report: &Report) -> String {
    let mut out = String::with_capacity(512);
    let _ = write_report(&mut out, report);
    out
}

fn write_report(out: &mut String, r: &Report) -> std::fmt::Result {
    writeln!(out, "# Report: {} {}", r.subject, r.subject_version)?;
    writeln!(out)?;
    writeln!(out, "- **Schema version:** {}", r.schema_version)?;
    if let Some(p) = &r.producer {
        writeln!(out, "- **Producer:** `{}`", p)?;
    }
    writeln!(
        out,
        "- **Started:** {}",
        r.started_at.format("%Y-%m-%d %H:%M:%S UTC")
    )?;
    if let Some(end) = r.finished_at {
        writeln!(
            out,
            "- **Finished:** {}",
            end.format("%Y-%m-%d %H:%M:%S UTC")
        )?;
    }
    writeln!(
        out,
        "- **Overall verdict:** **{}**",
        verdict_word(r.overall_verdict())
    )?;
    writeln!(out)?;
    write_summary_table(out, r)?;
    writeln!(out)?;
    writeln!(out, "## Checks")?;
    writeln!(out)?;
    for c in &r.checks {
        write_check(out, c)?;
    }
    Ok(())
}

fn write_summary_table(out: &mut String, r: &Report) -> std::fmt::Result {
    let (mut p, mut f, mut w, mut s) = (0usize, 0usize, 0usize, 0usize);
    for c in &r.checks {
        match c.verdict {
            Verdict::Pass => p += 1,
            Verdict::Fail => f += 1,
            Verdict::Warn => w += 1,
            Verdict::Skip => s += 1,
        }
    }
    writeln!(out, "| Verdict | Count |")?;
    writeln!(out, "|---------|-------|")?;
    writeln!(out, "| Fail    | {} |", f)?;
    writeln!(out, "| Warn    | {} |", w)?;
    writeln!(out, "| Pass    | {} |", p)?;
    writeln!(out, "| Skip    | {} |", s)?;
    writeln!(out, "| **Total** | **{}** |", r.checks.len())
}

fn write_check(out: &mut String, c: &CheckResult) -> std::fmt::Result {
    let sev = c
        .severity
        .map(|s| format!(" ({})", severity_word(s)))
        .unwrap_or_default();
    writeln!(
        out,
        "### {} - **{}**{}",
        c.name,
        verdict_word(c.verdict),
        sev
    )?;
    writeln!(out)?;
    if let Some(d) = c.duration_ms {
        writeln!(out, "- **Duration:** {} ms", d)?;
    }
    writeln!(out, "- **At:** {}", c.at.format("%Y-%m-%d %H:%M:%S UTC"))?;
    if !c.tags.is_empty() {
        let tags: Vec<String> = c.tags.iter().map(|t| format!("`{}`", t)).collect();
        writeln!(out, "- **Tags:** {}", tags.join(", "))?;
    }
    if let Some(detail) = &c.detail {
        writeln!(out, "- **Detail:** {}", detail)?;
    }
    if !c.evidence.is_empty() {
        writeln!(out)?;
        writeln!(out, "**Evidence:**")?;
        writeln!(out)?;
        for e in &c.evidence {
            write_evidence(out, &e.label, &e.data)?;
        }
    }
    writeln!(out)
}

fn write_evidence(out: &mut String, label: &str, data: &EvidenceData) -> std::fmt::Result {
    match data {
        EvidenceData::Numeric(n) => writeln!(out, "- **{}** (numeric): `{}`", label, n),
        EvidenceData::Snippet(s) => {
            writeln!(out, "- **{}** (snippet):", label)?;
            writeln!(out)?;
            writeln!(out, "  ```")?;
            for line in s.lines() {
                writeln!(out, "  {}", line)?;
            }
            writeln!(out, "  ```")
        }
        EvidenceData::FileRef(f) => {
            writeln!(out, "- **{}** (file): `{}`", label, file_ref_inline(f))
        }
        EvidenceData::KeyValue(map) => {
            writeln!(out, "- **{}** (key-value):", label)?;
            for (k, v) in map {
                writeln!(out, "  - `{}`: {}", k, v)?;
            }
            Ok(())
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

fn verdict_word(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Warn => "WARN",
        Verdict::Skip => "SKIP",
    }
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Critical => "critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Evidence;

    fn sample() -> Report {
        let mut r = Report::new("widget", "0.1.0").with_producer("dev-report-test");
        r.push(CheckResult::pass("compile").with_duration_ms(7));
        r.push(
            CheckResult::warn("flaky", Severity::Warning)
                .with_tag("bench")
                .with_evidence(Evidence::numeric("mean_ns", 1234.5))
                .with_evidence(Evidence::kv("env", [("CI", "true"), ("RUST_LOG", "debug")])),
        );
        r.push(
            CheckResult::fail("chaos::recover", Severity::Critical)
                .with_tags(["chaos", "recovery"])
                .with_detail("recovery did not restore final state")
                .with_evidence(Evidence::snippet("trace", "panicked at lib.rs:42"))
                .with_evidence(Evidence::file_ref_lines("site", "src/recover.rs", 10, 20)),
        );
        r.push(CheckResult::skip("not_applicable"));
        r.finish();
        r
    }

    #[test]
    fn renders_report_header() {
        let md = to_markdown(&sample());
        assert!(md.starts_with("# Report: widget 0.1.0"));
        assert!(md.contains("- **Schema version:** 1"));
        assert!(md.contains("- **Producer:** `dev-report-test`"));
    }

    #[test]
    fn renders_summary_table() {
        let md = to_markdown(&sample());
        assert!(md.contains("| Verdict | Count |"));
        assert!(md.contains("| Fail    | 1 |"));
        assert!(md.contains("| Warn    | 1 |"));
        assert!(md.contains("| Pass    | 1 |"));
        assert!(md.contains("| Skip    | 1 |"));
        assert!(md.contains("| **Total** | **4** |"));
    }

    #[test]
    fn renders_each_check_heading() {
        let md = to_markdown(&sample());
        assert!(md.contains("### compile - **PASS**"));
        assert!(md.contains("### flaky - **WARN** (warning)"));
        assert!(md.contains("### chaos::recover - **FAIL** (critical)"));
        assert!(md.contains("### not_applicable - **SKIP**"));
    }

    #[test]
    fn renders_overall_verdict() {
        let md = to_markdown(&sample());
        assert!(md.contains("**Overall verdict:** **FAIL**"));
    }

    #[test]
    fn renders_evidence_kinds() {
        let md = to_markdown(&sample());
        assert!(md.contains("**mean_ns** (numeric): `1234.5`"));
        assert!(md.contains("**env** (key-value):"));
        assert!(md.contains("`CI`: true"));
        assert!(md.contains("`RUST_LOG`: debug"));
        assert!(md.contains("**trace** (snippet):"));
        assert!(md.contains("panicked at lib.rs:42"));
        assert!(md.contains("**site** (file): `src/recover.rs:10-20`"));
    }

    #[test]
    fn renders_tags_and_detail() {
        let md = to_markdown(&sample());
        assert!(md.contains("- **Tags:** `chaos`, `recovery`"));
        assert!(md.contains("- **Detail:** recovery did not restore final state"));
    }

    #[test]
    fn pure_function_same_input_same_output() {
        let r = sample();
        assert_eq!(to_markdown(&r), to_markdown(&r));
    }

    #[test]
    fn empty_report_renders() {
        let r = Report::new("nothing", "0.0.0");
        let md = to_markdown(&r);
        assert!(md.contains("# Report: nothing 0.0.0"));
        assert!(md.contains("**Overall verdict:** **SKIP**"));
        assert!(md.contains("| **Total** | **0** |"));
    }
}
