//! # dev-report
//!
//! Structured, machine-readable reports for AI-assisted Rust development.
//!
//! `dev-report` is the foundation schema of the `dev-*` verification suite.
//! Every other crate in the suite (`dev-bench`, `dev-fixtures`, `dev-async`,
//! `dev-stress`, `dev-chaos`) emits results that conform to this schema.
//!
//! ## Why a separate crate
//!
//! AI agents need decision-grade output. A test runner that prints colored
//! checkmarks to a TTY is unreadable to an agent. `dev-report` defines a
//! stable, versioned schema that:
//!
//! - Serializes to JSON for programmatic consumption
//! - Carries enough evidence for an agent to decide accept / reject / retry
//! - Keeps verdicts separate from logs so consumers do not have to parse text
//!
//! ## Quick example
//!
//! ```no_run
//! use dev_report::{Report, Verdict, Severity, CheckResult};
//!
//! let mut report = Report::new("my-crate", "0.1.0");
//! report.push(CheckResult::pass("compile"));
//! report.push(CheckResult::fail("test_round_trip", Severity::Error)
//!     .with_detail("expected 42, got 41"));
//!
//! let verdict = report.overall_verdict();
//! let json = report.to_json().unwrap();
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Top-level verdict for a check or a whole report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Check passed. No action required.
    Pass,
    /// Check failed. Action required.
    Fail,
    /// Check produced a warning. Review recommended.
    Warn,
    /// Check was skipped. No data to report.
    Skip,
}

/// Severity classification when a check fails or warns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational. Does not block acceptance.
    Info,
    /// Warning. Acceptance allowed with explicit acknowledgement.
    Warning,
    /// Error. Blocks acceptance.
    Error,
    /// Critical. Blocks acceptance and signals a regression.
    Critical,
}

/// Result of a single check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Stable identifier for the check (e.g. `compile`, `test::round_trip`).
    pub name: String,
    /// Outcome of the check.
    pub verdict: Verdict,
    /// Severity when the verdict is `Fail` or `Warn`. `None` for `Pass` and `Skip`.
    pub severity: Option<Severity>,
    /// Human-readable detail. Optional.
    pub detail: Option<String>,
    /// Time the check ran. UTC.
    pub at: DateTime<Utc>,
    /// Duration of the check, in milliseconds. Optional.
    pub duration_ms: Option<u64>,
}

impl CheckResult {
    /// Build a passing check result with the given name.
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Pass,
            severity: None,
            detail: None,
            at: Utc::now(),
            duration_ms: None,
        }
    }

    /// Build a failing check result with the given name and severity.
    pub fn fail(name: impl Into<String>, severity: Severity) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Fail,
            severity: Some(severity),
            detail: None,
            at: Utc::now(),
            duration_ms: None,
        }
    }

    /// Build a warning check result with the given name and severity.
    pub fn warn(name: impl Into<String>, severity: Severity) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Warn,
            severity: Some(severity),
            detail: None,
            at: Utc::now(),
            duration_ms: None,
        }
    }

    /// Build a skipped check result with the given name.
    pub fn skip(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Skip,
            severity: None,
            detail: None,
            at: Utc::now(),
            duration_ms: None,
        }
    }

    /// Attach a human-readable detail to this check result.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Attach a duration measurement (milliseconds) to this check result.
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }
}

/// A full report. The output of one verification run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Schema version for this report format.
    pub schema_version: u32,
    /// Crate or project being reported on.
    pub subject: String,
    /// Version of the subject at the time of the run.
    pub subject_version: String,
    /// Producer of the report (e.g. `dev-bench`, `dev-async`).
    pub producer: Option<String>,
    /// Time the report was started.
    pub started_at: DateTime<Utc>,
    /// Time the report was finalized.
    pub finished_at: Option<DateTime<Utc>>,
    /// All individual check results in this report.
    pub checks: Vec<CheckResult>,
}

impl Report {
    /// Begin a new report for the given subject and version.
    pub fn new(subject: impl Into<String>, subject_version: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            subject: subject.into(),
            subject_version: subject_version.into(),
            producer: None,
            started_at: Utc::now(),
            finished_at: None,
            checks: Vec::new(),
        }
    }

    /// Set the producer of this report.
    pub fn with_producer(mut self, producer: impl Into<String>) -> Self {
        self.producer = Some(producer.into());
        self
    }

    /// Append a check result to this report.
    pub fn push(&mut self, result: CheckResult) {
        self.checks.push(result);
    }

    /// Mark the report as finished, stamping the finish time.
    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    /// Compute the overall verdict for this report.
    ///
    /// Rules:
    /// - Any `Fail` -> `Fail`
    /// - Else any `Warn` -> `Warn`
    /// - Else any `Pass` -> `Pass`
    /// - Else (all `Skip` or empty) -> `Skip`
    pub fn overall_verdict(&self) -> Verdict {
        let mut saw_fail = false;
        let mut saw_warn = false;
        let mut saw_pass = false;
        for c in &self.checks {
            match c.verdict {
                Verdict::Fail => saw_fail = true,
                Verdict::Warn => saw_warn = true,
                Verdict::Pass => saw_pass = true,
                Verdict::Skip => {}
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

    /// Serialize this report to JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a report from JSON.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}

/// A producer of reports. Implement this on your harness type to integrate
/// with the dev-* suite.
pub trait Producer {
    /// Run the producer and return a finalized report.
    fn produce(&self) -> Report;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_roundtrip_a_report() {
        let mut r = Report::new("widget", "0.1.0").with_producer("dev-report-self-test");
        r.push(CheckResult::pass("compile"));
        r.push(CheckResult::fail("unit::math", Severity::Error).with_detail("off by one"));
        r.finish();

        let json = r.to_json().unwrap();
        let parsed = Report::from_json(&json).unwrap();
        assert_eq!(parsed.subject, "widget");
        assert_eq!(parsed.checks.len(), 2);
        assert_eq!(parsed.overall_verdict(), Verdict::Fail);
    }

    #[test]
    fn empty_report_is_skip() {
        let r = Report::new("nothing", "0.0.0");
        assert_eq!(r.overall_verdict(), Verdict::Skip);
    }
}
