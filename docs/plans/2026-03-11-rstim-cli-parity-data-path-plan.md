# rstim CLI Data-Path Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add the highest-value remaining Stim-style CLI data-path parity for `sample`, `detect`, and `m2d` while preserving current default behavior.

**Architecture:** Introduce one shared reference-sample policy and route new CLI flags into library-facing helpers instead of scattering conditionals through command handlers. Extend `m2d` in two later passes: first with per-shot sweep-aware reference generation, then with a feedback-normalization pass that strips supported feedback instructions and records measurement-flip corrections.

**Tech Stack:** Rust, `clap`, existing `BitTable`/frame-simulator utilities, cargo integration tests (`CARGO_HOME=/tmp/rstim-cargo-home cargo test ...`)

---

## Preflight

- Work in `/Users/nzy/rcode/rstim/.worktrees/master-sync` on branch `plan/cli-data-path-parity`.
- Use `CARGO_HOME=/tmp/rstim-cargo-home` for every cargo command in this workspace. The default cargo cache under `~/.cargo` is not writable here.
- Keep commits small. Do not batch multiple feature tasks into one commit.

### Task 1: Shared Reference-Sample Helpers

**Files:**
- Create: `rstim/src/data_path.rs`
- Modify: `rstim/src/lib.rs`
- Modify: `rstim/src/sampler.rs`
- Test: `rstim/tests/data_path.rs`

**Step 1: Write the failing test**

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::data_path::{build_reference_sample, ReferenceSampleMode};
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch, sample_batch_with_options, SampleOptions};

#[test]
fn zero_reference_mode_returns_expected_measurement_width() {
    let instrs = parse_lines("X 0\nM 0\nM 0\n").unwrap();
    let sample = build_reference_sample(&instrs, ReferenceSampleMode::AssumeAllZero).unwrap();
    assert_eq!(sample, vec![false, false]);
}

#[test]
fn sample_batch_wrapper_matches_default_options() {
    let instrs = parse_lines("R 0\nM 0\n").unwrap();
    let mut rng_a = StdRng::seed_from_u64(7);
    let mut rng_b = StdRng::seed_from_u64(7);
    let wrapped = sample_batch(&instrs, 4, &mut rng_a).unwrap();
    let explicit = sample_batch_with_options(
        &instrs,
        4,
        &mut rng_b,
        SampleOptions::default(),
    )
    .unwrap();
    assert_eq!(wrapped.measurements, explicit.measurements);
    assert_eq!(wrapped.detections, explicit.detections);
    assert_eq!(wrapped.observable_flips, explicit.observable_flips);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test data_path
```

Expected: FAIL with unresolved imports for `rstim::data_path` and missing `sample_batch_with_options` / `SampleOptions`.

**Step 3: Write minimal implementation**

```rust
// rstim/src/data_path.rs
use crate::ir::StimInstr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSampleMode {
    SimulateNoiseless,
    AssumeAllZero,
}

impl Default for ReferenceSampleMode {
    fn default() -> Self {
        Self::SimulateNoiseless
    }
}

pub fn build_reference_sample(
    instrs: &[StimInstr],
    mode: ReferenceSampleMode,
) -> Result<Vec<bool>, String> {
    match mode {
        ReferenceSampleMode::SimulateNoiseless => crate::executor::reference_sample(instrs),
        ReferenceSampleMode::AssumeAllZero => Ok(vec![false; crate::stats::num_measurements(instrs)]),
    }
}
```

```rust
// rstim/src/sampler.rs
use crate::data_path::{build_reference_sample, ReferenceSampleMode};

#[derive(Debug, Clone, Copy, Default)]
pub struct SampleOptions {
    pub reference_sample_mode: ReferenceSampleMode,
}

pub fn sample_batch_with_options(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let num_qubits = max_qubit(instrs)?;
    let mut frame = FrameSimulator::new(num_qubits, n_shots);
    frame.run(instrs, &ref_sample, rng)?;
    Ok(BatchOutput {
        measurements: frame.measurements(&ref_sample),
        detections: frame.detections(),
        observable_flips: frame.observable_flips(),
    })
}

pub fn sample_batch(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
) -> Result<BatchOutput, String> {
    sample_batch_with_options(instrs, n_shots, rng, SampleOptions::default())
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test data_path --test cli_sample
```

Expected: PASS. The new helper tests pass and existing `cli_sample` coverage still passes through the default wrapper.

**Step 5: Commit**

```bash
git add rstim/src/data_path.rs rstim/src/lib.rs rstim/src/sampler.rs rstim/tests/data_path.rs
git commit -m "refactor: add shared reference sample helpers"
```

### Task 2: `sample --skip_reference_sample`

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_sample.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn sample_skip_reference_sample_matches_default_on_zero_reference() {
    let circuit = "R 0\nM 0\n";
    let default_out = run_with_stdin(
        &["sample", "--shots", "5", "--seed", "42"],
        circuit,
    );
    let skipped_out = run_with_stdin(
        &["sample", "--shots", "5", "--seed", "42", "--skip_reference_sample"],
        circuit,
    );
    assert!(default_out.status.success());
    assert!(skipped_out.status.success());
    assert_eq!(default_out.stdout, skipped_out.stdout);
}
```

```rust
#[test]
fn run_sample_via_dispatch_accepts_skip_reference_sample() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    let out_path = dir.path().join("out.txt");
    std::fs::write(&circuit_path, "R 0\nM 0\n").unwrap();
    let cli = cli::Cli::parse_from([
        "rstim",
        "sample",
        "--shots",
        "1",
        "--skip_reference_sample",
        "--in",
        circuit_path.to_str().unwrap(),
        "--out",
        out_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    assert_eq!(std::fs::read_to_string(&out_path).unwrap().trim(), "0");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test cli_sample --test cli_coverage skip_reference_sample
```

Expected: FAIL because clap rejects `--skip_reference_sample` for `sample`.

**Step 3: Write minimal implementation**

```rust
// rstim/src/cli.rs
Sample {
    #[arg(long)]
    shots: Option<u64>,
    #[arg(long = "out_format", default_value = "01")]
    out_format: String,
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,
},
```

```rust
pub fn run_sample(
    circuit_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    skip_reference_sample: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let instrs = parse_lines(circuit_text)?;
    let mut rng = make_rng(seed);
    let options = crate::sampler::SampleOptions {
        reference_sample_mode: if skip_reference_sample {
            crate::data_path::ReferenceSampleMode::AssumeAllZero
        } else {
            crate::data_path::ReferenceSampleMode::SimulateNoiseless
        },
    };
    let result = sample_batch_with_options(&instrs, shots, &mut rng, options)?;
    match fmt {
        OutputFormat::Dets => Err("dets format not applicable to sample command; use detect".to_string()),
        _ => write_format(fmt, &result.measurements, out),
    }
}
```

Update the dispatch arm to pass `skip_reference_sample`, and update all direct `run_sample(...)` call sites in tests.

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test cli_sample --test cli_coverage
```

Expected: PASS. New `sample` flag coverage passes and pre-existing `sample` behavior remains unchanged.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_sample.rs rstim/tests/cli_coverage.rs
git commit -m "feat: add sample skip-reference mode"
```

### Task 3: `detect --obs_out` / `--obs_out_format`

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_detect.rs`
- Modify: `rstim/tests/cli_coverage.rs`
- Regression: `rstim/tests/cli_sample_dem.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn detect_obs_out_writes_observables_separately() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--obs_out", obs_path.to_str().unwrap()],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "1");
}

#[test]
fn detect_obs_out_and_append_observables_both_work() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &[
            "detect",
            "--shots",
            "1",
            "--append_observables",
            "--obs_out",
            obs_path.to_str().unwrap(),
        ],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "11");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "1");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test cli_detect detect_obs_out
```

Expected: FAIL because clap rejects `--obs_out` for `detect`.

**Step 3: Write minimal implementation**

Add the new detect flags:

```rust
Detect {
    #[arg(long)]
    shots: Option<u64>,
    #[arg(long = "out_format", default_value = "01")]
    out_format: String,
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long = "append_observables")]
    append_observables: bool,
    #[arg(long = "obs_out")]
    obs_out: Option<String>,
    #[arg(long = "obs_out_format", default_value = "01")]
    obs_out_format: String,
},
```

Add one shared writer helper and use it from both `detect` and `sample_dem`:

```rust
fn write_detection_outputs(
    detections: &BitTable,
    observable_flips: &BitTable,
    fmt: OutputFormat,
    append_observables: bool,
    out: &mut dyn Write,
    obs_out: Option<(&mut dyn Write, OutputFormat)>,
) -> Result<(), String> {
    match fmt {
        OutputFormat::Dets => write_shots_dets(detections, observable_flips, out)
            .map_err(|e| format!("write error: {e}"))?,
        _ if append_observables => {
            let merged = merge_detections_observables(detections, observable_flips);
            write_format(fmt, &merged, out)?;
        }
        _ => {
            write_format(fmt, detections, out)?;
        }
    }

    if let Some((obs_writer, obs_fmt)) = obs_out {
        write_format(obs_fmt, observable_flips, obs_writer)?;
    }
    Ok(())
}
```

Dispatch `detect` like `sample_dem`: open `obs_out` when present, pass `append_observables` through unchanged, and keep main-output behavior identical when `obs_out` is absent.

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test cli_detect --test cli_sample_dem --test cli_coverage
```

Expected: PASS. New `detect` side-output tests pass, and existing `sample_dem --obs_out` coverage still passes through the shared helper.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_detect.rs rstim/tests/cli_coverage.rs
git commit -m "feat: add detect observable side output"
```

### Task 4: `m2d` Options Surface and `--skip_reference_sample`

**Files:**
- Modify: `rstim/src/m2d.rs`
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/m2d.rs`
- Modify: `rstim/tests/stim_m2d.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write the failing test**

```rust
use rstim::data_path::ReferenceSampleMode;
use rstim::m2d::{measurements_to_detections, measurements_to_detections_with_options, M2dOptions};

#[test]
fn m2d_skip_reference_sample_matches_default_on_zero_reference() {
    let instrs = parse_lines("R 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let default_out = measurements_to_detections(&instrs, &meas).unwrap();
    let skipped_out = measurements_to_detections_with_options(
        &instrs,
        &meas,
        None,
        M2dOptions {
            reference_sample_mode: ReferenceSampleMode::AssumeAllZero,
            ran_without_feedback: false,
        },
    )
    .unwrap();
    assert_eq!(default_out.detections.get(0, 0), skipped_out.detections.get(0, 0));
}
```

```rust
#[test]
fn run_m2d_dispatch_accepts_skip_reference_sample() {
    use clap::Parser;
    let cli = rstim::cli::Cli::parse_from([
        "rstim",
        "m2d",
        "--circuit",
        "test.stim",
        "--in",
        "shots.01",
        "--skip_reference_sample",
    ]);
    assert!(matches!(cli.command, Some(rstim::cli::Commands::M2d { skip_reference_sample: true, .. })));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test m2d --test stim_m2d --test cli_coverage skip_reference_sample
```

Expected: FAIL because `M2dOptions` / `measurements_to_detections_with_options` do not exist and clap does not know the new `m2d` flag.

**Step 3: Write minimal implementation**

Add a library-level options type but keep the old wrapper:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct M2dOptions {
    pub reference_sample_mode: ReferenceSampleMode,
    pub ran_without_feedback: bool,
}

impl Default for M2dOptions {
    fn default() -> Self {
        Self {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            ran_without_feedback: false,
        }
    }
}

pub fn measurements_to_detections_with_options(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    options: M2dOptions,
) -> Result<M2dOutput, String> {
    let reference = match options.reference_sample_mode {
        ReferenceSampleMode::SimulateNoiseless => crate::data_path::build_reference_sample(
            instrs,
            ReferenceSampleMode::SimulateNoiseless,
        )?,
        ReferenceSampleMode::AssumeAllZero => vec![false; crate::stats::num_measurements(instrs)],
    };
    // Keep the existing detector/observable collection logic for now.
    // Task 5 and Task 6 will start using sweep_table and ran_without_feedback.
}

pub fn measurements_to_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<M2dOutput, String> {
    measurements_to_detections_with_options(instrs, meas_table, None, M2dOptions::default())
}
```

In `rstim/src/cli.rs`:

- Add `#[arg(long = "skip_reference_sample")] skip_reference_sample: bool` to `Commands::M2d`.
- Factor the repeated input-decoding logic into one private helper so Task 5 can reuse it:

```rust
fn read_table_from_format(
    data: &[u8],
    format: &str,
    bits: usize,
    shots: Option<usize>,
) -> Result<BitTable, String> {
    match format {
        "01" => read_shots_01(data, bits),
        "b8" => read_shots_b8(data, bits),
        "r8" => read_shots_r8(data, bits),
        "hits" => read_shots_hits(data, bits),
        "ptb64" => {
            let n = shots.ok_or("--shots required for ptb64 input")?;
            read_shots_ptb64(data, bits, n)
        }
        _ => Err(format!("unknown in_format: {format}")),
    }
}
```

- Build `M2dOptions` from `skip_reference_sample` inside `run_m2d`.

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test m2d --test stim_m2d --test cli_coverage
```

Expected: PASS. The new options path works, and the old `measurements_to_detections(...)` API still behaves exactly as before.

**Step 5: Commit**

```bash
git add rstim/src/m2d.rs rstim/src/cli.rs rstim/tests/m2d.rs rstim/tests/stim_m2d.rs rstim/tests/cli_coverage.rs
git commit -m "feat: add m2d reference sample options"
```

### Task 5: Sweep-Aware `m2d`

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/src/m2d.rs`
- Modify: `rstim/src/executor.rs`
- Modify: `rstim/src/stats.rs`
- Modify: `rstim/tests/m2d.rs`
- Modify: `rstim/tests/stim_m2d.rs`
- Modify: `rstim/tests/stats.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn num_sweep_bits_tracks_highest_index() {
    let instrs = parse_lines("CX sweep[3] 0\nCX sweep[1] 2\n").unwrap();
    assert_eq!(rstim::stats::num_sweep_bits(&instrs), 4);
}
```

```rust
#[test]
fn m2d_sweep_controlled_reference_is_evaluated_per_shot() {
    let instrs = parse_lines("R 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let mut meas = BitTable::new(1, 2);
    meas.set(0, 1, true);

    let mut sweep = BitTable::new(1, 2);
    sweep.set(0, 1, true);

    let out = measurements_to_detections_with_options(
        &instrs,
        &meas,
        Some(&sweep),
        M2dOptions::default(),
    )
    .unwrap();

    assert!(!out.detections.get(0, 0));
    assert!(!out.detections.get(0, 1));
}

#[test]
fn m2d_sweep_shot_count_mismatch_errors() {
    let instrs = parse_lines("R 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[false]);
    let sweep = BitTable::new(1, 2);
    let err = measurements_to_detections_with_options(
        &instrs,
        &meas,
        Some(&sweep),
        M2dOptions::default(),
    )
    .unwrap_err();
    assert!(err.contains("sweep shots"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test stats --test stim_m2d sweep
```

Expected: FAIL because `num_sweep_bits` does not exist and `measurements_to_detections_with_options(..., Some(&sweep), ...)` ignores sweep data.

**Step 3: Write minimal implementation**

Add sweep-width stats:

```rust
pub fn num_sweep_bits(instrs: &[StimInstr]) -> usize {
    let mut max_k: Option<u32> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    if let crate::ir::StimTarget::Sweep(k) = t {
                        max_k = Some(max_k.map_or(*k, |m| m.max(*k)));
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_sweep_bits(body);
                if inner > 0 {
                    max_k = Some(max_k.map_or((inner - 1) as u32, |m| m.max((inner - 1) as u32)));
                }
            }
        }
    }
    max_k.map_or(0, |m| (m + 1) as usize)
}
```

Add a sweep-aware reference sampler used only by `m2d`:

```rust
pub fn reference_sample_with_sweep_bits(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<Vec<bool>, String> {
    // Clone the existing reference_sample path.
    // Only add explicit support for CX/CY/CZ family ops whose first target is
    // sweep[k] and second target is a qubit.
    // Treat absent sweep_bits as all false.
}
```

Implement the supported cases in the reference path exactly like a classical one-bit control:

```rust
match pair {
    [StimTarget::Sweep(k), StimTarget::Qubit(q)] => {
        if sweep_bits.and_then(|bits| bits.get(*k as usize)).copied().unwrap_or(false) {
            match name {
                "CX" | "CNOT" | "ZCX" => state.x_gate(*q as usize),
                "CY" | "ZCY" => state.y_gate(*q as usize),
                "CZ" | "ZCZ" => state.z_gate(*q as usize),
                _ => return Err(format!("unsupported sweep-controlled op {name}")),
            }
        }
    }
    [StimTarget::Qubit(c), StimTarget::Qubit(t)] => { /* existing path */ }
    _ => return Err("unsupported sweep target placement".to_string()),
}
```

Then wire CLI parsing:

- Add `#[arg(long = "sweep")] sweep: Option<String>`
- Add `#[arg(long = "sweep_format", default_value = "01")] sweep_format: String`
- Decode sweep data with `read_table_from_format(...)` using `stats::num_sweep_bits(&instrs)`
- Validate `sweep_table.num_minor() == meas_table.num_minor()`

In `rstim/src/m2d.rs`, compute the reference per shot when sweep data is present:

```rust
for shot in 0..n_shots {
    let reference = match options.reference_sample_mode {
        ReferenceSampleMode::AssumeAllZero => vec![false; n_meas],
        ReferenceSampleMode::SimulateNoiseless => {
            if let Some(sweep_table) = sweep_table {
                let sweep_row: Vec<bool> = (0..sweep_table.num_major())
                    .map(|i| sweep_table.get(i, shot))
                    .collect();
                crate::executor::reference_sample_with_sweep_bits(instrs, Some(&sweep_row))?
            } else {
                crate::executor::reference_sample_with_sweep_bits(instrs, None)?
            }
        }
    };
    // Then compute this shot's detector/observable flips as usual.
}
```

Do not optimize yet. Correctness first; repeated per-shot reference generation is acceptable in this tranche.

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test stats --test m2d --test stim_m2d --test cli_coverage
```

Expected: PASS. Sweep-aware shot-count checks, CLI routing, and per-shot reference behavior all pass.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/src/m2d.rs rstim/src/executor.rs rstim/src/stats.rs rstim/tests/m2d.rs rstim/tests/stim_m2d.rs rstim/tests/stats.rs rstim/tests/cli_coverage.rs
git commit -m "feat: add sweep-aware m2d conversion"
```

### Task 6: `m2d --ran_without_feedback`

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/src/m2d.rs`
- Modify: `rstim/src/transforms.rs`
- Modify: `rstim/tests/m2d.rs`
- Modify: `rstim/tests/stim_m2d.rs`
- Modify: `rstim/tests/transforms.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn normalize_feedbackless_m2d_strips_simple_controlled_x_and_records_correction() {
    let instrs = parse_lines(
        "R 0 1\nX 0\nM 0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1] rec[-2]\n",
    )
    .unwrap();

    let normalized = rstim::transforms::normalize_feedbackless_m2d(&instrs).unwrap();
    assert_eq!(
        rstim::ir::circuit_to_string(&normalized.circuit),
        "R 0 1\nX 0\nM 0\nM 1\nDETECTOR rec[-1] rec[-2]\n",
    );
    assert_eq!(normalized.measurement_corrections, vec![vec![], vec![0]]);
}

#[test]
fn normalize_feedbackless_m2d_rejects_intervening_quantum_op() {
    let instrs = parse_lines("M 0\nCX rec[-1] 1\nH 1\nM 1\n").unwrap();
    let err = rstim::transforms::normalize_feedbackless_m2d(&instrs).unwrap_err();
    assert!(err.contains("unsupported feedback"));
}
```

```rust
#[test]
fn m2d_ran_without_feedback_compensates_simple_classical_x_feedback() {
    let instrs = parse_lines(
        "R 0 1\nX 0\nM 0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1] rec[-2]\n",
    )
    .unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections_with_options(
        &instrs,
        &meas,
        None,
        M2dOptions {
            ran_without_feedback: true,
            ..M2dOptions::default()
        },
    )
    .unwrap();
    assert!(!out.detections.get(0, 0));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test transforms --test stim_m2d ran_without_feedback
```

Expected: FAIL because the transform does not exist and `m2d` still errors on feedback-controlled gates.

**Step 3: Write minimal implementation**

Add one focused normalization type in `rstim/src/transforms.rs`:

```rust
pub struct FeedbacklessM2dNormalization {
    pub circuit: Vec<StimInstr>,
    pub measurement_corrections: Vec<Vec<usize>>,
}

pub fn normalize_feedbackless_m2d(
    instrs: &[StimInstr],
) -> Result<FeedbacklessM2dNormalization, String> {
    let flat = flattened(instrs);
    // Walk flat instructions left-to-right.
    // Strip supported feedback ops: CX/CY/CZ with Rec control and Qubit target.
    // Record pending X/Z frame corrections per qubit using earlier measurement indices.
    // When a measurement consumes a qubit, emit a correction list for that measurement index.
    // If any other non-annotation op touches a qubit with pending feedback, return
    // "unsupported feedback ..." instead of guessing.
}
```

Use this exact normalization rule set:

- `CX rec[-k] q` adds the referenced earlier measurement index to the target qubit's pending `x` frame.
- `CZ rec[-k] q` adds it to the pending `z` frame.
- `CY rec[-k] q` adds it to both.
- `M/MZ/MR/MRZ q` consumes `pending_x[q]`.
- `MX/MRX q` consumes `pending_z[q]`.
- `MY/MRY q` consumes `pending_x[q] XOR pending_z[q]`.
- Any other op touching `q` while either pending frame is non-empty returns an explicit unsupported-feedback error.
- The stripped circuit preserves measurement order, detector annotations, observable annotations, and resets; only the supported feedback ops are removed.

Then use the normalization inside `rstim/src/m2d.rs` before the shot loop:

```rust
let normalization = if options.ran_without_feedback {
    Some(crate::transforms::normalize_feedbackless_m2d(instrs)?)
} else {
    None
};

let work_instrs = normalization
    .as_ref()
    .map(|n| n.circuit.as_slice())
    .unwrap_or(instrs);

let measurement_corrections: &[Vec<usize>] = normalization
    .as_ref()
    .map(|n| n.measurement_corrections.as_slice())
    .unwrap_or(&[]);
```

Use `work_instrs` for reference-sample generation and detector/observable collection. Inside each shot, compute effective measurement flips by folding in earlier corrected flips:

```rust
let mut effective_flips = vec![false; n_meas];
for i in 0..n_meas {
    let mut flip = meas_table.get(i, shot) ^ reference[i];
    if let Some(extra_terms) = measurement_corrections.get(i) {
        for &j in extra_terms {
            flip ^= effective_flips[j];
        }
    }
    effective_flips[i] = flip;
}
```

Finally, add `#[arg(long = "ran_without_feedback")] ran_without_feedback: bool` to `Commands::M2d` and plumb it into `M2dOptions`.

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --test transforms --test m2d --test stim_m2d --test cli_coverage
```

Expected: PASS. Supported feedback cases normalize correctly, and unsupported intermediate quantum uses fail explicitly.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/src/m2d.rs rstim/src/transforms.rs rstim/tests/m2d.rs rstim/tests/stim_m2d.rs rstim/tests/transforms.rs rstim/tests/cli_coverage.rs
git commit -m "feat: normalize feedbackless m2d paths"
```

## Final Verification

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test --workspace
```

Expected: PASS across the entire workspace.

Run:

```bash
git status --short
```

Expected: empty output.
