//! SARIF 2.1.0 export for [`Report`] and [`MultiReport`].
//!
//! Maps fail-verdict and warn-verdict [`CheckResult`]s to SARIF results.
//! Pass and skip checks are intentionally NOT emitted — SARIF is a defect
//! report format, not a test-result format. Use [`crate::junit`] (or the
//! native JSON schema) when you need every check.
//!
//! Severity → SARIF level:
//!
//! | [`Severity`]         | SARIF `level` |
//! |----------------------|---------------|
//! | `Critical`, `Error`  | `error`       |
//! | `Warning`            | `warning`     |
//! | `Info`               | `note`        |
//! | `None` (unreachable for fail/warn) | `none` |
//!
//! [`Evidence`] payloads of kind [`EvidenceData::FileRef`] become SARIF
//! `physicalLocation` entries with `region.startLine` / `region.endLine`
//! when the [`FileRef`] carries a line range.
//!
//! For a [`MultiReport`], each constituent [`Report`] becomes a separate
//! SARIF `run`, so consumers can tell which producer emitted which finding.
//!
//! Available with the `sarif` feature.
//!
//! [`Evidence`]: crate::Evidence
//! [`EvidenceData::FileRef`]: crate::EvidenceData::FileRef
//! [`FileRef`]: crate::FileRef
//! [`Severity`]: crate::Severity
//! [`CheckResult`]: crate::CheckResult

use serde_json::{json, Value};

use crate::{CheckResult, EvidenceData, MultiReport, Report, Severity, Verdict};

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA_URI: &str =
    "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";
const TOOL_INFO_URI: &str = "https://github.com/jamesgober/dev-report";

/// Render `report` as a SARIF 2.1.0 document.
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, Report, Severity};
///
/// let mut r = Report::new("crate", "0.1.0").with_producer("dev-bench");
/// r.push(CheckResult::fail("oops", Severity::Error).with_detail("explodes"));
///
/// let sarif = dev_report::sarif::to_sarif(&r);
/// assert!(sarif.contains("\"version\": \"2.1.0\""));
/// assert!(sarif.contains("\"ruleId\": \"oops\""));
/// ```
pub fn to_sarif(report: &Report) -> String {
    let log = json!({
        "version": SARIF_VERSION,
        "$schema": SARIF_SCHEMA_URI,
        "runs": [run_for(report)]
    });
    serde_json::to_string_pretty(&log).expect("SARIF JSON is always serializable")
}

/// Render `multi` as a SARIF 2.1.0 document with one `run` per constituent
/// [`Report`].
///
/// # Example
///
/// ```
/// use dev_report::{CheckResult, MultiReport, Report, Severity};
///
/// let mut bench = Report::new("crate", "0.1.0").with_producer("dev-bench");
/// bench.push(CheckResult::fail("a", Severity::Error));
/// let mut chaos = Report::new("crate", "0.1.0").with_producer("dev-chaos");
/// chaos.push(CheckResult::warn("b", Severity::Warning));
///
/// let mut multi = MultiReport::new("crate", "0.1.0");
/// multi.push(bench);
/// multi.push(chaos);
///
/// let sarif = dev_report::sarif::multi_to_sarif(&multi);
/// assert!(sarif.contains("\"version\": \"2.1.0\""));
/// ```
pub fn multi_to_sarif(multi: &MultiReport) -> String {
    let runs: Vec<Value> = multi.reports.iter().map(run_for).collect();
    let log = json!({
        "version": SARIF_VERSION,
        "$schema": SARIF_SCHEMA_URI,
        "runs": runs
    });
    serde_json::to_string_pretty(&log).expect("SARIF JSON is always serializable")
}

fn run_for(report: &Report) -> Value {
    let driver_name = report.producer.as_deref().unwrap_or("dev-report");
    let results: Vec<Value> = report
        .checks
        .iter()
        .filter(|c| matches!(c.verdict, Verdict::Fail | Verdict::Warn))
        .map(result_for)
        .collect();
    json!({
        "tool": {
            "driver": {
                "name": driver_name,
                "informationUri": TOOL_INFO_URI
            }
        },
        "results": results
    })
}

fn result_for(check: &CheckResult) -> Value {
    let level = level_for(check.severity);
    let message_text = check.detail.clone().unwrap_or_else(|| check.name.clone());
    let mut result = json!({
        "ruleId": check.name,
        "level": level,
        "message": { "text": message_text }
    });
    let locations: Vec<Value> = check
        .evidence
        .iter()
        .filter_map(|e| match &e.data {
            EvidenceData::FileRef(f) => Some(location_for(f)),
            _ => None,
        })
        .collect();
    if !locations.is_empty() {
        result["locations"] = Value::Array(locations);
    }
    result
}

fn level_for(severity: Option<Severity>) -> &'static str {
    match severity {
        Some(Severity::Critical) | Some(Severity::Error) => "error",
        Some(Severity::Warning) => "warning",
        Some(Severity::Info) => "note",
        None => "none",
    }
}

fn location_for(file_ref: &crate::FileRef) -> Value {
    let mut physical = serde_json::Map::new();
    physical.insert("artifactLocation".into(), json!({ "uri": file_ref.path }));
    if file_ref.line_start.is_some() || file_ref.line_end.is_some() {
        let mut region = serde_json::Map::new();
        if let Some(s) = file_ref.line_start {
            region.insert("startLine".into(), Value::Number(s.into()));
        }
        if let Some(e) = file_ref.line_end {
            region.insert("endLine".into(), Value::Number(e.into()));
        }
        physical.insert("region".into(), Value::Object(region));
    }
    json!({ "physicalLocation": Value::Object(physical) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evidence, FileRef};

    #[test]
    fn skips_pass_and_skip_checks() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::pass("ok"));
        r.push(CheckResult::skip("not_applicable"));
        r.push(CheckResult::fail("oops", Severity::Error));
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["ruleId"], "oops");
    }

    #[test]
    fn severity_maps_to_sarif_level() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("a", Severity::Critical));
        r.push(CheckResult::fail("b", Severity::Error));
        r.push(CheckResult::warn("c", Severity::Warning));
        r.push(CheckResult::warn("d", Severity::Info));
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["level"], "error");
        assert_eq!(results[1]["level"], "error");
        assert_eq!(results[2]["level"], "warning");
        assert_eq!(results[3]["level"], "note");
    }

    #[test]
    fn file_ref_evidence_becomes_location() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(
            CheckResult::fail("oops", Severity::Error)
                .with_evidence(Evidence::file_ref_lines("site", "src/lib.rs", 10, 20))
                .with_evidence(Evidence::numeric("ignored", 1.0)),
        );
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        let locs = v["runs"][0]["results"][0]["locations"].as_array().unwrap();
        assert_eq!(locs.len(), 1);
        let phys = &locs[0]["physicalLocation"];
        assert_eq!(phys["artifactLocation"]["uri"], "src/lib.rs");
        assert_eq!(phys["region"]["startLine"], 10);
        assert_eq!(phys["region"]["endLine"], 20);
    }

    #[test]
    fn file_ref_without_line_range_omits_region() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(
            CheckResult::fail("oops", Severity::Error).with_evidence(Evidence {
                label: "src".into(),
                data: EvidenceData::FileRef(FileRef::new("src/lib.rs")),
            }),
        );
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        let phys = &v["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert_eq!(phys["artifactLocation"]["uri"], "src/lib.rs");
        assert!(phys.get("region").is_none());
    }

    #[test]
    fn multi_emits_one_run_per_constituent_report() {
        let mut bench = Report::new("c", "0.1.0").with_producer("dev-bench");
        bench.push(CheckResult::fail("a", Severity::Error));
        let mut chaos = Report::new("c", "0.1.0").with_producer("dev-chaos");
        chaos.push(CheckResult::warn("b", Severity::Warning));
        let mut multi = MultiReport::new("c", "0.1.0");
        multi.push(bench);
        multi.push(chaos);
        let sarif = multi_to_sarif(&multi);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        let runs = v["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0]["tool"]["driver"]["name"], "dev-bench");
        assert_eq!(runs[1]["tool"]["driver"]["name"], "dev-chaos");
    }

    #[test]
    fn output_is_deterministic() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("a", Severity::Error).with_detail("bad"));
        r.push(CheckResult::warn("b", Severity::Warning));
        let s1 = to_sarif(&r);
        let s2 = to_sarif(&r);
        assert_eq!(s1, s2);
    }

    #[test]
    fn empty_report_emits_empty_results() {
        let r = Report::new("c", "0.1.0").with_producer("p");
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn detail_becomes_message_text() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("a", Severity::Error).with_detail("the exact reason"));
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(
            v["runs"][0]["results"][0]["message"]["text"],
            "the exact reason"
        );
    }

    #[test]
    fn missing_detail_falls_back_to_name() {
        let mut r = Report::new("c", "0.1.0").with_producer("p");
        r.push(CheckResult::fail("the_check", Severity::Error));
        let sarif = to_sarif(&r);
        let v: Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(v["runs"][0]["results"][0]["message"]["text"], "the_check");
    }
}
