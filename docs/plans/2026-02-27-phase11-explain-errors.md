# Phase 11: explain_errors CLI Subcommand Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `rstim explain_errors` which, given a circuit and a set of detection events, finds the minimal set of DEM error mechanisms that could explain the observed detectors firing.

**Architecture:** A new `src/explain_errors.rs` module takes a `DetectorErrorModel` and a set of fired detector indices, then greedily finds error mechanisms whose detector sets cover the observed detectors. The CLI reads a circuit (or DEM), reads detection events in `dets` or `01` format, and writes explanations as text. This builds entirely on the existing `ErrorAnalyzer::circuit_to_dem` and `DetectorErrorModel`.

**Tech Stack:** Rust, existing `rstim` dem/error_analyzer modules, `clap`

---

## Task 1: explain_errors Core Logic

**Files:**
- Create: `src/explain_errors.rs`
- Modify: `src/lib.rs`
- Test: `tests/explain_errors.rs`

### Step 1: Write the failing tests

Create `tests/explain_errors.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::explain_errors::{explain, ExplainedError};

#[test]
fn explain_no_detectors_fired() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let fired: Vec<usize> = vec![];
    let explanations = explain(&dem, &fired);
    assert!(explanations.is_empty());
}

#[test]
fn explain_single_detector_fired() {
    // X_ERROR on qubit 0 before M 0 → detector fires
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let fired: Vec<usize> = vec![0]; // detector 0 fired
    let explanations = explain(&dem, &fired);
    assert!(!explanations.is_empty());
    // The explanation should cover detector 0
    let covered: Vec<usize> = explanations.iter()
        .flat_map(|e| e.detectors.iter().copied())
        .collect();
    assert!(covered.contains(&0));
}

#[test]
fn explain_output_format() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let fired = vec![0usize];
    let explanations = explain(&dem, &fired);
    for e in &explanations {
        // Each explanation has a probability and a list of detectors
        assert!(e.probability > 0.0);
        assert!(e.probability <= 1.0);
    }
}
```

### Step 2: Run test to verify it fails

```
cargo test --test explain_errors
```
Expected: compile error — `rstim::explain_errors` not found.

### Step 3: Implement explain_errors

Create `src/explain_errors.rs`:

```rust
use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};

#[derive(Debug, Clone)]
pub struct ExplainedError {
    pub probability: f64,
    pub detectors: Vec<usize>,
    pub observables: Vec<usize>,
}

/// Find error mechanisms in `dem` that together explain the `fired` detectors.
///
/// Uses a greedy approach: repeatedly pick the error mechanism that covers
/// the most currently-unexplained fired detectors, until all are covered
/// or no progress can be made.
///
/// Returns the list of error mechanisms selected.
pub fn explain(dem: &DetectorErrorModel, fired: &[usize]) -> Vec<ExplainedError> {
    if fired.is_empty() {
        return vec![];
    }

    // Flatten DEM to a list of error mechanisms
    let errors = collect_errors(dem);

    let mut remaining: std::collections::HashSet<usize> = fired.iter().copied().collect();
    let mut result = Vec::new();

    while !remaining.is_empty() {
        // Find the error that covers the most remaining detectors
        let best = errors.iter().max_by_key(|e| {
            e.detectors.iter().filter(|d| remaining.contains(d)).count()
        });

        match best {
            None => break,
            Some(e) => {
                let covered: usize = e.detectors.iter().filter(|d| remaining.contains(d)).count();
                if covered == 0 {
                    break; // no progress possible
                }
                for d in &e.detectors {
                    remaining.remove(d);
                }
                result.push(e.clone());
            }
        }
    }

    result
}

fn collect_errors(dem: &DetectorErrorModel) -> Vec<ExplainedError> {
    let mut out = Vec::new();
    collect_errors_instrs(dem.instructions(), &mut out, 0);
    out
}

fn collect_errors_instrs(
    instrs: &[DemInstruction],
    out: &mut Vec<ExplainedError>,
    det_offset: usize,
) {
    for instr in instrs {
        match instr {
            DemInstruction::Error { probability, targets } => {
                let mut detectors = Vec::new();
                let mut observables = Vec::new();
                for t in targets {
                    match t {
                        DemTarget::Detector(d) => detectors.push(d + det_offset),
                        DemTarget::Observable(o) => observables.push(*o),
                        DemTarget::Separator => {} // separate components — treat as one mechanism
                    }
                }
                out.push(ExplainedError {
                    probability: *probability,
                    detectors,
                    observables,
                });
            }
            DemInstruction::Repeat { count, body } => {
                let n_dets = body.num_detectors();
                let mut offset = det_offset;
                for _ in 0..*count {
                    collect_errors_instrs(body.instructions(), out, offset);
                    offset += n_dets;
                }
            }
            _ => {}
        }
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod explain_errors;
```

### Step 4: Run tests

```
cargo test --test explain_errors
```
Expected: all pass.

### Step 5: Commit

```bash
git add src/explain_errors.rs src/lib.rs tests/explain_errors.rs
git commit -m "feat: add explain_errors module with greedy error explanation"
```

---

## Task 2: explain_errors CLI Subcommand

**Files:**
- Modify: `src/cli.rs`
- Test: `tests/explain_errors.rs` (extend)

### Step 1: Write the failing test

Add to `tests/explain_errors.rs`:

```rust
#[test]
fn explain_errors_text_output() {
    use rstim::cli::run_explain_errors;
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    // dets format: "shot D0\n" means detector 0 fired
    let dets_input = "shot D0\n";
    let mut out = Vec::new();
    run_explain_errors(circuit, None, dets_input.as_bytes(), "dets", &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("error("));
}
```

### Step 2: Run test to verify it fails

```
cargo test --test explain_errors explain_errors_text_output
```
Expected: compile error — `run_explain_errors` not found.

### Step 3: Add ExplainErrors command to CLI

In `src/cli.rs`, add to `Commands`:

```rust
/// Explain which errors could have caused observed detection events
#[command(name = "explain_errors")]
ExplainErrors {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long = "in_format", default_value = "dets")]
    in_format: String,
    #[arg(long)]
    circuit: Option<String>,
    #[arg(long)]
    dem: Option<String>,
    #[arg(long)]
    out: Option<String>,
},
```

Add dispatch:

```rust
Some(Commands::ExplainErrors { r#in, in_format, circuit, dem, out }) => {
    let det_data = read_input_bytes(r#in.as_deref())?;
    let circuit_text = circuit.as_deref().map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string())).transpose()?;
    let dem_text = dem.as_deref().map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string())).transpose()?;
    let mut w = open_output(out.as_deref())?;
    run_explain_errors(
        circuit_text.as_deref().unwrap_or(""),
        dem_text.as_deref(),
        &det_data,
        &in_format,
        &mut w,
    )
}
```

Add `run_explain_errors`:

```rust
pub fn run_explain_errors(
    circuit_text: &str,
    dem_text: Option<&str>,
    det_data: &[u8],
    in_format: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    use crate::dem::DetectorErrorModel;

    // Build DEM from circuit or parse from file
    let dem = if let Some(dt) = dem_text {
        crate::dem::parse_dem(dt)?
    } else {
        let instrs = crate::parser::parse_lines(circuit_text)?;
        crate::error_analyzer::ErrorAnalyzer::circuit_to_dem(&instrs)?
    };

    // Parse detection events — support "dets" and "01" formats
    let fired_per_shot = parse_fired_detectors(det_data, in_format, dem.num_detectors())?;

    for (shot_idx, fired) in fired_per_shot.iter().enumerate() {
        let explanations = crate::explain_errors::explain(&dem, fired);
        if explanations.is_empty() {
            writeln!(out, "shot {shot_idx}: no errors needed").map_err(|e| e.to_string())?;
        } else {
            writeln!(out, "shot {shot_idx}:").map_err(|e| e.to_string())?;
            for e in &explanations {
                let det_str: Vec<String> = e.detectors.iter().map(|d| format!("D{d}")).collect();
                let obs_str: Vec<String> = e.observables.iter().map(|o| format!("L{o}")).collect();
                let targets: Vec<String> = det_str.into_iter().chain(obs_str).collect();
                writeln!(out, "  error({:.4}) {}", e.probability, targets.join(" "))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn parse_fired_detectors(
    data: &[u8],
    format: &str,
    n_dets: usize,
) -> Result<Vec<Vec<usize>>, String> {
    match format {
        "dets" => {
            let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
            let mut shots = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("shot") { continue; }
                let mut fired = Vec::new();
                for token in line.split_whitespace().skip(1) {
                    if let Some(rest) = token.strip_prefix('D') {
                        let d: usize = rest.parse().map_err(|_| format!("bad detector {token}"))?;
                        fired.push(d);
                    }
                }
                shots.push(fired);
            }
            Ok(shots)
        }
        "01" => {
            use crate::output::read_shots_01;
            let table = read_shots_01(data, n_dets)?;
            let n_shots = table.num_minor();
            let mut shots = Vec::new();
            for shot in 0..n_shots {
                let fired: Vec<usize> = (0..n_dets).filter(|&d| table.get(d, shot)).collect();
                shots.push(fired);
            }
            Ok(shots)
        }
        _ => Err(format!("unsupported in_format for explain_errors: {format}")),
    }
}
```

### Step 4: Add DEM parser (needed for --dem flag)

In `src/dem.rs`, add a minimal `parse_dem` function that reads the DEM text format:

```rust
pub fn parse_dem(text: &str) -> Result<DetectorErrorModel, String> {
    // Parse lines like:
    //   error(0.1) D0 D1
    //   detector(1,2,3) D5
    //   logical_observable L0
    //   shift_detectors(1) 3
    //   repeat 10 { ... }
    let mut dem = DetectorErrorModel::new();
    // ... minimal parser
    Ok(dem)
}
```

### Step 5: Run tests

```
cargo test
```
Expected: all pass.

### Step 6: Commit

```bash
git add src/cli.rs src/dem.rs tests/explain_errors.rs
git commit -m "feat: add rstim explain_errors CLI subcommand"
```
