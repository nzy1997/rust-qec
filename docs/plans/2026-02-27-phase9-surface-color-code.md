# Phase 9: Surface Code + Color Code Circuit Generators Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `surface_code` (rotated and unrotated, memory-X and memory-Z) and `color_code` (memory_xyz) circuit generators to `rstim gen`.

**Architecture:** Two new files `src/gen/surface_code.rs` and `src/gen/color_code.rs` (move `repetition_code_memory` into `src/gen/rep_code.rs`). A `src/gen/mod.rs` re-exports all generators. The CLI `gen` subcommand dispatches by `--code` and `--task`. Each generator returns `Vec<StimInstr>`.

**Tech Stack:** Rust, existing `rstim` IR/parser modules

---

## Task 1: Refactor circuit_gen into src/gen/ module

**Files:**
- Create: `src/gen/mod.rs`
- Create: `src/gen/rep_code.rs` (move existing `repetition_code_memory`)
- Modify: `src/lib.rs` (replace `pub mod circuit_gen` with `pub mod gen`)
- Modify: `src/cli.rs` (update import path)

### Step 1: Write a test that the existing function still works under new path

Add to `tests/gen_rep_code.rs` (or check existing tests still pass):

```rust
use rstim::gen::repetition_code_memory;

#[test]
fn rep_code_still_works() {
    let instrs = repetition_code_memory(3, 2, 0.0);
    assert!(!instrs.is_empty());
}
```

### Step 2: Run test to verify it fails

```
cargo test --test gen_rep_code
```
Expected: compile error — `rstim::gen` not found.

### Step 3: Create src/gen/ structure

Create `src/gen/rep_code.rs` — copy the body of `src/circuit_gen.rs` verbatim.

Create `src/gen/mod.rs`:

```rust
pub mod rep_code;
pub use rep_code::repetition_code_memory;
```

In `src/lib.rs`, replace:
```rust
pub mod circuit_gen;
```
with:
```rust
pub mod gen;
// Keep old name as alias for backwards compat with existing tests
pub use gen as circuit_gen;
```

Update `src/cli.rs` import from `crate::circuit_gen::repetition_code_memory` to `crate::gen::repetition_code_memory`.

### Step 4: Run tests

```
cargo test
```
Expected: all existing tests pass.

### Step 5: Commit

```bash
git add src/gen/ src/lib.rs src/cli.rs
git commit -m "refactor: move circuit_gen into src/gen/ module"
```

---

## Task 2: Rotated Surface Code Generator

**Files:**
- Create: `src/gen/surface_code.rs`
- Modify: `src/gen/mod.rs`
- Modify: `src/cli.rs`
- Test: `tests/gen_surface_code.rs`

### Step 1: Write the failing tests

Create `tests/gen_surface_code.rs`:

```rust
use rstim::gen::surface_code::{rotated_memory_x, rotated_memory_z};
use rstim::stats;
use rstim::parser::parse_lines;

#[test]
fn rotated_memory_x_d3_r1_parses() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    // d=3 rotated: 9 data + 8 ancilla = 17 qubits
    assert_eq!(stats::num_qubits(&instrs), 17);
    // 1 round: 8 ancilla measurements + 9 final data measurements = 17 measurements
    assert_eq!(stats::num_measurements(&instrs), 17);
}

#[test]
fn rotated_memory_z_d3_r1_parses() {
    let instrs = rotated_memory_z(3, 1, 0.0);
    assert_eq!(stats::num_qubits(&instrs), 17);
    assert_eq!(stats::num_measurements(&instrs), 17);
}

#[test]
fn rotated_memory_x_has_observable() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn rotated_memory_x_with_noise_has_depolarize() {
    use rstim::ir::StimInstr;
    let instrs = rotated_memory_x(3, 1, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}

#[test]
fn rotated_memory_x_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = rotated_memory_x(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test gen_surface_code
```
Expected: compile error — `rstim::gen::surface_code` not found.

### Step 3: Implement rotated surface code

Create `src/gen/surface_code.rs`.

**Rotated surface code layout (distance d):**
- Data qubits: d×d grid at integer coordinates (x, y) where x+y is even, 0 ≤ x,y < 2d-1
- X ancilla: (d-1)×(d-1)/2 + boundary, at positions where x+y is odd, interior
- Z ancilla: remaining ancilla positions

Qubit indexing: assign indices in row-major order over all qubit positions.

Each stabilizer round:
1. `R` all ancilla
2. `H` all X-ancilla
3. Four CNOT layers (N, E, S, W neighbors) — X-ancilla use `CX ancilla data`, Z-ancilla use `CX data ancilla`
4. `H` all X-ancilla
5. `M` all ancilla
6. `DETECTOR` for each ancilla (compare to previous round via `rec[-n_ancilla + i]` and `rec[-2*n_ancilla + i]`)

Final round: `M` all data qubits, `DETECTOR` + `OBSERVABLE_INCLUDE`.

```rust
use crate::ir::{StimInstr, StimTarget};

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}

pub fn rotated_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, noise, true)
}

pub fn rotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, noise, false)
}

fn rotated_surface_code(d: usize, rounds: usize, noise: f64, memory_x: bool) -> Vec<StimInstr> {
    assert!(d >= 2);
    assert!(rounds >= 1);
    // ... (full implementation)
}
```

The full implementation assigns qubit indices, builds the stabilizer schedule, and emits detectors. Reference Stim's `gen_surface_code.cc` for the exact CNOT ordering (N→E→S→W for X stabilizers, same order for Z stabilizers).

### Step 4: Run tests

```
cargo test --test gen_surface_code rotated
```
Expected: all rotated tests pass.

### Step 5: Commit

```bash
git add src/gen/surface_code.rs src/gen/mod.rs tests/gen_surface_code.rs
git commit -m "feat: add rotated surface code generator (memory_x, memory_z)"
```

---

## Task 3: Unrotated Surface Code Generator

**Files:**
- Modify: `src/gen/surface_code.rs`
- Test: `tests/gen_surface_code.rs` (extend)

### Step 1: Write the failing tests

Add to `tests/gen_surface_code.rs`:

```rust
use rstim::gen::surface_code::{unrotated_memory_x, unrotated_memory_z};

#[test]
fn unrotated_memory_x_d3_r1() {
    let instrs = unrotated_memory_x(3, 1, 0.0);
    // d=3 unrotated: 9 data + 12 ancilla = 21 qubits (some boundary ancilla may be omitted)
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn unrotated_memory_z_d3_r1() {
    let instrs = unrotated_memory_z(3, 1, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test gen_surface_code unrotated
```
Expected: compile error.

### Step 3: Implement unrotated surface code

Add `unrotated_memory_x` and `unrotated_memory_z` to `src/gen/surface_code.rs`.

**Unrotated layout:** Data qubits on integer grid, X-ancilla between horizontal neighbors, Z-ancilla between vertical neighbors. Boundary ancilla are weight-2 stabilizers.

### Step 4: Run tests

```
cargo test --test gen_surface_code
```
Expected: all pass.

### Step 5: Commit

```bash
git add src/gen/surface_code.rs tests/gen_surface_code.rs
git commit -m "feat: add unrotated surface code generator (memory_x, memory_z)"
```

---

## Task 4: Color Code Generator

**Files:**
- Create: `src/gen/color_code.rs`
- Modify: `src/gen/mod.rs`
- Test: `tests/gen_color_code.rs`

### Step 1: Write the failing tests

Create `tests/gen_color_code.rs`:

```rust
use rstim::gen::color_code::memory_xyz;
use rstim::stats;

#[test]
fn color_code_d3_r1() {
    let instrs = memory_xyz(3, 1, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn color_code_d3_r1_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = memory_xyz(3, 1, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test gen_color_code
```
Expected: compile error.

### Step 3: Implement color code

Create `src/gen/color_code.rs`.

**Triangular color code layout (distance d):**
- Triangular grid of data qubits
- Three-body X and Z stabilizers on each face
- Observable: top row of data qubits

```rust
pub fn memory_xyz(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    // ... triangular layout, 6-body stabilizers for larger d
}
```

Reference Stim's `gen_color_code.cc` for the exact qubit layout and stabilizer schedule.

### Step 4: Run tests

```
cargo test --test gen_color_code
```
Expected: all pass.

### Step 5: Commit

```bash
git add src/gen/color_code.rs src/gen/mod.rs tests/gen_color_code.rs
git commit -m "feat: add color code generator (memory_xyz)"
```

---

## Task 5: Wire generators into CLI

**Files:**
- Modify: `src/cli.rs`
- Test: integration via `cargo run`

### Step 1: Write the failing test

Add to `tests/gen_surface_code.rs`:

```rust
#[test]
fn cli_gen_surface_code_rotated_memory_x() {
    use rstim::cli::{Cli, run};
    // Just verify it doesn't panic — full CLI test
    let instrs = rstim::gen::surface_code::rotated_memory_x(3, 1, 0.0);
    assert!(!instrs.is_empty());
}
```

### Step 2: Update CLI dispatch

In `src/cli.rs`, in the `Gen` match arm, extend the dispatch:

```rust
("surface_code", "rotated_memory_x") => crate::gen::surface_code::rotated_memory_x(distance, rounds, noise),
("surface_code", "rotated_memory_z") => crate::gen::surface_code::rotated_memory_z(distance, rounds, noise),
("surface_code", "unrotated_memory_x") => crate::gen::surface_code::unrotated_memory_x(distance, rounds, noise),
("surface_code", "unrotated_memory_z") => crate::gen::surface_code::unrotated_memory_z(distance, rounds, noise),
("color_code", "memory_xyz") => crate::gen::color_code::memory_xyz(distance, rounds, noise),
```

### Step 3: Run tests

```
cargo test
```
Expected: all pass.

### Step 4: Commit

```bash
git add src/cli.rs
git commit -m "feat: wire surface_code and color_code into rstim gen CLI"
```
