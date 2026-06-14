# rsinter Failure Taxonomy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured `failure_kind` taxonomy to `rsinter` benchmark rows and collection stats so consumers can distinguish logical failures, timeouts, solver failures, unsupported backends, and sampler failures without string matching.

**Architecture:** Introduce one shared `FailureKind` enum in `rsinter/src/failure.rs`, then thread it through JSONL benchmark rows, CSV-backed `TaskStats`, benchmark runner outcomes, and `collect`. Convert `rsinter` decoder traits from panic-oriented adapter calls to `Result` so backend failures can be recorded as structured rows instead of panics. Keep benchmark spec/preflight errors as whole-run `Err` results.

**Tech Stack:** Rust 2024, Cargo workspace, `serde`, `csv`, `toml`, `rsinter` integration tests, existing `rstim` sampler and `rsinter::decode` traits.

---

## File Structure

- Create `rsinter/src/failure.rs`: define `FailureKind`, string parsing/formatting, completed-run classification, error classification, and aggregate combination.
- Modify `rsinter/src/lib.rs`: export the new `failure` module.
- Modify `rsinter/src/bench/result.rs`: add `BenchmarkResultRow.failure_kind`, serialize as snake_case, and support legacy JSONL rows that omit the field.
- Modify `rsinter/tests/bench_result.rs`: cover JSON round trip and legacy JSONL inference.
- Modify `rsinter/src/task_stats.rs`: add `TaskStats.failure_kind` and combine it conservatively in `Add`.
- Modify `rsinter/src/csv_io.rs`: write/read the `failure_kind` CSV column and read legacy CSV files without it.
- Modify `rsinter/tests/csv_io.rs`: cover CSV round trip, legacy CSV inference, and `TaskStats::add` failure-kind priority.
- Modify `rsinter/src/decode.rs`: change `Decoder` and `CompiledDecoder` methods to return `Result`.
- Modify `rsinter/src/ilpqec_adapter.rs`, `rsinter/src/rmatching_adapter.rs`, and `rsinter/src/rbposd_adapter.rs`: return adapter errors instead of panicking for normal backend/decode failures.
- Modify direct decoder tests in `rsinter/tests/decode.rs`, `rsinter/tests/decode_ilp.rs`, `rsinter/tests/decode_rbposd.rs`, `rsinter/tests/decode_rmatching.rs`, and `rsinter/tests/css_surface_special.rs`: unwrap explicit `Result` values.
- Modify fake decoder implementations in `rsinter/tests/collect.rs` and `rsinter/src/bench/runners/mod.rs`: update trait signatures and add failure tests.
- Modify `rsinter/src/bench/runners/mod.rs`: classify normal, timeout, decoder, and sampler outcomes into `BenchmarkResultRow.failure_kind`.
- Modify `rsinter/tests/bench_runner_wrappers.rs` and `rsinter/tests/bench_run.rs`: assert structured benchmark output, including `rilpqec` unsupported Gurobi without the feature.
- Modify `rsinter/src/collect.rs`: return per-task `TaskStats` for decoder/sampler failures while preserving global setup errors.

---

### Task 1: Add `FailureKind` And Benchmark JSONL Compatibility

**Files:**
- Create: `rsinter/src/failure.rs`
- Modify: `rsinter/src/lib.rs`
- Modify: `rsinter/src/bench/result.rs`
- Modify: `rsinter/tests/bench_result.rs`
- Modify: `rsinter/tests/bench_merge.rs`
- Modify: `rsinter/tests/bench_plot.rs`

- [ ] **Step 1: Write failing JSONL tests**

In `rsinter/tests/bench_result.rs`, add this import:

```rust
use rsinter::failure::FailureKind;
```

Add this test after `result_row_serializes_round_trip_as_json`:

```rust
#[test]
fn result_row_serializes_failure_kind_as_snake_case() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::LogicalFailure,
        params: ParamMap::from_pairs([("distance", serde_json::json!(3))]),
        case_summary: CaseSummary::new(),
        metrics: MetricMap::from_pairs([("logical_errors", 2.0)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let encoded = serde_json::to_string(&row).unwrap();
    assert!(encoded.contains("\"failure_kind\":\"logical_failure\""));

    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.failure_kind, FailureKind::LogicalFailure);
}
```

Add this test after `results_jsonl_ignores_blank_lines`:

```rust
#[test]
fn results_jsonl_infers_missing_failure_kind_from_legacy_rows() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"clean\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"logical\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{\"logical_errors\":3.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"solver\",\"language\":\"rust\",\"status\":\"error\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":\"HiGHS backend error: solve failed\"}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"unsupported\",\"language\":\"rust\",\"status\":\"error\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":\"no ILP backend is available for kind Gurobi\"}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Ok);
    assert_eq!(rows[1].failure_kind, FailureKind::LogicalFailure);
    assert_eq!(rows[2].failure_kind, FailureKind::SolverFailure);
    assert_eq!(rows[3].failure_kind, FailureKind::Unsupported);
}
```

- [ ] **Step 2: Run the failing benchmark result tests**

Run:

```bash
cargo test -p rsinter --test bench_result
```

Expected: FAIL to compile because `rsinter::failure::FailureKind` and `BenchmarkResultRow.failure_kind` do not exist.

- [ ] **Step 3: Add the shared failure module**

Create `rsinter/src/failure.rs` with this content:

```rust
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Ok,
    LogicalFailure,
    Timeout,
    SolverFailure,
    Unsupported,
    SamplerError,
}

impl Default for FailureKind {
    fn default() -> Self {
        Self::Ok
    }
}

impl FailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::LogicalFailure => "logical_failure",
            Self::Timeout => "timeout",
            Self::SolverFailure => "solver_failure",
            Self::Unsupported => "unsupported",
            Self::SamplerError => "sampler_error",
        }
    }

    pub fn status(self) -> &'static str {
        match self {
            Self::Ok | Self::LogicalFailure | Self::Timeout => "ok",
            Self::SolverFailure | Self::Unsupported | Self::SamplerError => "error",
        }
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FailureKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ok" => Ok(Self::Ok),
            "logical_failure" => Ok(Self::LogicalFailure),
            "timeout" => Ok(Self::Timeout),
            "solver_failure" => Ok(Self::SolverFailure),
            "unsupported" => Ok(Self::Unsupported),
            "sampler_error" => Ok(Self::SamplerError),
            other => Err(format!("unknown failure_kind: {other}")),
        }
    }
}

pub fn classify_completed(logical_errors: u64, timed_out: bool) -> FailureKind {
    if timed_out {
        FailureKind::Timeout
    } else if logical_errors > 0 {
        FailureKind::LogicalFailure
    } else {
        FailureKind::Ok
    }
}

pub fn classify_error(message: &str, fallback: FailureKind) -> FailureKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("backendunavailable")
        || lower.contains("backend unavailable")
        || lower.contains("backend is unavailable")
        || lower.contains("no ilp backend is available")
        || lower.contains("unsupported")
    {
        FailureKind::Unsupported
    } else {
        fallback
    }
}

pub fn combine_failure_kind(a: FailureKind, b: FailureKind) -> FailureKind {
    if failure_priority(a) >= failure_priority(b) {
        a
    } else {
        b
    }
}

fn failure_priority(kind: FailureKind) -> u8 {
    match kind {
        FailureKind::Ok => 0,
        FailureKind::LogicalFailure => 1,
        FailureKind::Timeout => 2,
        FailureKind::SamplerError => 3,
        FailureKind::SolverFailure => 4,
        FailureKind::Unsupported => 5,
    }
}
```

In `rsinter/src/lib.rs`, add:

```rust
pub mod failure;
```

- [ ] **Step 4: Add `failure_kind` to benchmark rows with legacy deserialization**

In `rsinter/src/bench/result.rs`, add this import:

```rust
use crate::failure::{FailureKind, classify_error};
```

Replace the `BenchmarkResultRow` derive and struct with:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BenchmarkResultRow {
    pub benchmark: String,
    pub runner: String,
    pub language: String,
    pub status: String,
    pub failure_kind: FailureKind,
    pub params: ParamMap,
    pub case_summary: CaseSummary,
    pub metrics: MetricMap,
    pub artifacts: ArtifactMap,
    pub error: Option<String>,
}
```

Add this legacy helper and custom deserializer below the struct:

```rust
#[derive(Deserialize)]
struct RawBenchmarkResultRow {
    benchmark: String,
    runner: String,
    language: String,
    status: String,
    #[serde(default)]
    failure_kind: Option<FailureKind>,
    params: ParamMap,
    case_summary: CaseSummary,
    metrics: MetricMap,
    artifacts: ArtifactMap,
    error: Option<String>,
}

impl<'de> Deserialize<'de> for BenchmarkResultRow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawBenchmarkResultRow::deserialize(deserializer)?;
        let failure_kind = raw.failure_kind.unwrap_or_else(|| {
            infer_legacy_failure_kind(
                &raw.status,
                raw.error.as_deref(),
                raw.metrics.get("logical_errors").copied(),
            )
        });
        Ok(Self {
            benchmark: raw.benchmark,
            runner: raw.runner,
            language: raw.language,
            status: raw.status,
            failure_kind,
            params: raw.params,
            case_summary: raw.case_summary,
            metrics: raw.metrics,
            artifacts: raw.artifacts,
            error: raw.error,
        })
    }
}

fn infer_legacy_failure_kind(
    status: &str,
    error: Option<&str>,
    logical_errors: Option<f64>,
) -> FailureKind {
    if status == "error" {
        return error
            .map(|message| classify_error(message, FailureKind::SolverFailure))
            .unwrap_or(FailureKind::SolverFailure);
    }
    if logical_errors.unwrap_or(0.0) > 0.0 {
        FailureKind::LogicalFailure
    } else {
        FailureKind::Ok
    }
}
```

- [ ] **Step 5: Update existing benchmark row literals**

Add `failure_kind` to every `BenchmarkResultRow` literal in these files:

```text
rsinter/tests/bench_result.rs
rsinter/tests/bench_merge.rs
rsinter/tests/bench_plot.rs
```

Use `FailureKind::Ok` for rows with `status: "ok".into()` and no logical errors. Use `FailureKind::LogicalFailure` for rows with `status: "ok".into()` and `logical_errors > 0.0`. Use `FailureKind::SolverFailure` for rows with `status: "error".into()`.

In each file that constructs rows, add:

```rust
use rsinter::failure::FailureKind;
```

For an ok row, insert:

```rust
failure_kind: FailureKind::Ok,
```

For an ok row with a positive `logical_errors` metric, insert:

```rust
failure_kind: FailureKind::LogicalFailure,
```

For the existing error row in `rsinter/tests/bench_result.rs` and `rsinter/tests/bench_plot.rs`, insert:

```rust
failure_kind: FailureKind::SolverFailure,
```

- [ ] **Step 6: Run benchmark result and plot fixture tests**

Run:

```bash
cargo test -p rsinter --test bench_result
cargo test -p rsinter --test bench_merge
cargo test -p rsinter --test bench_plot
```

Expected: PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add rsinter/src/failure.rs rsinter/src/lib.rs rsinter/src/bench/result.rs rsinter/tests/bench_result.rs rsinter/tests/bench_merge.rs rsinter/tests/bench_plot.rs
git commit -m "feat: add rsinter failure kind model"
```

---

### Task 2: Add `TaskStats.failure_kind` And CSV Compatibility

**Files:**
- Modify: `rsinter/src/task_stats.rs`
- Modify: `rsinter/src/csv_io.rs`
- Modify: `rsinter/tests/csv_io.rs`
- Modify: `rsinter/src/collect.rs`
- Modify: `rsinter/tests/collect.rs`
- Modify: `rsinter/tests/decode_ilp.rs`

- [ ] **Step 1: Write failing CSV tests**

In `rsinter/tests/csv_io.rs`, add:

```rust
use rsinter::failure::FailureKind;
```

Update `sample_stats()` to include:

```rust
failure_kind: FailureKind::Ok,
```

In `csv_roundtrip`, add:

```rust
assert_eq!(recovered[0].failure_kind, FailureKind::Ok);
```

Append these tests:

```rust
#[test]
fn csv_reads_legacy_rows_without_failure_kind() {
    let input = concat!(
        "shots,errors,discards,seconds,decoder,strong_id,json_metadata,custom_counts\n",
        "100,0,0,1.0000,vacuous,clean,\"{\"\"d\"\":3}\",\"{}\"\n",
        "100,4,0,1.0000,vacuous,logical,\"{\"\"d\"\":3}\",\"{}\"\n"
    );

    let recovered = read_csv(input.as_bytes()).unwrap();

    assert_eq!(recovered[0].failure_kind, FailureKind::Ok);
    assert_eq!(recovered[1].failure_kind, FailureKind::LogicalFailure);
}

#[test]
fn task_stats_addition_keeps_strongest_failure_kind() {
    let a = TaskStats {
        failure_kind: FailureKind::LogicalFailure,
        ..sample_stats()
    };
    let b = TaskStats {
        shots: 500,
        errors: 0,
        seconds: 0.5,
        failure_kind: FailureKind::Timeout,
        ..sample_stats()
    };

    let c = a + b;

    assert_eq!(c.failure_kind, FailureKind::Timeout);
    assert_eq!(c.shots, 1500);
}
```

- [ ] **Step 2: Run the failing CSV tests**

Run:

```bash
cargo test -p rsinter --test csv_io
```

Expected: FAIL to compile because `TaskStats.failure_kind` is missing.

- [ ] **Step 3: Add `failure_kind` to `TaskStats`**

In `rsinter/src/task_stats.rs`, add imports:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Add;

use crate::failure::{FailureKind, combine_failure_kind};
```

Replace the struct with:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStats {
    pub strong_id: String,
    pub decoder: String,
    pub metadata: serde_json::Value,
    pub shots: u64,
    pub errors: u64,
    pub discards: u64,
    pub seconds: f64,
    #[serde(default)]
    pub failure_kind: FailureKind,
    pub custom_counts: HashMap<String, u64>,
}
```

In `impl Add for TaskStats`, include:

```rust
failure_kind: combine_failure_kind(self.failure_kind, rhs.failure_kind),
```

The returned `TaskStats` block should contain:

```rust
TaskStats {
    strong_id: self.strong_id,
    decoder: self.decoder,
    metadata: self.metadata,
    shots: self.shots + rhs.shots,
    errors: self.errors + rhs.errors,
    discards: self.discards + rhs.discards,
    seconds: self.seconds + rhs.seconds,
    failure_kind: combine_failure_kind(self.failure_kind, rhs.failure_kind),
    custom_counts: counts,
}
```

- [ ] **Step 4: Write and read the CSV column by header name**

In `rsinter/src/csv_io.rs`, add:

```rust
use crate::failure::{FailureKind, classify_completed};
```

Replace `write_csv` with:

```rust
pub fn write_csv(stats: &[TaskStats], out: &mut dyn Write) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(out);
    wtr.write_record(&[
        "shots",
        "errors",
        "discards",
        "seconds",
        "failure_kind",
        "decoder",
        "strong_id",
        "json_metadata",
        "custom_counts",
    ])
    .map_err(|e| e.to_string())?;
    for s in stats {
        wtr.write_record(&[
            s.shots.to_string(),
            s.errors.to_string(),
            s.discards.to_string(),
            format!("{:.4}", s.seconds),
            s.failure_kind.to_string(),
            s.decoder.clone(),
            s.strong_id.clone(),
            serde_json::to_string(&s.metadata).unwrap_or_default(),
            serde_json::to_string(&s.custom_counts).unwrap_or_default(),
        ])
        .map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(())
}
```

Replace `read_csv` with:

```rust
pub fn read_csv(data: &[u8]) -> Result<Vec<TaskStats>, String> {
    let mut rdr = csv::Reader::from_reader(data);
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let idx = |name: &str| -> Result<usize, String> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| format!("missing CSV column: {name}"))
    };
    let shots_idx = idx("shots")?;
    let errors_idx = idx("errors")?;
    let discards_idx = idx("discards")?;
    let seconds_idx = idx("seconds")?;
    let decoder_idx = idx("decoder")?;
    let strong_id_idx = idx("strong_id")?;
    let metadata_idx = idx("json_metadata")?;
    let custom_counts_idx = idx("custom_counts")?;
    let failure_kind_idx = headers
        .iter()
        .position(|header| header == "failure_kind");

    let mut results = Vec::new();
    for record in rdr.records() {
        let r = record.map_err(|e| e.to_string())?;
        let get = |index: usize, name: &str| -> Result<&str, String> {
            r.get(index)
                .ok_or_else(|| format!("missing CSV value for column: {name}"))
        };
        let errors = get(errors_idx, "errors")?
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let failure_kind = match failure_kind_idx {
            Some(index) => get(index, "failure_kind")?.parse::<FailureKind>()?,
            None => classify_completed(errors, false),
        };
        results.push(TaskStats {
            shots: get(shots_idx, "shots")?
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
            errors,
            discards: get(discards_idx, "discards")?
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
            seconds: get(seconds_idx, "seconds")?
                .parse()
                .map_err(|e: std::num::ParseFloatError| e.to_string())?,
            failure_kind,
            decoder: get(decoder_idx, "decoder")?.to_string(),
            strong_id: get(strong_id_idx, "strong_id")?.to_string(),
            metadata: serde_json::from_str(get(metadata_idx, "json_metadata")?)
                .unwrap_or(serde_json::Value::Null),
            custom_counts: serde_json::from_str(get(custom_counts_idx, "custom_counts")?)
                .unwrap_or_default(),
        });
    }
    Ok(results)
}
```

- [ ] **Step 5: Update existing `TaskStats` literals**

Add `failure_kind: FailureKind::Ok` to direct `TaskStats` literals in:

```text
rsinter/tests/csv_io.rs
rsinter/src/collect.rs
```

Add these imports where needed:

```rust
use crate::failure::{FailureKind, classify_completed};
```

or in tests:

```rust
use rsinter::failure::FailureKind;
```

- [ ] **Step 6: Run CSV and current collect tests**

Run:

```bash
cargo test -p rsinter --test csv_io
cargo test -p rsinter collect_single_task_vacuous
```

Expected: PASS.

- [ ] **Step 7: Commit Task 2**

Run:

```bash
git add rsinter/src/task_stats.rs rsinter/src/csv_io.rs rsinter/tests/csv_io.rs rsinter/src/collect.rs rsinter/tests/collect.rs rsinter/tests/decode_ilp.rs
git commit -m "feat: record failure kind in task stats"
```

---

### Task 3: Convert Decoder Traits To Explicit `Result`

**Files:**
- Modify: `rsinter/src/decode.rs`
- Modify: `rsinter/src/ilpqec_adapter.rs`
- Modify: `rsinter/src/rmatching_adapter.rs`
- Modify: `rsinter/src/rbposd_adapter.rs`
- Modify: `rsinter/tests/decode.rs`
- Modify: `rsinter/tests/decode_ilp.rs`
- Modify: `rsinter/tests/decode_rbposd.rs`
- Modify: `rsinter/tests/decode_rmatching.rs`
- Modify: `rsinter/tests/css_surface_special.rs`
- Modify: `rsinter/tests/collect.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`

- [ ] **Step 1: Write a failing `rilpqec` backend-unavailable test**

In `rsinter/tests/decode_ilp.rs`, replace the conditional import with an unconditional one:

```rust
use rilpqec::{BackendConfig, BackendKind, IlpDecoderConfig};
```

Append this test:

```rust
#[cfg(not(feature = "gurobi"))]
#[test]
fn ilp_dem_decoder_reports_unavailable_gurobi_backend() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\n").unwrap();
    let decoder = RsinterIlpDemDecoder::new(IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Gurobi,
            time_limit_seconds: None,
            mip_gap: None,
            threads: Some(1),
            verbose: false,
        },
    });
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let err = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 1, 1)
        .unwrap_err();

    assert!(
        err.contains("no ILP backend is available"),
        "error was: {err}"
    );
}
```

- [ ] **Step 2: Run the failing ILP test**

Run:

```bash
cargo test -p rsinter ilp_dem_decoder_reports_unavailable_gurobi_backend
```

Expected: FAIL to compile because decoder methods do not return `Result`.

- [ ] **Step 3: Change trait signatures and the vacuous decoder**

In `rsinter/src/decode.rs`, replace the traits and vacuous impls with:

```rust
pub trait CompiledDecoder: Send {
    /// Decode bit-packed detection events into bit-packed observable predictions.
    /// `dets`: `num_shots * ceil(num_dets/8)` bytes, b8 format.
    /// Returns: `num_shots * ceil(num_obs/8)` bytes, b8 format.
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String>;
}

pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String>;
}
```

Update `VacuousCompiled`:

```rust
impl CompiledDecoder for VacuousCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        let obs_bytes = (num_obs + 7) / 8;
        Ok(vec![0u8; num_shots * obs_bytes])
    }
}

impl Decoder for VacuousDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(VacuousCompiled))
    }
}
```

- [ ] **Step 4: Return `Result` from real adapters**

In `rsinter/src/ilpqec_adapter.rs`, update compile and decode:

```rust
impl Decoder for IlpDemDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        let decoder = rilpqec::IlpDemDecoder::from_dem(dem, self.config.clone())
            .map_err(|error| error.to_string())?;
        Ok(Box::new(CompiledIlpDemDecoder { decoder }))
    }
}

impl CompiledDecoder for CompiledIlpDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        self.decoder
            .decode_batch_bit_packed(dets, num_shots, num_dets, num_obs)
            .map_err(|error| error.to_string())
    }
}
```

In `rsinter/src/rmatching_adapter.rs`, update compile and decode:

```rust
impl Decoder for RmatchingDemDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        let matching = Matching::from_dem(&dem.to_string()).map_err(|error| error.to_string())?;
        Ok(Box::new(CompiledRmatchingDemDecoder {
            matching: Mutex::new(matching),
        }))
    }
}

impl CompiledDecoder for CompiledRmatchingDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        Ok(self
            .matching
            .lock()
            .map_err(|error| error.to_string())?
            .decode_shots_bit_packed(dets, num_shots, num_dets, num_obs))
    }
}
```

In `rsinter/src/rbposd_adapter.rs`, change `compile_for_dem` to return `Result` and replace the two compile-time `expect` calls with `map_err`:

```rust
let pcm = ParityCheckMatrix::from_sparse_columns(
    num_dets,
    filtered_detector_columns.len(),
    filtered_detector_columns,
)
.map_err(|error| format!("invalid rbposd parity matrix: {error}"))?;

Some(
    BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(filtered_probabilities),
        self.config.clone(),
    )
    .map_err(|error| format!("failed to compile rbposd decoder: {error}"))?,
)
```

Return the boxed compiled decoder with:

```rust
Ok(Box::new(CompiledRbposdDemDecoder {
    decoder,
    num_dets,
    num_obs,
    observable_columns: filtered_observable_columns,
    forced_syndrome,
    baseline_observables,
}))
```

In `CompiledRbposdDemDecoder::decode_shots_bit_packed`, change the return type to `Result<Vec<u8>, String>`, replace `expect("rbposd decode failed")` with:

```rust
let result = decoder
    .decode(&Syndrome::from(syndrome_bits))
    .map_err(|error| format!("rbposd decode failed: {error}"))?;
```

and return:

```rust
Ok(out)
```

- [ ] **Step 5: Update all direct decoder calls**

Apply these mechanical changes in the listed files:

```text
rsinter/tests/decode.rs
rsinter/tests/decode_ilp.rs
rsinter/tests/decode_rbposd.rs
rsinter/tests/decode_rmatching.rs
rsinter/tests/css_surface_special.rs
```

Change:

```rust
let compiled = decoder.compile_for_dem(&dem);
```

to:

```rust
let compiled = decoder.compile_for_dem(&dem).unwrap();
```

Change direct decode calls used as values:

```rust
let predictions = compiled.decode_shots_bit_packed(&dets, shots, num_dets, num_obs);
```

to:

```rust
let predictions = compiled
    .decode_shots_bit_packed(&dets, shots, num_dets, num_obs)
    .unwrap();
```

For indexed inline calls in `rsinter/tests/decode_rbposd.rs`, change:

```rust
let predicted = compiled.decode_shots_bit_packed(&[det_byte], 1, 2, 1)[0] & 1 != 0;
```

to:

```rust
let predicted = compiled
    .decode_shots_bit_packed(&[det_byte], 1, 2, 1)
    .unwrap()[0]
    & 1
    != 0;
```

- [ ] **Step 6: Update fake decoder impl signatures**

In `rsinter/tests/collect.rs` and `rsinter/src/bench/runners/mod.rs`, update fake decoder impls from:

```rust
fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder>
```

to:

```rust
fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String>
```

and wrap boxed return values in `Ok(...)`.

Update fake `decode_shots_bit_packed` methods from:

```rust
) -> Vec<u8> {
    vec![0u8; num_shots * obs_bytes]
}
```

to:

```rust
) -> Result<Vec<u8>, String> {
    Ok(vec![0u8; num_shots * obs_bytes])
}
```

- [ ] **Step 7: Run decoder tests**

Run:

```bash
cargo test -p rsinter --test decode
cargo test -p rsinter --test decode_ilp
cargo test -p rsinter --test decode_rbposd
cargo test -p rsinter --test decode_rmatching
```

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add rsinter/src/decode.rs rsinter/src/ilpqec_adapter.rs rsinter/src/rmatching_adapter.rs rsinter/src/rbposd_adapter.rs rsinter/tests/decode.rs rsinter/tests/decode_ilp.rs rsinter/tests/decode_rbposd.rs rsinter/tests/decode_rmatching.rs rsinter/tests/css_surface_special.rs rsinter/tests/collect.rs rsinter/src/bench/runners/mod.rs
git commit -m "refactor: return decoder failures explicitly"
```

---

### Task 4: Classify Benchmark Runner Outcomes

**Files:**
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/src/bench/runners/rmatching.rs`
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/src/bench/runners/rilpqec.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

- [ ] **Step 1: Write failing structured failure tests**

In the test module in `rsinter/src/bench/runners/mod.rs`, add:

```rust
use crate::failure::FailureKind;
```

Add these fake decoders in the same test module:

```rust
struct OnePredictionDecoder;

struct OnePredictionCompiled;

impl Decoder for OnePredictionDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(OnePredictionCompiled))
    }
}

impl CompiledDecoder for OnePredictionCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        let obs_bytes = num_obs.div_ceil(8);
        Ok(vec![0xffu8; num_shots * obs_bytes])
    }
}

struct CompileErrorDecoder {
    message: &'static str,
}

impl Decoder for CompileErrorDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        Err(self.message.to_string())
    }
}

struct DecodeErrorDecoder {
    message: &'static str,
}

struct DecodeErrorCompiled {
    message: &'static str,
}

impl Decoder for DecodeErrorDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(DecodeErrorCompiled {
            message: self.message,
        }))
    }
}

impl CompiledDecoder for DecodeErrorCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        _num_shots: usize,
        _num_dets: usize,
        _num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        Err(self.message.to_string())
    }
}
```

Add this helper:

```rust
fn surface_point(p: f64, max_shots: u64, max_errors: u64) -> BenchCasePoint {
    BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots,
        max_errors,
        max_wall_seconds: None,
        batch_size: 1,
        decoder_params: BTreeMap::new(),
    }
}
```

Add these tests:

```rust
#[test]
fn failure_kind_is_structured_for_completed_benchmark_rows() {
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "fake".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };
    let decoder_params = crate::bench::result::ParamMap::new();

    let ok_row = run_decoder_point(
        "fake",
        &SlowPredictionDecoder {
            sleep: Duration::from_millis(0),
        },
        &surface_point(0.0, 2, 10),
        &ctx,
        &decoder_params,
    )
    .unwrap();
    assert_eq!(ok_row.failure_kind, FailureKind::Ok);

    let logical_row = run_decoder_point(
        "fake",
        &OnePredictionDecoder,
        &surface_point(0.0, 2, 10),
        &ctx,
        &decoder_params,
    )
    .unwrap();
    assert_eq!(logical_row.failure_kind, FailureKind::LogicalFailure);

    let mut timeout_point = surface_point(0.0, 20, 20);
    timeout_point.max_wall_seconds = Some(0.09);
    let timeout_row = run_decoder_point(
        "fake",
        &SlowPredictionDecoder {
            sleep: Duration::from_millis(35),
        },
        &timeout_point,
        &ctx,
        &decoder_params,
    )
    .unwrap();
    assert_eq!(timeout_row.failure_kind, FailureKind::Timeout);
}

#[test]
fn benchmark_runner_records_compile_failure_as_structured_row() {
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "fake".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };
    let decoder_params = crate::bench::result::ParamMap::new();

    let row = run_decoder_point(
        "fake",
        &CompileErrorDecoder {
            message: "no ILP backend is available for kind Gurobi",
        },
        &surface_point(0.002, 1, 1),
        &ctx,
        &decoder_params,
    )
    .unwrap();

    assert_eq!(row.status, "error");
    assert_eq!(row.failure_kind, FailureKind::Unsupported);
    assert!(row.error.unwrap().contains("no ILP backend is available"));
}

#[test]
fn benchmark_runner_records_decode_failure_as_structured_row() {
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "fake".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };
    let decoder_params = crate::bench::result::ParamMap::new();

    let row = run_decoder_point(
        "fake",
        &DecodeErrorDecoder {
            message: "HiGHS backend error: solve failed",
        },
        &surface_point(0.002, 1, 1),
        &ctx,
        &decoder_params,
    )
    .unwrap();

    assert_eq!(row.status, "error");
    assert_eq!(row.failure_kind, FailureKind::SolverFailure);
    assert!(row.error.unwrap().contains("HiGHS backend error"));
}
```

- [ ] **Step 2: Run the failing runner tests**

Run:

```bash
cargo test -p rsinter failure_kind_is_structured
cargo test -p rsinter benchmark_runner_records_compile_failure_as_structured_row
cargo test -p rsinter benchmark_runner_records_decode_failure_as_structured_row
```

Expected: FAIL because `run_decoder_point` still propagates decoder errors and completed rows do not set `failure_kind`.

- [ ] **Step 3: Classify completed and failed benchmark rows**

In `rsinter/src/bench/runners/mod.rs`, add imports:

```rust
use crate::failure::{FailureKind, classify_completed, classify_error};
```

Add this helper near `under_wall_budget`:

```rust
fn make_result_row(
    ctx: &BenchRunContext,
    result_params: crate::bench::result::ParamMap,
    case_summary: crate::bench::result::CaseSummary,
    metrics: MetricMap,
    failure_kind: FailureKind,
    error: Option<String>,
) -> BenchmarkResultRow {
    BenchmarkResultRow {
        benchmark: ctx.benchmark_name.clone(),
        runner: ctx.runner_name.clone(),
        language: ctx.language.clone(),
        status: failure_kind.status().into(),
        failure_kind,
        params: result_params,
        case_summary,
        metrics,
        artifacts: BTreeMap::new(),
        error,
    }
}
```

In `run_decoder_point`, after building the circuit and DEM, change compile to:

```rust
let compiled = match decoder.compile_for_dem(&dem) {
    Ok(compiled) => compiled,
    Err(error) => {
        let mut result_params = built.params.clone();
        for (key, value) in decoder_params {
            result_params.insert(key.clone(), value.clone());
        }
        let mut summary = built.case_summary.clone();
        summary.insert("num_dets".into(), serde_json::json!(dem.effective_num_detectors()));
        summary.insert("num_obs".into(), serde_json::json!(dem.num_observables()));
        summary.insert("num_shots_generated".into(), serde_json::json!(0));
        return Ok(make_result_row(
            ctx,
            result_params,
            summary,
            MetricMap::from_pairs([
                ("shots_used", 0.0),
                ("logical_errors", 0.0),
                ("logical_error_rate", 0.0),
                ("compile_us", compile_us),
                ("total_decode_us", 0.0),
                ("wall_seconds", 0.0),
                ("decode_us_per_shot", 0.0),
            ]),
            classify_error(&error, FailureKind::SolverFailure),
            Some(error),
        ));
    }
};
```

In the decode loop, change decode to:

```rust
let predictions = match compiled.decode_shots_bit_packed(&dets, batch_shots, num_dets, num_obs) {
    Ok(predictions) => predictions,
    Err(error) => {
        total_decode_us += decode_started.elapsed().as_secs_f64() * 1e6;
        wall_seconds += batch_started.elapsed().as_secs_f64();
        let mut result_params = built.params.clone();
        for (key, value) in decoder_params {
            result_params.insert(key.clone(), value.clone());
        }
        let mut summary = built.case_summary.clone();
        summary.insert("num_dets".into(), serde_json::json!(num_dets));
        summary.insert("num_obs".into(), serde_json::json!(num_obs));
        summary.insert(
            "num_shots_generated".into(),
            serde_json::json!(generated_shots),
        );
        return Ok(make_result_row(
            ctx,
            result_params,
            summary,
            MetricMap::from_pairs([
                ("shots_used", shots_used as f64),
                ("logical_errors", logical_errors as f64),
                (
                    "logical_error_rate",
                    if shots_used == 0 {
                        0.0
                    } else {
                        logical_errors as f64 / shots_used as f64
                    },
                ),
                ("compile_us", compile_us),
                ("total_decode_us", total_decode_us),
                ("wall_seconds", wall_seconds),
                (
                    "decode_us_per_shot",
                    if shots_used == 0 {
                        0.0
                    } else {
                        total_decode_us / shots_used as f64
                    },
                ),
            ]),
            classify_error(&error, FailureKind::SolverFailure),
            Some(error),
        ));
    }
};
```

For wrong prediction length and wrong observable length, replace `return Err(...)` with structured rows using `FailureKind::SolverFailure` for decoder prediction length and `FailureKind::SamplerError` for observable buffer length.

At the final `Ok(BenchmarkResultRow { ... })`, compute and include:

```rust
let timed_out = point
    .max_wall_seconds
    .is_some_and(|max_seconds| wall_seconds >= max_seconds);
let failure_kind = classify_completed(logical_errors as u64, timed_out);
```

and build the return row through `make_result_row(...)` so it includes:

```rust
failure_kind,
status: failure_kind.status().into(),
error: None,
```

- [ ] **Step 4: Keep runner wrappers passing**

Update any `BenchmarkResultRow` assumptions in `rsinter/tests/bench_runner_wrappers.rs` by adding these assertions:

```rust
assert_eq!(row.failure_kind, rsinter::failure::FailureKind::Ok);
```

for the zero-shot `rbposd` and `rilpqec` wrapper tests.

- [ ] **Step 5: Run benchmark runner tests**

Run:

```bash
cargo test -p rsinter failure_kind_is_structured
cargo test -p rsinter benchmark_runner_records_compile_failure_as_structured_row
cargo test -p rsinter benchmark_runner_records_decode_failure_as_structured_row
cargo test -p rsinter --test bench_runner_wrappers
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add rsinter/src/bench/runners/mod.rs rsinter/src/bench/runners/rmatching.rs rsinter/src/bench/runners/rbposd.rs rsinter/src/bench/runners/rilpqec.rs rsinter/tests/bench_runner_wrappers.rs
git commit -m "feat: classify benchmark runner failures"
```

---

### Task 5: Classify `collect` Outcomes

**Files:**
- Modify: `rsinter/src/collect.rs`
- Modify: `rsinter/tests/collect.rs`

- [ ] **Step 1: Write failing collect failure-kind tests**

In `rsinter/tests/collect.rs`, add:

```rust
use rsinter::failure::FailureKind;
```

Add these fake decoder helpers below the slow decoder helpers:

```rust
struct FailingDecoder {
    message: &'static str,
}

impl Decoder for FailingDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Result<Box<dyn CompiledDecoder>, String> {
        Err(self.message.to_string())
    }
}

fn make_failing_decoders(
    message: &'static str,
) -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(FailingDecoder { message }));
    decoders
}
```

Add this helper:

```rust
fn make_clean_task() -> Task {
    let circuit = parse_lines("M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    Task {
        circuit,
        decoder: "vacuous".into(),
        dem,
        metadata: serde_json::json!({"d": 3, "clean": true}),
        collection_options: CollectionOptions {
            max_shots: Some(16),
            max_errors: None,
            max_wall_seconds: None,
        },
    }
}
```

Append these tests:

```rust
#[test]
fn collect_reports_ok_failure_kind_for_clean_runs() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(16),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(16),
        start_batch_size: 16,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(vec![make_clean_task()], make_decoders(), &options).unwrap();

    assert_eq!(results[0].failure_kind, FailureKind::Ok);
}

#[test]
fn collect_reports_logical_failure_kind_for_logical_errors() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(vec![make_task()], make_decoders(), &options).unwrap();

    assert!(results[0].errors > 0);
    assert_eq!(results[0].failure_kind, FailureKind::LogicalFailure);
}

#[test]
fn collect_reports_timeout_failure_kind_for_wall_clock_stop() {
    let mut task = make_clean_task();
    task.collection_options.max_shots = None;

    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.09),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![task],
        make_slow_decoders(Duration::from_millis(35)),
        &options,
    )
    .unwrap();

    assert_eq!(results[0].failure_kind, FailureKind::Timeout);
}

#[test]
fn collect_records_decoder_failure_as_task_stats() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![make_clean_task()],
        make_failing_decoders("HiGHS backend error: compile failed"),
        &options,
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SolverFailure);
}
```

- [ ] **Step 2: Run the failing collect taxonomy tests**

Run:

```bash
cargo test -p rsinter collect_reports_ok_failure_kind_for_clean_runs
cargo test -p rsinter collect_reports_logical_failure_kind_for_logical_errors
cargo test -p rsinter collect_reports_timeout_failure_kind_for_wall_clock_stop
cargo test -p rsinter collect_records_decoder_failure_as_task_stats
```

Expected: FAIL because `collect` does not yet classify completed runs or convert decoder failures into `TaskStats`.

- [ ] **Step 3: Add classification in `collect`**

In `rsinter/src/collect.rs`, add:

```rust
use crate::failure::{FailureKind, classify_completed, classify_error};
```

Replace the parallel body with a result-aware collection:

```rust
let results: Vec<Result<TaskStats, String>> = pool.install(|| {
    tasks
        .par_iter()
        .map(|task| collect_one_task(task, &decoders, &existing, options))
        .collect()
});
let results: Vec<TaskStats> = results.into_iter().collect::<Result<_, _>>()?;
```

Move the current per-task logic into a new helper below `collect`:

```rust
fn collect_one_task(
    task: &Task,
    decoders: &HashMap<String, Box<dyn Decoder>>,
    existing: &HashMap<String, TaskStats>,
    options: &CollectOptions,
) -> Result<TaskStats, String> {
    let strong_id = task.strong_id();
    let decoder = decoders
        .get(&task.decoder)
        .ok_or_else(|| format!("decoder not found: {}", task.decoder))?;

    let mut total_shots: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut total_seconds: f64 = 0.0;

    if let Some(prev) = existing.get(&strong_id) {
        total_shots = prev.shots;
        total_errors = prev.errors;
        total_seconds = prev.seconds;
    }

    let compiled = match decoder.compile_for_dem(&task.dem) {
        Ok(compiled) => compiled,
        Err(error) => {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds,
                classify_error(&error, FailureKind::SolverFailure),
            ));
        }
    };

    let num_dets = task.dem.effective_num_detectors();
    let num_obs = task.dem.num_observables();
    let obs_bytes_per_shot = (num_obs + 7) / 8;

    let max_shots = task
        .collection_options
        .max_shots
        .or(options.max_shots)
        .unwrap_or(u64::MAX);
    let max_errors = task
        .collection_options
        .max_errors
        .or(options.max_errors)
        .unwrap_or(u64::MAX);
    let max_wall_seconds = task
        .collection_options
        .max_wall_seconds
        .or(options.max_wall_seconds);

    let mut batch_size = options.start_batch_size;
    let mut rng = StdRng::from_entropy();

    while should_continue_collecting(
        total_shots,
        total_errors,
        total_seconds,
        max_shots,
        max_errors,
        max_wall_seconds,
    ) {
        let remaining = (max_shots - total_shots) as usize;
        let n = batch_size.min(remaining);
        if n == 0 {
            break;
        }

        let batch_started = Instant::now();
        let batch = match sample_batch(&task.circuit, n, &mut rng) {
            Ok(batch) => batch,
            Err(_error) => {
                return Ok(make_task_stats(
                    task,
                    strong_id,
                    total_shots,
                    total_errors,
                    total_seconds + batch_started.elapsed().as_secs_f64(),
                    FailureKind::SamplerError,
                ));
            }
        };

        let mut det_buf = Vec::new();
        if let Err(_error) = write_shots_b8(&batch.detections, &mut det_buf) {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SamplerError,
            ));
        }
        let mut obs_buf = Vec::new();
        if let Err(_error) = write_shots_b8(&batch.observable_flips, &mut obs_buf) {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SamplerError,
            ));
        }

        let predictions = match compiled.decode_shots_bit_packed(&det_buf, n, num_dets, num_obs) {
            Ok(predictions) => predictions,
            Err(error) => {
                return Ok(make_task_stats(
                    task,
                    strong_id,
                    total_shots,
                    total_errors,
                    total_seconds + batch_started.elapsed().as_secs_f64(),
                    classify_error(&error, FailureKind::SolverFailure),
                ));
            }
        };
        let expected_len = n * obs_bytes_per_shot;
        if predictions.len() != expected_len || obs_buf.len() != expected_len {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SolverFailure,
            ));
        }

        let mut batch_errors = 0u64;
        for shot in 0..n {
            let offset = shot * obs_bytes_per_shot;
            let mut mismatch = false;
            for byte in 0..obs_bytes_per_shot {
                if predictions[offset + byte] != obs_buf[offset + byte] {
                    mismatch = true;
                    break;
                }
            }
            if mismatch {
                batch_errors += 1;
            }
        }

        total_shots += n as u64;
        total_errors += batch_errors;
        total_seconds += batch_started.elapsed().as_secs_f64();

        if let Some(max) = options.max_batch_size {
            batch_size = (batch_size * 2).min(max);
        } else {
            batch_size *= 2;
        }
    }

    let timed_out = max_wall_seconds.is_some_and(|max_seconds| total_seconds >= max_seconds);
    Ok(make_task_stats(
        task,
        strong_id,
        total_shots,
        total_errors,
        total_seconds,
        classify_completed(total_errors, timed_out),
    ))
}
```

Add this helper below `collect_one_task`:

```rust
fn make_task_stats(
    task: &Task,
    strong_id: String,
    shots: u64,
    errors: u64,
    seconds: f64,
    failure_kind: FailureKind,
) -> TaskStats {
    TaskStats {
        strong_id,
        decoder: task.decoder.clone(),
        metadata: task.metadata.clone(),
        shots,
        errors,
        discards: 0,
        seconds,
        failure_kind,
        custom_counts: HashMap::new(),
    }
}
```

- [ ] **Step 4: Run collect tests**

Run:

```bash
cargo test -p rsinter --test collect
```

Expected: PASS.

- [ ] **Step 5: Commit Task 5**

Run:

```bash
git add rsinter/src/collect.rs rsinter/tests/collect.rs
git commit -m "feat: classify collect task failures"
```

---

### Task 6: Add End-To-End Unsupported Backend Coverage

**Files:**
- Modify: `rsinter/tests/bench_run.rs`

- [ ] **Step 1: Write the unsupported `rilpqec` benchmark test**

In `rsinter/tests/bench_run.rs`, add:

```rust
use rsinter::failure::FailureKind;
```

Append this test near the other `rilpqec` benchmark tests:

```rust
#[cfg(not(feature = "gurobi"))]
#[test]
fn rilpqec_gurobi_without_feature_records_unsupported_failure_kind() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_gurobi"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 1
max_errors = 1
batch_size = 1
backend = "gurobi"

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rilpqec_gurobi")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "error");
    assert_eq!(rows[0].failure_kind, FailureKind::Unsupported);
    assert!(
        rows[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("no ILP backend is available"),
        "row error was: {:?}",
        rows[0].error
    );
}
```

- [ ] **Step 2: Run the unsupported backend test**

Run:

```bash
cargo test -p rsinter rilpqec_gurobi_without_feature_records_unsupported_failure_kind
```

Expected: PASS when the `gurobi` feature is not enabled. When the feature is enabled, Cargo reports the test as filtered out by `cfg`.

- [ ] **Step 3: Run focused regression checks**

Run:

```bash
cargo test -p rsinter failure_kind_is_structured
cargo test -p rsinter --test bench_result
cargo test -p rsinter --test csv_io
cargo test -p rsinter --test collect
cargo test -p rsinter rilpqec_gurobi_without_feature_records_unsupported_failure_kind
```

Expected: PASS.

- [ ] **Step 4: Commit Task 6**

Run:

```bash
git add rsinter/tests/bench_run.rs
git commit -m "test: cover unsupported rilpqec failure kind"
```

---

### Task 7: Full Verification

**Files:**
- Verify: all changed `rsinter` files

- [ ] **Step 1: Format the Rust workspace**

Run:

```bash
cargo fmt
```

Expected: exits 0 with no output on success.

- [ ] **Step 2: Run all `rsinter` tests**

Run:

```bash
cargo test -p rsinter
```

Expected: PASS.

- [ ] **Step 3: Inspect git status**

Run:

```bash
git status --short
```

Expected: only intentional changes remain. If `cargo fmt` changed files after the last task commit, commit those formatting changes with:

```bash
git add rsinter
git commit -m "style: format rsinter failure taxonomy"
```

- [ ] **Step 4: Final implementation summary**

Record these facts in the final handoff:

```text
- Benchmark JSONL rows now include failure_kind.
- Legacy JSONL and CSV rows without failure_kind remain readable.
- TaskStats and CSV resume data now carry failure_kind.
- Decoder backend failures return Result and are classified instead of panicking.
- rilpqec backend=gurobi without the feature records unsupported.
- cargo test -p rsinter passed.
```
