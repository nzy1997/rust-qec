# Issue 457 Packed Inverse Measurement And Reset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement biased packed inverse-tableau measurement, measurement-reset, and reset for Z, X, and Y bases.

**Architecture:** Add a focused integration test that drives `PackedInverseTableau` directly, then extend `PackedInverseTableau` with packed canonical scratch-row collapse and conversion back to raw inverse rows. X/Y operations use the existing packed Clifford basis changes around the Z-basis primitive so the semantics match the legacy reference.

**Tech Stack:** Rust 2024, `rstim` crate, integration tests under `rstim/tests/`, Cargo test runner.

## Global Constraints

- Provide packed implementations of `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`, and `RY`.
- Measurements append one bit per target in target order.
- Random outcomes choose `false`; deterministic outcomes return their true bit.
- `MR`, `MRX`, and `MRY` append the bit, then prepare `+Z`, `+X`, or `+Y`.
- `R`, `RX`, and `RY` append no bit and prepare `+Z`, `+X`, or `+Y`.
- A `!` target flips only the reported bit, not the post-reset state.
- No operation may materialize the full Boolean tableau.
- Do not route general sampling through this backend or implement noisy frame evolution.
- The focused acceptance command is `cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture`.
- The acceptance test must print `PASS packed inverse measurement and reset`.
- The final verification command required by Agent Desk is `cargo test`.

---

### Task 1: Packed Measurement Acceptance Tests

**Files:**
- Create: `rstim/tests/packed_inverse_tableau_measurement.rs`

**Interfaces:**
- Consumes:
  - `rstim::parser::parse_lines`
  - `rstim::ir::{StimInstr, StimTarget}`
  - `rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau}`
  - `rstim::sim::tableau::StabilizerState`
- Produces:
  - `apply_packed_circuit(num_qubits: usize, circuit: &str) -> (Vec<bool>, CanonicalTableauSnapshot)`
  - `apply_legacy_circuit(num_qubits: usize, circuit: &str) -> (Vec<bool>, CanonicalTableauSnapshot)`
  - Known-answer and differential integration tests.

- [ ] **Step 1: Write the failing test file**

Create `rstim/tests/packed_inverse_tableau_measurement.rs` with these helpers:

```rust
use rstim::ir::{StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau};
use rstim::sim::tableau::StabilizerState;

fn apply_packed_circuit(
    num_qubits: usize,
    circuit: &str,
) -> (Vec<bool>, CanonicalTableauSnapshot) {
    let instrs = parse_lines(circuit).expect("test circuit parses");
    let mut tableau = PackedInverseTableau::identity(num_qubits);
    let mut measurements = Vec::new();
    apply_packed_instrs(&mut tableau, &instrs, &mut measurements);
    (measurements, tableau.canonical_snapshot())
}

fn apply_legacy_circuit(
    num_qubits: usize,
    circuit: &str,
) -> (Vec<bool>, CanonicalTableauSnapshot) {
    let instrs = parse_lines(circuit).expect("test circuit parses");
    let mut state = StabilizerState::new(num_qubits);
    let mut measurements = Vec::new();
    apply_legacy_instrs(&mut state, &instrs, &mut measurements);
    (measurements, state.canonical_snapshot())
}
```

Add dispatch helpers that accept only the operations in issue scope plus the setup Clifford gates:

```rust
fn apply_packed_instrs(
    tableau: &mut PackedInverseTableau,
    instrs: &[StimInstr],
    measurements: &mut Vec<bool>,
) {
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    apply_packed_instrs(tableau, body, measurements);
                }
            }
            StimInstr::Op { name, targets, .. } => apply_packed_op(tableau, name, targets, measurements),
        }
    }
}

fn apply_packed_op(
    tableau: &mut PackedInverseTableau,
    name: &str,
    targets: &[StimTarget],
    measurements: &mut Vec<bool>,
) {
    match name {
        "H" => for q in plain_qubits(targets) { tableau.h(q); },
        "S" => for q in plain_qubits(targets) { tableau.s(q); },
        "S_DAG" => for q in plain_qubits(targets) { tableau.s_dag(q); },
        "X" => for q in plain_qubits(targets) { tableau.x_gate(q); },
        "Y" => for q in plain_qubits(targets) { tableau.y_gate(q); },
        "Z" => for q in plain_qubits(targets) { tableau.z_gate(q); },
        "CX" => for (c, t) in plain_pairs(targets) { tableau.cx(c, t); },
        "M" | "MZ" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_z_biased(q, inv));
        },
        "MX" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_x_biased(q, inv));
        },
        "MY" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_y_biased(q, inv));
        },
        "MR" | "MRZ" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_reset_z_biased(q, inv));
        },
        "MRX" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_reset_x_biased(q, inv));
        },
        "MRY" => for (q, inv) in qubits_with_inversion(targets) {
            measurements.push(tableau.measure_reset_y_biased(q, inv));
        },
        "R" | "RZ" => for q in plain_qubits(targets) { tableau.reset_z_biased(q); },
        "RX" => for q in plain_qubits(targets) { tableau.reset_x_biased(q); },
        "RY" => for q in plain_qubits(targets) { tableau.reset_y_biased(q); },
        other => panic!("unsupported packed test operation {other}"),
    }
}
```

Add a legacy dispatcher using the same names and the existing `StabilizerState` methods:

```rust
fn legacy_measure_z(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    (state.measure_z_biased(q) == 1) ^ inv
}

fn legacy_measure_x(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.h(q);
    let bit = legacy_measure_z(state, q, inv);
    state.h(q);
    bit
}

fn legacy_measure_y(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.s_dag(q);
    state.h(q);
    let bit = legacy_measure_z(state, q, inv);
    state.h(q);
    state.s(q);
    bit
}
```

For legacy measure-reset, preserve the raw bit before applying target inversion:

```rust
fn legacy_measure_reset_z(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    let raw = state.measure_z_biased(q) == 1;
    if raw {
        state.x_gate(q);
    }
    raw ^ inv
}

fn legacy_measure_reset_x(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.h(q);
    let bit = legacy_measure_reset_z(state, q, inv);
    state.h(q);
    bit
}

fn legacy_measure_reset_y(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.s_dag(q);
    state.h(q);
    let bit = legacy_measure_reset_z(state, q, inv);
    state.h(q);
    state.s(q);
    bit
}
```

Add target helpers:

```rust
fn plain_qubits(targets: &[StimTarget]) -> Vec<usize> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => *q as usize,
            other => panic!("expected plain qubit target, got {other:?}"),
        })
        .collect()
}

fn qubits_with_inversion(targets: &[StimTarget]) -> Vec<(usize, bool)> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => (*q as usize, false),
            StimTarget::QubitInv(q) => (*q as usize, true),
            other => panic!("expected measurement qubit target, got {other:?}"),
        })
        .collect()
}

fn plain_pairs(targets: &[StimTarget]) -> Vec<(usize, usize)> {
    let qubits = plain_qubits(targets);
    assert_eq!(qubits.len() % 2, 0, "pair operation requires even target count");
    qubits.chunks_exact(2).map(|pair| (pair[0], pair[1])).collect()
}
```

Add the known-answer test:

```rust
#[test]
fn packed_measurement_known_answers() {
    let cases = [
        (1, "M 0\n", vec![false]),
        (1, "X 0\nM 0\n", vec![true]),
        (1, "H 0\nMX 0\n", vec![false]),
        (1, "H 0\nZ 0\nMX 0\n", vec![true]),
        (1, "H 0\nS 0\nMY 0\n", vec![false]),
        (1, "H 0\nS_DAG 0\nMY 0\n", vec![true]),
        (1, "X 0\nMR 0\nM 0\n", vec![true, false]),
        (1, "H 0\nZ 0\nMRX 0\nMX 0\n", vec![true, false]),
        (1, "H 0\nS_DAG 0\nMRY 0\nMY 0\n", vec![true, false]),
        (3, "X 0\nR 0\nM 0\nRX 1\nMX 1\nRY 2\nMY 2\n", vec![false, false, false]),
        (2, "H 0\nCX 0 1\nM 0 1\n", vec![false, false]),
        (130, "H 63\nCX 63 64\nM 63 64\nH 64\nCX 64 129\nM 64 129\n", vec![false, false, false, false]),
    ];

    for (num_qubits, circuit, expected) in cases {
        let (bits, _) = apply_packed_circuit(num_qubits, circuit);
        assert_eq!(bits, expected, "circuit:\n{circuit}");
    }
}
```

Add an inversion-only-state test:

```rust
#[test]
fn inverted_measurement_target_only_flips_reported_bit() {
    let (bits, snapshot) = apply_packed_circuit(1, "X 0\nMR !0\nM 0\n");
    assert_eq!(bits, vec![false, false]);

    let (_, expected_snapshot) = apply_packed_circuit(1, "X 0\nMR 0\nM 0\n");
    assert_eq!(snapshot, expected_snapshot);
}
```

Add the deterministic differential test:

```rust
#[test]
fn packed_and_legacy_measurement_sequence_match() {
    let circuit = deterministic_measurement_sequence(0x457, 130, 512);
    let (packed_bits, packed_snapshot) = apply_packed_circuit(130, &circuit);
    let (legacy_bits, legacy_snapshot) = apply_legacy_circuit(130, &circuit);
    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(packed_snapshot, legacy_snapshot);
    println!("PASS packed inverse measurement and reset");
}
```

Use a deterministic generator whose prefix contains every operation in scope:

```rust
fn deterministic_measurement_sequence(seed: u64, num_qubits: usize, len: usize) -> String {
    let mut lines = vec![
        "H 0".to_string(),
        "S 1".to_string(),
        "S_DAG 2".to_string(),
        "X 3".to_string(),
        "Y 4".to_string(),
        "Z 5".to_string(),
        "CX 6 7".to_string(),
        "M 0".to_string(),
        "MX 1".to_string(),
        "MY 2".to_string(),
        "MR 3".to_string(),
        "MRX 4".to_string(),
        "MRY 5".to_string(),
        "R 6".to_string(),
        "RX 7".to_string(),
        "RY 8".to_string(),
    ];
    let mut state = seed;
    while lines.len() < len {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let q = ((state >> 17) as usize) % num_qubits;
        let q2 = ((state >> 31) as usize) % (num_qubits - 1);
        let t = if q2 >= q { q2 + 1 } else { q2 };
        let inv = if ((state >> 9) & 1) == 1 { "!" } else { "" };
        lines.push(match ((state >> 61) % 15) as u8 {
            0 => format!("H {q}"),
            1 => format!("S {q}"),
            2 => format!("S_DAG {q}"),
            3 => format!("X {q}"),
            4 => format!("Y {q}"),
            5 => format!("Z {q}"),
            6 => format!("CX {q} {t}"),
            7 => format!("M {inv}{q}"),
            8 => format!("MX {inv}{q}"),
            9 => format!("MY {inv}{q}"),
            10 => format!("MR {inv}{q}"),
            11 => format!("MRX {inv}{q}"),
            12 => format!("MRY {inv}{q}"),
            13 => format!("R {q}"),
            14 => format!("RX {q}"),
            _ => format!("RY {q}"),
        });
    }
    lines.push(String::new());
    lines.join("\n")
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture
```

Expected: FAIL with missing `PackedInverseTableau` measurement and reset methods.

- [ ] **Step 3: Commit the red test**

Run:

```bash
git add rstim/tests/packed_inverse_tableau_measurement.rs
git commit -m "test: cover packed inverse measurement reset"
```

Expected: commit succeeds with the failing acceptance test isolated from implementation.

---

### Task 2: Packed Canonical Scratch Rows And Z Collapse

**Files:**
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`

**Interfaces:**
- Consumes:
  - existing `words_for_bits`, `bit_from_words`, `set_bit`, `toggle_bit`, `z_dot_x_parity`, `words_y_count_mod4`, and `sign_from_words`
  - existing `PackedInverseTableau::{num_rows, words_per_row, raw_matrix_bit, evaluate_coeff_words}`
- Produces:
  - private `PackedCanonicalRows`
  - `PackedInverseTableau::canonical_rows(&self) -> PackedCanonicalRows`
  - `PackedInverseTableau::replace_from_canonical_rows(&mut self, rows: &PackedCanonicalRows)`
  - private `PackedInverseTableau::measure_z_raw_biased(&mut self) -> bool`

- [ ] **Step 1: Add the packed canonical scratch type**

Add this private type below the `PackedInverseTableau` struct:

```rust
#[derive(Debug, Clone)]
struct PackedCanonicalRows {
    num_qubits: usize,
    words_per_row: usize,
    x_plane: Vec<u64>,
    z_plane: Vec<u64>,
    signs: Vec<u64>,
}
```

Add methods with these exact signatures:

```rust
impl PackedCanonicalRows {
    fn new(num_qubits: usize) -> Self;
    fn num_rows(&self) -> usize;
    fn row_start(&self, row: usize) -> usize;
    fn x(&self, row: usize, qubit: usize) -> bool;
    fn sign_bit(&self, row: usize) -> bool;
    fn set_sign_bit(&mut self, row: usize, negative: bool);
    fn set_basis_row(&mut self, row: usize);
    fn copy_row(&mut self, src: usize, dst: usize);
    fn row_exponent_mod4(&self, row: usize) -> u8;
    fn multiply_row_into(&mut self, src: usize, dst: usize);
    fn multiply_row_into_acc(
        &self,
        src: usize,
        acc_x: &mut [u64],
        acc_z: &mut [u64],
        exponent: &mut u8,
    );
    fn evaluate_coeff_words(&self, coeff: &[u64]) -> (Vec<u64>, Vec<u64>, bool);
}
```

`set_basis_row(row)` clears both planes for `row`, sets `X_q` for rows `0..n`, sets `Z_q` for rows `n..2n`, and clears the sign bit.

- [ ] **Step 2: Convert inverse rows to packed canonical rows**

Add this method to `impl PackedInverseTableau`:

```rust
fn canonical_rows(&self) -> PackedCanonicalRows {
    let mut rows = PackedCanonicalRows::new(self.num_qubits);
    let coeff_words = words_for_bits(self.num_rows());

    for target in 0..self.num_rows() {
        let mut coeff = vec![0; coeff_words];
        for coeff_index in 0..self.num_rows() {
            if self.symplectic_inverse_coeff_bit(target, coeff_index) {
                set_bit(&mut coeff, coeff_index);
            }
        }

        let row_start = rows.row_start(target);
        for qubit in 0..self.num_qubits {
            if bit_from_words(&coeff, qubit) {
                rows.x_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
            }
            if bit_from_words(&coeff, self.num_qubits + qubit) {
                rows.z_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
            }
        }

        let (eval_x, eval_z, negative) = self.evaluate_coeff_words(&coeff, false);
        debug_assert!(self.is_basis_words(&eval_x, &eval_z, target));
        rows.set_sign_bit(target, negative);
    }

    rows
}
```

- [ ] **Step 3: Convert packed canonical rows back to inverse storage**

Add this method to `impl PackedInverseTableau`:

```rust
fn replace_from_canonical_rows(&mut self, rows: &PackedCanonicalRows) {
    assert_eq!(rows.num_qubits, self.num_qubits);
    let coeff_words = words_for_bits(self.num_rows());

    for target in 0..self.num_rows() {
        let mut coeff = vec![0; coeff_words];
        for coeff_index in 0..self.num_rows() {
            let source_row = if coeff_index < self.num_qubits {
                self.num_qubits + coeff_index
            } else {
                coeff_index - self.num_qubits
            };
            let source_col = if target < self.num_qubits {
                self.num_qubits + target
            } else {
                target - self.num_qubits
            };
            if canonical_raw_matrix_bit(rows, source_row, source_col) {
                set_bit(&mut coeff, coeff_index);
            }
        }

        let row_start = self.row_start(target);
        self.x_plane[row_start..row_start + self.words_per_row].fill(0);
        self.z_plane[row_start..row_start + self.words_per_row].fill(0);
        for qubit in 0..self.num_qubits {
            if bit_from_words(&coeff, qubit) {
                self.x_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
            }
            if bit_from_words(&coeff, self.num_qubits + qubit) {
                self.z_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
            }
        }

        let (eval_x, eval_z, negative) = rows.evaluate_coeff_words(&coeff);
        debug_assert!(self.is_basis_words(&eval_x, &eval_z, target));
        self.set_sign_bit(target, negative);
        self.mask_row_padding(target);
    }
}
```

Add the private free function used above:

```rust
fn canonical_raw_matrix_bit(rows: &PackedCanonicalRows, row: usize, col: usize) -> bool;
```

- [ ] **Step 4: Add the Z-basis raw measurement primitive**

Add:

```rust
fn measure_z_raw_biased(&mut self, q: usize) -> bool {
    self.check_qubit(q);
    let mut rows = self.canonical_rows();
    let mut pivot = None;
    for row in self.num_qubits..self.num_rows() {
        if rows.x(row, q) {
            pivot = Some(row);
            break;
        }
    }

    let raw = if let Some(p) = pivot {
        for row in 0..self.num_rows() {
            if row != p && rows.x(row, q) {
                rows.multiply_row_into(p, row);
            }
        }
        let destabilizer = p - self.num_qubits;
        rows.copy_row(p, destabilizer);
        rows.set_basis_row(p);
        false
    } else {
        let mut temp_x = vec![0; self.words_per_row];
        let mut temp_z = vec![0; self.words_per_row];
        temp_z[q / 64] |= 1u64 << (q % 64);
        let mut exponent = 0u8;
        for row in 0..self.num_qubits {
            if rows.x(row, q) {
                rows.multiply_row_into_acc(
                    row + self.num_qubits,
                    &mut temp_x,
                    &mut temp_z,
                    &mut exponent,
                );
            }
        }
        sign_from_words(&temp_x, &temp_z, exponent)
    };

    if pivot.is_some() {
        self.replace_from_canonical_rows(&rows);
    }
    raw
}
```

- [ ] **Step 5: Run the focused test to verify the expected partial failure**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture
```

Expected: FAIL because the public measurement/reset methods are still missing.

- [ ] **Step 6: Commit the packed collapse internals**

Run:

```bash
git add rstim/src/sim/packed_inverse_tableau.rs
git commit -m "feat: add packed inverse measurement collapse core"
```

Expected: commit succeeds with packed internals and no public behavior exposed beyond private helpers.

---

### Task 3: Public Measurement And Reset Methods

**Files:**
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`

**Interfaces:**
- Consumes:
  - `PackedInverseTableau::measure_z_raw_biased`
  - existing `PackedInverseTableau::{h,s,s_dag,x_gate}`
- Produces:
  - `measure_z_biased`, `measure_x_biased`, `measure_y_biased`
  - `measure_reset_z_biased`, `measure_reset_x_biased`, `measure_reset_y_biased`
  - `reset_z_biased`, `reset_x_biased`, `reset_y_biased`

- [ ] **Step 1: Add public Z, X, and Y measurement wrappers**

Add:

```rust
pub fn measure_z_biased(&mut self, q: usize, inverted: bool) -> bool {
    self.measure_z_raw_biased(q) ^ inverted
}

pub fn measure_x_biased(&mut self, q: usize, inverted: bool) -> bool {
    self.h(q);
    let bit = self.measure_z_biased(q, inverted);
    self.h(q);
    bit
}

pub fn measure_y_biased(&mut self, q: usize, inverted: bool) -> bool {
    self.s_dag(q);
    self.h(q);
    let bit = self.measure_z_biased(q, inverted);
    self.h(q);
    self.s(q);
    bit
}
```

- [ ] **Step 2: Add public measure-reset wrappers**

Add:

```rust
pub fn measure_reset_z_biased(&mut self, q: usize, inverted: bool) -> bool {
    let raw = self.measure_z_raw_biased(q);
    if raw {
        self.x_gate(q);
    }
    raw ^ inverted
}

pub fn measure_reset_x_biased(&mut self, q: usize, inverted: bool) -> bool {
    self.h(q);
    let bit = self.measure_reset_z_biased(q, inverted);
    self.h(q);
    bit
}

pub fn measure_reset_y_biased(&mut self, q: usize, inverted: bool) -> bool {
    self.s_dag(q);
    self.h(q);
    let bit = self.measure_reset_z_biased(q, inverted);
    self.h(q);
    self.s(q);
    bit
}
```

- [ ] **Step 3: Add public reset wrappers**

Add:

```rust
pub fn reset_z_biased(&mut self, q: usize) {
    let raw = self.measure_z_raw_biased(q);
    if raw {
        self.x_gate(q);
    }
}

pub fn reset_x_biased(&mut self, q: usize) {
    self.h(q);
    self.reset_z_biased(q);
    self.h(q);
}

pub fn reset_y_biased(&mut self, q: usize) {
    self.s_dag(q);
    self.h(q);
    self.reset_z_biased(q);
    self.h(q);
    self.s(q);
}
```

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture
```

Expected: PASS and print `PASS packed inverse measurement and reset`.

- [ ] **Step 5: Run storage and Clifford regressions**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_storage -- --nocapture
cargo test -p rstim --test packed_inverse_tableau_clifford -- --nocapture
```

Expected: both PASS, printing their existing acceptance messages.

- [ ] **Step 6: Commit public operations**

Run:

```bash
git add rstim/src/sim/packed_inverse_tableau.rs rstim/tests/packed_inverse_tableau_measurement.rs
git commit -m "feat: implement packed inverse measurement reset"
```

Expected: commit succeeds with the focused test green.

---

### Task 4: Final Verification And PR

**Files:**
- Modify only if verification exposes a defect in Task 1, Task 2, or Task 3.

**Interfaces:**
- Consumes focused tests and full Cargo verification.
- Produces pushed worker branch and a pull request against `master`.

- [ ] **Step 1: Run issue acceptance**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture
```

Expected: PASS and print `PASS packed inverse measurement and reset`.

- [ ] **Step 2: Run final Agent Desk verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 3: Run final code review**

Use `superpowers:requesting-code-review` with the branch diff from `git merge-base master HEAD` to `HEAD`.

Expected: no Critical or Important issues remain. Fix any Critical or Important findings before continuing.

- [ ] **Step 4: Finish branch with PR option**

Use `superpowers:finishing-a-development-branch`.

Automatically choose:

```text
2. Push and create a Pull Request
```

Run:

```bash
git push -u origin agent/issue-457-implement-biased-measurement-and-reset-on-the-pa-run-1
gh pr create --repo nzy1997/rstim --base master --head agent/issue-457-implement-biased-measurement-and-reset-on-the-pa-run-1 --title "Implement packed inverse measurement and reset" --body-file /tmp/issue-457-pr-body.md
```

Expected: the PR URL is printed and recorded in the final Agent Desk response.
