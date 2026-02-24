# Phase 5: Detector Error Model (DEM) — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Detector Error Model (DEM) subsystem: data structures, file format I/O, circuit→DEM conversion via backward error analysis, and DEM sampling.

**Architecture:** The DEM is a list of independent error mechanisms, each with a probability, symptoms (detectors), and frame changes (observables). The `ErrorAnalyzer` converts a circuit to a DEM by iterating *backwards* through the circuit, tracking per-qubit X/Z sensitivity sets (which detectors/observables are currently sensitive to X or Z errors on each qubit). Gates transform sensitivities via reverse conjugation. Measurements transfer sensitivities from qubits to the measurement record. When a noise channel is encountered, the current sensitivities determine which detectors/observables it can flip, producing a DEM `error` instruction. The DEM sampler independently flips each error mechanism with its probability, XORing the affected detectors/observables.

**Tech Stack:** Rust, `rand` crate, `cargo test`

---

## Background: The ErrorAnalyzer Algorithm

The ErrorAnalyzer processes a circuit **in reverse** (last instruction first). It maintains:

1. **Per-qubit X and Z sensitivity sets** (`SparseXorVec<DemTarget>`): For each qubit, which detectors/observables are currently sensitive to an X or Z error on that qubit. Y sensitivity is implicit (XOR of X and Z sets).

2. **Measurement record stack**: Maps measurement indices to their associated sensitivities. When DETECTOR/OBSERVABLE_INCLUDE instructions are encountered (before the measurements they reference), their targets are noted. When the corresponding measurements are reached, those detector/observable targets become part of the qubit's sensitivity set.

### Key operations (all in reverse):

- **DETECTOR/OBSERVABLE_INCLUDE**: Look up measurement record offsets. For each referenced measurement, XOR the detector/observable target into that measurement's pending sensitivity.
- **M/MZ (undo)**: The Z measurement's X sensitivity becomes the error sensitivity. Transfer pending measurement sensitivities into the qubit's X sensitivity. Clear the qubit (reset in reverse = randomize = clear sensitivities).
- **MX (undo)**: Same but operates on Z sensitivity (X errors don't flip X measurements; Z errors do).
- **MY (undo)**: Both X and Z sensitivities are affected.
- **MR/MRZ (undo)**: Like MZ but the reset in forward = in reverse the qubit sensitivities are cleared before the measurement transfer.
- **R/RZ (undo)**: Check that the qubit's X sensitivity is empty (determinism check), then clear both X and Z.
- **Gates (undo)**: Transform sensitivities by the *inverse* conjugation. For Clifford C applied forward, the reverse conjugation of Pauli P is C†PC. Examples:
  - H: swap X↔Z sensitivities
  - S (undo): X sensitivity gets XORed with Z sensitivity (z_sens ^= x_sens)
  - CX(c,t) (undo): x_sens[c] ^= x_sens[t]; z_sens[t] ^= z_sens[c]
  - CZ(a,b) (undo): z_sens[a] ^= x_sens[b]; z_sens[b] ^= x_sens[a]
  - SWAP: swap both X and Z sensitivities
- **Noise channels (undo)**: Read current sensitivities to determine the DEM error. For example:
  - X_ERROR(p) on qubit q: the X sensitivity of q tells which detectors/observables are flipped. Record `error(p) <those targets>`.
  - Z_ERROR(p) on qubit q: use Z sensitivity.
  - Y_ERROR(p) on qubit q: use XOR of X and Z sensitivities.
  - DEPOLARIZE1(p) on qubit q: record up to 3 error channels (X, Y, Z) each with probability p/3.
  - CORRELATED_ERROR(p) targets: combine sensitivities of all Pauli targets.

---

## Task 1: DEM IR and Data Structures

**Files:**
- Create: `src/dem.rs`
- Modify: `src/lib.rs` (add `pub mod dem;`)
- Test: `tests/dem_ir.rs`

### Step 1: Write the failing test

Create `tests/dem_ir.rs`:

```rust
use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

#[test]
fn dem_empty() {
    let dem = DetectorErrorModel::new();
    assert_eq!(dem.instructions().len(), 0);
    assert_eq!(dem.num_detectors(), 0);
    assert_eq!(dem.num_observables(), 0);
}

#[test]
fn dem_error_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.1, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    assert_eq!(dem.instructions().len(), 1);
    assert_eq!(dem.num_detectors(), 2);
    match &dem.instructions()[0] {
        DemInstruction::Error { probability, targets, .. } => {
            assert_eq!(*probability, 0.1);
            assert_eq!(targets.len(), 2);
        }
        _ => panic!("expected error instruction"),
    }
}

#[test]
fn dem_detector_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(5, vec![1.0, 2.5]);
    assert_eq!(dem.num_detectors(), 6);
    match &dem.instructions()[0] {
        DemInstruction::Detector { index, coords } => {
            assert_eq!(*index, 5);
            assert_eq!(coords, &vec![1.0, 2.5]);
        }
        _ => panic!("expected detector instruction"),
    }
}

#[test]
fn dem_observable_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_observable(2);
    assert_eq!(dem.num_observables(), 3);
}

#[test]
fn dem_error_with_observable() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.01, vec![
        DemTarget::Detector(0),
        DemTarget::Observable(0),
    ]);
    assert_eq!(dem.num_detectors(), 1);
    assert_eq!(dem.num_observables(), 1);
}

#[test]
fn dem_repeat_block() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    body.add_shift_detectors(1, vec![0.0, 1.0]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(10, body);
    assert_eq!(dem.instructions().len(), 1);
}

#[test]
fn dem_shift_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_shift_detectors(3, vec![0.0, 0.5]);
    match &dem.instructions()[0] {
        DemInstruction::ShiftDetectors { detector_offset, coord_offsets } => {
            assert_eq!(*detector_offset, 3);
            assert_eq!(coord_offsets, &vec![0.0, 0.5]);
        }
        _ => panic!("expected shift_detectors"),
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test dem_ir`
Expected: FAIL — module `dem` not found

### Step 3: Write minimal implementation

Create `src/dem.rs`:

```rust
/// Targets that can appear in DEM error instructions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DemTarget {
    /// Relative detector index (D#)
    Detector(usize),
    /// Logical observable index (L#)
    Observable(usize),
    /// Component separator (^) for decomposition hints
    Separator,
}

/// A single instruction in a detector error model.
#[derive(Debug, Clone, PartialEq)]
pub enum DemInstruction {
    /// An error mechanism: probability + affected detectors/observables
    Error {
        probability: f64,
        targets: Vec<DemTarget>,
    },
    /// Declares a detector with optional coordinates
    Detector {
        index: usize,
        coords: Vec<f64>,
    },
    /// Declares a logical observable
    LogicalObservable {
        index: usize,
    },
    /// Shifts the detector index offset and coordinate offsets
    ShiftDetectors {
        detector_offset: usize,
        coord_offsets: Vec<f64>,
    },
    /// Repeated block
    Repeat {
        count: u64,
        body: DetectorErrorModel,
    },
}

/// A detector error model: a list of DEM instructions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetectorErrorModel {
    instrs: Vec<DemInstruction>,
    num_detectors: usize,
    num_observables: usize,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instructions(&self) -> &[DemInstruction] {
        &self.instrs
    }

    pub fn num_detectors(&self) -> usize {
        self.num_detectors
    }

    pub fn num_observables(&self) -> usize {
        self.num_observables
    }

    pub fn add_error(&mut self, probability: f64, targets: Vec<DemTarget>) {
        for t in &targets {
            match t {
                DemTarget::Detector(d) => {
                    self.num_detectors = self.num_detectors.max(*d + 1);
                }
                DemTarget::Observable(o) => {
                    self.num_observables = self.num_observables.max(*o + 1);
                }
                DemTarget::Separator => {}
            }
        }
        self.instrs.push(DemInstruction::Error { probability, targets });
    }

    pub fn add_detector(&mut self, index: usize, coords: Vec<f64>) {
        self.num_detectors = self.num_detectors.max(index + 1);
        self.instrs.push(DemInstruction::Detector { index, coords });
    }

    pub fn add_observable(&mut self, index: usize) {
        self.num_observables = self.num_observables.max(index + 1);
        self.instrs.push(DemInstruction::LogicalObservable { index });
    }

    pub fn add_shift_detectors(&mut self, detector_offset: usize, coord_offsets: Vec<f64>) {
        self.instrs.push(DemInstruction::ShiftDetectors {
            detector_offset,
            coord_offsets,
        });
    }

    pub fn add_repeat(&mut self, count: u64, body: DetectorErrorModel) {
        self.instrs.push(DemInstruction::Repeat { count, body });
    }

    pub fn push(&mut self, instr: DemInstruction) {
        match &instr {
            DemInstruction::Error { targets, .. } => {
                for t in targets {
                    match t {
                        DemTarget::Detector(d) => self.num_detectors = self.num_detectors.max(*d + 1),
                        DemTarget::Observable(o) => self.num_observables = self.num_observables.max(*o + 1),
                        DemTarget::Separator => {}
                    }
                }
            }
            DemInstruction::Detector { index, .. } => {
                self.num_detectors = self.num_detectors.max(*index + 1);
            }
            DemInstruction::LogicalObservable { index } => {
                self.num_observables = self.num_observables.max(*index + 1);
            }
            _ => {}
        }
        self.instrs.push(instr);
    }
}
```

Add to `src/lib.rs`:

```rust
pub mod dem;
```

### Step 4: Run test to verify it passes

Run: `cargo test --test dem_ir`
Expected: PASS

### Step 5: Commit

```bash
git add src/dem.rs src/lib.rs tests/dem_ir.rs
git commit -m "feat: DEM IR data structures (DemTarget, DemInstruction, DetectorErrorModel)"
```

---

## Task 2: DEM File Format — Writer and Parser

**Files:**
- Modify: `src/dem.rs` (add `write` and `parse` methods)
- Test: `tests/dem_format.rs`

### Step 1: Write the failing test

Create `tests/dem_format.rs`:

```rust
use rstim::dem::{DemTarget, DetectorErrorModel};

#[test]
fn dem_write_empty() {
    let dem = DetectorErrorModel::new();
    assert_eq!(dem.to_string(), "");
}

#[test]
fn dem_write_simple_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.1, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    assert_eq!(dem.to_string(), "error(0.1) D0 D1\n");
}

#[test]
fn dem_write_error_with_observable() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.01, vec![
        DemTarget::Detector(2), DemTarget::Detector(3), DemTarget::Observable(0)
    ]);
    assert_eq!(dem.to_string(), "error(0.01) D2 D3 L0\n");
}

#[test]
fn dem_write_error_with_separator() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.02, vec![
        DemTarget::Detector(2), DemTarget::Observable(0),
        DemTarget::Separator,
        DemTarget::Detector(5), DemTarget::Detector(6),
    ]);
    assert_eq!(dem.to_string(), "error(0.02) D2 L0 ^ D5 D6\n");
}

#[test]
fn dem_write_detector_with_coords() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(4, vec![2.5, 3.5, 6.0]);
    assert_eq!(dem.to_string(), "detector(2.5, 3.5, 6) D4\n");
}

#[test]
fn dem_write_shift_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_shift_detectors(2, vec![0.0, 0.5]);
    assert_eq!(dem.to_string(), "shift_detectors(0, 0.5) 2\n");
}

#[test]
fn dem_write_repeat() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(10, body);
    let expected = "repeat 10 {\n    error(0.01) D0 D1\n    shift_detectors 1\n}\n";
    assert_eq!(dem.to_string(), expected);
}

#[test]
fn dem_parse_simple() {
    let input = "error(0.1) D0 D1\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 1);
    assert_eq!(dem.num_detectors(), 2);
}

#[test]
fn dem_parse_round_trip() {
    let input = "\
error(0.1) D0 D1
error(0.01) D2 D3 L0
detector(1, 2) D0
shift_detectors(0, 0.5) 2
repeat 5 {
    error(0.05) D0 D1
    shift_detectors 1
}
";
    let dem = DetectorErrorModel::parse(input).unwrap();
    let output = dem.to_string();
    let dem2 = DetectorErrorModel::parse(&output).unwrap();
    assert_eq!(dem.instructions().len(), dem2.instructions().len());
}

#[test]
fn dem_parse_comments_and_blank_lines() {
    let input = "# This is a comment\n\nerror(0.1) D0\n# Another comment\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 1);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test dem_format`
Expected: FAIL — `to_string` / `parse` not found

### Step 3: Write minimal implementation

Add to `src/dem.rs` — a `Display` impl for writing and a `parse` classmethod for parsing:

**Writer** (`impl std::fmt::Display for DetectorErrorModel`):
- `error(prob) D# D# L#` for Error instructions
- `detector(coords...) D#` for Detector instructions
- `logical_observable L#` for LogicalObservable instructions
- `shift_detectors(coord_offsets...) detector_offset` for ShiftDetectors
- `repeat N {\n    <body indented>\n}` for Repeat blocks

**Parser** (`DetectorErrorModel::parse(input: &str)`):
- Line-by-line parsing, skip comments (`#`) and blank lines
- Parse instruction name, optional args in `()`, targets
- Targets: `D#` → Detector, `L#` → Observable, `^` → Separator, bare `#` → numeric
- Handle `repeat N {` ... `}` blocks recursively

Format floats cleanly: strip trailing zeros, use integer format when possible.

### Step 4: Run test to verify it passes

Run: `cargo test --test dem_format`
Expected: PASS

### Step 5: Commit

```bash
git add src/dem.rs tests/dem_format.rs
git commit -m "feat: DEM file format writer and parser"
```

---

## Task 3: ErrorAnalyzer — Core Backward Propagation

**Files:**
- Create: `src/error_analyzer.rs`
- Modify: `src/lib.rs` (add `pub mod error_analyzer;`)
- Test: `tests/error_analyzer.rs`

This is the most complex task. The ErrorAnalyzer iterates backwards through the circuit, tracking per-qubit X/Z sensitivity sets. Each sensitivity set is a `Vec<DemTarget>` stored as a sorted, deduplicated list (XOR semantics: adding a target that already exists removes it).

### Data Structures

```rust
use crate::dem::{DemTarget, DetectorErrorModel};
use crate::ir::{StimInstr, StimTarget, PauliBasis};

/// Sparse XOR vector: sorted set with XOR (toggle) semantics.
/// Adding an element that exists removes it; adding a new element inserts it.
#[derive(Debug, Clone, Default)]
struct SparseXorVec {
    targets: Vec<DemTarget>,
}

impl SparseXorVec {
    fn xor_item(&mut self, item: DemTarget) { /* toggle item in sorted vec */ }
    fn xor_other(&mut self, other: &SparseXorVec) { /* XOR all items from other */ }
    fn is_empty(&self) -> bool { self.targets.is_empty() }
    fn clear(&mut self) { self.targets.clear(); }
    fn sorted_targets(&self) -> &[DemTarget] { &self.targets }
}

pub struct ErrorAnalyzer {
    /// Per-qubit X error sensitivity: which detectors/observables are triggered by X error on qubit q
    x_sens: Vec<SparseXorVec>,
    /// Per-qubit Z error sensitivity
    z_sens: Vec<SparseXorVec>,
    /// Measurement record: for each measurement (indexed from 0), which sensitivities are pending
    measurement_sens: Vec<SparseXorVec>,
    /// Total measurement count (decremented as we go backward)
    num_measurements: usize,
    /// Output DEM (built in reverse, then reversed at the end)
    output: DetectorErrorModel,
}
```

### Key Methods

```rust
impl ErrorAnalyzer {
    /// Convert a circuit to a DEM.
    pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String>;

    /// Process instructions in reverse order.
    fn undo_circuit(&mut self, instrs: &[StimInstr]) -> Result<(), String>;

    /// Process a single op in reverse.
    fn undo_op(&mut self, name: &str, args: &[f64], targets: &[StimTarget]) -> Result<(), String>;

    // --- Annotation undo ---
    fn undo_detector(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_observable_include(&mut self, args: &[f64], targets: &[StimTarget]);

    // --- Measurement undo (key operation) ---
    fn undo_mz(&mut self, targets: &[StimTarget]);   // Transfer x_sens to measurement, clear qubit
    fn undo_mx(&mut self, targets: &[StimTarget]);   // Transfer z_sens to measurement, clear qubit
    fn undo_my(&mut self, targets: &[StimTarget]);   // Transfer x_sens XOR z_sens, clear qubit
    fn undo_mrz(&mut self, targets: &[StimTarget]);  // Reset first (clear sens), then undo_mz
    fn undo_mrx(&mut self, targets: &[StimTarget]);
    fn undo_mry(&mut self, targets: &[StimTarget]);

    // --- Reset undo ---
    fn undo_rz(&mut self, targets: &[StimTarget]);   // Check x_sens empty, clear both
    fn undo_rx(&mut self, targets: &[StimTarget]);   // Check z_sens empty, clear both
    fn undo_ry(&mut self, targets: &[StimTarget]);   // Check x_sens == z_sens, clear both

    // --- Gate undo (transform sensitivities) ---
    fn undo_h(&mut self, targets: &[StimTarget]);    // Swap x_sens[q] ↔ z_sens[q]
    fn undo_s(&mut self, targets: &[StimTarget]);    // x_sens[q] ^= z_sens[q]
    fn undo_s_dag(&mut self, targets: &[StimTarget]);// Same as undo_s in sensitivity picture
    fn undo_sqrt_x(&mut self, targets: &[StimTarget]); // z_sens[q] ^= x_sens[q]
    fn undo_cx(&mut self, targets: &[StimTarget]);   // x_sens[t] ^= x_sens[c]; z_sens[c] ^= z_sens[t]
    fn undo_cz(&mut self, targets: &[StimTarget]);   // z_sens[a] ^= x_sens[b]; z_sens[b] ^= x_sens[a]
    fn undo_swap(&mut self, targets: &[StimTarget]); // Swap x_sens and z_sens for both qubits
    // ... (other gates follow the same pattern as frame sim, transforming sensitivities)

    // --- Noise undo (record errors) ---
    fn undo_x_error(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_z_error(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_y_error(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_depolarize1(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_depolarize2(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_correlated_error(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_else_correlated_error(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_pauli_channel_1(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_pauli_channel_2(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_heralded_erase(&mut self, args: &[f64], targets: &[StimTarget]);
    fn undo_heralded_pauli_channel_1(&mut self, args: &[f64], targets: &[StimTarget]);
}
```

### Measurement undo detail (for M/MZ)

Processing M/MZ in reverse for each qubit q:
1. Decrement `self.num_measurements` to get measurement index `m_idx`
2. The measurement sensitivity `measurement_sens[m_idx]` contains the detectors/observables waiting for this measurement
3. XOR `measurement_sens[m_idx]` into `x_sens[q]` (because Z-basis measurement is sensitive to X errors)
4. The current `x_sens[q]` now tells us the full error sensitivity at this measurement point
5. Clear `x_sens[q]` and `z_sens[q]` (the measurement+reset destroys prior state)

Note: For M (without reset), we only clear x_sens[q] and randomize z_sens[q]. But in error analysis, the reset distinction doesn't matter since we're tracking sensitivities, not state.

### Noise undo detail (for X_ERROR)

Processing X_ERROR(p) in reverse for each qubit q:
1. Read `x_sens[q].sorted_targets()` — these are the detectors/observables flipped by an X error
2. If non-empty, add `error(p) <targets>` to the output DEM

### Step 1: Write the failing test

Create `tests/error_analyzer.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::dem::DemTarget;

#[test]
fn analyze_x_error_single_detector() {
    // X_ERROR before Z measurement, detected by a detector
    let instrs = parse_lines("X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert_eq!(dem.instructions().len(), 1);
    // error(0.1) D0
    let instr = &dem.instructions()[0];
    match instr {
        rstim::dem::DemInstruction::Error { probability, targets } => {
            assert!((probability - 0.1).abs() < 1e-10);
            assert_eq!(targets, &vec![DemTarget::Detector(0)]);
        }
        _ => panic!("expected error"),
    }
}

#[test]
fn analyze_z_error_invisible_to_z_measurement() {
    // Z_ERROR before Z measurement: no effect on detectors
    let instrs = parse_lines("Z_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert_eq!(dem.instructions().len(), 0);
}

#[test]
fn analyze_z_error_visible_to_x_measurement() {
    let instrs = parse_lines("Z_ERROR(0.1) 0\nMX 0\nDETECTOR rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert_eq!(dem.instructions().len(), 1);
}

#[test]
fn analyze_two_detector_error() {
    // X_ERROR between two Z measurements detected by separate detectors
    let instrs = parse_lines("M 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // The X error flips the second measurement but not the first (which already happened)
    // So only D0 (rec[-1]) is flipped
    assert_eq!(dem.instructions().len(), 1);
}

#[test]
fn analyze_repetition_code_detector_pair() {
    let circuit = "\
R 0 1 2
X_ERROR(0.01) 0
M 0 1 2
DETECTOR rec[-3] rec[-2]
DETECTOR rec[-2] rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // X_ERROR on qubit 0 flips M[0], which appears in DETECTOR rec[-3] rec[-2]
    // That detector XORs rec[-3](=M0) and rec[-2](=M1)
    // X error on q0 flips M0, so D0 fires
    assert!(dem.instructions().len() >= 1);
}

#[test]
fn analyze_observable_include() {
    let instrs = parse_lines("X_ERROR(0.1) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert_eq!(dem.instructions().len(), 1);
    match &dem.instructions()[0] {
        rstim::dem::DemInstruction::Error { targets, .. } => {
            assert!(targets.contains(&DemTarget::Observable(0)));
        }
        _ => panic!("expected error"),
    }
}

#[test]
fn analyze_noiseless_circuit_empty_dem() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert_eq!(dem.instructions().len(), 0);
}

#[test]
fn analyze_depolarize1() {
    // DEPOLARIZE1 produces up to 3 error channels
    let instrs = parse_lines("DEPOLARIZE1(0.03) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // X and Y errors flip Z measurement → both produce error(0.01) D0
    // These get combined: p_combined = p1*(1-p2) + p2*(1-p1)
    // Or kept as separate channels: error(0.01) D0 from X, error(0.01) D0 from Y
    // Simplest: merge identical targets → error(~0.02) D0 (approximately)
    assert!(dem.instructions().len() >= 1);
}

#[test]
fn analyze_error_propagates_through_cnot() {
    // X_ERROR on control of CNOT propagates to target too
    let circuit = "\
X_ERROR(0.1) 0
CNOT 0 1
M 0 1
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // X error on q0, then CNOT(0,1): X propagates from control to target
    // So both M0 and M1 are flipped → both D0 and D1 fire
    assert_eq!(dem.instructions().len(), 1);
    match &dem.instructions()[0] {
        rstim::dem::DemInstruction::Error { targets, .. } => {
            assert!(targets.contains(&DemTarget::Detector(0)));
            assert!(targets.contains(&DemTarget::Detector(1)));
        }
        _ => panic!("expected error"),
    }
}

#[test]
fn analyze_correlated_error() {
    let instrs = parse_lines("CORRELATED_ERROR(0.1) X0 Z1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // X on q0 flips M0 (=rec[-2]) → D0. Z on q1 doesn't flip M1.
    assert_eq!(dem.instructions().len(), 1);
    match &dem.instructions()[0] {
        rstim::dem::DemInstruction::Error { targets, .. } => {
            assert_eq!(targets, &vec![DemTarget::Detector(0)]);
        }
        _ => panic!("expected error"),
    }
}

#[test]
fn analyze_repeat_block() {
    let circuit = "\
M 0
REPEAT 3 {
    X_ERROR(0.01) 0
    M 0
    DETECTOR rec[-1] rec[-2]
}
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // Each round produces error(0.01) D_i
    assert!(dem.instructions().len() >= 3);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test error_analyzer`
Expected: FAIL — module not found

### Step 3: Write minimal implementation

Create `src/error_analyzer.rs` implementing the `ErrorAnalyzer` struct with:

1. **Initialization**: Count measurements and qubits. Allocate per-qubit sensitivity vectors and measurement sensitivity vector.
2. **`undo_circuit`**: Iterate instructions in reverse. For `Repeat`, iterate the body `count` times in reverse.
3. **`undo_op`**: Dispatch to specific handlers based on instruction name.
4. **Gate handlers**: Transform x_sens/z_sens using the *reverse* of the frame sim rules. The key insight: the error analyzer sensitivity transformations are the *same* as the frame sim's frame transformations, because both track how Pauli errors propagate through Clifford gates.
5. **Measurement handlers**: Transfer measurement sensitivities to qubit sensitivities.
6. **Noise handlers**: Read current sensitivities and emit DEM error instructions.
7. **Error merging**: When the same set of targets appears with different probabilities, merge using `p_combined = p1 + p2 - 2*p1*p2`.

Add `pub mod error_analyzer;` to `src/lib.rs`.

### Step 4: Run test to verify it passes

Run: `cargo test --test error_analyzer`
Expected: PASS

### Step 5: Commit

```bash
git add src/error_analyzer.rs src/lib.rs tests/error_analyzer.rs
git commit -m "feat: ErrorAnalyzer — circuit to DEM conversion via backward propagation"
```

---

## Task 4: DEM Sampler

**Files:**
- Modify: `src/dem.rs` (add `sample` method)
- Test: `tests/dem_sampler.rs`

### Step 1: Write the failing test

Create `tests/dem_sampler.rs`:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::dem::{DemTarget, DetectorErrorModel};

#[test]
fn dem_sample_deterministic_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0), DemTarget::Observable(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, obs) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true]);
    assert_eq!(obs, vec![true]);
}

#[test]
fn dem_sample_no_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.0, vec![DemTarget::Detector(0)]);
    dem.add_detector(0, vec![]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, obs) = dem.sample(&mut rng);
    assert_eq!(dets, vec![false]);
    assert_eq!(obs, vec![]);
}

#[test]
fn dem_sample_statistical() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.5, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut count = 0;
    for _ in 0..10000 {
        let (dets, _) = dem.sample(&mut rng);
        if dets[0] { count += 1; }
    }
    assert!((count as f64 / 10000.0 - 0.5).abs() < 0.05);
}

#[test]
fn dem_sample_two_errors_xor() {
    // Two independent errors both flipping D0 → XOR semantics
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    // Both fire → XOR → false
    assert_eq!(dets, vec![false]);
}

#[test]
fn dem_sample_with_repeat() {
    let mut body = DetectorErrorModel::new();
    body.add_error(1.0, vec![DemTarget::Detector(0)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(3, body);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    // 3 detectors, each fired once
    assert_eq!(dets, vec![true, true, true]);
}

#[test]
fn dem_sample_batch() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.5, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let results = dem.sample_batch(1000, &mut rng);
    assert_eq!(results.detections.num_major(), 1); // 1 detector
    assert_eq!(results.detections.num_minor(), 1000); // 1000 shots
    let count: usize = (0..1000).filter(|&s| results.detections.get(0, s)).count();
    assert!((count as f64 / 1000.0 - 0.5).abs() < 0.05);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test dem_sampler`
Expected: FAIL

### Step 3: Write minimal implementation

Add to `src/dem.rs`:

**`sample` method**: Single-shot DEM sampling.
1. Initialize detector and observable boolean vectors (all false).
2. Track `detector_offset` (incremented by `shift_detectors`).
3. For each instruction:
   - `Error { probability, targets }`: flip a coin. If heads, toggle each D#/L# target.
   - `ShiftDetectors { detector_offset: d, .. }`: add `d` to current offset.
   - `Repeat { count, body }`: iterate `count` times, processing body instructions.
   - `Detector`/`LogicalObservable`: ensure vectors are large enough.
4. Return `(Vec<bool>, Vec<bool>)` for detectors and observables.

**`sample_batch` method**: Batch DEM sampling using BitTable.
1. Similar logic but bit-packed across shots using `BitTable`.
2. For each error, generate random bits with the error probability, then XOR into the appropriate detector/observable rows.
3. Return a `DemBatchOutput { detections: BitTable, observable_flips: BitTable }`.

The batch method uses the same `random_bits_with_prob` pattern from the frame simulator.

```rust
pub struct DemBatchOutput {
    pub detections: BitTable,
    pub observable_flips: BitTable,
}
```

### Step 4: Run test to verify it passes

Run: `cargo test --test dem_sampler`
Expected: PASS

### Step 5: Commit

```bash
git add src/dem.rs tests/dem_sampler.rs
git commit -m "feat: DEM sampler (single-shot and batch)"
```

---

## Task 5: Integration — Circuit→DEM→Sample Pipeline and Cross-Validation

**Files:**
- Test: `tests/dem_integration.rs`

### Step 1: Write the test

Create `tests/dem_integration.rs`:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::sampler::sample_batch;

/// Cross-validate: circuit sampling and DEM sampling should produce
/// statistically similar detection event rates.
#[test]
fn circuit_vs_dem_detection_rates() {
    let circuit = "\
R 0 1
X_ERROR(0.05) 0
M 0 1
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();

    // Circuit-based sampling
    let mut rng = StdRng::seed_from_u64(42);
    let circuit_out = sample_batch(&instrs, 10000, &mut rng).unwrap();
    let circuit_d0: usize = (0..10000).filter(|&s| circuit_out.detections.get(0, s)).count();
    let circuit_d1: usize = (0..10000).filter(|&s| circuit_out.detections.get(1, s)).count();

    // DEM-based sampling
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let mut rng2 = StdRng::seed_from_u64(99);
    let dem_out = dem.sample_batch(10000, &mut rng2);
    let dem_d0: usize = (0..10000).filter(|&s| dem_out.detections.get(0, s)).count();
    let dem_d1: usize = (0..10000).filter(|&s| dem_out.detections.get(1, s)).count();

    // D0 should fire ~5% of the time, D1 should be ~0%
    let circuit_rate_0 = circuit_d0 as f64 / 10000.0;
    let dem_rate_0 = dem_d0 as f64 / 10000.0;
    assert!((circuit_rate_0 - dem_rate_0).abs() < 0.03,
        "circuit={circuit_rate_0}, dem={dem_rate_0}");
    assert!((circuit_d1 as f64 / 10000.0) < 0.01);
    assert!((dem_d1 as f64 / 10000.0) < 0.01);
}

#[test]
fn circuit_to_dem_to_string_round_trip() {
    let circuit = "\
X_ERROR(0.1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
    assert!(dem_str.contains("D0"));
    assert!(dem_str.contains("L0"));

    // Parse it back
    let dem2 = rstim::dem::DetectorErrorModel::parse(&dem_str).unwrap();
    assert_eq!(dem.instructions().len(), dem2.instructions().len());
}

#[test]
fn repetition_code_circuit_to_dem() {
    let circuit = "\
R 0 1 2 3
TICK
CNOT 0 1
CNOT 2 1
CNOT 2 3
TICK
M 1 3
DETECTOR rec[-2]
DETECTOR rec[-1]
REPEAT 2 {
    R 1 3
    TICK
    X_ERROR(0.01) 0 2
    CNOT 0 1
    CNOT 2 1
    CNOT 2 3
    TICK
    M 1 3
    DETECTOR rec[-2] rec[-4]
    DETECTOR rec[-1] rec[-3]
}
M 0 2
OBSERVABLE_INCLUDE(0) rec[-1] rec[-2]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // Should produce error instructions involving detectors and possibly L0
    assert!(dem.instructions().len() > 0);
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test dem_integration`
Expected: FAIL (depends on Tasks 1-4 being complete)

### Step 3: Fix any issues found during integration testing

This step is about making sure the full pipeline works end-to-end. Debug any failures by:
1. Checking the DEM output for known circuits against expected results
2. Verifying detection event rates match between circuit sampling and DEM sampling
3. Ensuring the DEM format round-trips correctly

### Step 4: Run test to verify it passes

Run: `cargo test`
Expected: ALL PASS

### Step 5: Commit

```bash
git add tests/dem_integration.rs
git commit -m "test: integration tests for circuit→DEM→sample pipeline"
```

---

## Summary

| Task | Component | Key Files | Complexity |
|------|-----------|-----------|------------|
| 1 | DEM IR | `src/dem.rs` | Low |
| 2 | DEM Format I/O | `src/dem.rs` | Medium |
| 3 | ErrorAnalyzer | `src/error_analyzer.rs` | High |
| 4 | DEM Sampler | `src/dem.rs` | Medium |
| 5 | Integration Tests | `tests/dem_integration.rs` | Medium |

The ErrorAnalyzer (Task 3) is the most complex. Its gate transformations mirror the frame simulator's, but operate on sparse sensitivity sets instead of bit-packed frames. The key difference: the error analyzer processes the circuit *backwards* and tracks *which detectors* are affected, while the frame simulator processes *forwards* and tracks *which shots* have errors.
