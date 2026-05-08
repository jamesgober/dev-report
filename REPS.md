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

A valid `Report` MUST have:

- `schema_version: u32`
- `subject: String`
- `subject_version: String`
- `started_at: DateTime<Utc>`
- `checks: Vec<CheckResult>`

A valid `Report` SHOULD have, when known:

- `producer: Option<String>`
- `finished_at: Option<DateTime<Utc>>`

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
