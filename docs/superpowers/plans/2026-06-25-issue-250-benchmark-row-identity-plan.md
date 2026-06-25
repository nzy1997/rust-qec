# Issue 250 Benchmark Row Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add stable benchmark row identities and use them to safely merge compatible repeated benchmark rows.

**Architecture:** Add identity computation and JSONL serialization support to `BenchmarkResultRow`, then make merge group by identity and validate duplicate rows before summing additive counters. Keep the identity derived from row content instead of storing mutable state.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, existing `sha2` dependency, existing `rsinter` integration tests.

## Global Constraints

- Only update `rsinter/src/bench/result.rs`, `rsinter/src/bench/merge.rs`, `rsinter/src/bin/rsinter.rs`, `rsinter/tests/bench_result.rs`, and `rsinter/tests/bench_merge.rs` for behavior and test coverage.
- `BenchmarkResultRow::identity(&self) -> Result<String, String>` returns `sha256:<64 lowercase hex chars>`.
- The identity input includes `schema = "rsinter.benchmark_result_row.v1"`, benchmark, runner, language, params, and stable case summary fields.
- The identity excludes status, failure kind, metrics, artifacts, error, and `case_summary.num_shots_generated`.
- JSONL output includes a computed `identity` field, and JSONL input without that field remains accepted.
- `merge_result_rows` returns `Result<Vec<BenchmarkResultRow>, String>`.
- Compatible rows with the same identity sum additive metric counters `shots_used`, `logical_errors`, `compile_us`, `total_decode_us`, and `wall_seconds`.
- Compatible rows with the same identity sum `case_summary.num_shots_generated` when present.
- Merged rows recompute `logical_error_rate` and `decode_us_per_shot` from merged counters.
- Rows with the same identity but conflicting non-additive metadata or unknown non-additive metrics return a clear `Err(String)`.
- Preserve existing output ordering after merge: runner, distance, and `p`.
- Required focused verification command: `cargo test -p rsinter --test bench_merge benchmark_merge_combines_rows_with_same_identity`.
- Broader requested verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/result.rs`: add identity input serialization, SHA-256 hex formatting, custom row serialization including `identity`, and tests for JSONL identity behavior.
- Modify `rsinter/src/bench/merge.rs`: return `Result`, group by identity, enforce compatibility checks, sum additive counters, recompute derived metrics, and retain the current sort comparator.
- Modify `rsinter/src/bin/rsinter.rs`: propagate merge errors with `?`.
- Modify `rsinter/tests/bench_result.rs`: assert serialized rows include identity and legacy rows without identity still deserialize.
- Modify `rsinter/tests/bench_merge.rs`: update existing API call to unwrap and add the requested merge regression test.

---

### Task 1: Stable Row Identity and JSONL Output

**Files:**
- Modify: `rsinter/src/bench/result.rs`
- Test: `rsinter/tests/bench_result.rs`

**Interfaces:**
- Consumes: existing `BenchmarkResultRow` fields.
- Produces: `BenchmarkResultRow::identity(&self) -> Result<String, String>` and JSON serialization containing a computed `identity` field.

- [ ] **Step 1: Write the failing result identity tests**

Add this test to `rsinter/tests/bench_result.rs` after `result_row_serializes_round_trip_as_json`:

```rust
#[test]
fn result_row_serializes_stable_identity_field() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::Ok,
        params: ParamMap::from_pairs([
            (
                "decoder_options",
                serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap(),
            ),
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
            ("num_shots_generated", serde_json::json!(2000)),
        ]),
        metrics: MetricMap::from_pairs([("shots_used", 2000.0)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let identity = row.identity().unwrap();
    assert!(identity.starts_with("sha256:"));
    assert_eq!(identity.len(), "sha256:".len() + 64);

    let encoded = serde_json::to_string(&row).unwrap();
    let encoded_value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(encoded_value["identity"], serde_json::json!(identity));

    let mut reordered = row.clone();
    reordered.params.insert(
        "decoder_options".into(),
        serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap(),
    );
    assert_eq!(row.identity().unwrap(), reordered.identity().unwrap());

    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.identity().unwrap(), identity);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_result result_row_serializes_stable_identity_field
```

Expected: FAIL before the production change because `BenchmarkResultRow::identity` does not exist and serialized JSON has no `identity` field.

- [ ] **Step 3: Implement canonical identity helpers**

In `rsinter/src/bench/result.rs`, replace the `serde` import:

```rust
use serde::{Deserialize, Serialize};
```

with:

```rust
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
```

Remove `Serialize` from the `BenchmarkResultRow` derive:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkResultRow {
```

Add these helpers below the struct definition:

```rust
const ROW_IDENTITY_SCHEMA: &str = "rsinter.benchmark_result_row.v1";
const CASE_SUMMARY_ADDITIVE_KEYS: [&str; 1] = ["num_shots_generated"];

#[derive(Serialize)]
struct BenchmarkResultRowIdentityInput<'a> {
    schema: &'static str,
    benchmark: &'a str,
    runner: &'a str,
    language: &'a str,
    params: &'a ParamMap,
    case_summary: CaseSummary,
}

impl BenchmarkResultRow {
    pub fn identity(&self) -> Result<String, String> {
        let input = BenchmarkResultRowIdentityInput {
            schema: ROW_IDENTITY_SCHEMA,
            benchmark: &self.benchmark,
            runner: &self.runner,
            language: &self.language,
            params: &self.params,
            case_summary: stable_case_summary(&self.case_summary),
        };
        let bytes = serde_json::to_vec(&input).map_err(|error| error.to_string())?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", lower_hex(&digest)))
    }
}

pub(crate) fn stable_case_summary(case_summary: &CaseSummary) -> CaseSummary {
    case_summary
        .iter()
        .filter(|(key, _)| !CASE_SUMMARY_ADDITIVE_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn case_summary_additive_keys() -> &'static [&'static str] {
    &CASE_SUMMARY_ADDITIVE_KEYS
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
```

- [ ] **Step 4: Implement custom row serialization**

Add this `Serialize` implementation below the identity helpers:

```rust
impl Serialize for BenchmarkResultRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let identity = self.identity().map_err(serde::ser::Error::custom)?;
        let mut row = serializer.serialize_struct("BenchmarkResultRow", 11)?;
        row.serialize_field("identity", &identity)?;
        row.serialize_field("benchmark", &self.benchmark)?;
        row.serialize_field("runner", &self.runner)?;
        row.serialize_field("language", &self.language)?;
        row.serialize_field("status", &self.status)?;
        row.serialize_field("failure_kind", &self.failure_kind)?;
        row.serialize_field("params", &self.params)?;
        row.serialize_field("case_summary", &self.case_summary)?;
        row.serialize_field("metrics", &self.metrics)?;
        row.serialize_field("artifacts", &self.artifacts)?;
        row.serialize_field("error", &self.error)?;
        row.end()
    }
}
```

- [ ] **Step 5: Run result tests**

Run:

```bash
cargo test -p rsinter --test bench_result result_row_serializes_stable_identity_field
```

Expected: PASS.

Then run:

```bash
cargo test -p rsinter --test bench_result
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add rsinter/src/bench/result.rs rsinter/tests/bench_result.rs
git commit -m "feat: add stable benchmark row identities"
```

---

### Task 2: Identity-Aware Benchmark Merge

**Files:**
- Modify: `rsinter/src/bench/merge.rs`
- Modify: `rsinter/src/bin/rsinter.rs`
- Test: `rsinter/tests/bench_merge.rs`

**Interfaces:**
- Consumes: `BenchmarkResultRow::identity`, `stable_case_summary`, and `case_summary_additive_keys`.
- Produces: `merge_result_rows(row_sets: Vec<Vec<BenchmarkResultRow>>) -> Result<Vec<BenchmarkResultRow>, String>`.

- [ ] **Step 1: Write the failing merge regression test**

Update the existing `merge_result_rows_concatenates_and_sorts_by_runner_then_distance_then_p`
test so the existing `merge_result_rows` call ends with `.unwrap()` before
the result is assigned to `rows`.

Add this test to `rsinter/tests/bench_merge.rs`:

```rust
#[test]
fn benchmark_merge_combines_rows_with_same_identity() {
    let first = ok_row(
        serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap(),
        100.0,
        1.0,
        300.0,
        0.5,
    );
    let second = ok_row(
        serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap(),
        300.0,
        5.0,
        900.0,
        1.5,
    );
    assert_eq!(first.identity().unwrap(), second.identity().unwrap());

    let rows = merge_result_rows(vec![vec![first], vec![second]]).unwrap();

    assert_eq!(rows.len(), 1);
    let metrics = &rows[0].metrics;
    assert_eq!(metrics["shots_used"], 400.0);
    assert_eq!(metrics["logical_errors"], 6.0);
    assert_eq!(metrics["total_decode_us"], 1200.0);
    assert_eq!(metrics["wall_seconds"], 2.0);
    assert_eq!(metrics["logical_error_rate"], 0.015);
    assert_eq!(metrics["decode_us_per_shot"], 3.0);
    assert_eq!(
        rows[0].case_summary["num_shots_generated"],
        serde_json::json!(400)
    );

    let different_decoder = ok_row(serde_json::json!({"a": 1, "b": 3}), 50.0, 0.0, 50.0, 0.2);
    let distinct = merge_result_rows(vec![vec![rows[0].clone()], vec![different_decoder]]).unwrap();
    assert_eq!(distinct.len(), 2);

    let mut incompatible = rows[0].clone();
    incompatible.status = "error".into();
    incompatible.failure_kind = FailureKind::SolverFailure;
    incompatible.error = Some("solver failed".into());
    let err = merge_result_rows(vec![vec![rows[0].clone()], vec![incompatible]])
        .expect_err("same identity with conflicting status must fail");
    assert!(err.contains("conflicting status"), "{err}");
}

fn ok_row(
    decoder_options: serde_json::Value,
    shots_used: f64,
    logical_errors: f64,
    total_decode_us: f64,
    wall_seconds: f64,
) -> BenchmarkResultRow {
    BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::LogicalFailure,
        params: ParamMap::from_pairs([
            ("decoder_options", decoder_options),
            ("decoder_impl", serde_json::json!("rmatching")),
            ("distance", serde_json::json!(3)),
            ("max_errors", serde_json::json!(100)),
            ("max_shots", serde_json::json!(1000)),
            ("p", serde_json::json!(0.002)),
            ("seed", serde_json::json!(12345)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
            ("num_shots_generated", serde_json::json!(shots_used as u64)),
        ]),
        metrics: MetricMap::from_pairs([
            ("shots_used", shots_used),
            ("logical_errors", logical_errors),
            ("logical_error_rate", logical_errors / shots_used),
            ("total_decode_us", total_decode_us),
            ("wall_seconds", wall_seconds),
            ("decode_us_per_shot", total_decode_us / shots_used),
        ]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_merge benchmark_merge_combines_rows_with_same_identity
```

Expected: FAIL before the merge implementation because duplicate identity rows are not combined and `merge_result_rows` still returns `Vec`.

- [ ] **Step 3: Implement identity-aware merge**

Replace `rsinter/src/bench/merge.rs` with implementation that:

```rust
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use serde_json::Value;

use crate::bench::result::{
    BenchmarkResultRow, MetricMap, case_summary_additive_keys, stable_case_summary,
};
```

Define additive and derived metric constants:

```rust
const ADDITIVE_METRICS: [&str; 5] = [
    "shots_used",
    "logical_errors",
    "compile_us",
    "total_decode_us",
    "wall_seconds",
];
const DERIVED_METRICS: [&str; 2] = ["logical_error_rate", "decode_us_per_shot"];
```

Implement `merge_result_rows` as a `Result` that groups rows by identity,
calls `merge_row_into` for duplicate identities, collects values, and calls
the existing runner/distance/`p` comparator before returning `Ok(rows)`.

Implement `merge_row_into(identity: &str, base: &mut BenchmarkResultRow, incoming: BenchmarkResultRow) -> Result<(), String>` to:

- compare benchmark, runner, language, status, failure kind, params, error, and artifacts;
- compare `stable_case_summary(base)` to `stable_case_summary(&incoming.case_summary)`;
- merge `case_summary.num_shots_generated` by summing JSON numbers;
- merge additive metrics by summing values from either row;
- reject conflicting unknown non-additive metrics;
- recompute derived metrics.

Use field-specific error strings like:

```rust
format!("cannot merge benchmark rows with identity {identity}: conflicting {field}")
```

- [ ] **Step 4: Update the CLI merge caller**

In `rsinter/src/bin/rsinter.rs`, replace:

```rust
let merged = merge_result_rows(row_sets);
```

with:

```rust
let merged = merge_result_rows(row_sets)?;
```

- [ ] **Step 5: Run merge tests**

Run:

```bash
cargo test -p rsinter --test bench_merge benchmark_merge_combines_rows_with_same_identity
```

Expected: PASS.

Then run:

```bash
cargo test -p rsinter --test bench_merge
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
git add rsinter/src/bench/merge.rs rsinter/src/bin/rsinter.rs rsinter/tests/bench_merge.rs
git commit -m "feat: merge benchmark rows by identity"
```
