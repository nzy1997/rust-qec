# rsinter — Parallel Sampling & Statistics for rstim

## Overview

rsinter is a new crate in the rstim workspace that provides sinter-compatible parallel sampling, decoding, and statistical analysis for quantum error correction benchmarks.

## Package Structure

Cargo workspace with two members:
- `rstim/` — existing simulator (lean, minimal deps)
- `rsinter/` — new sampling harness (depends on rstim)

## Data Types

### Fit
Binomial confidence interval result.
```rust
pub struct Fit { pub low: Option<f64>, pub best: Option<f64>, pub high: Option<f64> }
```

### Task
A single decoding experiment.
```rust
pub struct Task {
    pub circuit: Vec<StimInstr>,
    pub decoder: String,
    pub dem: DetectorErrorModel,
    pub metadata: serde_json::Value,
    pub collection_options: CollectionOptions,
}
pub struct CollectionOptions { pub max_shots: Option<u64>, pub max_errors: Option<u64> }
```

### TaskStats
Results from sampling. Identified by `strong_id` (SHA256 of circuit+decoder+metadata).
```rust
pub struct TaskStats {
    pub strong_id: String,
    pub decoder: String,
    pub metadata: serde_json::Value,
    pub shots: u64, pub errors: u64, pub discards: u64,
    pub seconds: f64,
    pub custom_counts: HashMap<String, u64>,
}
```

## Probability Utilities (`rsinter::stats`)

- `fit_binomial(num_shots, num_hits, max_likelihood_factor) -> Fit` — binary search over log_binomial
- `log_binomial(p, n, hits) -> f64` — log-probability of binomial outcome
- `shot_error_rate_to_piece_error_rate(shot_rate, pieces) -> f64` — per-shot to per-round conversion

No external math deps — pure Rust with std f64 operations.

## Decoder Interface (`rsinter::decode`)

```rust
pub trait CompiledDecoder: Send {
    fn decode_shots_bit_packed(&self, dets: &[u8], num_shots: usize, num_dets: usize) -> Vec<u8>;
}
pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder>;
}
pub struct VacuousDecoder; // always predicts no errors
```

## Collection Engine (`rsinter::collect`)

```rust
pub struct CollectOptions {
    pub num_workers: usize,
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
    pub max_batch_size: Option<usize>,
    pub start_batch_size: usize,
    pub save_resume_filepath: Option<PathBuf>,
    pub print_progress: bool,
}
pub fn collect(tasks: Vec<Task>, decoders: HashMap<String, Box<dyn Decoder>>, options: &CollectOptions) -> Result<Vec<TaskStats>, String>
```

Flow:
1. Load existing CSV if resume path exists
2. Compute strong_id per task (SHA256)
3. Rayon parallel iteration across tasks
4. Per worker: sample_batch → m2d → decode → compare → count errors
5. Ramp batch size from start_batch_size up to max_batch_size
6. Append incremental results to CSV
7. Stop per task when max_shots or max_errors reached

## CSV Format (sinter-compatible)

```
shots,errors,discards,seconds,decoder,strong_id,json_metadata,custom_counts
```

## Crate Layout

```
rsinter/src/
  lib.rs
  stats.rs          — fit_binomial, log_binomial, error rate conversion
  decode.rs         — Decoder, CompiledDecoder traits, VacuousDecoder
  task.rs           — Task, CollectionOptions, strong_id
  task_stats.rs     — TaskStats, AnonTaskStats, CSV serialization
  collect.rs        — collect(), parallel sampling engine
  csv_io.rs         — read/write sinter-compatible CSV
```

## Dependencies

- rstim (workspace)
- rayon (parallelism)
- serde + serde_json (metadata)
- csv (CSV I/O)
- sha2 (strong_id)

## Non-goals for v1

- No CLI (library only)
- No plotting (users can use Python/matplotlib on the CSV output)
- No real decoders (trait + VacuousDecoder only)
