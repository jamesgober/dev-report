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

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "terminal")]
#[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
pub mod terminal;

#[cfg(feature = "markdown")]
#[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
pub mod markdown;

#[cfg(feature = "sarif")]
#[cfg_attr(docsrs, doc(cfg(feature = "sarif")))]
pub mod sarif;

#[cfg(feature = "junit")]
#[cfg_attr(docsrs, doc(cfg(feature = "junit")))]
pub mod junit;

mod diff;
pub use diff::{Diff, DiffOptions, DurationRegression, SeverityChange};

mod multi;
pub use multi::MultiReport;

/// Top-level verdict for a check or a whole report.
///
/// Precedence when summarized over many checks: `Fail` > `Warn` > `Pass` > `Skip`.
///
/// # Example
///
/// ```
/// use dev_report::Verdict;
///
/// let v = Verdict::Pass;
/// assert_ne!(v, Verdict::Fail);
/// ```
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
///
/// `None` for `Pass` and `Skip` verdicts; `Some(_)` for `Fail` and `Warn`.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Severity};
///
/// let c = CheckResult::fail("oops", Severity::Error);
/// assert_eq!(c.severity, Some(Severity::Error));
/// ```
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

/// Reference to a file at a specific (optional) line range.
///
/// # Example
///
/// ```
/// use dev_report::FileRef;
///
/// let r = FileRef::new("src/lib.rs").with_line_range(10, 20);
/// assert_eq!(r.path, "src/lib.rs");
/// assert_eq!(r.line_start, Some(10));
/// assert_eq!(r.line_end, Some(20));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRef {
    /// Path to the file. Either absolute or relative to the producer's CWD.
    pub path: String,
    /// Optional starting line (1-indexed, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Optional ending line (1-indexed, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
}

impl FileRef {
    /// Build a [`FileRef`] for the given path with no line range.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line_start: None,
            line_end: None,
        }
    }

    /// Attach a `[start, end]` line range (1-indexed, inclusive).
    pub fn with_line_range(mut self, start: u32, end: u32) -> Self {
        self.line_start = Some(start);
        self.line_end = Some(end);
        self
    }
}

/// Discriminator describing the shape of an [`Evidence`] payload.
///
/// Returned by [`Evidence::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    /// A single labeled numeric measurement.
    Numeric,
    /// A bag of string-to-string pairs.
    KeyValue,
    /// A short text or code snippet.
    Snippet,
    /// A reference to a file on disk.
    FileRef,
}

/// Typed payload for an [`Evidence`] attachment.
///
/// Externally tagged: a numeric evidence serializes as
/// `{ "numeric": 42.0 }`, a snippet as `{ "snippet": "..." }`, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceData {
    /// A single floating-point value (e.g. `ops_per_sec`, `mean_ns`).
    Numeric(f64),
    /// String-to-string pairs (e.g. environment, configuration).
    ///
    /// Stored as a `BTreeMap` so JSON output is deterministic.
    KeyValue(BTreeMap<String, String>),
    /// Short snippet of text or code.
    Snippet(String),
    /// File reference with optional line range.
    FileRef(FileRef),
}

/// A piece of structured evidence backing a [`CheckResult`].
///
/// Use this to attach decision-grade data (numbers, key-value pairs,
/// code snippets, file refs) instead of formatting them into the
/// free-form `detail` field. Consumers can read the typed payload
/// directly without parsing text.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Evidence};
///
/// let check = CheckResult::pass("bench::parse")
///     .with_evidence(Evidence::numeric("mean_ns", 1234.0))
///     .with_evidence(Evidence::numeric("baseline_ns", 1100.0));
///
/// assert_eq!(check.evidence.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Short human-readable label (e.g. `"ops_per_sec"`).
    pub label: String,
    /// Typed payload.
    pub data: EvidenceData,
}

impl Evidence {
    /// Build a numeric-evidence attachment.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::numeric("ops_per_sec", 12_500.0);
    /// assert_eq!(e.label, "ops_per_sec");
    /// ```
    pub fn numeric(label: impl Into<String>, value: f64) -> Self {
        // JSON does not have a representation for NaN / +Inf / -Inf;
        // serde_json::to_string would fail at serialize time. Coerce
        // non-finite values to 0.0 at construction so a `Report`
        // built from arbitrary measurements can always be serialized.
        let value = if value.is_finite() { value } else { 0.0 };
        Self {
            label: label.into(),
            data: EvidenceData::Numeric(value),
        }
    }

    /// Build a numeric-evidence attachment from an integer value.
    ///
    /// Preserves precision for counters that exceed `f64`'s 53-bit
    /// integer range (e.g. iteration counts, byte sizes). The value is
    /// stored as `f64` on the wire (the schema is unchanged), but
    /// callers don't have to perform a possibly-lossy `as f64` cast.
    ///
    /// For values up to `2^53` the round-trip is exact. Above that,
    /// precision degrades the same way it would for any `f64`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::numeric_int("iterations", 1_000_000_i64);
    /// assert_eq!(e.label, "iterations");
    /// ```
    pub fn numeric_int(label: impl Into<String>, value: i64) -> Self {
        Self::numeric(label, value as f64)
    }

    /// Build a key-value-evidence attachment from any iterable of pairs.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::kv("env", [("RUST_LOG", "debug"), ("CI", "true")]);
    /// assert_eq!(e.label, "env");
    /// ```
    pub fn kv<I, K, V>(label: impl Into<String>, pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let map: BTreeMap<String, String> = pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        Self {
            label: label.into(),
            data: EvidenceData::KeyValue(map),
        }
    }

    /// Build a snippet-evidence attachment.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::snippet("panic", "thread 'main' panicked at ...");
    /// assert_eq!(e.label, "panic");
    /// ```
    pub fn snippet(label: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            data: EvidenceData::Snippet(text.into()),
        }
    }

    /// Build a file-reference-evidence attachment with no line range.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::file_ref("source", "src/lib.rs");
    /// assert_eq!(e.label, "source");
    /// ```
    pub fn file_ref(label: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            data: EvidenceData::FileRef(FileRef::new(path)),
        }
    }

    /// Build a file-reference-evidence attachment with a `[start, end]`
    /// line range (1-indexed, inclusive).
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Evidence;
    ///
    /// let e = Evidence::file_ref_lines("call_site", "src/lib.rs", 42, 47);
    /// assert_eq!(e.label, "call_site");
    /// ```
    pub fn file_ref_lines(
        label: impl Into<String>,
        path: impl Into<String>,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            label: label.into(),
            data: EvidenceData::FileRef(FileRef::new(path).with_line_range(start, end)),
        }
    }

    /// Discriminator for the payload variant.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{Evidence, EvidenceKind};
    ///
    /// assert_eq!(Evidence::numeric("x", 1.0).kind(), EvidenceKind::Numeric);
    /// ```
    pub fn kind(&self) -> EvidenceKind {
        match &self.data {
            EvidenceData::Numeric(_) => EvidenceKind::Numeric,
            EvidenceData::KeyValue(_) => EvidenceKind::KeyValue,
            EvidenceData::Snippet(_) => EvidenceKind::Snippet,
            EvidenceData::FileRef(_) => EvidenceKind::FileRef,
        }
    }
}

/// Result of a single check.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Severity, Verdict};
///
/// let c = CheckResult::fail("unit::math", Severity::Error)
///     .with_detail("expected 42, got 41")
///     .with_duration_ms(7);
/// assert_eq!(c.verdict, Verdict::Fail);
/// ```
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
    /// Free-form tags for filtering (e.g. `"slow"`, `"flaky"`, `"bench"`).
    ///
    /// Defaults to empty. v0.1.0 reports deserialize cleanly with no tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Structured evidence backing this check.
    ///
    /// Defaults to empty. v0.1.0 reports deserialize cleanly with no evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
}

impl CheckResult {
    /// Build a passing check result with the given name.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Verdict};
    ///
    /// let c = CheckResult::pass("compile");
    /// assert_eq!(c.verdict, Verdict::Pass);
    /// assert!(c.severity.is_none());
    /// ```
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Pass,
            severity: None,
            detail: None,
            at: Utc::now(),
            duration_ms: None,
            tags: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// Build a failing check result with the given name and severity.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Severity, Verdict};
    ///
    /// let c = CheckResult::fail("test::round_trip", Severity::Error);
    /// assert_eq!(c.verdict, Verdict::Fail);
    /// assert_eq!(c.severity, Some(Severity::Error));
    /// ```
    pub fn fail(name: impl Into<String>, severity: Severity) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Fail,
            severity: Some(severity),
            detail: None,
            at: Utc::now(),
            duration_ms: None,
            tags: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// Build a warning check result with the given name and severity.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Severity, Verdict};
    ///
    /// let c = CheckResult::warn("flaky", Severity::Warning);
    /// assert_eq!(c.verdict, Verdict::Warn);
    /// ```
    pub fn warn(name: impl Into<String>, severity: Severity) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Warn,
            severity: Some(severity),
            detail: None,
            at: Utc::now(),
            duration_ms: None,
            tags: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// Build a skipped check result with the given name.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Verdict};
    ///
    /// let c = CheckResult::skip("not_applicable");
    /// assert_eq!(c.verdict, Verdict::Skip);
    /// ```
    pub fn skip(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Skip,
            severity: None,
            detail: None,
            at: Utc::now(),
            duration_ms: None,
            tags: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// Attach a human-readable detail to this check result.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::CheckResult;
    ///
    /// let c = CheckResult::pass("a").with_detail("ran in single thread");
    /// assert_eq!(c.detail.as_deref(), Some("ran in single thread"));
    /// ```
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Attach a duration measurement (milliseconds) to this check result.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::CheckResult;
    ///
    /// let c = CheckResult::pass("a").with_duration_ms(42);
    /// assert_eq!(c.duration_ms, Some(42));
    /// ```
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Override the severity of this check result.
    ///
    /// Useful when escalating or de-escalating a check after construction
    /// (e.g. promote a `Warn+Warning` to `Warn+Error` based on a config flag).
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Severity};
    ///
    /// let c = CheckResult::warn("flaky", Severity::Warning)
    ///     .with_severity(Severity::Error);
    /// assert_eq!(c.severity, Some(Severity::Error));
    /// ```
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Attach a single tag to this check result.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::CheckResult;
    ///
    /// let c = CheckResult::pass("compile").with_tag("slow");
    /// assert!(c.has_tag("slow"));
    /// ```
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Attach many tags at once from any iterable of strings.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::CheckResult;
    ///
    /// let c = CheckResult::pass("compile").with_tags(["slow", "flaky"]);
    /// assert!(c.has_tag("flaky"));
    /// ```
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Return `true` if this check has the given tag.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::CheckResult;
    ///
    /// let c = CheckResult::pass("compile").with_tag("slow");
    /// assert!(c.has_tag("slow"));
    /// assert!(!c.has_tag("flaky"));
    /// ```
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Attach a single piece of [`Evidence`] to this check result.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Evidence};
    ///
    /// let c = CheckResult::pass("bench")
    ///     .with_evidence(Evidence::numeric("mean_ns", 1234.0));
    /// assert_eq!(c.evidence.len(), 1);
    /// ```
    pub fn with_evidence(mut self, e: Evidence) -> Self {
        self.evidence.push(e);
        self
    }

    /// Attach many [`Evidence`] items at once from any iterable.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Evidence};
    ///
    /// let c = CheckResult::pass("bench").with_evidences([
    ///     Evidence::numeric("mean_ns", 1234.0),
    ///     Evidence::numeric("baseline_ns", 1100.0),
    /// ]);
    /// assert_eq!(c.evidence.len(), 2);
    /// ```
    pub fn with_evidences<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = Evidence>,
    {
        self.evidence.extend(items);
        self
    }
}

/// A full report. The output of one verification run.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report, Verdict};
///
/// let mut r = Report::new("my-crate", "0.1.0").with_producer("my-harness");
/// r.push(CheckResult::pass("compile"));
/// r.finish();
/// assert_eq!(r.overall_verdict(), Verdict::Pass);
/// ```
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
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Report;
    ///
    /// let r = Report::new("my-crate", "0.1.0");
    /// assert_eq!(r.subject, "my-crate");
    /// assert_eq!(r.schema_version, 1);
    /// ```
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
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Report;
    ///
    /// let r = Report::new("crate", "0.1.0").with_producer("dev-bench");
    /// assert_eq!(r.producer.as_deref(), Some("dev-bench"));
    /// ```
    pub fn with_producer(mut self, producer: impl Into<String>) -> Self {
        self.producer = Some(producer.into());
        self
    }

    /// Append a check result to this report.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report};
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// r.push(CheckResult::pass("compile"));
    /// assert_eq!(r.checks.len(), 1);
    /// ```
    pub fn push(&mut self, result: CheckResult) {
        self.checks.push(result);
    }

    /// Mark the report as finished, stamping the finish time.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Report;
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// r.finish();
    /// assert!(r.finished_at.is_some());
    /// ```
    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    /// Override `started_at` with a fixed timestamp.
    ///
    /// Useful when reconstructing a `Report` from external data
    /// (replay, import from a different schema, deterministic test
    /// fixtures). Most producers should let `Report::new` capture the
    /// real start time and not call this.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::TimeZone;
    /// use dev_report::Report;
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// let frozen = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    /// r.set_started_at(frozen);
    /// assert_eq!(r.started_at, frozen);
    /// ```
    pub fn set_started_at(&mut self, ts: DateTime<Utc>) {
        self.started_at = ts;
    }

    /// Override `finished_at` with a fixed timestamp.
    ///
    /// Useful for replay / import scenarios where the real finish
    /// time is known but `Utc::now()` would be wrong.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::TimeZone;
    /// use dev_report::Report;
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// let frozen = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap();
    /// r.set_finished_at(Some(frozen));
    /// assert_eq!(r.finished_at, Some(frozen));
    /// ```
    pub fn set_finished_at(&mut self, ts: Option<DateTime<Utc>>) {
        self.finished_at = ts;
    }

    /// Count of checks per verdict, returned as `(pass, fail, warn, skip)`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity};
    ///
    /// let mut r = Report::new("c", "0.1.0");
    /// r.push(CheckResult::pass("a"));
    /// r.push(CheckResult::pass("b"));
    /// r.push(CheckResult::fail("c", Severity::Error));
    /// let (pass, fail, warn, skip) = r.verdict_counts();
    /// assert_eq!((pass, fail, warn, skip), (2, 1, 0, 0));
    /// ```
    pub fn verdict_counts(&self) -> (usize, usize, usize, usize) {
        let (mut p, mut f, mut w, mut s) = (0, 0, 0, 0);
        for c in &self.checks {
            match c.verdict {
                Verdict::Pass => p += 1,
                Verdict::Fail => f += 1,
                Verdict::Warn => w += 1,
                Verdict::Skip => s += 1,
            }
        }
        (p, f, w, s)
    }

    /// Compute the overall verdict for this report.
    ///
    /// Rules:
    /// - Any `Fail` -> `Fail`
    /// - Else any `Warn` -> `Warn`
    /// - Else any `Pass` -> `Pass`
    /// - Else (all `Skip` or empty) -> `Skip`
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity, Verdict};
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// r.push(CheckResult::pass("a"));
    /// r.push(CheckResult::fail("b", Severity::Error));
    /// assert_eq!(r.overall_verdict(), Verdict::Fail);
    /// ```
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

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Pass`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report};
    ///
    /// let mut r = Report::new("c", "0.1.0");
    /// r.push(CheckResult::pass("ok"));
    /// assert!(r.passed());
    /// ```
    pub fn passed(&self) -> bool {
        self.overall_verdict() == Verdict::Pass
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Fail`.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity};
    ///
    /// let mut r = Report::new("c", "0.1.0");
    /// r.push(CheckResult::fail("oops", Severity::Error));
    /// assert!(r.failed());
    /// ```
    pub fn failed(&self) -> bool {
        self.overall_verdict() == Verdict::Fail
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Warn`.
    pub fn warned(&self) -> bool {
        self.overall_verdict() == Verdict::Warn
    }

    /// `true` when [`overall_verdict`](Self::overall_verdict) is `Skip`
    /// (all checks were skipped, or there were no checks).
    pub fn skipped(&self) -> bool {
        self.overall_verdict() == Verdict::Skip
    }

    /// Iterate over checks whose [`severity`](CheckResult::severity)
    /// matches the given level.
    ///
    /// `Pass` and `Skip` checks have `severity = None` and never match.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity};
    ///
    /// let mut r = Report::new("c", "0.1.0");
    /// r.push(CheckResult::fail("a", Severity::Error));
    /// r.push(CheckResult::warn("b", Severity::Warning));
    /// r.push(CheckResult::fail("c", Severity::Error));
    ///
    /// let errors: Vec<_> = r.checks_with_severity(Severity::Error).collect();
    /// assert_eq!(errors.len(), 2);
    /// ```
    pub fn checks_with_severity(&self, severity: Severity) -> impl Iterator<Item = &CheckResult> {
        self.checks
            .iter()
            .filter(move |c| c.severity == Some(severity))
    }

    /// Iterate over checks that carry the given tag.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report};
    ///
    /// let mut r = Report::new("crate", "0.1.0");
    /// r.push(CheckResult::pass("a").with_tag("slow"));
    /// r.push(CheckResult::pass("b"));
    /// r.push(CheckResult::pass("c").with_tag("slow"));
    ///
    /// let slow: Vec<_> = r.checks_with_tag("slow").collect();
    /// assert_eq!(slow.len(), 2);
    /// ```
    pub fn checks_with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a CheckResult> {
        self.checks.iter().filter(move |c| c.has_tag(tag))
    }

    /// Serialize this report to JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Report;
    ///
    /// let r = Report::new("crate", "0.1.0");
    /// let json = r.to_json().unwrap();
    /// assert!(json.contains("\"subject\": \"crate\""));
    /// ```
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a report from JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::Report;
    ///
    /// let r = Report::new("crate", "0.1.0");
    /// let json = r.to_json().unwrap();
    /// let parsed = Report::from_json(&json).unwrap();
    /// assert_eq!(parsed.subject, "crate");
    /// ```
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }

    /// Render this report to a TTY-friendly string. Monochrome.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal(&self) -> String {
        terminal::to_terminal(self)
    }

    /// Render this report with ANSI color codes.
    ///
    /// Available with the `terminal` feature.
    #[cfg(feature = "terminal")]
    #[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
    pub fn to_terminal_color(&self) -> String {
        terminal::to_terminal_color(self)
    }

    /// Render this report to a Markdown string.
    ///
    /// Available with the `markdown` feature.
    #[cfg(feature = "markdown")]
    #[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
    pub fn to_markdown(&self) -> String {
        markdown::to_markdown(self)
    }

    /// Render this report as a SARIF 2.1.0 document.
    ///
    /// Only `Fail` and `Warn` checks are emitted; `Pass` and `Skip` are
    /// omitted (SARIF is a defect report format). See [`crate::sarif`]
    /// for the severity-to-level mapping.
    ///
    /// Available with the `sarif` feature.
    #[cfg(feature = "sarif")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sarif")))]
    pub fn to_sarif(&self) -> String {
        sarif::to_sarif(self)
    }

    /// Render this report as a JUnit XML document.
    ///
    /// Every check becomes a `<testcase>`; fail verdicts emit a
    /// `<failure>` child, skip verdicts emit a `<skipped/>` child. See
    /// [`crate::junit`] for the verdict-to-element mapping.
    ///
    /// Available with the `junit` feature.
    #[cfg(feature = "junit")]
    #[cfg_attr(docsrs, doc(cfg(feature = "junit")))]
    pub fn to_junit_xml(&self) -> String {
        junit::to_junit_xml(self)
    }

    /// Compare this report against a baseline using default options.
    ///
    /// `self` is the new report; `baseline` is the previous one.
    /// Default options flag duration regressions over 20% slower.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, Report, Severity};
    ///
    /// let mut prev = Report::new("c", "0.1.0");
    /// prev.push(CheckResult::pass("a"));
    ///
    /// let mut curr = Report::new("c", "0.1.0");
    /// curr.push(CheckResult::fail("a", Severity::Error));
    ///
    /// let diff = curr.diff(&prev);
    /// assert_eq!(diff.newly_failing, vec!["a".to_string()]);
    /// ```
    pub fn diff(&self, baseline: &Self) -> Diff {
        diff::diff_reports(self, baseline, &DiffOptions::default())
    }

    /// Compare this report against a baseline using custom options.
    ///
    /// # Example
    ///
    /// ```
    /// use dev_report::{CheckResult, DiffOptions, Report};
    ///
    /// let mut prev = Report::new("c", "0.1.0");
    /// prev.push(CheckResult::pass("a").with_duration_ms(100));
    ///
    /// let mut curr = Report::new("c", "0.1.0");
    /// curr.push(CheckResult::pass("a").with_duration_ms(150));
    ///
    /// let opts = DiffOptions {
    ///     duration_regression_pct: Some(10.0),
    ///     duration_regression_abs_ms: None,
    /// };
    /// let diff = curr.diff_with(&prev, &opts);
    /// assert_eq!(diff.duration_regressions.len(), 1);
    /// ```
    pub fn diff_with(&self, baseline: &Self, opts: &DiffOptions) -> Diff {
        diff::diff_reports(self, baseline, opts)
    }
}

/// A producer of reports. Implement this on your harness type to integrate
/// with the dev-* suite.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Producer, Report};
///
/// struct CompileChecker;
/// impl Producer for CompileChecker {
///     fn produce(&self) -> Report {
///         let mut r = Report::new("my-crate", "0.1.0").with_producer("compile-checker");
///         r.push(CheckResult::pass("compile"));
///         r.finish();
///         r
///     }
/// }
///
/// let r = CompileChecker.produce();
/// assert_eq!(r.checks.len(), 1);
/// ```
pub trait Producer {
    /// Run the producer and return a finalized report.
    ///
    /// Implementations SHOULD encode setup failures as `Fail` checks
    /// inside the returned `Report` rather than panicking.
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

    #[test]
    fn tags_attach_and_query() {
        let c = CheckResult::pass("compile")
            .with_tag("slow")
            .with_tags(["flaky", "bench"]);
        assert!(c.has_tag("slow"));
        assert!(c.has_tag("flaky"));
        assert!(c.has_tag("bench"));
        assert!(!c.has_tag("missing"));
        assert_eq!(c.tags.len(), 3);
    }

    #[test]
    fn evidence_constructors_set_kind() {
        assert_eq!(Evidence::numeric("x", 1.0).kind(), EvidenceKind::Numeric);
        assert_eq!(
            Evidence::kv("env", [("K", "V")]).kind(),
            EvidenceKind::KeyValue
        );
        assert_eq!(
            Evidence::snippet("log", "boom").kind(),
            EvidenceKind::Snippet
        );
        assert_eq!(
            Evidence::file_ref("src", "lib.rs").kind(),
            EvidenceKind::FileRef
        );
        assert_eq!(
            Evidence::file_ref_lines("src", "lib.rs", 1, 2).kind(),
            EvidenceKind::FileRef
        );
    }

    #[test]
    fn evidence_round_trips_through_json() {
        let mut r = Report::new("subject", "0.2.0");
        r.push(
            CheckResult::pass("bench::parse")
                .with_tag("bench")
                .with_evidence(Evidence::numeric("mean_ns", 1234.5))
                .with_evidence(Evidence::kv("env", [("RUST_LOG", "debug"), ("CI", "true")]))
                .with_evidence(Evidence::snippet("note", "fast path taken"))
                .with_evidence(Evidence::file_ref_lines("site", "src/parse.rs", 10, 20)),
        );
        r.finish();

        let json = r.to_json().unwrap();
        let parsed = Report::from_json(&json).unwrap();
        assert_eq!(parsed.checks.len(), 1);
        let c = &parsed.checks[0];
        assert_eq!(c.tags, vec!["bench".to_string()]);
        assert_eq!(c.evidence.len(), 4);
        assert_eq!(c.evidence[0].kind(), EvidenceKind::Numeric);
        assert_eq!(c.evidence[1].kind(), EvidenceKind::KeyValue);
        assert_eq!(c.evidence[2].kind(), EvidenceKind::Snippet);
        assert_eq!(c.evidence[3].kind(), EvidenceKind::FileRef);
    }

    #[test]
    fn v0_1_0_json_deserializes_with_empty_tags_and_evidence() {
        // A v0.1.0-shaped report (no tags, no evidence) MUST still parse.
        let v0_1_0_json = r#"{
            "schema_version": 1,
            "subject": "legacy",
            "subject_version": "0.1.0",
            "producer": "dev-report-self-test",
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "2026-01-01T00:00:01Z",
            "checks": [
                {
                    "name": "compile",
                    "verdict": "pass",
                    "severity": null,
                    "detail": null,
                    "at": "2026-01-01T00:00:00Z",
                    "duration_ms": null
                }
            ]
        }"#;
        let parsed = Report::from_json(v0_1_0_json).unwrap();
        assert_eq!(parsed.checks.len(), 1);
        let c = &parsed.checks[0];
        assert!(c.tags.is_empty());
        assert!(c.evidence.is_empty());
        assert_eq!(parsed.overall_verdict(), Verdict::Pass);
    }

    #[test]
    fn checks_with_tag_filters() {
        let mut r = Report::new("subject", "0.2.0");
        r.push(CheckResult::pass("a").with_tag("slow"));
        r.push(CheckResult::pass("b"));
        r.push(CheckResult::pass("c").with_tags(["slow", "flaky"]));
        let slow: Vec<&CheckResult> = r.checks_with_tag("slow").collect();
        assert_eq!(slow.len(), 2);
        assert_eq!(slow[0].name, "a");
        assert_eq!(slow[1].name, "c");
    }

    #[test]
    fn with_severity_overrides_severity() {
        let c = CheckResult::warn("x", Severity::Warning).with_severity(Severity::Error);
        assert_eq!(c.severity, Some(Severity::Error));
    }

    #[test]
    fn empty_tags_and_evidence_are_omitted_in_json() {
        let mut r = Report::new("s", "0.2.0");
        r.push(CheckResult::pass("a"));
        let json = r.to_json().unwrap();
        assert!(!json.contains("\"tags\""));
        assert!(!json.contains("\"evidence\""));
    }

    #[test]
    fn evidence_numeric_int_preserves_value() {
        let e = Evidence::numeric_int("count", 1_000_000_i64);
        if let EvidenceData::Numeric(n) = e.data {
            assert_eq!(n as i64, 1_000_000_i64);
        } else {
            panic!("expected Numeric");
        }
    }

    #[test]
    fn evidence_numeric_coerces_nan_and_inf_to_zero() {
        // JSON has no representation for NaN / Inf, so serializing a
        // report containing them would fail at runtime. The constructor
        // coerces non-finite to 0.0 so a Report can always be serialized.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let e = Evidence::numeric("x", bad);
            if let EvidenceData::Numeric(n) = e.data {
                assert_eq!(n, 0.0, "non-finite input should coerce to 0.0");
            } else {
                panic!("expected Numeric");
            }
        }
        // A round-trip through JSON now succeeds even when the original
        // measurement was non-finite.
        let mut r = Report::new("c", "0.1.0");
        r.push(CheckResult::pass("k").with_evidence(Evidence::numeric("ratio", f64::NAN)));
        let json = r.to_json().expect("non-finite must not break serialization");
        assert!(json.contains("\"ratio\""));
    }

    #[test]
    fn report_passed_failed_warned_skipped_shortcuts() {
        let mut p = Report::new("c", "0.1.0");
        p.push(CheckResult::pass("ok"));
        assert!(p.passed() && !p.failed() && !p.warned() && !p.skipped());

        let mut f = Report::new("c", "0.1.0");
        f.push(CheckResult::fail("oops", Severity::Error));
        assert!(f.failed() && !f.passed());

        let mut w = Report::new("c", "0.1.0");
        w.push(CheckResult::warn("flaky", Severity::Warning));
        assert!(w.warned() && !w.passed() && !w.failed());

        let s = Report::new("c", "0.1.0");
        assert!(s.skipped() && !s.passed());
    }

    #[test]
    fn checks_with_severity_filters_by_severity() {
        let mut r = Report::new("c", "0.1.0");
        r.push(CheckResult::fail("a", Severity::Error));
        r.push(CheckResult::warn("b", Severity::Warning));
        r.push(CheckResult::fail("c", Severity::Error));
        r.push(CheckResult::pass("d"));

        let errs: Vec<_> = r.checks_with_severity(Severity::Error).collect();
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].name, "a");
        assert_eq!(errs[1].name, "c");

        let warns: Vec<_> = r.checks_with_severity(Severity::Warning).collect();
        assert_eq!(warns.len(), 1);
    }

    #[test]
    fn report_verdict_counts() {
        let mut r = Report::new("c", "0.1.0");
        r.push(CheckResult::pass("a"));
        r.push(CheckResult::pass("b"));
        r.push(CheckResult::fail("c", Severity::Error));
        r.push(CheckResult::warn("d", Severity::Warning));
        r.push(CheckResult::skip("e"));
        assert_eq!(r.verdict_counts(), (2, 1, 1, 1));
    }

    #[test]
    fn report_set_started_finished_at_overrides() {
        use chrono::TimeZone;
        let mut r = Report::new("c", "0.1.0");
        let frozen_start = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let frozen_end = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap();
        r.set_started_at(frozen_start);
        r.set_finished_at(Some(frozen_end));
        assert_eq!(r.started_at, frozen_start);
        assert_eq!(r.finished_at, Some(frozen_end));
        // Round-trip through JSON preserves the override.
        let json = r.to_json().unwrap();
        let parsed = Report::from_json(&json).unwrap();
        assert_eq!(parsed.started_at, frozen_start);
        assert_eq!(parsed.finished_at, Some(frozen_end));
    }

    // ------------------------------------------------------------
    // Verdict precedence: explicit coverage of every transition edge.
    // Required by DIRECTIVES.md section 7 and REPS section 6.
    // Order: Fail > Warn > Pass > Skip (and empty -> Skip).
    // ------------------------------------------------------------

    fn r_with(checks: &[Verdict]) -> Report {
        let mut r = Report::new("vp", "0.0.0");
        for v in checks {
            r.push(match v {
                Verdict::Pass => CheckResult::pass("c"),
                Verdict::Fail => CheckResult::fail("c", Severity::Error),
                Verdict::Warn => CheckResult::warn("c", Severity::Warning),
                Verdict::Skip => CheckResult::skip("c"),
            });
        }
        r
    }

    #[test]
    fn vp_empty_is_skip() {
        assert_eq!(r_with(&[]).overall_verdict(), Verdict::Skip);
    }

    #[test]
    fn vp_only_skip_is_skip() {
        assert_eq!(
            r_with(&[Verdict::Skip, Verdict::Skip]).overall_verdict(),
            Verdict::Skip
        );
    }

    #[test]
    fn vp_only_pass_is_pass() {
        assert_eq!(r_with(&[Verdict::Pass]).overall_verdict(), Verdict::Pass);
    }

    #[test]
    fn vp_pass_with_skip_is_pass() {
        assert_eq!(
            r_with(&[Verdict::Skip, Verdict::Pass, Verdict::Skip]).overall_verdict(),
            Verdict::Pass
        );
    }

    #[test]
    fn vp_only_warn_is_warn() {
        assert_eq!(r_with(&[Verdict::Warn]).overall_verdict(), Verdict::Warn);
    }

    #[test]
    fn vp_warn_with_pass_is_warn() {
        assert_eq!(
            r_with(&[Verdict::Pass, Verdict::Warn]).overall_verdict(),
            Verdict::Warn
        );
    }

    #[test]
    fn vp_warn_with_skip_is_warn() {
        assert_eq!(
            r_with(&[Verdict::Skip, Verdict::Warn]).overall_verdict(),
            Verdict::Warn
        );
    }

    #[test]
    fn vp_warn_with_pass_and_skip_is_warn() {
        assert_eq!(
            r_with(&[Verdict::Pass, Verdict::Skip, Verdict::Warn]).overall_verdict(),
            Verdict::Warn
        );
    }

    #[test]
    fn vp_only_fail_is_fail() {
        assert_eq!(r_with(&[Verdict::Fail]).overall_verdict(), Verdict::Fail);
    }

    #[test]
    fn vp_fail_with_pass_is_fail() {
        assert_eq!(
            r_with(&[Verdict::Pass, Verdict::Fail]).overall_verdict(),
            Verdict::Fail
        );
    }

    #[test]
    fn vp_fail_with_warn_is_fail() {
        assert_eq!(
            r_with(&[Verdict::Warn, Verdict::Fail]).overall_verdict(),
            Verdict::Fail
        );
    }

    #[test]
    fn vp_fail_with_skip_is_fail() {
        assert_eq!(
            r_with(&[Verdict::Skip, Verdict::Fail]).overall_verdict(),
            Verdict::Fail
        );
    }

    #[test]
    fn vp_fail_dominates_all_others() {
        assert_eq!(
            r_with(&[Verdict::Skip, Verdict::Pass, Verdict::Warn, Verdict::Fail,])
                .overall_verdict(),
            Verdict::Fail
        );
    }

    #[test]
    fn vp_order_independence() {
        // Precedence MUST NOT depend on insertion order.
        let a = r_with(&[Verdict::Fail, Verdict::Warn, Verdict::Pass]).overall_verdict();
        let b = r_with(&[Verdict::Pass, Verdict::Warn, Verdict::Fail]).overall_verdict();
        let c = r_with(&[Verdict::Warn, Verdict::Pass, Verdict::Fail]).overall_verdict();
        assert_eq!(a, Verdict::Fail);
        assert_eq!(a, b);
        assert_eq!(b, c);
    }
}
