//! Aggregation of multiple [`Report`]s into a [`MultiReport`].
//!
//! A CI run typically invokes several producers (`dev-bench`,
//! `dev-fixtures`, `dev-async`, ...) and wants to publish a single
//! aggregate document. `MultiReport` carries those reports without
//! merging checks across producers; check identity is
//! `(producer, name)`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CheckResult, Report, Verdict};

/// Aggregate of multiple [`Report`]s emitted in a single run.
///
/// Identity of a check is `(producer, name)`. Two checks with the same
/// `name` from different producers are kept separate.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, MultiReport, Report, Severity, Verdict};
///
/// let mut bench = Report::new("crate", "0.1.0").with_producer("dev-bench");
/// bench.push(CheckResult::pass("hot_path"));
///
/// let mut chaos = Report::new("crate", "0.1.0").with_producer("dev-chaos");
/// chaos.push(CheckResult::fail("recover", Severity::Error));
///
/// let mut multi = MultiReport::new("crate", "0.1.0");
/// multi.push(bench);
/// multi.push(chaos);
/// multi.finish();
///
/// assert_eq!(multi.overall_verdict(), Verdict::Fail);
/// assert_eq!(multi.total_check_count(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiReport {
    /// Schema version. Tracks the same number as [`Report::schema_version`].
    pub schema_version: u32,
    /// Crate or project being reported on.
    pub subject: String,
    /// Version of the subject.
    pub subject_version: String,
    /// When aggregation started.
    pub started_at: DateTime<Utc>,
    /// When aggregation finished, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    /// Constituent reports.
    pub reports: Vec<Report>,
}

impl MultiReport {
    /// Begin a new aggregate for the given subject and version.
    pub fn new(subject: impl Into<String>, subject_version: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            subject: subject.into(),
            subject_version: subject_version.into(),
            started_at: Utc::now(),
            finished_at: None,
            reports: Vec::new(),
        }
    }

    /// Append a constituent report.
    pub fn push(&mut self, r: Report) {
        self.reports.push(r);
    }

    /// Mark aggregation finished, stamping the finish time.
    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    /// Compute the overall verdict across every check in every report.
    ///
    /// Follows the same precedence as [`Report::overall_verdict`]:
    /// `Fail > Warn > Pass > Skip`.
    pub fn overall_verdict(&self) -> Verdict {
        let mut saw_fail = false;
        let mut saw_warn = false;
        let mut saw_pass = false;
        for r in &self.reports {
            for c in &r.checks {
                match c.verdict {
                    Verdict::Fail => saw_fail = true,
                    Verdict::Warn => saw_warn = true,
                    Verdict::Pass => saw_pass = true,
                    Verdict::Skip => {}
                }
            }
        }
        if saw_fail {
            Verdict::Fail
        } else if saw_warn {
            Verdict::Warn
        } else if saw_pass {
            Verdict::Pass
        } else {
            Verdict::Skip
        }
    }

    /// Total number of checks across all constituent reports.
    pub fn total_check_count(&self) -> usize {
        self.reports.iter().map(|r| r.checks.len()).sum()
    }

    /// Iterate over the constituent reports.
    ///
    /// Equivalent to `self.reports.iter()` but reads cleaner at the
    /// call site and makes the public iteration API explicit.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, MultiReport, Report};
    ///
    /// let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
    /// bench.push(CheckResult::pass("hot"));
    /// let mut multi = MultiReport::new("c", "0.1.0");
    /// multi.push(bench);
    ///
    /// for r in multi.iter_reports() {
    ///     assert_eq!(r.subject, "c");
    /// }
    /// ```
    pub fn iter_reports(&self) -> impl Iterator<Item = &Report> {
        self.reports.iter()
    }

    /// Find the constituent report from a specific producer, if any.
    ///
    /// Returns the first match (`MultiReport` doesn't enforce
    /// uniqueness; producers can appear multiple times).
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, MultiReport, Report};
    ///
    /// let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
    /// bench.push(CheckResult::pass("hot"));
    /// let mut multi = MultiReport::new("c", "0.1.0");
    /// multi.push(bench);
    ///
    /// let found = multi.report_from("dev-bench").unwrap();
    /// assert_eq!(found.checks.len(), 1);
    /// ```
    pub fn report_from(&self, producer: &str) -> Option<&Report> {
        self.reports
            .iter()
            .find(|r| r.producer.as_deref() == Some(producer))
    }

    /// Aggregate verdict counts across all constituent reports as
    /// `(pass, fail, warn, skip)`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, MultiReport, Report, Severity};
    ///
    /// let mut a = Report::new("c", "0.1.0").with_producer("a");
    /// a.push(CheckResult::pass("x"));
    /// a.push(CheckResult::fail("y", Severity::Error));
    /// let mut b = Report::new("c", "0.1.0").with_producer("b");
    /// b.push(CheckResult::pass("z"));
    /// let mut multi = MultiReport::new("c", "0.1.0");
    /// multi.push(a);
    /// multi.push(b);
    /// assert_eq!(multi.verdict_counts(), (2, 1, 0, 0));
    /// ```
    pub fn verdict_counts(&self) -> (usize, usize, usize, usize) {
        let (mut p, mut f, mut w, mut s) = (0, 0, 0, 0);
        for r in &self.reports {
            for c in &r.checks {
                match c.verdict {
                    Verdict::Pass => p += 1,
                    Verdict::Fail => f += 1,
                    Verdict::Warn => w += 1,
                    Verdict::Skip => s += 1,
                }
            }
        }
        (p, f, w, s)
    }

    /// Iterate over every check across every constituent report,
    /// paired with the producer that emitted it.
    ///
    /// Producers without a `producer` field are emitted as `None`.
    pub fn iter_checks(&self) -> impl Iterator<Item = (Option<&str>, &CheckResult)> {
        self.reports.iter().flat_map(|r| {
            let p = r.producer.as_deref();
            r.checks.iter().map(move |c| (p, c))
        })
    }

    /// Iterate over checks carrying the given tag, paired with their producer.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, MultiReport, Report};
    ///
    /// let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
    /// bench.push(CheckResult::pass("hot").with_tag("slow"));
    /// bench.push(CheckResult::pass("cold"));
    ///
    /// let mut multi = MultiReport::new("c", "0.1.0");
    /// multi.push(bench);
    ///
    /// let slow: Vec<_> = multi.checks_with_tag("slow").collect();
    /// assert_eq!(slow.len(), 1);
    /// assert_eq!(slow[0].0, Some("dev-bench"));
    /// ```
    pub fn checks_with_tag<'a>(
        &'a self,
        tag: &'a str,
    ) -> impl Iterator<Item = (Option<&'a str>, &'a CheckResult)> {
        self.iter_checks().filter(move |(_, c)| c.has_tag(tag))
    }

    /// Serialize this multi-report to JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a multi-report from JSON.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Pass`.
    pub fn passed(&self) -> bool {
        self.overall_verdict() == Verdict::Pass
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Fail`.
    pub fn failed(&self) -> bool {
        self.overall_verdict() == Verdict::Fail
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Warn`.
    pub fn warned(&self) -> bool {
        self.overall_verdict() == Verdict::Warn
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Skip`.
    pub fn skipped(&self) -> bool {
        self.overall_verdict() == Verdict::Skip
    }

    /// Iterate over checks with the given severity, paired with their producer.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, MultiReport, Report, Severity};
    ///
    /// let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
    /// bench.push(CheckResult::fail("a", Severity::Error));
    ///
    /// let mut multi = MultiReport::new("c", "0.1.0");
    /// multi.push(bench);
    ///
    /// let errors: Vec<_> = multi.checks_with_severity(Severity::Error).collect();
    /// assert_eq!(errors.len(), 1);
    /// ```
    pub fn checks_with_severity(
        &self,
        severity: crate::Severity,
    ) -> impl Iterator<Item = (Option<&str>, &CheckResult)> {
        self.iter_checks()
            .filter(move |(_, c)| c.severity == Some(severity))
    }

    /// Render this multi-report to a TTY-friendly string. Monochrome.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal(&self) -> String {
        crate::terminal::multi_to_terminal(self)
    }

    /// Render this multi-report with ANSI color codes.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal_color(&self) -> String {
        crate::terminal::multi_to_terminal_color(self)
    }

    /// Render this multi-report to a Markdown string.
    ///
    /// Available with the `markdown` feature.
    #[cfg(feature = "markdown")]
    #[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
    pub fn to_markdown(&self) -> String {
        crate::markdown::multi_to_markdown(self)
    }

    /// Render this multi-report as a SARIF 2.1.0 document.
    ///
    /// Each constituent [`Report`] becomes its own SARIF `run`, so
    /// consumers can tell which producer emitted which finding. Only
    /// `Fail` and `Warn` checks are emitted.
    ///
    /// Available with the `sarif` feature.
    #[cfg(feature = "sarif")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sarif")))]
    pub fn to_sarif(&self) -> String {
        crate::sarif::multi_to_sarif(self)
    }

    /// Render this multi-report as a JUnit XML document with one
    /// `<testsuite>` per constituent [`Report`].
    ///
    /// Available with the `junit` feature.
    #[cfg(feature = "junit")]
    #[cfg_attr(docsrs, doc(cfg(feature = "junit")))]
    pub fn to_junit_xml(&self) -> String {
        crate::junit::multi_to_junit_xml(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    fn rep(producer: &str, checks: Vec<CheckResult>) -> Report {
        let mut r = Report::new("c", "0.1.0").with_producer(producer);
        for c in checks {
            r.push(c);
        }
        r.finish();
        r
    }

    #[test]
    fn empty_multi_is_skip() {
        let m = MultiReport::new("c", "0.1.0");
        assert_eq!(m.overall_verdict(), Verdict::Skip);
        assert_eq!(m.total_check_count(), 0);
    }

    #[test]
    fn fail_in_any_report_dominates() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("a", vec![CheckResult::pass("x")]));
        m.push(rep("b", vec![CheckResult::fail("y", Severity::Error)]));
        m.push(rep("c", vec![CheckResult::warn("z", Severity::Warning)]));
        assert_eq!(m.overall_verdict(), Verdict::Fail);
    }

    #[test]
    fn warn_dominates_pass_and_skip() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("a", vec![CheckResult::pass("x")]));
        m.push(rep("b", vec![CheckResult::skip("y")]));
        m.push(rep("c", vec![CheckResult::warn("z", Severity::Warning)]));
        assert_eq!(m.overall_verdict(), Verdict::Warn);
    }

    #[test]
    fn pass_dominates_skip() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("a", vec![CheckResult::skip("x")]));
        m.push(rep("b", vec![CheckResult::pass("y")]));
        assert_eq!(m.overall_verdict(), Verdict::Pass);
    }

    #[test]
    fn same_name_across_producers_is_kept_separate() {
        // Both producers emit a check named "compile". MultiReport must
        // NOT collapse them into one entry.
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("p1", vec![CheckResult::pass("compile")]));
        m.push(rep(
            "p2",
            vec![CheckResult::fail("compile", Severity::Error)],
        ));
        assert_eq!(m.total_check_count(), 2);
        assert_eq!(m.overall_verdict(), Verdict::Fail);

        let producers: Vec<_> = m
            .iter_checks()
            .filter(|(_, c)| c.name == "compile")
            .map(|(p, _)| p)
            .collect();
        assert_eq!(producers, vec![Some("p1"), Some("p2")]);
    }

    #[test]
    fn iter_checks_pairs_with_producer() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep(
            "p1",
            vec![CheckResult::pass("a"), CheckResult::pass("b")],
        ));
        m.push(rep("p2", vec![CheckResult::pass("c")]));
        let v: Vec<_> = m.iter_checks().map(|(p, c)| (p, c.name.clone())).collect();
        assert_eq!(
            v,
            vec![
                (Some("p1"), "a".to_string()),
                (Some("p1"), "b".to_string()),
                (Some("p2"), "c".to_string()),
            ]
        );
    }

    #[test]
    fn json_round_trip() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep(
            "p1",
            vec![CheckResult::fail("x", Severity::Error)
                .with_tag("regression")
                .with_detail("regressed")],
        ));
        m.finish();
        let json = m.to_json().unwrap();
        let parsed = MultiReport::from_json(&json).unwrap();
        assert_eq!(parsed.subject, "c");
        assert_eq!(parsed.reports.len(), 1);
        assert_eq!(parsed.overall_verdict(), Verdict::Fail);
    }

    #[test]
    fn iter_reports_yields_each_report() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("a", vec![CheckResult::pass("x")]));
        m.push(rep("b", vec![CheckResult::pass("y")]));
        let producers: Vec<&str> = m
            .iter_reports()
            .filter_map(|r| r.producer.as_deref())
            .collect();
        assert_eq!(producers, vec!["a", "b"]);
    }

    #[test]
    fn report_from_finds_by_producer() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep("dev-bench", vec![CheckResult::pass("hot")]));
        m.push(rep("dev-chaos", vec![CheckResult::pass("recover")]));
        assert!(m.report_from("dev-bench").is_some());
        assert!(m.report_from("dev-chaos").is_some());
        assert!(m.report_from("not-here").is_none());
    }

    #[test]
    fn multi_verdict_counts_aggregates() {
        let mut m = MultiReport::new("c", "0.1.0");
        m.push(rep(
            "a",
            vec![
                CheckResult::pass("x"),
                CheckResult::fail("y", Severity::Error),
            ],
        ));
        m.push(rep(
            "b",
            vec![
                CheckResult::pass("z"),
                CheckResult::warn("w", Severity::Warning),
            ],
        ));
        assert_eq!(m.verdict_counts(), (2, 1, 1, 0));
    }
}
