# Issue 456 Packed Inverse Clifford Evolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement packed inverse-tableau updates for `H`, `S`, `S_DAG`, `X`, `Y`, `Z`, and directed `CX`, plus canonical snapshots that match the audited legacy tableau oracle.

**Architecture:** Extend `PackedInverseTableau` with packed row evaluation using Aaronson-Gottesman phase accounting, then invert the packed inverse basis for test-visible canonical snapshots. Add one read-only `StabilizerState` snapshot accessor and guard the audited legacy gate bodies with an integrity test.

**Tech Stack:** Rust 2024, `rstim` crate, integration tests under `rstim/tests/`, Cargo test runner.

## Global Constraints

- Supported packed gates are exactly `H`, `S`, `S_DAG`, `X`, `Y`, `Z`, and directed `CX`.
- Packed evolution must not expand rows into Boolean vectors.
- `PackedInverseTableau` rows contain inverse images `U^\dagger X_i U` and `U^\dagger Z_i U`.
- `CanonicalTableauSnapshot` has `num_qubits: usize`, `x: Vec<Vec<bool>>`, `z: Vec<Vec<bool>>`, and `phase: Vec<u8>`.
- Canonical row order and Pauli semantics match `StabilizerState`: rows `0..n` are destabilizers and rows `n..2n` are stabilizers.
- The packed adapter must normalize and invert raw inverse rows; it must not merely relabel raw packed rows.
- A `#[doc(hidden)]` read-only snapshot accessor may be added to `StabilizerState`.
- No existing legacy gate body may change.
- The test must separately assert that the only change to the legacy oracle is the read-only snapshot accessor.
- Negative controls must fail for swapped `CX`, raw inverse row relabeling on an `S` sequence, and legacy gate body edits.
- The focused acceptance command is `cargo test -p rstim --test packed_inverse_tableau_clifford -- --nocapture`.
- The final verification command required by Agent Desk is `cargo test`.

---

### Task 1: Differential Clifford Acceptance Tests

**Files:**
- Create: `rstim/tests/packed_inverse_tableau_clifford.rs`

**Interfaces:**
- Consumes:
  - `rstim::sim::packed_inverse_tableau::PackedInverseTableau`
  - `rstim::sim::tableau::StabilizerState`
- Produces:
  - Four issue-required integration tests.
  - Helpers for applying supported gates to packed and legacy tableaus.
  - Oracle-integrity comparison against audited commit `47ffef302a8a471475a5b954a418880cd192c475`.

- [ ] **Step 1: Write the failing test file**

Create tests that call the not-yet-implemented packed gate methods and snapshot accessors. Include:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    H(usize),
    S(usize),
    SDag(usize),
    X(usize),
    Y(usize),
    Z(usize),
    Cx(usize, usize),
}
```

Use helpers:

```rust
fn apply_legacy(state: &mut StabilizerState, gate: Gate) { /* dispatch to legacy */ }
fn apply_packed(tableau: &mut PackedInverseTableau, gate: Gate) { /* dispatch to packed */ }
fn assert_matches_after_each(num_qubits: usize, gates: &[Gate]) { /* compare snapshots */ }
```

The required test bodies are:

- `each_supported_gate_matches_pinned_legacy`: one focused sequence per supported gate plus `H 0; CX 0 1`.
- `directed_cx_0_to_1_is_not_cx_1_to_0`: compare `H 0; CX 0 1` after each gate and assert a deliberately swapped packed `CX 1 0` diverges.
- `packed_evolution_crosses_words_63_64_129`: compare after each instruction in `H 63; S 64; S_DAG 129; CX 63 64; CX 64 129; X 63; Y 64; Z 129; H 129`.
- `fixed_seed_sequences_match_after_every_gate`: for seeds `0x455`, `0xC0FFEE`, and `0x5EED5EED`, build 4,096 gates on 130 qubits with every supported gate forced into the prefix, then compare snapshots after every gate.

Add negative-control assertions:

- raw packed rows converted without inversion diverge after a non-self-inverse `S` sequence;
- swapped `CX` direction diverges in the exact direction circuit;
- oracle-integrity comparison strips only the marked accessor block before comparing to the audited file from git.

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_clifford -- --nocapture
```

Expected: FAIL because the new packed gate methods and snapshot accessors do not exist yet.

---

### Task 2: Packed Gate Evolution And Snapshot Adapter

**Files:**
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`
- Modify: `rstim/src/sim/tableau.rs`

**Interfaces:**
- Produces:
  - `pub struct CanonicalTableauSnapshot`
  - `PackedInverseTableau::{h,s,s_dag,x_gate,y_gate,z_gate,cx}`
  - `PackedInverseTableau::canonical_snapshot(&self) -> CanonicalTableauSnapshot`
  - `StabilizerState::canonical_snapshot(&self) -> CanonicalTableauSnapshot`

- [ ] **Step 1: Add snapshot types and legacy accessor**

Add `CanonicalTableauSnapshot` in `packed_inverse_tableau.rs`.

Add one marked block to `tableau.rs`:

```rust
    // BEGIN issue-456 read-only snapshot accessor
    #[doc(hidden)]
    pub fn canonical_snapshot(&self) -> crate::sim::packed_inverse_tableau::CanonicalTableauSnapshot {
        crate::sim::packed_inverse_tableau::CanonicalTableauSnapshot {
            num_qubits: self.n,
            x: self.x.clone(),
            z: self.z.clone(),
            phase: self.phase.clone(),
        }
    }
    // END issue-456 read-only snapshot accessor
```

- [ ] **Step 2: Implement packed row evaluation**

Add private helpers that keep evolution packed:

```rust
fn row_y_count_mod4(&self, row: usize) -> u8;
fn row_exponent_mod4(&self, row: usize) -> u8;
fn z_dot_x_parity(acc_z: &[u64], src_x: &[u64]) -> bool;
fn multiply_row_into_acc(&self, src: usize, acc_x: &mut [u64], acc_z: &mut [u64], exponent: &mut u8);
fn evaluate_selected_rows(&self, selected_rows: &[usize], input_y_count_mod4: u8, input_negative: bool) -> (Vec<u64>, Vec<u64>, bool);
fn set_row_words(&mut self, row: usize, x: &[u64], z: &[u64], negative: bool);
```

Use the convention `(-1)^r i^(x dot z) X^x Z^z`. When multiplying accumulator row `a` by source row `b`, add source exponent and `2 * (a_z dot b_x)` to the exponent before XORing the packed planes.

- [ ] **Step 3: Implement gate methods**

Use the row evaluator:

```rust
pub fn h(&mut self, q: usize) { /* swap q and n+q */ }
pub fn s(&mut self, q: usize) { /* row q = image of -Y_q */ }
pub fn s_dag(&mut self, q: usize) { /* row q = image of Y_q */ }
pub fn x_gate(&mut self, q: usize) { /* toggle n+q */ }
pub fn z_gate(&mut self, q: usize) { /* toggle q */ }
pub fn y_gate(&mut self, q: usize) { /* toggle q and n+q */ }
pub fn cx(&mut self, c: usize, t: usize) { /* row c = X_c X_t, row n+t = Z_c Z_t */ }
```

- [ ] **Step 4: Implement canonical snapshot inversion**

Build a packed `2n x 2n` matrix from raw inverse rows, row-reduce it with an identity coefficient sidecar, and for each target basis row:

1. read coefficients from the reduced sidecar;
2. set forward X/Z booleans from coefficient bits;
3. evaluate those coefficients through the raw inverse tableau with zero input sign;
4. set canonical phase to `2` if the evaluated basis row is negative, otherwise `0`.

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_clifford -- --nocapture
```

Expected: PASS and print `PASS packed inverse Clifford evolution`.

---

### Task 3: Verification And Branch Completion

**Files:**
- Modify only if verification exposes a defect in Task 1 or Task 2.

**Interfaces:**
- Consumes the focused test and full workspace test command.
- Produces a committed branch and pull request.

- [ ] **Step 1: Run focused storage regression**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_storage -- --nocapture
```

Expected: PASS and print `PASS packed inverse-tableau storage`.

- [ ] **Step 2: Run issue acceptance**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_clifford -- --nocapture
```

Expected: PASS and print `PASS packed inverse Clifford evolution`.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Review, commit, push, and create PR**

Run review per the Superpowers completion workflow, commit the implementation, push `agent/issue-456-implement-packed-inverse-clifford-evolution-run-1`, and create a PR against `master` that closes #456.
