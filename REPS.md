# dev-report — Project Specification (REPS)

> Rust Engineering Project Specification.
> Normative language follows RFC 2119.

## 1. Purpose

`dev-report` MUST provide a stable, versioned report schema that all other
crates in the `dev-*` verification suite emit. The schema MUST be machine-
readable and decision-grade: an AI agent or CI script MUST be able to
consume a report and decide accept / reject / retry / escalate without
parsing free-form log output.

## 2. Scope

`dev-report` is a schema and serialization crate. It MUST NOT:

- Run tests
- Run benchmarks
- Inject failures
- Make HTTP calls
- Depend on any test runner (`criterion`, `tokio-test`, etc.)

It MAY provide:

- Producer / consumer traits
- JSON serialization
- Optional terminal pretty-printing (feature `terminal`)
- Optional Markdown export (feature `markdown`)

## 3. Stability

The `schema_version` field on `Report` MUST be incremented when a
breaking change is made to the schema. Consumers MUST check
`schema_version` before deserializing fields that may differ between
versions.

Through the `0.x` line, the schema MAY change between minor versions.
The `1.0` release MUST pin the schema, and the `schema_version` field
MUST follow strict semver from that point forward.

## 4. Terminology

- **Report** - a finalized record of one verification run on one subject.
- **Subject** - the crate or project being verified.
- **Producer** - any tool that emits a `Report` (e.g. `dev-bench`).
- **Consumer** - any tool or agent that reads a `Report` to make a decision.
- **Check** - a single test, measurement, or probe inside a report.
- **Verdict** - the outcome of a check or a whole report.

## 5. Required fields

### 5.1 `Report`

A valid `Report` MUST have:

- `schema_version: u32`
- `subject: String`
- `subject_version: String`
- `started_at: DateTime<Utc>`
- `checks: Vec<CheckResult>`

A valid `Report` SHOULD have, when known:

- `producer: Option<String>`
- `finished_at: Option<DateTime<Utc>>`

### 5.2 `CheckResult`

A valid `CheckResult` MUST have:

- `name: String` - stable identifier for the check (e.g. `compile`,
  `test::round_trip`).
- `verdict: Verdict` - one of `pass`, `fail`, `warn`, `skip`.
- `at: DateTime<Utc>` - time the check ran.

A valid `CheckResult` SHOULD have, when known:

- `severity: Option<Severity>` - one of `info`, `warning`, `error`,
  `critical`. MUST be `None` when verdict is `pass` or `skip`. MUST be
  `Some(_)` when verdict is `fail` or `warn`.
- `detail: Option<String>` - human-readable detail.
- `duration_ms: Option<u64>` - duration of the check, in milliseconds.

A valid `CheckResult` MAY have:

- `tags: Vec<String>` - free-form category tags. Defaults to empty.
  Producers SHOULD use stable identifiers (e.g. `bench`, `flaky`,
  `slow`) so consumers can filter without parsing names.
- `evidence: Vec<Evidence>` - structured backing data. Defaults to
  empty. See section 5.3.

Empty `tags` and `evidence` MUST be omitted from the JSON wire format
to preserve byte-equivalence with v0.1.0-shaped reports.

### 5.3 `Evidence`

An `Evidence` attachment carries decision-grade data backing a
`CheckResult`. Each `Evidence` MUST have:

- `label: String` - short human-readable label (e.g. `mean_ns`,
  `env`, `panic`, `source`).
- `data: EvidenceData` - typed payload (see 5.4).

### 5.4 `EvidenceData`

`EvidenceData` is an externally tagged enum. Exactly one variant MUST
be present in the wire form, keyed by the lowercase variant name:

- `numeric: f64` - a single labeled measurement.
- `key_value: { String: String }` - a string-to-string map. The
  serialization order MUST be deterministic (alphabetical by key).
- `snippet: String` - short text or code snippet.
- `file_ref: FileRef` - reference to a file, optionally with a line
  range (see 5.5).

### 5.5 `FileRef`

A `FileRef` MUST have:

- `path: String` - file path. MAY be absolute or relative to the
  producer's working directory.

A `FileRef` MAY have:

- `line_start: Option<u32>` - 1-indexed inclusive start line.
- `line_end: Option<u32>` - 1-indexed inclusive end line.

When `line_start` and `line_end` are both `Some`, `line_end` SHOULD be
`>= line_start`. Consumers MAY tolerate inverted ranges by swapping.

Absent `line_*` fields MUST be omitted from the JSON wire format
(`skip_serializing_if = "Option::is_none"`).

## 6. Verdict precedence

`Report::overall_verdict()` MUST resolve in the following order:

1. Any `Fail` check results in overall `Fail`.
2. Else any `Warn` check results in overall `Warn`.
3. Else any `Pass` check results in overall `Pass`.
4. Else (all `Skip` or empty) results in overall `Skip`.

This precedence MUST be stable across all `0.x` and `1.x` versions.

## 7. JSON wire format

The JSON wire format MUST round-trip through `to_json` / `from_json`
without information loss. Field names MUST use `snake_case`. Enum
variants MUST use `lowercase`.

### 7.1 Backward compatibility (additive fields)

When new optional fields are added during the `0.x` line:

- The new field MUST default cleanly when absent from the input
  (typically `Option::None` or an empty `Vec`).
- The new field MUST be omitted from the output when it carries the
  default value, so output is byte-stable for producers that do not
  use the new field.
- `schema_version` MUST NOT be bumped for additive fields.

A consumer reading a `0.1.0` report with a `0.2.x+` parser MUST see
the new field default-populated; a `0.1.0` parser reading a `0.2.x+`
report SHOULD ignore unknown fields (serde default behavior).

## 8. Optional features

### 8.1 `terminal`

When the `terminal` feature is enabled, `dev-report` MUST provide a
formatter that renders a `Report` to a TTY-friendly string. The
formatter:

- MUST render correctly under 80 columns for a typical report.
- MUST NOT add transitive dependencies that bloat the default build.
- MUST be a pure function of the input `Report` (no side effects, no
  global state, no locale dependence).

### 8.2 `markdown`

When the `markdown` feature is enabled, `dev-report` MUST provide a
formatter that renders a `Report` to a Markdown string. The formatter:

- MUST emit valid CommonMark-compatible Markdown.
- MUST preserve every fact in the report (verdict, severity, tags,
  evidence labels, durations).
- MUST be a pure function of the input `Report`.

Both formatters are output-only. There is no Markdown / terminal
parser. JSON remains the only round-trippable wire format.

## 9. Aggregation and diffing

### 9.1 `Report::diff`

`Report::diff(other: &Report) -> Diff` MUST be a pure function. The
returned `Diff` MUST call out:

- Newly failing checks (present and `fail` in `self`, not `fail` in `other`).
- Newly passing checks (present and `pass` in `self`, not `pass` in `other`).
- Severity changes (same check name, different severity).
- Duration regressions exceeding a configurable threshold.

Two diffs of the same input pair MUST produce equal `Diff` values.

### 9.2 `MultiReport`

A `MultiReport` aggregates multiple `Report`s emitted in a single CI
run by different producers. Aggregation MUST NOT silently merge checks
with the same name across producers; check identity is `(producer, name)`.

The `MultiReport`'s `overall_verdict` MUST follow the same precedence
rules as `Report::overall_verdict` applied across all checks from all
constituent reports.
