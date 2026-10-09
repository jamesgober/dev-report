//! Markdown exporter. Available with the `markdown` feature.
//!
//! Pure function over a [`Report`], [`Diff`], or [`MultiReport`]
//! producing a CommonMark-compatible string. Every fact (verdict,
//! severity, tags, evidence, durations) is preserved in the output.
//! No external dependencies.
//!
//! [`Diff`]: crate::Diff
//! [`MultiReport`]: crate::MultiReport

use std::fmt::Write as _;

use crate::{CheckResult, Diff, EvidenceData, FileRef, MultiReport, Report, Severity, Verdict};

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

/// Render a [`Diff`] to a CommonMark-compatible Markdown string.
///
/// # Example
///
/// ```
/// use dev_report::{markdown, CheckResult, Report, Severity};
///
/// let mut prev = Report::new("c", "0.1.0");
/// prev.push(CheckResult::pass("a"));
/// let mut curr = Report::new("c", "0.1.0");
/// curr.push(CheckResult::fail("a", Severity::Error));
///
/// let diff = curr.diff(&prev);
/// let md = markdown::diff_to_markdown(&diff);
/// assert!(md.starts_with("# Diff"));
/// assert!(md.contains("Newly failing"));
/// ```
pub fn diff_to_markdown(diff: &Diff) -> String {
    let mut out = String::with_capacity(256);
    let _ = write_diff(&mut out, diff);
    out
}

/// Render a [`MultiReport`] to a CommonMark-compatible Markdown string.
///
/// Renders a top-level summary followed by each constituent report
/// as an `## H2` section.
///
/// # Example
///
/// ```
/// use dev_report::{markdown, CheckResult, MultiReport, Report};
///
/// let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
/// bench.push(CheckResult::pass("hot"));
/// let mut multi = MultiReport::new("c", "0.1.0");
/// multi.push(bench);
///
/// let md = markdown::multi_to_markdown(&multi);
/// assert!(md.starts_with("# MultiReport"));
/// ```
pub fn multi_to_markdown(multi: &MultiReport) -> String {
    let mut out = String::with_capacity(512);
    let _ = write_multi(&mut out, multi);
    out
}

fn write_report(out: &mut String, r: &Report) -> std::fmt::Result {
    writeln!(out, "# Report: {} {}", r.subject, r.subject_version)?;
    writeln!(out)?;
    writeln!(out, "- **Schema version:** {}", r.schema_version)?;
    if let Some(p) = &r.producer {
        writeln!(out, "- **Producer:** {}", code_span(p))?;
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
        one_line(&c.name),
        verdict_word(c.verdict),
        sev
    )?;
    writeln!(out)?;
    if let Some(d) = c.duration_ms {
        writeln!(out, "- **Duration:** {} ms", d)?;
    }
    writeln!(out, "- **At:** {}", c.at.format("%Y-%m-%d %H:%M:%S UTC"))?;
    if !c.tags.is_empty() {
        let tags: Vec<String> = c.tags.iter().map(|t| code_span(t)).collect();
        writeln!(out, "- **Tags:** {}", tags.join(", "))?;
    }
    if let Some(detail) = &c.detail {
        writeln!(out, "- **Detail:** {}", indent_continuation(detail, "  "))?;
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
            // The fence must be longer than any backtick run inside the
            // snippet, or a snippet containing ``` would close it early.
            let fence = "`".repeat((longest_backtick_run(s) + 1).max(3));
            writeln!(out, "- **{}** (snippet):", label)?;
            writeln!(out)?;
            writeln!(out, "  {}", fence)?;
            for line in s.lines() {
                writeln!(out, "  {}", line)?;
            }
            writeln!(out, "  {}", fence)
        }
        EvidenceData::FileRef(f) => {
            writeln!(
                out,
                "- **{}** (file): {}",
                label,
                code_span(&file_ref_inline(f))
            )
        }
        EvidenceData::KeyValue(map) => {
            writeln!(out, "- **{}** (key-value):", label)?;
            for (k, v) in map {
                writeln!(
                    out,
                    "  - {}: {}",
                    code_span(k),
                    indent_continuation(v, "    ")
                )?;
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

fn write_diff(out: &mut String, d: &Diff) -> std::fmt::Result {
    writeln!(out, "# Diff")?;
    writeln!(out)?;
    if d.is_clean() {
        writeln!(out, "_clean (no differences)_")?;
        return Ok(());
    }
    write_diff_list(out, "Newly failing", &d.newly_failing)?;
    write_diff_list(out, "Newly passing", &d.newly_passing)?;
    write_diff_list(out, "Added", &d.added)?;
    write_diff_list(out, "Removed", &d.removed)?;
    if !d.severity_changes.is_empty() {
        writeln!(out, "## Severity changes")?;
        writeln!(out)?;
        writeln!(out, "| Check | From | To |")?;
        writeln!(out, "|-------|------|----|")?;
        for c in &d.severity_changes {
            let from = c.from.map(severity_word).unwrap_or("none");
            let to = c.to.map(severity_word).unwrap_or("none");
            writeln!(
                out,
                "| {} | {} | {} |",
                escape_table_cell(&c.name),
                from,
                to
            )?;
        }
        writeln!(out)?;
    }
    if !d.duration_regressions.is_empty() {
        writeln!(out, "## Duration regressions")?;
        writeln!(out)?;
        writeln!(out, "| Check | Baseline (ms) | Current (ms) | Delta |")?;
        writeln!(out, "|-------|---------------|--------------|-------|")?;
        for r in &d.duration_regressions {
            writeln!(
                out,
                "| {} | {} | {} | {:+.2}% |",
                escape_table_cell(&r.name),
                r.baseline_ms,
                r.current_ms,
                r.delta_pct
            )?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// Escape a string for safe inclusion in a markdown table cell.
///
/// The cell delimiter is `|`; a literal pipe inside a value would
/// split the cell and shift later columns. CommonMark allows
/// backslash-escaping the pipe (`\|`) inside a table cell. Newlines
/// also break tables; replace them with `<br>` so the layout survives.
fn escape_table_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
    out
}

/// Collapse line breaks to spaces for contexts that must stay on one
/// line (headings, code spans).
fn one_line(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// Indent every line after the first so multi-line text stays inside
/// the surrounding list item.
fn indent_continuation(s: &str, indent: &str) -> String {
    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
    normalized.replace('\n', &format!("\n{}", indent))
}

fn longest_backtick_run(s: &str) -> usize {
    let (mut longest, mut current) = (0, 0);
    for ch in s.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// Wrap `s` in an inline code span that survives backticks and line
/// breaks in the value. CommonMark: the delimiter is a backtick run
/// longer than any run inside, padded with spaces when the content
/// starts or ends with a backtick.
fn code_span(s: &str) -> String {
    let body = one_line(s);
    let ticks = "`".repeat(longest_backtick_run(&body) + 1);
    if body.starts_with('`') || body.ends_with('`') {
        format!("{ticks} {body} {ticks}")
    } else {
        format!("{ticks}{body}{ticks}")
    }
}

fn write_diff_list(out: &mut String, title: &str, items: &[String]) -> std::fmt::Result {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(out, "## {}", title)?;
    writeln!(out)?;
    for name in items {
        writeln!(out, "- {}", code_span(name))?;
    }
    writeln!(out)
}

fn write_multi(out: &mut String, m: &MultiReport) -> std::fmt::Result {
    writeln!(out, "# MultiReport: {} {}", m.subject, m.subject_version)?;
    writeln!(out)?;
    writeln!(out, "- **Schema version:** {}", m.schema_version)?;
    writeln!(out, "- **Reports:** {}", m.reports.len())?;
    writeln!(out, "- **Total checks:** {}", m.total_check_count())?;
    writeln!(
        out,
        "- **Started:** {}",
        m.started_at.format("%Y-%m-%d %H:%M:%S UTC")
    )?;
    if let Some(end) = m.finished_at {
        writeln!(
            out,
            "- **Finished:** {}",
            end.format("%Y-%m-%d %H:%M:%S UTC")
        )?;
    }
    writeln!(
        out,
        "- **Overall verdict:** **{}**",
        verdict_word(m.overall_verdict())
    )?;
    writeln!(out)?;
    writeln!(out, "---")?;
    writeln!(out)?;
    for r in &m.reports {
        write_report(out, r)?;
        writeln!(out)?;
    }
    Ok(())
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
    fn diff_table_escapes_pipes_in_check_names() {
        use crate::{DiffOptions, Verdict};

        // Curr has a fail; baseline has a pass, with a check name
        // containing pipes — must be escaped or the markdown table breaks.
        let mut base = Report::new("c", "0.1.0");
        base.push(CheckResult::pass("a|b|c").with_duration_ms(100));
        let mut curr = Report::new("c", "0.1.0");
        curr.push(CheckResult::fail("a|b|c", Severity::Error).with_duration_ms(220));
        let diff = curr.diff_with(
            &base,
            &DiffOptions {
                duration_regression_pct: Some(20.0),
                duration_regression_abs_ms: None,
            },
        );
        assert!(!diff.is_clean());
        let md = diff.to_markdown();

        // Pipes inside the check name must be backslash-escaped so the
        // surrounding `|`-delimited table layout survives.
        assert!(
            md.contains(r"a\|b\|c"),
            "check-name pipes not escaped: {}",
            md
        );
        // And the raw form (unescaped pipes) must NOT appear in the table
        // row, since that would corrupt the column count.
        assert!(
            !md.lines().any(|l| l.starts_with("| a|b|c |")),
            "raw pipe leaked into table row: {}",
            md
        );
        // sanity: this test only matters if the diff actually emits the
        // expected sections.
        assert!(matches!(curr.overall_verdict(), Verdict::Fail));
    }

    #[test]
    fn backticks_and_line_breaks_cannot_break_the_layout() {
        let mut r = Report::new("c", "0.1.0").with_producer("dev`x");
        r.push(
            CheckResult::fail("multi\nline", Severity::Error)
                .with_tag("a`b")
                .with_detail("first\nsecond")
                .with_evidence(Evidence::snippet("md", "before\n```\nafter"))
                .with_evidence(Evidence::kv("env", [("K`", "v1\nv2")]))
                .with_evidence(Evidence::file_ref("f", "dir/`odd`.rs")),
        );
        let md = to_markdown(&r);
        assert!(md.contains("- **Producer:** ``dev`x``"), "{md}");
        assert!(md.contains("### multi line - **FAIL** (error)"), "{md}");
        assert!(md.contains("- **Tags:** ``a`b``"), "{md}");
        assert!(md.contains("- **Detail:** first\n  second\n"), "{md}");
        // Snippet fence is longer than the ``` inside it.
        assert!(
            md.contains("  ````\n  before\n  ```\n  after\n  ````\n"),
            "{md}"
        );
        assert!(md.contains("  - `` K` ``: v1\n    v2\n"), "{md}");
        assert!(md.contains("(file): ``dir/`odd`.rs``"), "{md}");
    }

    #[test]
    fn diff_list_names_with_backticks_are_code_spans() {
        let prev = Report::new("c", "0.1.0");
        let mut curr = Report::new("c", "0.1.0");
        curr.push(CheckResult::fail("a`b", Severity::Error));
        let md = diff_to_markdown(&curr.diff(&prev));
        assert!(md.contains("- ``a`b``"), "{md}");
    }

    #[test]
    fn empty_report_renders() {
        let r = Report::new("nothing", "0.0.0");
        let md = to_markdown(&r);
        assert!(md.contains("# Report: nothing 0.0.0"));
        assert!(md.contains("**Overall verdict:** **SKIP**"));
        assert!(md.contains("| **Total** | **0** |"));
    }

    #[test]
    fn diff_clean_renders() {
        let mut a = Report::new("c", "0.1.0");
        a.push(CheckResult::pass("x"));
        let b = a.clone();
        let md = diff_to_markdown(&a.diff(&b));
        assert!(md.starts_with("# Diff"));
        assert!(md.contains("clean"));
    }

    #[test]
    fn diff_with_changes_renders_sections() {
        let mut prev = Report::new("c", "0.1.0");
        prev.push(CheckResult::pass("a"));
        prev.push(CheckResult::pass("b"));

        let mut curr = Report::new("c", "0.1.0");
        curr.push(CheckResult::fail("a", Severity::Error));
        curr.push(CheckResult::pass("c"));

        let md = diff_to_markdown(&curr.diff(&prev));
        assert!(md.contains("## Newly failing"));
        assert!(md.contains("- `a`"));
        assert!(md.contains("## Added"));
        assert!(md.contains("- `c`"));
        assert!(md.contains("## Removed"));
        assert!(md.contains("- `b`"));
    }

    #[test]
    fn multi_renders_each_report() {
        let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
        bench.push(CheckResult::pass("hot"));
        let mut chaos = Report::new("c", "0.1.0").with_producer("dev-chaos");
        chaos.push(CheckResult::fail("recover", Severity::Critical));

        let mut multi = MultiReport::new("c", "0.1.0");
        multi.push(bench);
        multi.push(chaos);

        let md = multi_to_markdown(&multi);
        assert!(md.starts_with("# MultiReport"));
        assert!(md.contains("**Reports:** 2"));
        assert!(md.contains("**Total checks:** 2"));
        assert!(md.contains("# Report: c 0.1.0")); // each report rendered as section
    }
}
