# rsinter Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build rsinter, a sinter-compatible parallel sampling and statistics crate for rstim.

**Architecture:** Cargo workspace with rstim (simulator) and rsinter (sampling harness). rsinter depends on rstim as a library. Six modules: stats, decode, task, task_stats, csv_io, collect.

**Tech Stack:** Rust, rayon, serde/serde_json, csv, sha2

---

### Task 1: Convert to Cargo Workspace

**Files:**
- Modify: `Cargo.toml` (root — becomes workspace manifest)
- Create: `rstim/Cargo.toml` (move existing package config here)
- Move: `src/` → `rstim/src/`, `tests/` → `rstim/tests/`
- Create: `rsinter/Cargo.toml`
- Create: `rsinter/src/lib.rs`

**Step 1: Create workspace root Cargo.toml**

Replace root `Cargo.toml` with:
```toml
[workspace]
members = ["rstim", "rsinter"]
resolver = "3"
```

**Step 2: Move rstim into subdirectory**

```bash
mkdir rstim
mv src rstim/
mv tests rstim/
# Move the old Cargo.toml content into rstim/Cargo.toml
```

Create `rstim/Cargo.toml`:
```toml
[package]
name = "rstim"
version = "0.1.0"
edition = "2024"

[dependencies]
rand = "0.8"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
tempfile = "3"
```

**Step 3: Create rsinter crate skeleton**

```bash
mkdir -p rsinter/src
```

Create `rsinter/Cargo.toml`:
```toml
[package]
name = "rsinter"
version = "0.1.0"
edition = "2024"

[dependencies]
rstim = { path = "../rstim" }
rand = "0.8"
rayon = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
csv = "1"
sha2 = "0.10"
```

Create `rsinter/src/lib.rs`:
```rust
pub mod stats;
pub mod decode;
pub mod task;
pub mod task_stats;
pub mod csv_io;
pub mod collect;
```

**Step 4: Verify workspace builds**

```bash
cargo build
cargo test --workspace
```
Expected: all existing rstim tests pass, rsinter compiles (empty modules).

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: convert to cargo workspace with rstim + rsinter"
```

---

### Task 2: Probability Utilities (stats.rs)

**Files:**
- Create: `rsinter/src/stats.rs`
- Create: `rsinter/tests/stats.rs`

**Step 1: Write failing tests**

Create `rsinter/tests/stats.rs`:
```rust
use rsinter::stats::{Fit, log_binomial, log_factorial, fit_binomial, shot_error_rate_to_piece_error_rate};

#[test]
fn log_factorial_base_cases() {
    assert_eq!(log_factorial(0), 0.0);
    assert_eq!(log_factorial(1), 0.0);
    assert!((log_factorial(2) - 2.0_f64.ln()).abs() < 1e-10);
}

#[test]
fn log_binomial_fair_coin() {
    // P(50 heads in 100 flips of fair coin) ≈ exp(-2.53)
    let result = log_binomial(0.5, 100, 50);
    assert!((result - (-2.5308762)).abs() < 0.01);
}

#[test]
fn log_binomial_edge_p0() {
    // P(hits>0 | p=0) = -inf
    assert!(log_binomial(0.0, 100, 1).is_infinite());
    assert!(log_binomial(0.0, 100, 1) < 0.0);
}

#[test]
fn log_binomial_edge_p1() {
    // P(misses>0 | p=1) = -inf
    assert!(log_binomial(1.0, 100, 99).is_infinite());
}

#[test]
fn log_binomial_all_hits_p1() {
    // P(100 hits | p=1) = 1, ln(1) = 0
    assert!((log_binomial(1.0, 100, 100) - 0.0).abs() < 1e-6);
}

#[test]
fn fit_binomial_zero_shots() {
    let f = fit_binomial(0, 0, 1000.0);
    assert_eq!(f.best, Some(0.5));
    assert_eq!(f.low, Some(0.0));
    assert_eq!(f.high, Some(1.0));
}

#[test]
fn fit_binomial_100m_shots_2_hits() {
    // sinter: Fit(low=2e-10, best=2e-08, high=1.259e-07)
    let f = fit_binomial(100_000_000, 2, 1000.0);
    assert!((f.best.unwrap() - 2e-8).abs() < 1e-10);
    assert!(f.low.unwrap() < f.best.unwrap());
    assert!(f.high.unwrap() > f.best.unwrap());
}

#[test]
fn fit_binomial_10_shots_5_hits() {
    // sinter: Fit(low=0.202, best=0.5, high=0.798)
    let f = fit_binomial(10, 5, 9.0);
    assert!((f.best.unwrap() - 0.5).abs() < 1e-6);
    assert!((f.low.unwrap() - 0.202).abs() < 0.01);
    assert!((f.high.unwrap() - 0.798).abs() < 0.01);
}

#[test]
fn piece_error_rate_identity() {
    // pieces=1 → same rate
    let r = shot_error_rate_to_piece_error_rate(0.1, 1.0);
    assert!((r - 0.1).abs() < 1e-10);
}

#[test]
fn piece_error_rate_2_pieces() {
    // sinter: 0.05278640450004207
    let r = shot_error_rate_to_piece_error_rate(0.1, 2.0);
    assert!((r - 0.05278640450004207).abs() < 1e-8);
}

#[test]
fn piece_error_rate_100_pieces() {
    // sinter: 1.000000082740371e-11
    let r = shot_error_rate_to_piece_error_rate(1e-9, 100.0);
    assert!((r - 1e-11).abs() < 1e-13);
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test --package rsinter --test stats
```
Expected: FAIL — module `stats` not found.

**Step 3: Implement stats.rs**

Create `rsinter/src/stats.rs`:
```rust
/// Binomial confidence interval result.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub low: Option<f64>,
    pub best: Option<f64>,
    pub high: Option<f64>,
}

/// ln(n!) via lgamma(n+1)
pub fn log_factorial(n: u64) -> f64 {
    // lgamma(n+1) = ln(n!)
    let v = (n as f64 + 1.0).ln_gamma().0;
    v
}

/// ln(P(hits = B(n, p))) — log-probability of binomial outcome.
pub fn log_binomial(p: f64, n: u64, hits: u64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    let misses = n - hits;

    if hits > 0 && p == 0.0 { return f64::NEG_INFINITY; }
    if misses > 0 && p == 1.0 { return f64::NEG_INFINITY; }

    let mut result = 0.0;
    if p > 0.0 { result += p.ln() * hits as f64; }
    if p < 1.0 { result += (1.0 - p).ln() * misses as f64; }
    result += log_factorial(n) - log_factorial(misses) - log_factorial(hits);
    result
}

/// Integer binary search over monotonically ascending function.
fn binary_search(func: impl Fn(i64) -> f64, min_x: i64, max_x: i64, target: f64) -> i64 {
    let mut lo = min_x;
    let mut hi = max_x;
    while hi > lo + 1 {
        let mid = lo + (hi - lo) / 2;
        let v = func(mid);
        if v < target { lo = mid; }
        else if v > target { hi = mid; }
        else { return mid; }
    }
    let fhi = func(hi);
    let flo = func(lo);
    let dhi = if fhi == target { 0.0 } else { (fhi - target).abs() };
    let dlo = if flo == target { 0.0 } else { (flo - target).abs() };
    if dhi < dlo { hi } else { lo }
}

/// Determine hypothesis probabilities compatible with the given hit ratio.
/// Uses binary search over log_binomial likelihoods.
pub fn fit_binomial(num_shots: u64, num_hits: u64, max_likelihood_factor: f64) -> Fit {
    if num_shots == 0 {
        return Fit { low: Some(0.0), best: Some(0.5), high: Some(1.0) };
    }
    let best_p = num_hits as f64 / num_shots as f64;
    let log_ml = log_binomial(best_p, num_shots, num_hits);
    let target = log_ml - max_likelihood_factor.ln();
    let acc: i64 = 100;

    let low = binary_search(
        |exp_err| log_binomial(exp_err as f64 / (acc as f64 * num_shots as f64), num_shots, num_hits),
        0,
        num_hits as i64 * acc,
        target,
    );
    let high = binary_search(
        |exp_err| -log_binomial(exp_err as f64 / (acc as f64 * num_shots as f64), num_shots, num_hits),
        num_hits as i64 * acc,
        num_shots as i64 * acc,
        -target,
    );

    Fit {
        best: Some(best_p),
        low: Some(low as f64 / (acc as f64 * num_shots as f64)),
        high: Some(high as f64 / (acc as f64 * num_shots as f64)),
    }
}

/// Convert per-shot error rate to per-piece (per-round) error rate.
/// Inverse of: shot_rate = 1 - (1 - 2*piece_rate)^pieces / 2
pub fn shot_error_rate_to_piece_error_rate(shot_error_rate: f64, pieces: f64) -> f64 {
    assert!((0.0..=1.0).contains(&shot_error_rate));
    assert!(pieces > 0.0);
    if pieces == 1.0 { return shot_error_rate; }
    if shot_error_rate > 0.5 {
        return 1.0 - shot_error_rate_to_piece_error_rate(1.0 - shot_error_rate, pieces);
    }
    let randomize_rate = 2.0 * shot_error_rate;
    let round_randomize_rate = 1.0 - (1.0 - randomize_rate).powf(1.0 / pieces);
    let round_error_rate = round_randomize_rate / 2.0;
    if round_error_rate == 0.0 {
        return shot_error_rate / pieces;
    }
    round_error_rate
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test --package rsinter --test stats
```
Expected: all PASS.

**Step 5: Commit**

```bash
git add rsinter/src/stats.rs rsinter/tests/stats.rs
git commit -m "feat(rsinter): add probability utilities (fit_binomial, log_binomial)"
```

---

### Task 3: Decoder Trait + VacuousDecoder (decode.rs)

**Files:**
- Create: `rsinter/src/decode.rs`
- Create: `rsinter/tests/decode.rs`

**Step 1: Write failing tests**

Create `rsinter/tests/decode.rs`:
```rust
use rsinter::decode::{Decoder, CompiledDecoder, VacuousDecoder};
use rstim::dem::DetectorErrorModel;

#[test]
fn vacuous_decoder_returns_all_zeros() {
    let dem = DetectorErrorModel::parse("error(0.1) D0").unwrap();
    let decoder = VacuousDecoder;
    let compiled = decoder.compile_for_dem(&dem);
    // 2 shots, 1 detector, 1 observable → dets_bytes=1 per shot, obs_bytes=1 per shot
    let dets = vec![0b1, 0b0]; // shot0: D0 fired, shot1: nothing
    let predictions = compiled.decode_shots_bit_packed(&dets, 2, 1, 1);
    // Vacuous always predicts 0
    assert_eq!(predictions, vec![0u8, 0u8]);
}

#[test]
fn vacuous_decoder_multi_obs() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0 L1").unwrap();
    let decoder = VacuousDecoder;
    let compiled = decoder.compile_for_dem(&dem);
    let dets = vec![0b1]; // 1 shot
    let predictions = compiled.decode_shots_bit_packed(&dets, 1, 1, 2);
    assert_eq!(predictions, vec![0u8]);
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test --package rsinter --test decode
```

**Step 3: Implement decode.rs**

Create `rsinter/src/decode.rs`:
```rust
use rstim::dem::DetectorErrorModel;

pub trait CompiledDecoder: Send {
    /// Decode bit-packed detection events → bit-packed observable predictions.
    /// dets: num_shots * ceil(num_dets/8) bytes, b8 format
    /// returns: num_shots * ceil(num_obs/8) bytes, b8 format
    fn decode_shots_bit_packed(
        &self, dets: &[u8], num_shots: usize, num_dets: usize, num_obs: usize,
    ) -> Vec<u8>;
}

pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder>;
}

/// Always predicts no observable flips. Useful for testing the pipeline.
pub struct VacuousDecoder;

struct VacuousCompiled { obs_bytes: usize }

impl CompiledDecoder for VacuousCompiled {
    fn decode_shots_bit_packed(
        &self, _dets: &[u8], num_shots: usize, _num_dets: usize, _num_obs: usize,
    ) -> Vec<u8> {
        vec![0u8; num_shots * self.obs_bytes]
    }
}

impl Decoder for VacuousDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let num_obs = dem.num_observables();
        Box::new(VacuousCompiled { obs_bytes: (num_obs + 7) / 8 })
    }
}
```

**Step 4: Run tests, verify pass**

```bash
cargo test --package rsinter --test decode
```

**Step 5: Commit**

```bash
git commit -m "feat(rsinter): add Decoder/CompiledDecoder traits and VacuousDecoder"
```

---

### Task 4: Task + strong_id (task.rs)

**Files:**
- Create: `rsinter/src/task.rs`
- Create: `rsinter/tests/task.rs`

**Step 1: Write failing tests**

Create `rsinter/tests/task.rs`:
```rust
use rsinter::task::{Task, CollectionOptions};
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;

#[test]
fn strong_id_deterministic() {
    let circuit = parse_lines("X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let t1 = Task {
        circuit: circuit.clone(),
        decoder: "vacuous".into(),
        dem: dem.clone(),
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions::default(),
    };
    let t2 = Task {
        circuit, decoder: "vacuous".into(), dem,
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions::default(),
    };
    assert_eq!(t1.strong_id(), t2.strong_id());
    assert_eq!(t1.strong_id().len(), 64); // SHA256 hex
}

#[test]
fn strong_id_changes_with_decoder() {
    let circuit = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let t1 = Task {
        circuit: circuit.clone(), decoder: "a".into(), dem: dem.clone(),
        metadata: serde_json::Value::Null,
        collection_options: CollectionOptions::default(),
    };
    let t2 = Task {
        circuit, decoder: "b".into(), dem,
        metadata: serde_json::Value::Null,
        collection_options: CollectionOptions::default(),
    };
    assert_ne!(t1.strong_id(), t2.strong_id());
}
```

**Step 2: Run tests to verify they fail**

**Step 3: Implement task.rs**

Create `rsinter/src/task.rs`:
```rust
use rstim::ir::{StimInstr, circuit_to_string};
use rstim::dem::DetectorErrorModel;
use sha2::{Sha256, Digest};

#[derive(Clone, Debug, Default)]
pub struct CollectionOptions {
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub circuit: Vec<StimInstr>,
    pub decoder: String,
    pub dem: DetectorErrorModel,
    pub metadata: serde_json::Value,
    pub collection_options: CollectionOptions,
}

impl Task {
    pub fn strong_id(&self) -> String {
        let obj = serde_json::json!({
            "circuit": circuit_to_string(&self.circuit),
            "decoder": self.decoder,
            "dem": self.dem.to_string(),
            "metadata": self.metadata,
        });
        let text = serde_json::to_string(&obj).unwrap();
        let hash = Sha256::digest(text.as_bytes());
        format!("{:x}", hash)
    }
}
```

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git commit -m "feat(rsinter): add Task with strong_id (SHA256)"
```

---

### Task 5: TaskStats + CSV I/O (task_stats.rs, csv_io.rs)

**Files:**
- Create: `rsinter/src/task_stats.rs`
- Create: `rsinter/src/csv_io.rs`
- Create: `rsinter/tests/csv_io.rs`

**Step 1: Write failing tests**

Create `rsinter/tests/csv_io.rs`:
```rust
use rsinter::task_stats::TaskStats;
use rsinter::csv_io::{write_csv, read_csv};
use std::collections::HashMap;

fn sample_stats() -> TaskStats {
    TaskStats {
        strong_id: "abc123".into(),
        decoder: "vacuous".into(),
        metadata: serde_json::json!({"d": 3}),
        shots: 1000,
        errors: 5,
        discards: 0,
        seconds: 1.23,
        custom_counts: HashMap::new(),
    }
}

#[test]
fn csv_roundtrip() {
    let stats = vec![sample_stats()];
    let mut buf = Vec::new();
    write_csv(&stats, &mut buf).unwrap();
    let recovered = read_csv(&buf).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].shots, 1000);
    assert_eq!(recovered[0].errors, 5);
    assert_eq!(recovered[0].strong_id, "abc123");
}

#[test]
fn task_stats_addition() {
    let a = sample_stats();
    let b = TaskStats { shots: 500, errors: 2, seconds: 0.5, ..sample_stats() };
    let c = a + b;
    assert_eq!(c.shots, 1500);
    assert_eq!(c.errors, 7);
    assert!((c.seconds - 1.73).abs() < 0.01);
}
```

**Step 2: Run tests to verify they fail**

**Step 3: Implement task_stats.rs**

Create `rsinter/src/task_stats.rs`:
```rust
use std::collections::HashMap;
use std::ops::Add;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStats {
    pub strong_id: String,
    pub decoder: String,
    pub metadata: serde_json::Value,
    pub shots: u64,
    pub errors: u64,
    pub discards: u64,
    pub seconds: f64,
    pub custom_counts: HashMap<String, u64>,
}

impl Add for TaskStats {
    type Output = TaskStats;
    fn add(self, rhs: TaskStats) -> TaskStats {
        let mut counts = self.custom_counts;
        for (k, v) in rhs.custom_counts { *counts.entry(k).or_insert(0) += v; }
        TaskStats {
            strong_id: self.strong_id,
            decoder: self.decoder,
            metadata: self.metadata,
            shots: self.shots + rhs.shots,
            errors: self.errors + rhs.errors,
            discards: self.discards + rhs.discards,
            seconds: self.seconds + rhs.seconds,
            custom_counts: counts,
        }
    }
}
```

**Step 4: Implement csv_io.rs**

Create `rsinter/src/csv_io.rs`:
```rust
use crate::task_stats::TaskStats;
use std::collections::HashMap;
use std::io::{Read, Write};

pub fn write_csv(stats: &[TaskStats], out: &mut dyn Write) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(out);
    wtr.write_record(&["shots","errors","discards","seconds","decoder","strong_id","json_metadata","custom_counts"])
        .map_err(|e| e.to_string())?;
    for s in stats {
        wtr.write_record(&[
            s.shots.to_string(),
            s.errors.to_string(),
            s.discards.to_string(),
            format!("{:.4}", s.seconds),
            s.decoder.clone(),
            s.strong_id.clone(),
            serde_json::to_string(&s.metadata).unwrap_or_default(),
            serde_json::to_string(&s.custom_counts).unwrap_or_default(),
        ]).map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_csv(data: &[u8]) -> Result<Vec<TaskStats>, String> {
    let mut rdr = csv::Reader::from_reader(data);
    let mut results = Vec::new();
    for record in rdr.records() {
        let r = record.map_err(|e| e.to_string())?;
        results.push(TaskStats {
            shots: r[0].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            errors: r[1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            discards: r[2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            seconds: r[3].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?,
            decoder: r[4].to_string(),
            strong_id: r[5].to_string(),
            metadata: serde_json::from_str(&r[6]).unwrap_or(serde_json::Value::Null),
            custom_counts: serde_json::from_str(&r[7]).unwrap_or_default(),
        });
    }
    Ok(results)
}
```

**Step 5: Run tests, verify pass**

**Step 6: Commit**

```bash
git commit -m "feat(rsinter): add TaskStats with CSV read/write"
```

---

### Task 6: Collection Engine (collect.rs)

**Files:**
- Create: `rsinter/src/collect.rs`
- Create: `rsinter/tests/collect.rs`

**Step 1: Write failing tests**

Create `rsinter/tests/collect.rs`:
```rust
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::VacuousDecoder;
use rsinter::task::{Task, CollectionOptions};
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use std::collections::HashMap;

fn make_task() -> Task {
    let circuit = parse_lines("X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    Task {
        circuit, decoder: "vacuous".into(), dem,
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions { max_shots: Some(1000), max_errors: None },
    }
}

#[test]
fn collect_single_task_vacuous() {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };
    let results = collect(vec![make_task()], decoders, &options).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].shots >= 1000);
    // With X_ERROR(0.1) and vacuous decoder, ~10% error rate
    assert!(results[0].errors > 0);
}

#[test]
fn collect_respects_max_errors() {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options = CollectOptions {
        num_workers: 1,
        max_shots: None,
        max_errors: Some(10),
        max_batch_size: Some(64),
        start_batch_size: 16,
        save_resume_filepath: None,
        print_progress: false,
    };
    let results = collect(vec![make_task()], decoders, &options).unwrap();
    assert!(results[0].errors >= 10);
}

#[test]
fn collect_csv_resume() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(500),
        max_errors: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: Some(path.clone()),
        print_progress: false,
    };
    let r1 = collect(vec![make_task()], decoders, &options).unwrap();

    // Resume — should load existing and continue
    let mut decoders2: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders2.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options2 = CollectOptions {
        max_shots: Some(1000),
        save_resume_filepath: Some(path),
        ..options
    };
    let r2 = collect(vec![make_task()], decoders2, &options2).unwrap();
    assert!(r2[0].shots >= 1000);
    assert!(r2[0].shots >= r1[0].shots);
}
```

**Step 2: Run tests to verify they fail**

**Step 3: Implement collect.rs**

Create `rsinter/src/collect.rs`:
```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use rayon::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstim::sampler::sample_batch;
use rstim::output::{write_shots_b8, read_shots_b8};
use rstim::sim::bit_table::BitTable;

use crate::decode::Decoder;
use crate::task::Task;
use crate::task_stats::TaskStats;
use crate::csv_io;

pub struct CollectOptions {
    pub num_workers: usize,
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
    pub max_batch_size: Option<usize>,
    pub start_batch_size: usize,
    pub save_resume_filepath: Option<PathBuf>,
    pub print_progress: bool,
}

pub fn collect(
    tasks: Vec<Task>,
    decoders: HashMap<String, Box<dyn Decoder>>,
    options: &CollectOptions,
) -> Result<Vec<TaskStats>, String> {
    // Load existing data if resume path exists
    let mut existing: HashMap<String, TaskStats> = HashMap::new();
    if let Some(ref path) = options.save_resume_filepath {
        if path.exists() {
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            for s in csv_io::read_csv(&data)? {
                existing.entry(s.strong_id.clone())
                    .and_modify(|e| { let merged = e.clone() + s.clone(); *e = merged; })
                    .or_insert(s);
            }
        }
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.num_workers)
        .build()
        .map_err(|e| e.to_string())?;

    let results: Vec<TaskStats> = pool.install(|| {
        tasks.par_iter().map(|task| {
            let strong_id = task.strong_id();
            let compiled = decoders.get(&task.decoder)
                .expect("decoder not found")
                .compile_for_dem(&task.dem);

            let num_dets = task.dem.num_detectors();
            let num_obs = task.dem.num_observables();

            let mut total_shots: u64 = 0;
            let mut total_errors: u64 = 0;
            let mut total_seconds: f64 = 0.0;

            // Account for existing data
            if let Some(prev) = existing.get(&strong_id) {
                total_shots = prev.shots;
                total_errors = prev.errors;
                total_seconds = prev.seconds;
            }

            let max_shots = task.collection_options.max_shots
                .or(options.max_shots).unwrap_or(u64::MAX);
            let max_errors = task.collection_options.max_errors
                .or(options.max_errors).unwrap_or(u64::MAX);

            let mut batch_size = options.start_batch_size;
            let mut rng = StdRng::from_entropy();

            while total_shots < max_shots && total_errors < max_errors {
                let remaining = (max_shots - total_shots) as usize;
                let n = batch_size.min(remaining);
                if n == 0 { break; }

                let start = Instant::now();
                let batch = sample_batch(&task.circuit, n, &mut rng).unwrap();
                let elapsed = start.elapsed().as_secs_f64();

                // Get detection events as b8
                let det_bytes_per_shot = (num_dets + 7) / 8;
                let obs_bytes_per_shot = (num_obs + 7) / 8;
                let mut det_buf = Vec::new();
                write_shots_b8(&batch.detections, &mut det_buf).unwrap();
                let mut obs_buf = Vec::new();
                write_shots_b8(&batch.observable_flips, &mut obs_buf).unwrap();

                // Decode
                let predictions = compiled.decode_shots_bit_packed(
                    &det_buf, n, num_dets, num_obs,
                );

                // Compare predictions to actual observable flips
                let mut batch_errors = 0u64;
                for shot in 0..n {
                    let pred_start = shot * obs_bytes_per_shot;
                    let actual_start = shot * obs_bytes_per_shot;
                    let mut mismatch = false;
                    for byte in 0..obs_bytes_per_shot {
                        if predictions[pred_start + byte] != obs_buf[actual_start + byte] {
                            mismatch = true;
                            break;
                        }
                    }
                    if mismatch { batch_errors += 1; }
                }

                total_shots += n as u64;
                total_errors += batch_errors;
                total_seconds += elapsed;

                // Ramp batch size
                if let Some(max) = options.max_batch_size {
                    batch_size = (batch_size * 2).min(max);
                } else {
                    batch_size *= 2;
                }
            }

            TaskStats {
                strong_id: strong_id.clone(),
                decoder: task.decoder.clone(),
                metadata: task.metadata.clone(),
                shots: total_shots,
                errors: total_errors,
                discards: 0,
                seconds: total_seconds,
                custom_counts: HashMap::new(),
            }
        }).collect()
    });

    // Save to CSV if path specified
    if let Some(ref path) = options.save_resume_filepath {
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        csv_io::write_csv(&results, &mut file)?;
    }

    if options.print_progress {
        for r in &results {
            eprintln!("[rsinter] {} shots={} errors={} ({:.2}s)",
                &r.strong_id[..8], r.shots, r.errors, r.seconds);
        }
    }

    Ok(results)
}
```

**Step 4: Run tests, verify pass**

```bash
cargo test --package rsinter --test collect
```

**Step 5: Commit**

```bash
git commit -m "feat(rsinter): add parallel collection engine"
```

---

### Task 7: Wire up lib.rs + Integration Test

**Files:**
- Modify: `rsinter/src/lib.rs`
- Create: `rsinter/tests/integration.rs`

**Step 1: Ensure lib.rs exports all modules**

`rsinter/src/lib.rs` should already have:
```rust
pub mod stats;
pub mod decode;
pub mod task;
pub mod task_stats;
pub mod csv_io;
pub mod collect;
```

**Step 2: Write integration test**

Create `rsinter/tests/integration.rs`:
```rust
//! End-to-end: generate circuit → collect → fit_binomial
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::VacuousDecoder;
use rsinter::task::{Task, CollectionOptions};
use rsinter::stats::fit_binomial;
use rstim::codegen::repetition_code_memory;
use rstim::error_analyzer::ErrorAnalyzer;
use std::collections::HashMap;

#[test]
fn end_to_end_rep_code() {
    let circuit = repetition_code_memory(3, 1, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let task = Task {
        circuit, decoder: "vacuous".into(), dem,
        metadata: serde_json::json!({"d": 3, "p": 0.01}),
        collection_options: CollectionOptions { max_shots: Some(10_000), max_errors: None },
    };
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options = CollectOptions {
        num_workers: 2, max_shots: Some(10_000), max_errors: None,
        max_batch_size: Some(1024), start_batch_size: 64,
        save_resume_filepath: None, print_progress: false,
    };
    let results = collect(vec![task], decoders, &options).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].shots >= 10_000);

    let fit = fit_binomial(results[0].shots, results[0].errors, 1000.0);
    assert!(fit.low.unwrap() < fit.best.unwrap());
    assert!(fit.best.unwrap() < fit.high.unwrap());
}
```

**Step 3: Run full test suite**

```bash
cargo test --workspace
```

**Step 4: Commit**

```bash
git commit -m "feat(rsinter): add end-to-end integration test"
```
