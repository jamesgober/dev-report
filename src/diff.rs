//! Report diffing. Available unconditionally (no feature gate).
//!
//! Compare two reports and surface differences a CI gate or AI agent
//! can act on: newly failing/passing checks, severity changes, duration
//! regressions, added/removed checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CheckResult, Report, Severity, Verdict};

/// Options controlling diff sensitivity.
///
/// # Example
///
/// ```
/// use dev_report::DiffOptions;
///
/// let opts = DiffOptions {
///     duration_regression_pct: Some(50.0),
///     duration_regression_abs_ms: Some(100),
/// };
/// assert_eq!(opts.duration_regression_pct, Some(50.0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DiffOptions {
    /// Flag a duration regression when `current_ms` exceeds
    /// `baseline_ms * (1 + pct / 100)`. `None` disables percent-based
    /// detection.
    pub duration_regression_pct: Option<f64>,
    /// Flag a duration regression when `current_ms - baseline_ms`
    /// exceeds this absolute number of milliseconds. `None` disables
    /// absolute-threshold detection.
    pub duration_regression_abs_ms: Option<u64>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            duration_regression_pct: Some(20.0),
            duration_regression_abs_ms: None,
        }
    }
}

/// A change in severity for a check that exists in both reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverityChange {
    /// Check name.
    pub name: String,
    /// Severity in the baseline report.
    pub from: Option<Severity>,
    /// Severity in the current report.
    pub to: Option<Severity>,
}

/// A duration regression for a check that exists in both reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationRegression {
    /// Check name.
    pub name: String,
    /// Baseline duration, in milliseconds.
    pub baseline_ms: u64,
    /// Current duration, in milliseconds.
    pub current_ms: u64,
    /// Percent slower than baseline (e.g. `25.0` for 25% slower).
    pub delta_pct: f64,
}

/// Result of comparing two [`Report`]s.
///
/// All vectors are sorted alphabetically by check name so two diffs of
/// the same input pair produce equal `Diff` values.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report, Severity};
///
/// let mut prev = Report::new("crate", "0.1.0");
/// prev.push(CheckResult::pass("a"));
/// prev.push(CheckResult::pass("b"));
///
/// let mut curr = Report::new("crate", "0.1.0");
/// curr.push(CheckResult::pass("a"));
/// curr.push(CheckResult::fail("b", Severity::Error));
/// curr.push(CheckResult::pass("c"));
///
/// let diff = curr.diff(&prev);
/// assert_eq!(diff.newly_failing, vec!["b".to_string()]);
/// assert_eq!(diff.added, vec!["c".to_string()]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    /// Checks that are `Fail` in current but were not `Fail` in baseline
    /// (or did not exist in baseline).
    pub newly_failing: Vec<String>,
    /// Checks that are `Pass` in current but were not `Pass` in baseline.
    pub newly_passing: Vec<String>,
    /// Severity transitions for checks present in both reports.
    pub severity_changes: Vec<SeverityChange>,
    /// Duration regressions for checks present in both reports, where
    /// the slowdown exceeds the configured threshold.
    pub duration_regressions: Vec<DurationRegression>,
    /// Checks present in current but not in baseline.
    pub added: Vec<String>,
    /// Checks present in baseline but not in current.
    pub removed: Vec<String>,
}

impl Diff {
    /// `true` if the diff contains no differences worth flagging.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report};
    ///
    /// let mut a = Report::new("c", "0.1.0");
    /// a.push(CheckResult::pass("x"));
    /// let b = a.clone();
    /// assert!(a.diff(&b).is_clean());
    /// ```
    pub fn is_clean(&self) -> bool {
        self.newly_failing.is_empty()
            && self.newly_passing.is_empty()
            && self.severity_changes.is_empty()
            && self.duration_regressions.is_empty()
            && self.added.is_empty()
            && self.removed.is_empty()
    }

    /// One-line summary of the diff suitable for log output or CI status.
    ///
    /// Returns `"clean"` when [`is_clean`](Self::is_clean) is true; otherwise
    /// returns a comma-separated list of non-empty categories with counts,
    /// e.g. `"2 newly failing, 1 added, 1 duration regression"`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity};
    ///
    /// // Identical reports -> "clean"
    /// let mut a = Report::new("c", "0.1.0");
    /// a.push(CheckResult::pass("x"));
    /// assert_eq!(a.diff(&a).summary(), "clean");
    ///
    /// // Mixed differences -> comma-separated counts.
    /// let mut prev = Report::new("c", "0.1.0");
    /// prev.push(CheckResult::pass("a"));
    /// let mut curr = Report::new("c", "0.1.0");
    /// curr.push(CheckResult::fail("a", Severity::Error));
    /// let s = curr.diff(&prev).summary();
    /// assert!(s.contains("1 newly failing"));
    /// assert!(s.contains("1 severity change"));
    /// ```
    pub fn summary(&self) -> String {
        if self.is_clean() {
            return "clean".to_string();
        }
        let mut parts = Vec::new();
        if !self.newly_failing.is_empty() {
            parts.push(format!("{} newly failing", self.newly_failing.len()));
        }
        if !self.newly_passing.is_empty() {
            parts.push(format!("{} newly passing", self.newly_passing.len()));
        }
        if !self.severity_changes.is_empty() {
            parts.push(format!(
                "{} severity {}",
                self.severity_changes.len(),
                if self.severity_changes.len() == 1 {
                    "change"
                } else {
                    "changes"
                }
            ));
        }
        if !self.duration_regressions.is_empty() {
            parts.push(format!(
                "{} duration {}",
                self.duration_regressions.len(),
                if self.duration_regressions.len() == 1 {
                    "regression"
                } else {
                    "regressions"
                }
            ));
        }
        if !self.added.is_empty() {
            parts.push(format!("{} added", self.added.len()));
        }
        if !self.removed.is_empty() {
            parts.push(format!("{} removed", self.removed.len()));
        }
        parts.join(", ")
    }

    /// Render this diff as a TTY-friendly string. Monochrome.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal(&self) -> String {
        crate::terminal::diff_to_terminal(self)
    }

    /// Render this diff with ANSI color codes.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal_color(&self) -> String {
        crate::terminal::diff_to_terminal_color(self)
    }

    /// Render this diff to a Markdown string.
    ///
    /// Available with the `markdown` feature.
    #[cfg(feature = "markdown")]
    #[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
    pub fn to_markdown(&self) -> String {
        crate::markdown::diff_to_markdown(self)
    }
}

pub(crate) fn diff_reports(current: &Report, baseline: &Report, opts: &DiffOptions) -> Diff {
    // First-occurrence-wins indexing on check name.
    let curr_idx: BTreeMap<&str, &CheckResult> = index_first(&current.checks);
    let base_idx: BTreeMap<&str, &CheckResult> = index_first(&baseline.checks);

    let mut newly_failing = Vec::new();
    let mut newly_passing = Vec::new();
    let mut severity_changes = Vec::new();
    let mut duration_regressions = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();

    // Walk current to find: newly failing, newly passing, severity
    // changes, duration regressions, added.
    for (name, c) in &curr_idx {
        match base_idx.get(name) {
            None => {
                added.push((*name).to_string());
                if c.verdict == Verdict::Fail {
                    newly_failing.push((*name).to_string());
                }
                if c.verdict == Verdict::Pass {
                    newly_passing.push((*name).to_string());
                }
            }
            Some(b) => {
                if c.verdict == Verdict::Fail && b.verdict != Verdict::Fail {
                    newly_failing.push((*name).to_string());
                }
                if c.verdict == Verdict::Pass && b.verdict != Verdict::Pass {
                    newly_passing.push((*name).to_string());
                }
                if c.severity != b.severity {
                    severity_changes.push(SeverityChange {
                        name: (*name).to_string(),
                        from: b.severity,
                        to: c.severity,
                    });
                }
                if let Some(reg) = duration_regression(name, b, c, opts) {
                    duration_regressions.push(reg);
                }
            }
        }
    }

    // Walk baseline to find removed.
    for name in base_idx.keys() {
        if !curr_idx.contains_key(name) {
            removed.push((*name).to_string());
        }
    }

    // BTreeMap iteration is alphabetical; vectors are already sorted.
    Diff {
        newly_failing,
        newly_passing,
        severity_changes,
        duration_regressions,
        added,
        removed,
    }
}

fn index_first(checks: &[CheckResult]) -> BTreeMap<&str, &CheckResult> {
    let mut map = BTreeMap::new();
    for c in checks {
        map.entry(c.name.as_str()).or_insert(c);
    }
    map
}

fn duration_regression(
    name: &str,
    baseline: &CheckResult,
    current: &CheckResult,
    opts: &DiffOptions,
) -> Option<DurationRegression> {
    let base = baseline.duration_ms?;
    let curr = current.duration_ms?;
    if curr <= base {
        return None;
    }
    let delta_ms = curr - base;
    let mut flagged = false;

    if let Some(abs) = opts.duration_regression_abs_ms {
        if delta_ms > abs {
            flagged = true;
        }
    }
    if let Some(pct) = opts.duration_regression_pct {
        let allowed = base as f64 * (1.0 + pct / 100.0);
        if (curr as f64) > allowed {
            flagged = true;
        }
    }
    if !flagged {
        return None;
    }
    let delta_pct = if base == 0 {
        f64::INFINITY
    } else {
        (delta_ms as f64 / base as f64) * 100.0
    };
    Some(DurationRegression {
        name: name.to_string(),
        baseline_ms: base,
        current_ms: curr,
        delta_pct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckResult, Report, Severity};

    fn r(name: &str, version: &str) -> Report {
        Report::new(name, version)
    }

    #[test]
    fn identical_reports_are_clean() {
        let mut a = r("c", "0.1.0");
        a.push(CheckResult::pass("x"));
        a.push(CheckResult::pass("y").with_duration_ms(10));
        let b = a.clone();
        let d = diff_reports(&a, &b, &DiffOptions::default());
        assert!(d.is_clean());
    }

    #[test]
    fn newly_failing_detected() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a"));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::fail("a", Severity::Error));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        assert_eq!(d.newly_failing, vec!["a".to_string()]);
    }

    #[test]
    fn newly_passing_detected() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::fail("a", Severity::Error));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a"));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        assert_eq!(d.newly_passing, vec!["a".to_string()]);
    }

    #[test]
    fn added_and_removed_detected() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a"));
        prev.push(CheckResult::pass("gone"));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a"));
        curr.push(CheckResult::pass("new"));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        assert_eq!(d.added, vec!["new".to_string()]);
        assert_eq!(d.removed, vec!["gone".to_string()]);
    }

    #[test]
    fn severity_change_detected() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::warn("a", Severity::Warning));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::warn("a", Severity::Error));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        assert_eq!(d.severity_changes.len(), 1);
        assert_eq!(d.severity_changes[0].name, "a");
        assert_eq!(d.severity_changes[0].from, Some(Severity::Warning));
        assert_eq!(d.severity_changes[0].to, Some(Severity::Error));
    }

    #[test]
    fn duration_regression_pct_threshold() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a").with_duration_ms(100));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a").with_duration_ms(150));
        let d = diff_reports(
            &curr,
            &prev,
            &DiffOptions {
                duration_regression_pct: Some(20.0),
                duration_regression_abs_ms: None,
            },
        );
        assert_eq!(d.duration_regressions.len(), 1);
        let reg = &d.duration_regressions[0];
        assert_eq!(reg.name, "a");
        assert_eq!(reg.baseline_ms, 100);
        assert_eq!(reg.current_ms, 150);
        assert!((reg.delta_pct - 50.0).abs() < 0.0001);
    }

    #[test]
    fn duration_regression_below_threshold_ignored() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a").with_duration_ms(100));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a").with_duration_ms(105));
        let d = diff_reports(
            &curr,
            &prev,
            &DiffOptions {
                duration_regression_pct: Some(20.0),
                duration_regression_abs_ms: None,
            },
        );
        assert!(d.duration_regressions.is_empty());
    }

    #[test]
    fn duration_regression_abs_threshold() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a").with_duration_ms(100));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a").with_duration_ms(120));
        let d = diff_reports(
            &curr,
            &prev,
            &DiffOptions {
                duration_regression_pct: None,
                duration_regression_abs_ms: Some(10),
            },
        );
        assert_eq!(d.duration_regressions.len(), 1);
    }

    #[test]
    fn duration_regression_speedup_ignored() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a").with_duration_ms(100));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a").with_duration_ms(50));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        assert!(d.duration_regressions.is_empty());
    }

    #[test]
    fn diff_is_deterministic() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("z"));
        prev.push(CheckResult::pass("a"));
        prev.push(CheckResult::pass("m"));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::fail("z", Severity::Error));
        curr.push(CheckResult::fail("m", Severity::Error));
        curr.push(CheckResult::pass("a"));
        let d1 = diff_reports(&curr, &prev, &DiffOptions::default());
        let d2 = diff_reports(&curr, &prev, &DiffOptions::default());
        assert_eq!(d1, d2);
        // Names sorted alphabetically.
        assert_eq!(d1.newly_failing, vec!["m".to_string(), "z".to_string()]);
    }

    #[test]
    fn diff_round_trips_through_json() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::pass("a"));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::fail("a", Severity::Error));
        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        let json = serde_json::to_string(&d).unwrap();
        let back: Diff = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn summary_reports_clean_when_identical() {
        let mut a = r("c", "0.1.0");
        a.push(CheckResult::pass("x"));
        let b = a.clone();
        assert_eq!(
            diff_reports(&a, &b, &DiffOptions::default()).summary(),
            "clean"
        );
    }

    #[test]
    fn summary_lists_all_categories() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::fail("a", Severity::Error));
        prev.push(CheckResult::pass("gone"));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::pass("a")); // newly_passing
        curr.push(CheckResult::fail("b", Severity::Error)); // newly_failing + added
        curr.push(CheckResult::pass("new")); // newly_passing + added

        let d = diff_reports(&curr, &prev, &DiffOptions::default());
        let s = d.summary();
        assert!(s.contains("newly failing"));
        assert!(s.contains("newly passing"));
        assert!(s.contains("added"));
        assert!(s.contains("removed"));
    }

    #[test]
    fn summary_pluralizes_correctly() {
        let mut prev = r("c", "0.1.0");
        prev.push(CheckResult::warn("a", Severity::Warning));
        let mut curr = r("c", "0.1.0");
        curr.push(CheckResult::warn("a", Severity::Error));
        // Single severity change -> singular.
        let s = diff_reports(&curr, &prev, &DiffOptions::default()).summary();
        assert!(s.contains("1 severity change"));
        assert!(!s.contains("changes"));
    }
}
