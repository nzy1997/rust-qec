# Phase 7: Polish and Extended Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Round out rstim with circuit transforms (`flattened`, `inverse`, `without_noise`, `without_tags`), a circuit statistics API, remaining exotic Clifford gates (C_XYZ family, negated Hadamard variants), and a circuit generation command (`rstim gen`) for producing annotated QEC circuits.

**Architecture:** Circuit transforms and statistics live in new library modules (`src/transforms.rs`, `src/stats.rs`) operating on `Vec<StimInstr>`. Exotic gates are added to the existing simulator pipelines (tableau, frame, error analyzer). Circuit generation produces `Vec<StimInstr>` programmatically. The CLI gains a `gen` subcommand.

**Tech Stack:** Rust, existing `rstim` library modules, `clap` for CLI

---

## Task 1: Circuit Statistics API

**Files:**
- Create: `src/stats.rs`
- Modify: `src/lib.rs` (add `pub mod stats;`)
- Test: `tests/stats.rs`

### Step 1: Write the failing test

Create `tests/stats.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::stats;

#[test]
fn num_qubits_simple() {
    let instrs = parse_lines("H 0\nCX 0 3\nM 3").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 4); // 0..=3
}

#[test]
fn num_qubits_empty() {
    let instrs = parse_lines("").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 0);
}

#[test]
fn num_measurements_simple() {
    let instrs = parse_lines("M 0 1 2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}

#[test]
fn num_measurements_with_repeat() {
    let instrs = parse_lines("REPEAT 10 {\n  M 0 1\n}").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 20);
}

#[test]
fn num_measurements_mpp() {
    let instrs = parse_lines("MPP X0*Y1 Z2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 2);
}

#[test]
fn num_detectors_simple() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]").unwrap();
    assert_eq!(stats::num_detectors(&instrs), 2);
}

#[test]
fn num_detectors_with_repeat() {
    let instrs = parse_lines("REPEAT 5 {\n  M 0\n  DETECTOR rec[-1]\n}").unwrap();
    assert_eq!(stats::num_detectors(&instrs), 5);
}

#[test]
fn num_observables() {
    let instrs = parse_lines("M 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-2]").unwrap();
    assert_eq!(stats::num_observables(&instrs), 3); // 0..=2
}

#[test]
fn num_ticks() {
    let instrs = parse_lines("H 0\nTICK\nM 0\nTICK").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 2);
}

#[test]
fn num_ticks_with_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  TICK\n}").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 3);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test stats`
Expected: FAIL (module doesn't exist)

### Step 3: Write minimal implementation

Create `src/stats.rs`:

```rust
use crate::ir::StimInstr;

/// One more than the largest qubit index used in the circuit.
pub fn num_qubits(instrs: &[StimInstr]) -> usize {
    let mut max_q: Option<u32> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    if let Some(q) = t.qubit_index() {
                        max_q = Some(max_q.map_or(q, |m: u32| m.max(q)));
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_qubits(body);
                if inner > 0 {
                    let q = (inner - 1) as u32;
                    max_q = Some(max_q.map_or(q, |m| m.max(q)));
                }
            }
        }
    }
    max_q.map_or(0, |m| (m + 1) as usize)
}

/// Total measurement count (M, MX, MY, MR, MRX, MRY produce 1 per target;
/// MPP produces 1 per Pauli product; MXX/MYY/MZZ produce 1 per pair; MPAD produces its arg count).
pub fn num_measurements(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, args, .. } => {
                let n = name.as_str();
                count += match n {
                    "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => targets.len(),
                    "MPP" => targets.split(|t| matches!(t, crate::ir::StimTarget::Combiner)).filter(|g| !g.is_empty()).count(),
                    "MXX" | "MYY" | "MZZ" => targets.len() / 2,
                    "MPAD" => args.first().map_or(0, |a| *a as usize),
                    "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => targets.len(),
                    _ => 0,
                };
            }
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_measurements(body);
            }
        }
    }
    count
}

/// Total DETECTOR annotation count.
pub fn num_detectors(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == "DETECTOR" => count += 1,
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_detectors(body);
            }
            _ => {}
        }
    }
    count
}

/// One more than the largest OBSERVABLE_INCLUDE index.
pub fn num_observables(instrs: &[StimInstr]) -> usize {
    let mut max_idx: Option<usize> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, .. } if name == "OBSERVABLE_INCLUDE" => {
                if let Some(&idx) = args.first() {
                    let i = idx as usize;
                    max_idx = Some(max_idx.map_or(i, |m| m.max(i)));
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_observables(body);
                if inner > 0 {
                    let i = inner - 1;
                    max_idx = Some(max_idx.map_or(i, |m| m.max(i)));
                }
            }
            _ => {}
        }
    }
    max_idx.map_or(0, |m| m + 1)
}

/// Total TICK count.
pub fn num_ticks(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == "TICK" => count += 1,
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_ticks(body);
            }
            _ => {}
        }
    }
    count
}
```

Note: `StimTarget` needs a `qubit_index()` helper method. Add to `src/ir.rs`:

```rust
impl StimTarget {
    pub fn qubit_index(&self) -> Option<u32> {
        match self {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Some(*q),
            StimTarget::Pauli { qubit, .. } => Some(*qubit),
            _ => None,
        }
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod stats;
```

### Step 4: Run test to verify it passes

Run: `cargo test --test stats`
Expected: PASS

### Step 5: Commit

```bash
git add src/stats.rs src/ir.rs src/lib.rs tests/stats.rs
git commit -m "feat: circuit statistics API (num_qubits, num_measurements, num_detectors, num_observables, num_ticks)"
```

---

## Task 2: Circuit Transforms — flattened, without_noise, without_tags

**Files:**
- Create: `src/transforms.rs`
- Modify: `src/lib.rs` (add `pub mod transforms;`)
- Test: `tests/transforms.rs`

### Step 1: Write the failing test

Create `tests/transforms.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::transforms;

#[test]
fn flattened_no_repeat() {
    let instrs = parse_lines("H 0\nM 0").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 2);
}

#[test]
fn flattened_simple_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  M 0\n}").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 6); // 3 * 2
}

#[test]
fn flattened_nested_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 3 {\n    H 0\n  }\n}").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 6); // 2 * 3 * 1
}

#[test]
fn flattened_mixed() {
    let instrs = parse_lines("X 0\nREPEAT 2 {\n  H 0\n}\nM 0").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 4); // X, H, H, M
}

#[test]
fn without_noise_removes_errors() {
    let instrs = parse_lines("H 0\nDEPOLARIZE1(0.01) 0\nX_ERROR(0.1) 0\nM 0").unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 2);
    assert_eq!(clean[0].name().unwrap(), "H");
    assert_eq!(clean[1].name().unwrap(), "M");
}

#[test]
fn without_noise_preserves_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  DEPOLARIZE1(0.01) 0\n  M 0\n}").unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 1); // REPEAT block
    if let rstim::ir::StimInstr::Repeat { body, .. } = &clean[0] {
        assert_eq!(body.len(), 2); // H and M
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn without_noise_all_noise_types() {
    let instrs = parse_lines(
        "X_ERROR(0.1) 0\nY_ERROR(0.1) 0\nZ_ERROR(0.1) 0\n\
         DEPOLARIZE1(0.01) 0\nDEPOLARIZE2(0.01) 0 1\n\
         PAULI_CHANNEL_1(0.1,0,0) 0\nPAULI_CHANNEL_2(0.1,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\n\
         CORRELATED_ERROR(0.1) X0\nELSE_CORRELATED_ERROR(0.1) Z0\n\
         HERALDED_ERASE(0.1) 0\nHERALDED_PAULI_CHANNEL_1(0.1,0,0,0) 0\n\
         I_ERROR(0.1) 0\nII_ERROR(0.1) 0 1\n\
         H 0"
    ).unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 1);
    assert_eq!(clean[0].name().unwrap(), "H");
}

#[test]
fn without_tags_removes_tags() {
    let instrs = parse_lines("H[my_tag] 0\nCX 0 1\nM[readout] 0").unwrap();
    let clean = transforms::without_tags(&instrs);
    for instr in &clean {
        if let rstim::ir::StimInstr::Op { tag, .. } = instr {
            assert!(tag.is_none());
        }
    }
}

#[test]
fn without_tags_preserves_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  H[tag] 0\n}").unwrap();
    let clean = transforms::without_tags(&instrs);
    if let rstim::ir::StimInstr::Repeat { body, .. } = &clean[0] {
        if let rstim::ir::StimInstr::Op { tag, .. } = &body[0] {
            assert!(tag.is_none());
        }
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test transforms`
Expected: FAIL

### Step 3: Write minimal implementation

Create `src/transforms.rs`:

```rust
use crate::ir::StimInstr;

const NOISE_OPS: &[&str] = &[
    "X_ERROR", "Y_ERROR", "Z_ERROR",
    "DEPOLARIZE1", "DEPOLARIZE2",
    "PAULI_CHANNEL_1", "PAULI_CHANNEL_2",
    "CORRELATED_ERROR", "ELSE_CORRELATED_ERROR", "E",
    "HERALDED_ERASE", "HERALDED_PAULI_CHANNEL_1",
    "I_ERROR", "II_ERROR",
];

/// Expand all REPEAT blocks, producing a flat list of Op instructions.
pub fn flattened(instrs: &[StimInstr]) -> Vec<StimInstr> {
    let mut out = Vec::new();
    for instr in instrs {
        match instr {
            StimInstr::Op { .. } => out.push(instr.clone()),
            StimInstr::Repeat { count, body } => {
                let flat_body = flattened(body);
                for _ in 0..*count {
                    out.extend(flat_body.iter().cloned());
                }
            }
        }
    }
    out
}

/// Remove all noise instructions, preserving structure.
pub fn without_noise(instrs: &[StimInstr]) -> Vec<StimInstr> {
    let mut out = Vec::new();
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } => {
                if !NOISE_OPS.contains(&name.as_str()) {
                    out.push(instr.clone());
                }
            }
            StimInstr::Repeat { count, body } => {
                let clean_body = without_noise(body);
                if !clean_body.is_empty() {
                    out.push(StimInstr::Repeat {
                        count: *count,
                        body: clean_body,
                    });
                }
            }
        }
    }
    out
}

/// Remove all instruction tags, preserving structure.
pub fn without_tags(instrs: &[StimInstr]) -> Vec<StimInstr> {
    instrs.iter().map(|instr| match instr {
        StimInstr::Op { name, tag: _, args, targets } => StimInstr::Op {
            name: name.clone(),
            tag: None,
            args: args.clone(),
            targets: targets.clone(),
        },
        StimInstr::Repeat { count, body } => StimInstr::Repeat {
            count: *count,
            body: without_tags(body),
        },
    }).collect()
}
```

Add to `src/lib.rs`:
```rust
pub mod transforms;
```

### Step 4: Run test to verify it passes

Run: `cargo test --test transforms`
Expected: PASS

### Step 5: Commit

```bash
git add src/transforms.rs src/lib.rs tests/transforms.rs
git commit -m "feat: circuit transforms (flattened, without_noise, without_tags)"
```

---

## Task 3: Circuit Transform — inverse()

**Files:**
- Modify: `src/transforms.rs`
- Modify: `tests/transforms.rs`

### Step 1: Write the failing test

Append to `tests/transforms.rs`:

```rust
#[test]
fn inverse_single_qubit_gates() {
    let instrs = parse_lines("S 0\nH 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[0].name().unwrap(), "H");
    assert_eq!(inv[1].name().unwrap(), "S_DAG");
}

#[test]
fn inverse_two_qubit_gates() {
    let instrs = parse_lines("CX 0 1\nCZ 2 3").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[0].name().unwrap(), "CZ");
    assert_eq!(inv[1].name().unwrap(), "CX");
}

#[test]
fn inverse_self_inverse_gates() {
    let instrs = parse_lines("H 0\nX 0\nY 0\nZ 0\nCX 0 1\nCZ 0 1\nSWAP 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 7);
    // All self-inverse, reversed order
    assert_eq!(inv[0].name().unwrap(), "SWAP");
    assert_eq!(inv[6].name().unwrap(), "H");
}

#[test]
fn inverse_s_and_sqrt_gates() {
    let instrs = parse_lines("S 0\nSQRT_X 0\nSQRT_Y 0").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "SQRT_Y_DAG");
    assert_eq!(inv[1].name().unwrap(), "SQRT_X_DAG");
    assert_eq!(inv[2].name().unwrap(), "S_DAG");
}

#[test]
fn inverse_dag_gates() {
    let instrs = parse_lines("S_DAG 0\nSQRT_X_DAG 0\nSQRT_Y_DAG 0\nISWAP_DAG 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "ISWAP");
    assert_eq!(inv[1].name().unwrap(), "SQRT_Y");
    assert_eq!(inv[2].name().unwrap(), "SQRT_X");
    assert_eq!(inv[3].name().unwrap(), "S");
}

#[test]
fn inverse_fails_on_measurement() {
    let instrs = parse_lines("M 0").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_fails_on_noise() {
    let instrs = parse_lines("X_ERROR(0.1) 0").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_repeat_block() {
    let instrs = parse_lines("REPEAT 3 {\n  S 0\n  H 0\n}").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 1);
    if let rstim::ir::StimInstr::Repeat { count, body } = &inv[0] {
        assert_eq!(*count, 3);
        assert_eq!(body[0].name().unwrap(), "H");
        assert_eq!(body[1].name().unwrap(), "S_DAG");
    } else {
        panic!("expected Repeat");
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test transforms`
Expected: FAIL

### Step 3: Write minimal implementation

Add to `src/transforms.rs`:

```rust
/// Invert a gate name. Returns Err for non-invertible operations.
fn invert_gate(name: &str) -> Result<String, String> {
    match name {
        // Self-inverse gates
        "I" | "X" | "Y" | "Z" | "H" | "H_XY" | "H_YZ" | "H_NXY" | "H_NXZ" | "H_NYZ"
        | "CX" | "CY" | "CZ" | "CNOT" | "ZCX" | "ZCY" | "ZCZ"
        | "XCX" | "XCY" | "XCZ" | "YCX" | "YCY" | "YCZ"
        | "SWAP" => Ok(name.to_string()),

        // Paired inverse gates
        "S" => Ok("S_DAG".to_string()),
        "S_DAG" | "SQRT_Z_DAG" => Ok("S".to_string()),
        "SQRT_X" => Ok("SQRT_X_DAG".to_string()),
        "SQRT_X_DAG" => Ok("SQRT_X".to_string()),
        "SQRT_Y" => Ok("SQRT_Y_DAG".to_string()),
        "SQRT_Y_DAG" => Ok("SQRT_Y".to_string()),
        "SQRT_Z" => Ok("S_DAG".to_string()),
        "ISWAP" => Ok("ISWAP_DAG".to_string()),
        "ISWAP_DAG" => Ok("ISWAP".to_string()),
        "CXSWAP" => Ok("SWAPCX".to_string()),
        "SWAPCX" => Ok("CXSWAP".to_string()),
        "CZSWAP" => Ok("CZSWAP".to_string()),

        // Period-3 gates
        "C_XYZ" => Ok("C_ZYX".to_string()),
        "C_ZYX" => Ok("C_XYZ".to_string()),
        "C_NXYZ" => Ok("C_XYNZ".to_string()),
        "C_XYNZ" => Ok("C_NXYZ".to_string()),
        "C_XNYZ" => Ok("C_ZNYX".to_string()),
        "C_ZNYX" => Ok("C_XNYZ".to_string()),
        "C_NZYX" => Ok("C_ZYNX".to_string()),
        "C_ZYNX" => Ok("C_NZYX".to_string()),

        // Non-invertible (measurements, resets, noise, annotations)
        n if NOISE_OPS.contains(&n) => Err(format!("cannot invert noise operation: {n}")),
        "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ"
        | "MPP" | "MXX" | "MYY" | "MZZ" | "MPAD" => {
            Err(format!("cannot invert measurement: {n}"))
        }
        "R" | "RX" | "RY" | "RZ" => Err(format!("cannot invert reset: {n}")),
        "DETECTOR" | "OBSERVABLE_INCLUDE" | "TICK" | "QUBIT_COORDS"
        | "SHIFT_COORDS" => Err(format!("cannot invert annotation: {n}")),
        _ => Err(format!("unknown gate for inverse: {name}")),
    }
}

/// Reverse the circuit and invert each gate. Fails if any instruction is non-invertible.
pub fn inverse(instrs: &[StimInstr]) -> Result<Vec<StimInstr>, String> {
    let mut out = Vec::with_capacity(instrs.len());
    for instr in instrs.iter().rev() {
        match instr {
            StimInstr::Op { name, tag, args, targets } => {
                let inv_name = invert_gate(name)?;
                out.push(StimInstr::Op {
                    name: inv_name,
                    tag: tag.clone(),
                    args: args.clone(),
                    targets: targets.clone(),
                });
            }
            StimInstr::Repeat { count, body } => {
                let inv_body = inverse(body)?;
                out.push(StimInstr::Repeat {
                    count: *count,
                    body: inv_body,
                });
            }
        }
    }
    Ok(out)
}
```

### Step 4: Run test to verify it passes

Run: `cargo test --test transforms`
Expected: PASS

### Step 5: Commit

```bash
git add src/transforms.rs tests/transforms.rs
git commit -m "feat: circuit inverse() transform"
```

---

## Task 4: Exotic Gates — C_XYZ Family + Negated Hadamard Variants

**Files:**
- Modify: `src/sim/tableau.rs` (add gate cases)
- Modify: `src/sim/frame.rs` (add gate cases)
- Modify: `src/error_analyzer.rs` (add undo cases)
- Modify: `src/executor.rs` (add dispatch)
- Test: `tests/exotic_gates.rs`

These are 11 new single-qubit gates:
- **C_XYZ family** (period-3, 8 gates): C_XYZ, C_ZYX, C_NXYZ, C_NZYX, C_XNYZ, C_XYNZ, C_ZNYX, C_ZYNX
- **Negated Hadamard variants** (3 gates): H_NXY, H_NXZ, H_NYZ

### Step 1: Write the failing test

Create `tests/exotic_gates.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::executor::Executor;

fn exec(circuit: &str) -> Vec<bool> {
    let instrs = parse_lines(circuit).unwrap();
    Executor::run(&instrs).unwrap().measurements
}

// C_XYZ: X->Y, Z->X. Applied 3 times = identity.
#[test]
fn c_xyz_period_3() {
    // Start |0> (Z eigenstate), apply C_XYZ three times, measure Z => deterministic 0
    let m = exec("R 0\nC_XYZ 0\nC_XYZ 0\nC_XYZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn c_xyz_x_to_y() {
    // X|0> = |+>, C_XYZ maps X->Y, so C_XYZ|+> is Y eigenstate
    // Measuring Y (via H_XY then M) should give deterministic result
    let m = exec("R 0\nX 0\nC_XYZ 0\nC_XYZ 0\nC_XYZ 0\nX 0\nM 0");
    assert_eq!(m, vec![false]);
}

// C_ZYX: X->Z, Z->Y. Inverse of C_XYZ.
#[test]
fn c_zyx_inverse_of_c_xyz() {
    let m = exec("R 0\nC_XYZ 0\nC_ZYX 0\nM 0");
    assert_eq!(m, vec![false]);
}

// H_NXY: swaps -X and +Y. X->-Y, Z->-Z.
#[test]
fn h_nxy_period_2() {
    let m = exec("R 0\nH_NXY 0\nH_NXY 0\nM 0");
    assert_eq!(m, vec![false]);
}

// H_NXZ: swaps -X and +Z. X->-Z, Z->-X.
#[test]
fn h_nxz_period_2() {
    let m = exec("R 0\nH_NXZ 0\nH_NXZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

// H_NYZ: swaps -Y and +Z. X->-X, Z->-Y.
#[test]
fn h_nyz_period_2() {
    let m = exec("R 0\nH_NYZ 0\nH_NYZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

// All C_* gates: verify period-3 property
#[test]
fn all_c_gates_period_3() {
    for gate in &["C_XYZ", "C_ZYX", "C_NXYZ", "C_NZYX", "C_XNYZ", "C_XYNZ", "C_ZNYX", "C_ZYNX"] {
        let circuit = format!("R 0\n{g} 0\n{g} 0\n{g} 0\nM 0", g = gate);
        let m = exec(&circuit);
        assert_eq!(m, vec![false], "gate {gate} is not period-3");
    }
}

// All H_N* gates: verify period-2 (involution)
#[test]
fn all_h_n_gates_period_2() {
    for gate in &["H_NXY", "H_NXZ", "H_NYZ"] {
        let circuit = format!("R 0\n{g} 0\n{g} 0\nM 0", g = gate);
        let m = exec(&circuit);
        assert_eq!(m, vec![false], "gate {gate} is not period-2");
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test exotic_gates`
Expected: FAIL

### Step 3: Write minimal implementation

Each gate needs its Pauli conjugation rules added to:
1. `src/sim/tableau.rs` — the tableau simulator gate dispatch
2. `src/sim/frame.rs` — the frame simulator gate dispatch
3. `src/error_analyzer.rs` — the backward propagation `undo_op`
4. `src/executor.rs` — the executor's `apply_op` dispatch

**Conjugation tables:**

| Gate | X → | Z → |
|------|-----|-----|
| C_XYZ | +Y | +X |
| C_ZYX | +Z | +Y |
| C_NXYZ | -Y | -X |
| C_NZYX | -Z | -Y |
| C_XNYZ | -Y | +X |
| C_XYNZ | +Y | -X |
| C_ZNYX | +Z | -Y |
| C_ZYNX | -Z | +Y |
| H_NXY | -Y | -Z |
| H_NXZ | -Z | -X |
| H_NYZ | -X | -Y |

For the tableau: each gate modifies the X and Z stabilizer columns.
For the frame: each gate modifies the x_frame and z_frame bits.
For the error analyzer: the undo functions use the inverse conjugation.

Add a helper function for each gate in each simulator file following the existing patterns for H, H_XY, H_YZ, S, SQRT_X, SQRT_Y.

### Step 4: Run test to verify it passes

Run: `cargo test --test exotic_gates`
Expected: PASS

### Step 5: Commit

```bash
git add src/sim/tableau.rs src/sim/frame.rs src/error_analyzer.rs src/executor.rs tests/exotic_gates.rs
git commit -m "feat: exotic Clifford gates (C_XYZ family, H_NXY, H_NXZ, H_NYZ)"
```

---

## Task 5: Circuit Generation — Repetition Code

**Files:**
- Create: `src/gen.rs`
- Modify: `src/lib.rs` (add `pub mod gen;`)
- Test: `tests/gen.rs`

### Step 1: Write the failing test

Create `tests/gen.rs`:

```rust
use rstim::gen;
use rstim::stats;
use rstim::sampler::sample_batch;
use rstim::error_analyzer::ErrorAnalyzer;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn rep_code_basic_structure() {
    let instrs = gen::repetition_code_memory(3, 2, 0.001);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_detectors(&instrs) > 0);
    assert_eq!(stats::num_observables(&instrs), 1);
}

#[test]
fn rep_code_noiseless_no_detections() {
    let instrs = gen::repetition_code_memory(3, 5, 0.0);
    let mut rng = StdRng::seed_from_u64(42);
    let result = sample_batch(&instrs, 100, &mut rng).unwrap();
    // No noise => no detections
    for shot in 0..100 {
        for det in 0..result.detections.num_major() {
            assert!(!result.detections.get(det, shot), "unexpected detection at d={det} shot={shot}");
        }
    }
}

#[test]
fn rep_code_produces_valid_dem() {
    let instrs = gen::repetition_code_memory(3, 3, 0.001);
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
    assert!(dem_str.contains("D"));
}

#[test]
fn rep_code_distance_5() {
    let instrs = gen::repetition_code_memory(5, 3, 0.001);
    assert_eq!(stats::num_qubits(&instrs), 9); // 5 data + 4 ancilla
}

#[test]
fn rep_code_single_round() {
    let instrs = gen::repetition_code_memory(3, 1, 0.001);
    assert!(stats::num_detectors(&instrs) > 0);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test gen`
Expected: FAIL

### Step 3: Write minimal implementation

Create `src/gen.rs`:

```rust
use crate::ir::{StimInstr, StimTarget};

/// Generate a repetition code memory experiment circuit.
///
/// Layout: d data qubits (0..d-1) interleaved with d-1 ancilla qubits (d..2d-2).
/// Each round: reset ancillas, CNOT from data[i] and data[i+1] to ancilla[i],
/// measure ancillas, compare with previous round via DETECTOR.
/// Final round: measure all data qubits, create detectors and observable.
pub fn repetition_code_memory(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    assert!(distance >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    let n_data = distance;
    let n_ancilla = distance - 1;
    let mut instrs = Vec::new();

    // Initialize data qubits
    let data: Vec<u32> = (0..n_data as u32).collect();
    let ancilla: Vec<u32> = (n_data as u32..(n_data + n_ancilla) as u32).collect();

    // Reset all qubits
    for &q in data.iter().chain(ancilla.iter()) {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }

    // Qubit coordinates
    for (i, &q) in data.iter().enumerate() {
        instrs.push(op_with_args("QUBIT_COORDS", &[2.0 * i as f64, 0.0], &[StimTarget::Qubit(q)]));
    }
    for (i, &q) in ancilla.iter().enumerate() {
        instrs.push(op_with_args("QUBIT_COORDS", &[2.0 * i as f64 + 1.0, 0.0], &[StimTarget::Qubit(q)]));
    }

    for round in 0..rounds {
        instrs.push(op("TICK", &[], &[]));

        // Reset ancillas
        for &a in &ancilla {
            instrs.push(op("R", &[], &[StimTarget::Qubit(a)]));
        }

        // CNOTs: data[i] -> ancilla[i], data[i+1] -> ancilla[i]
        for (i, &a) in ancilla.iter().enumerate() {
            instrs.push(op("CX", &[], &[StimTarget::Qubit(data[i]), StimTarget::Qubit(a)]));
        }
        for (i, &a) in ancilla.iter().enumerate() {
            instrs.push(op("CX", &[], &[StimTarget::Qubit(data[i + 1]), StimTarget::Qubit(a)]));
        }

        // Noise on data qubits
        if noise > 0.0 {
            for &d in &data {
                instrs.push(op_with_args("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(d)]));
            }
        }

        // Measure ancillas
        for &a in &ancilla {
            instrs.push(op("M", &[], &[StimTarget::Qubit(a)]));
        }

        // Detectors comparing this round to previous
        for i in 0..n_ancilla {
            let rec_offset = -(n_ancilla as i32) + i as i32;
            if round == 0 {
                // First round: compare to initialization
                instrs.push(op_with_args(
                    "DETECTOR",
                    &[2.0 * i as f64 + 1.0, 0.0, round as f64],
                    &[StimTarget::Rec(rec_offset)],
                ));
            } else {
                // Compare to previous round's measurement
                let prev_offset = rec_offset - n_ancilla as i32;
                instrs.push(op_with_args(
                    "DETECTOR",
                    &[2.0 * i as f64 + 1.0, 0.0, round as f64],
                    &[StimTarget::Rec(rec_offset), StimTarget::Rec(prev_offset)],
                ));
            }
        }
    }

    // Final data measurement
    instrs.push(op("TICK", &[], &[]));
    for &d in &data {
        if noise > 0.0 {
            instrs.push(op_with_args("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(d)]));
        }
        instrs.push(op("M", &[], &[StimTarget::Qubit(d)]));
    }

    // Final detectors: compare adjacent data measurements
    for i in 0..n_ancilla {
        let last_ancilla_offset = -(n_data as i32) - (n_ancilla as i32) + i as i32;
        let data_i_offset = -(n_data as i32) + i as i32;
        let data_i1_offset = data_i_offset + 1;
        instrs.push(op_with_args(
            "DETECTOR",
            &[2.0 * i as f64 + 1.0, 0.0, rounds as f64],
            &[
                StimTarget::Rec(data_i_offset),
                StimTarget::Rec(data_i1_offset),
                StimTarget::Rec(last_ancilla_offset),
            ],
        ));
    }

    // Observable: parity of first data qubit
    instrs.push(op_with_args(
        "OBSERVABLE_INCLUDE",
        &[0.0],
        &[StimTarget::Rec(-(n_data as i32))],
    ));

    instrs
}

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}

fn op_with_args(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod gen;
```

### Step 4: Run test to verify it passes

Run: `cargo test --test gen`
Expected: PASS

### Step 5: Commit

```bash
git add src/gen.rs src/lib.rs tests/gen.rs
git commit -m "feat: repetition code circuit generation"
```

---

## Task 6: CLI gen Command + Circuit Info

**Files:**
- Modify: `src/cli.rs` (add Gen subcommand and `--info` support)
- Test: `tests/cli_gen.rs`

### Step 1: Write the failing test

Create `tests/cli_gen.rs`:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn gen_repetition_code() {
    let output = rstim_cmd()
        .args(["gen", "--code", "repetition_code", "--task", "memory",
               "--distance", "3", "--rounds", "2",
               "--after_clifford_depolarization", "0.001"])
        .output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("R "));
    assert!(s.contains("CX "));
    assert!(s.contains("M "));
    assert!(s.contains("DETECTOR"));
    assert!(s.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_noiseless() {
    let output = rstim_cmd()
        .args(["gen", "--code", "repetition_code", "--task", "memory",
               "--distance", "3", "--rounds", "1"])
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(!s.contains("DEPOLARIZE"));
}

#[test]
fn gen_unknown_code_fails() {
    let output = rstim_cmd()
        .args(["gen", "--code", "unknown", "--task", "memory",
               "--distance", "3", "--rounds", "1"])
        .output().unwrap();
    assert!(!output.status.success());
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test cli_gen`
Expected: FAIL

### Step 3: Write minimal implementation

Add to the `Commands` enum in `src/cli.rs`:

```rust
    /// Generate a common QEC circuit
    #[command(name = "gen")]
    Gen {
        #[arg(long)]
        code: String,
        #[arg(long)]
        task: String,
        #[arg(long)]
        distance: usize,
        #[arg(long)]
        rounds: usize,
        #[arg(long = "after_clifford_depolarization", default_value = "0")]
        noise: f64,
        #[arg(long)]
        out: Option<String>,
    },
```

Add the dispatch in `run()`:

```rust
        Some(Commands::Gen { code, task, distance, rounds, noise, out }) => {
            cmd_gen(&code, &task, distance, rounds, noise, out.as_deref())
        }
```

Add the `cmd_gen` function:

```rust
pub fn cmd_gen(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
    out_path: Option<&str>,
) -> Result<(), String> {
    let instrs = match (code, task) {
        ("repetition_code", "memory") => crate::gen::repetition_code_memory(distance, rounds, noise),
        _ => return Err(format!("unknown code/task: {code}/{task}")),
    };
    // Serialize circuit
    let circuit_text = crate::ir::circuit_to_string(&instrs);
    let mut out = open_output(out_path)?;
    out.write_all(circuit_text.as_bytes()).map_err(|e| format!("write error: {e}"))
}
```

Add `circuit_to_string` to `src/ir.rs` — a function to serialize `Vec<StimInstr>` back to `.stim` text format:

```rust
pub fn circuit_to_string(instrs: &[StimInstr]) -> String {
    let mut s = String::new();
    write_instrs(&mut s, instrs, 0);
    s
}

fn write_instrs(s: &mut String, instrs: &[StimInstr], indent: usize) {
    let pad = "    ".repeat(indent);
    for instr in instrs {
        match instr {
            StimInstr::Op { name, tag, args, targets } => {
                s.push_str(&pad);
                s.push_str(name);
                if let Some(t) = tag {
                    s.push('[');
                    s.push_str(t);
                    s.push(']');
                }
                if !args.is_empty() {
                    s.push('(');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 { s.push_str(", "); }
                        if *a == (*a as i64) as f64 {
                            write!(s, "{}", *a as i64).unwrap();
                        } else {
                            write!(s, "{}", a).unwrap();
                        }
                    }
                    s.push(')');
                }
                for t in targets {
                    s.push(' ');
                    match t {
                        StimTarget::Qubit(q) => write!(s, "{q}").unwrap(),
                        StimTarget::QubitInv(q) => write!(s, "!{q}").unwrap(),
                        StimTarget::Rec(r) => write!(s, "rec[{r}]").unwrap(),
                        StimTarget::Pauli { qubit, basis, inverted } => {
                            if *inverted { s.push('!'); }
                            s.push(match basis {
                                PauliBasis::X => 'X',
                                PauliBasis::Y => 'Y',
                                PauliBasis::Z => 'Z',
                            });
                            write!(s, "{qubit}").unwrap();
                        }
                        StimTarget::Combiner => s.push('*'),
                    }
                }
                s.push('\n');
            }
            StimInstr::Repeat { count, body } => {
                s.push_str(&pad);
                write!(s, "REPEAT {count} {{\n").unwrap();
                write_instrs(s, body, indent + 1);
                s.push_str(&pad);
                s.push_str("}\n");
            }
        }
    }
}
```

Note: use `std::fmt::Write` for `write!` on `String`.

### Step 4: Run test to verify it passes

Run: `cargo test --test cli_gen`
Expected: PASS

### Step 5: Commit

```bash
git add src/cli.rs src/ir.rs tests/cli_gen.rs
git commit -m "feat: rstim gen command for QEC circuit generation"
```
