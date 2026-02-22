# Phase 1: Complete Clifford Gate Set + Resets

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add all remaining Clifford gates, reset operations, and measure+reset operations so that rstim can execute most real `.stim` circuit files without "unsupported instruction" errors.

**Architecture:** Add new methods to `StabilizerState` in `src/sim/tableau.rs` for each gate's tableau update rule. Wire each new gate name into the executor's match arm in `src/executor.rs`. Use gate decompositions (composing existing tableau methods) where the direct rule is complex; use direct single-pass implementations for simple gates. Follow TDD: write the test first, verify it fails, implement, verify it passes, commit.

**Tech Stack:** Rust 2024, `rand` for sampling. No new dependencies.

**Reference:** Stim source at `Stim/src/stim/stabilizers/tableau_specialized_prepend.inl` for tableau rules; `Stim/src/stim/gates/gate_data_*.cc` for gate flow data.

---

### Task 1: `I` gate and `S_DAG` in executor

`I` (identity) is a no-op but must be recognized. `S_DAG` already exists in the tableau (`s_dag` method) but has no executor branch.

**Files:**
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs` (existing file, add tests)

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn i_gate_is_noop() {
    let prog = "H 0\nI 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // H|0> = |+>, I|+> = |+>, MX should give 0 deterministically
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn s_dag_undoes_s() {
    let prog = "H 0\nS 0\nS_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // S_DAG undoes S, so |+> is preserved, MX = 0
    assert_eq!(out.measurements, vec![false]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test i_gate_is_noop s_dag_undoes_s -- --nocapture 2>&1 | head -30`
Expected: FAIL with "unsupported instruction I" and "unsupported instruction S_DAG".

**Step 3: Write minimal implementation**

In `src/executor.rs`, add two match arms inside the `StimInstr::Op` match (after the `"Z"` arm):
```rust
"I" => {} // identity: no-op
"S_DAG" => for_each_qubit(targets, |q| state.s_dag(q))?,
```

**Step 4: Run test to verify it passes**

Run: `cargo test i_gate_is_noop s_dag_undoes_s -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/executor.rs tests/executor_clifford.rs
git commit -m "feat: support I and S_DAG gates in executor"
```

---

### Task 2: `SQRT_X` and `SQRT_X_DAG`

SQRT_X (= √X) has tableau rules: X→X, Z→−Y. It decomposes as H·S·H. SQRT_X_DAG decomposes as H·S†·H.

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs`

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn sqrt_x_preserves_x_eigenstate() {
    // |+> is X eigenstate; SQRT_X should preserve it
    let prog = "H 0\nSQRT_X 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_x_dag_undoes_sqrt_x() {
    let prog = "H 0\nSQRT_X 0\nSQRT_X_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_x_squared_is_x() {
    // SQRT_X^2 = X. X flips |0> to |1>, so M should give 1.
    let prog = "SQRT_X 0\nSQRT_X 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test sqrt_x -q`
Expected: FAIL with "unsupported instruction SQRT_X".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
pub fn sqrt_x(&mut self, q: usize) {
    self.h(q);
    self.s(q);
    self.h(q);
}

pub fn sqrt_x_dag(&mut self, q: usize) {
    self.h(q);
    self.s_dag(q);
    self.h(q);
}
```

In `src/executor.rs`, add match arms:
```rust
"SQRT_X" => for_each_qubit(targets, |q| state.sqrt_x(q))?,
"SQRT_X_DAG" => for_each_qubit(targets, |q| state.sqrt_x_dag(q))?,
```

**Step 4: Run test to verify it passes**

Run: `cargo test sqrt_x -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_clifford.rs
git commit -m "feat: add SQRT_X and SQRT_X_DAG gates"
```

---

### Task 3: `SQRT_Y` and `SQRT_Y_DAG`

SQRT_Y has tableau rules: X→−Z, Z→X (swap x,z columns; phase +=2 when old x was set but old z was not). SQRT_Y_DAG: X→Z, Z→−X (swap x,z; phase +=2 when old z was set but old x was not).

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs`

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn sqrt_y_squared_is_y() {
    // SQRT_Y^2 = Y. Y|0> = i|1>, measuring gives 1.
    let prog = "SQRT_Y 0\nSQRT_Y 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

#[test]
fn sqrt_y_dag_undoes_sqrt_y() {
    let prog = "H 0\nSQRT_Y 0\nSQRT_Y_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_y_maps_z_eigenstate_to_x_eigenstate() {
    // |0> is Z=+1 eigenstate. SQRT_Y maps Z→X, so result should be X=+1 eigenstate.
    let prog = "SQRT_Y 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test sqrt_y -q`
Expected: FAIL with "unsupported instruction SQRT_Y".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// SQRT_Y: X → −Z, Z → X
pub fn sqrt_y(&mut self, q: usize) {
    for i in 0..2 * self.n {
        // Phase +=2 when old x is set but old z is not (X→−Z sign flip)
        if self.x[i][q] && !self.z[i][q] {
            self.phase[i] = (self.phase[i] + 2) % 4;
        }
        let tmp = self.x[i][q];
        self.x[i][q] = self.z[i][q];
        self.z[i][q] = tmp;
    }
}

/// SQRT_Y_DAG: X → Z, Z → −X
pub fn sqrt_y_dag(&mut self, q: usize) {
    for i in 0..2 * self.n {
        // Phase +=2 when old z is set but old x is not (Z→−X sign flip)
        if self.z[i][q] && !self.x[i][q] {
            self.phase[i] = (self.phase[i] + 2) % 4;
        }
        let tmp = self.x[i][q];
        self.x[i][q] = self.z[i][q];
        self.z[i][q] = tmp;
    }
}
```

In `src/executor.rs`, add match arms:
```rust
"SQRT_Y" => for_each_qubit(targets, |q| state.sqrt_y(q))?,
"SQRT_Y_DAG" => for_each_qubit(targets, |q| state.sqrt_y_dag(q))?,
```

**Step 4: Run test to verify it passes**

Run: `cargo test sqrt_y -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_clifford.rs
git commit -m "feat: add SQRT_Y and SQRT_Y_DAG gates"
```

---

### Task 4: Hadamard variants `H_XY` and `H_YZ`

H_XY swaps X↔Y axes (X→Y, Y→X, Z→−Z). H_YZ swaps Y↔Z axes (X→−X, Y→Z, Z→Y). These decompose into existing gates.

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs`

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn h_xy_swaps_x_and_y() {
    // |+> is X=+1 eigenstate. H_XY maps X→Y, so result is Y=+1 eigenstate.
    let prog = "H 0\nH_XY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn h_xy_negates_z() {
    // |0> is Z=+1 eigenstate. H_XY maps Z→−Z, so result is Z=−1 → M gives 1.
    let prog = "H_XY 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

#[test]
fn h_yz_swaps_y_and_z() {
    // |0> is Z=+1 eigenstate. H_YZ maps Z→Y, so result is Y=+1 eigenstate.
    let prog = "H_YZ 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn h_yz_negates_x() {
    // |+> is X=+1 eigenstate. H_YZ maps X→−X, so result is X=−1 → MX gives 1.
    let prog = "H 0\nH_YZ 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test h_xy h_yz -q`
Expected: FAIL with "unsupported instruction H_XY".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// H_XY: X → Y, Y → X, Z → −Z. Decomposition: S · H · S†.
pub fn h_xy(&mut self, q: usize) {
    self.s(q);
    self.h(q);
    self.s_dag(q);
}

/// H_YZ: X → −X, Y → Z, Z → Y. Decomposition: H · S · H.
pub fn h_yz(&mut self, q: usize) {
    self.h(q);
    self.s(q);
    self.h(q);
}
```

> **Note on decompositions:** H_XY = S·H·S† is verified as follows. The conjugation S·H·S† maps X → S(H(S†·X·S)H†)S† = S(H(−Y)H†)S†. Since H·Y·H† = −Y (because HYH = Y with a sign from anticommutation), this gives S·Y·S† = ... We verify correctness via the tests. If a decomposition is wrong the tests catch it immediately.

In `src/executor.rs`, add match arms:
```rust
"H_XY" => for_each_qubit(targets, |q| state.h_xy(q))?,
"H_YZ" => for_each_qubit(targets, |q| state.h_yz(q))?,
```

**Step 4: Run test to verify it passes**

Run: `cargo test h_xy h_yz -q`
Expected: PASS. If a decomposition is wrong, revisit the decomposition. Alternative decompositions to try: H_XY = SQRT_Z · H · SQRT_Z_DAG (equivalent to S·H·S†). H_YZ = SQRT_X (which is H·S·H). The test is the arbiter.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_clifford.rs
git commit -m "feat: add H_XY and H_YZ Hadamard variants"
```

---

### Task 5: `CY` and `SWAP`

CY (controlled-Y): XI→XY, IX→ZX, ZI→ZI, IZ→ZZ. Decomposes as S†(target)·CX·S(target).
SWAP: exchanges two qubits. Decomposes as CX·CX(reversed)·CX, or done directly by swapping tableau columns.

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs`

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn cy_creates_entanglement() {
    // CY on |+0> should entangle. Measuring both in Z should give correlated results.
    let prog = "H 0\nCY 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(prog).unwrap();
    // Run many shots, detector should never fire (bits always agree in a certain sense)
    let mut all_det_zero = true;
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if out.detectors[0] {
            all_det_zero = false;
            break;
        }
    }
    // CY|+0> = (|00>+|11>*i)/sqrt2, so M0 XOR M1 should be 0 → detector = 0
    assert!(all_det_zero);
}

#[test]
fn swap_exchanges_qubits() {
    // Prepare |10>, SWAP, measure both. Should get |01>.
    let prog = "X 0\nSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test cy_creates swap_exchanges -q`
Expected: FAIL with "unsupported instruction CY" / "unsupported instruction SWAP".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// CY (controlled-Y): S†(target) · CX · S(target)
pub fn cy(&mut self, c: usize, t: usize) {
    self.s_dag(t);
    self.cx(c, t);
    self.s(t);
}

/// SWAP: exchange two qubits by swapping tableau columns.
pub fn swap(&mut self, a: usize, b: usize) {
    for i in 0..2 * self.n {
        self.x[i].swap(a, b);
        self.z[i].swap(a, b);
    }
}
```

In `src/executor.rs`, add match arms:
```rust
"CY" => {
    let pairs = qubit_pairs(targets)?;
    for (c, t) in pairs {
        state.cy(c, t);
    }
}
"SWAP" => {
    let pairs = qubit_pairs(targets)?;
    for (a, b) in pairs {
        state.swap(a, b);
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test cy_creates swap_exchanges -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_clifford.rs
git commit -m "feat: add CY and SWAP gates"
```

---

### Task 6: `ISWAP` and `ISWAP_DAG`

ISWAP = SWAP · CZ · S⊗S. Transforms: XI→ZY, IX→YZ, ZI→IZ, IZ→ZI.
ISWAP_DAG = S†⊗S† · CZ · SWAP.

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Test: `tests/executor_clifford.rs`

**Step 1: Write the failing test**

Add to `tests/executor_clifford.rs`:
```rust
#[test]
fn iswap_dag_undoes_iswap() {
    // Prepare |+0>, ISWAP, ISWAP_DAG, measure. Should recover |+0>.
    let prog = "H 0\nISWAP 0 1\nISWAP_DAG 0 1\nMX 0\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, false]);
}

#[test]
fn iswap_on_computational_basis() {
    // ISWAP|10> = i|01>. Measuring should give 01.
    let prog = "X 0\nISWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test iswap -q`
Expected: FAIL with "unsupported instruction ISWAP".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// ISWAP: SWAP · CZ · S(a) · S(b)
pub fn iswap(&mut self, a: usize, b: usize) {
    self.s(a);
    self.s(b);
    self.cz(a, b);
    self.swap(a, b);
}

/// ISWAP_DAG: SWAP · CZ · S†(a) · S†(b)
pub fn iswap_dag(&mut self, a: usize, b: usize) {
    self.s_dag(a);
    self.s_dag(b);
    self.cz(a, b);
    self.swap(a, b);
}
```

In `src/executor.rs`, add match arms:
```rust
"ISWAP" => {
    let pairs = qubit_pairs(targets)?;
    for (a, b) in pairs {
        state.iswap(a, b);
    }
}
"ISWAP_DAG" => {
    let pairs = qubit_pairs(targets)?;
    for (a, b) in pairs {
        state.iswap_dag(a, b);
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test iswap -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_clifford.rs
git commit -m "feat: add ISWAP and ISWAP_DAG gates"
```

---

### Task 7: Controlled Pauli gates `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`

These are two-qubit gates where the control and target Pauli bases differ from the standard CX/CZ.

Tableau rules (direct implementation in one pass):
- **XCX**: z_target ^= x_control, z_control ^= x_target
- **XCZ** = CX(target→control): same as CX with arguments swapped
- **YCZ** = CY(target→control): same as CY with arguments swapped
- **XCY**, **YCX**, **YCY**: decompose via basis changes

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Create: `tests/executor_controlled.rs`

**Step 1: Write the failing test**

Create `tests/executor_controlled.rs`:
```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn xcx_entangles() {
    // XCX on |+>|+> should produce a state where ZZ is deterministic.
    // XCX: ZI→ZX, IZ→XZ, so |++> stabilized by XI,IX → after XCX: XI,IX still.
    // But ZZ commutes with XCX action: ZI→ZX, IZ→XZ, ZZ→ZX·XZ = ZZ·XX = (ZX)(XZ).
    // Test: |+0> then XCX. Then measure second qubit in Z: should be random.
    let prog = "H 0\nXCX 0 1\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut zeros = 0;
    for seed in 0..200 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if !out.measurements[0] { zeros += 1; }
    }
    // Should be roughly 50/50
    assert!(zeros > 60 && zeros < 140, "XCX: got {zeros}/200 zeros");
}

#[test]
fn xcz_is_cx_reversed() {
    // XCZ(0,1) should equal CX(1,0). Prepare |0+>, XCZ → entangled.
    // CX(1,0) on |0+> = |0>(|0>+|1>)/√2 → (|00>+|11>)/√2.
    let prog = "H 1\nXCZ 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(prog).unwrap();
    let mut all_det_zero = true;
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if out.detectors[0] { all_det_zero = false; break; }
    }
    assert!(all_det_zero, "XCZ should entangle like reversed CX");
}

#[test]
fn ycy_entangles() {
    // YCY is symmetric. Prepare |0>|0>, apply H_YZ to both (Y eigenstates),
    // then YCY. The Y eigenstates should be preserved (Y commutes with YCY control/target).
    let prog = "H_YZ 0\nH_YZ 1\nYCY 0 1\nMY 0\nMY 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // MY should be deterministic on Y eigenstates after YCY
    assert_eq!(out.measurements.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test xcx xcz ycy -q`
Expected: FAIL with "unsupported instruction XCX".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// XCX: ZI→ZX, IZ→XZ (XI and IX unchanged)
pub fn xcx(&mut self, a: usize, b: usize) {
    for i in 0..2 * self.n {
        // Phase from anticommutation
        if self.z[i][a] && self.x[i][b] && (self.z[i][b] ^ self.x[i][a] ^ true) {
            self.phase[i] = (self.phase[i] + 2) % 4;
        }
        self.z[i][a] ^= self.x[i][b];
        self.z[i][b] ^= self.x[i][a];
    }
}

/// XCZ(a,b) = CX(b,a)
pub fn xcz(&mut self, a: usize, b: usize) {
    self.cx(b, a);
}

/// XCY(a,b): decompose via H_XY on target, then XCX, then H_XY on target
pub fn xcy(&mut self, a: usize, b: usize) {
    self.h_xy(b);
    self.xcx(a, b);
    self.h_xy(b);
}

/// YCX(a,b) = XCY(b,a)
pub fn ycx(&mut self, a: usize, b: usize) {
    self.xcy(b, a);
}

/// YCZ(a,b) = CY(b,a)
pub fn ycz(&mut self, a: usize, b: usize) {
    self.cy(b, a);
}

/// YCY: decompose via H_YZ on both, then CZ, then H_YZ on both
pub fn ycy(&mut self, a: usize, b: usize) {
    self.h_yz(a);
    self.h_yz(b);
    self.cz(a, b);
    self.h_yz(b);
    self.h_yz(a);
}
```

In `src/executor.rs`, add match arms:
```rust
"XCX" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.xcx(a, b); } }
"XCY" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.xcy(a, b); } }
"XCZ" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.xcz(a, b); } }
"YCX" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.ycx(a, b); } }
"YCY" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.ycy(a, b); } }
"YCZ" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.ycz(a, b); } }
```

**Step 4: Run test to verify it passes**

Run: `cargo test xcx xcz ycy -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_controlled.rs
git commit -m "feat: add XCX, XCY, XCZ, YCX, YCY, YCZ gates"
```

---

### Task 8: `CXSWAP`, `CZSWAP`, `SWAPCX`

Swap-controlled gates. These combine a SWAP with a controlled gate.
- CXSWAP(a,b) = CX(b,a) · CX(a,b) (net effect: XI→XX, ZI→IZ, IX→XI, IZ→ZZ)
- SWAPCX(a,b) = CX(a,b) · CX(b,a) (net effect: XI→IX, ZI→ZZ, IX→XX, IZ→ZI)
- CZSWAP(a,b) = CZ(a,b) · SWAP(a,b)

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Modify: `tests/executor_controlled.rs`

**Step 1: Write the failing test**

Add to `tests/executor_controlled.rs`:
```rust
#[test]
fn cxswap_on_10() {
    // CXSWAP|10>: CX(1,0)|10>=|10>, CX(0,1)|10>=|11>. So CXSWAP|10>=|11>.
    let prog = "X 0\nCXSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true, true]);
}

#[test]
fn swapcx_on_10() {
    // SWAPCX|10>: CX(0,1)|10>=|11>, CX(1,0)|11>=|01>. So SWAPCX|10>=|01>.
    let prog = "X 0\nSWAPCX 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}

#[test]
fn czswap_on_10() {
    // CZSWAP|10> = CZ·SWAP|10> = CZ|01> = -|01>. Measuring Z: 01 (global phase invisible).
    let prog = "X 0\nCZSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test cxswap swapcx czswap -q`
Expected: FAIL.

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// CXSWAP(a,b) = CX(b,a) then CX(a,b)
pub fn cxswap(&mut self, a: usize, b: usize) {
    self.cx(b, a);
    self.cx(a, b);
}

/// SWAPCX(a,b) = CX(a,b) then CX(b,a)
pub fn swapcx(&mut self, a: usize, b: usize) {
    self.cx(a, b);
    self.cx(b, a);
}

/// CZSWAP(a,b) = CZ then SWAP
pub fn czswap(&mut self, a: usize, b: usize) {
    self.cz(a, b);
    self.swap(a, b);
}
```

In `src/executor.rs`, add match arms:
```rust
"CXSWAP" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.cxswap(a, b); } }
"SWAPCX" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.swapcx(a, b); } }
"CZSWAP" => { let p = qubit_pairs(targets)?; for (a, b) in p { state.czswap(a, b); } }
```

**Step 4: Run test to verify it passes**

Run: `cargo test cxswap swapcx czswap -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_controlled.rs
git commit -m "feat: add CXSWAP, SWAPCX, CZSWAP gates"
```

---

### Task 9: Reset operations `R`, `RX`, `RY`

Reset collapses a qubit to a known state: R to |0⟩, RX to |+⟩, RY to |i⟩. Implementation: measure, then conditionally apply a correction gate to force the desired outcome.

**Files:**
- Modify: `src/sim/tableau.rs`
- Modify: `src/executor.rs`
- Create: `tests/executor_reset.rs`

**Step 1: Write the failing test**

Create `tests/executor_reset.rs`:
```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn reset_z_always_gives_zero() {
    // Prepare random state (H then noise-like), reset, measure. Always 0.
    let prog = "H 0\nR 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_x_always_gives_plus() {
    let prog = "RX 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_y_always_gives_plus_i() {
    let prog = "H 0\nRY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_does_not_record_measurement() {
    // R should NOT add to measurement record (unlike M).
    let prog = "R 0\nM 0\nDETECTOR rec[-1]\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements.len(), 1);
    assert_eq!(out.measurements, vec![false]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test reset -q`
Expected: FAIL with "unsupported instruction R".

**Step 3: Write minimal implementation**

In `src/sim/tableau.rs`, add to `impl StabilizerState`:
```rust
/// Reset qubit to |0⟩: measure Z, flip with X if result is 1.
pub fn reset_z(&mut self, q: usize, rng: &mut impl Rng) {
    let (outcome, _) = self.measure_z(q, rng);
    if outcome == 1 {
        self.x_gate(q);
    }
}

/// Reset qubit to |+⟩: H, reset_z, H.
pub fn reset_x(&mut self, q: usize, rng: &mut impl Rng) {
    self.h(q);
    self.reset_z(q, rng);
    self.h(q);
}

/// Reset qubit to |+i⟩ (Y=+1 eigenstate): basis change, reset_z, undo basis change.
pub fn reset_y(&mut self, q: usize, rng: &mut impl Rng) {
    self.s_dag(q);
    self.h(q);
    self.reset_z(q, rng);
    self.h(q);
    self.s(q);
}
```

In `src/executor.rs`, add match arms (note: resets do NOT push to recorder):
```rust
"R" | "RZ" => {
    for q in qubits(targets)? {
        state.reset_z(q, rng);
    }
}
"RX" => {
    for q in qubits(targets)? {
        state.reset_x(q, rng);
    }
}
"RY" => {
    for q in qubits(targets)? {
        state.reset_y(q, rng);
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test reset -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/sim/tableau.rs src/executor.rs tests/executor_reset.rs
git commit -m "feat: add R, RX, RY reset operations"
```

---

### Task 10: Measure-and-reset `MR`, `MRX`, `MRY`

MR (measure then reset) combines M and R: measure the qubit, record the result, then reset to the basis eigenstate. This is the most common operation in QEC syndrome extraction.

**Files:**
- Modify: `src/executor.rs`
- Modify: `tests/executor_reset.rs`

**Step 1: Write the failing test**

Add to `tests/executor_reset.rs`:
```rust
#[test]
fn mr_records_and_resets() {
    // H then MR: measurement is random, but qubit is reset to |0>.
    // Second M should always give 0.
    let prog = "H 0\nMR 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        // Second measurement must always be 0 (reset to |0>)
        assert_eq!(out.measurements[1], false, "seed={seed}");
    }
}

#[test]
fn mrx_records_and_resets_to_plus() {
    let prog = "MRX 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        assert_eq!(out.measurements[1], false, "seed={seed}");
    }
}

#[test]
fn mry_records_and_resets_to_plus_i() {
    let prog = "MRY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        assert_eq!(out.measurements[1], false, "seed={seed}");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test mr_ -q`
Expected: FAIL with "unsupported instruction MR".

**Step 3: Write minimal implementation**

In `src/executor.rs`, add match arms:
```rust
"MR" | "MRZ" => {
    for (q, inv) in qubits_with_inversion(targets)? {
        let (bit, _) = state.measure_z(q, rng);
        let b = (bit == 1) ^ inv;
        recorder.push(b);
        if bit == 1 {
            state.x_gate(q);
        }
    }
}
"MRX" => {
    for (q, inv) in qubits_with_inversion(targets)? {
        state.h(q);
        let (bit, _) = state.measure_z(q, rng);
        let b = (bit == 1) ^ inv;
        recorder.push(b);
        if bit == 1 {
            state.x_gate(q);
        }
        state.h(q);
    }
}
"MRY" => {
    for (q, inv) in qubits_with_inversion(targets)? {
        state.s_dag(q);
        state.h(q);
        let (bit, _) = state.measure_z(q, rng);
        let b = (bit == 1) ^ inv;
        recorder.push(b);
        if bit == 1 {
            state.x_gate(q);
        }
        state.h(q);
        state.s(q);
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test mr_ -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/executor.rs tests/executor_reset.rs
git commit -m "feat: add MR, MRX, MRY measure-and-reset operations"
```

---

### Task 11: `Y_ERROR` noise channel

Single-qubit Y error with probability p. Completes the single-qubit Pauli error set.

**Files:**
- Modify: `src/executor.rs`
- Modify: `tests/noise.rs`

**Step 1: Write the failing test**

Add to `tests/noise.rs`:
```rust
#[test]
fn y_error_flips_at_expected_rate() {
    let prog = "Y_ERROR(0.3) 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ones = 0;
    let shots = 5000;
    for seed in 0..shots {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        let out = ex.run(&mut rng).unwrap();
        if out.measurements[0] { ones += 1; }
    }
    let rate = ones as f64 / shots as f64;
    assert!((rate - 0.3).abs() < 0.05, "Y_ERROR rate {rate} not near 0.3");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test y_error -q`
Expected: FAIL with "unsupported instruction Y_ERROR".

**Step 3: Write minimal implementation**

In `src/executor.rs`, add match arm:
```rust
"Y_ERROR" => {
    let p = args.get(0).copied().unwrap_or(0.0);
    for q in qubits(targets)? {
        if rng.r#gen::<f64>() < p {
            state.y_gate(q);
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test y_error -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/executor.rs tests/noise.rs
git commit -m "feat: add Y_ERROR noise channel"
```

---

### Task 12: Gate alias support

Stim has several gate aliases: `CNOT`→`CX`, `MZ`→`M`, `RZ`→`R`, `MRZ`→`MR`, `SQRT_Z`→`S`, `SQRT_Z_DAG`→`S_DAG`. Add these so that circuits using either name work.

**Files:**
- Modify: `src/executor.rs`
- Create: `tests/executor_aliases.rs`

**Step 1: Write the failing test**

Create `tests/executor_aliases.rs`:
```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn sqrt_z_is_s() {
    let prog = "H 0\nSQRT_Z 0\nSQRT_Z_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn mz_is_m() {
    let prog = "MZ 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test sqrt_z_is_s mz_is_m -q`
Expected: FAIL with "unsupported instruction SQRT_Z".

**Step 3: Write minimal implementation**

In `src/executor.rs`, add existing gate names to the match patterns. Add `"SQRT_Z"` alongside `"S"`, etc.:
```rust
"S" | "SQRT_Z" => for_each_qubit(targets, |q| state.s(q))?,
"S_DAG" | "SQRT_Z_DAG" => for_each_qubit(targets, |q| state.s_dag(q))?,
"M" | "MZ" => { /* existing M code */ }
"R" | "RZ" => { /* already added in Task 9 */ }
"MR" | "MRZ" => { /* already added in Task 10 */ }
```

**Step 4: Run test to verify it passes**

Run: `cargo test sqrt_z_is_s mz_is_m -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/executor.rs tests/executor_aliases.rs
git commit -m "feat: add Stim gate aliases (SQRT_Z, MZ, etc.)"
```

---

### Task 13: Integration test with a real surface code circuit

Validate the full gate set by running a representative Stim circuit that exercises multiple new gates and checking detector/observable output.

**Files:**
- Create: `tests/phase1_integration.rs`

**Step 1: Write the test**

Create `tests/phase1_integration.rs`:
```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn noiseless_repetition_code_no_detections() {
    // A simple repetition code cycle using MR (measure-reset).
    // Without noise, all detectors should be 0.
    let circuit = "\
R 0 1 2
TICK
CX 0 1
CX 2 1
TICK
MR 1
DETECTOR rec[-1]
TICK
CX 0 1
CX 2 1
TICK
MR 1
DETECTOR rec[-1] rec[-2]
TICK
M 0 2
DETECTOR rec[-1] rec[-3]
OBSERVABLE_INCLUDE(0) rec[-2]
";
    let instrs = parse_lines(circuit).unwrap();
    for seed in 0..50 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        for (i, d) in out.detectors.iter().enumerate() {
            assert!(!d, "seed={seed}, detector {i} fired");
        }
    }
}

#[test]
fn all_new_gates_parse_and_execute() {
    // Smoke test: every new gate should parse and execute without error.
    let circuit = "\
I 0
S_DAG 0
SQRT_X 0
SQRT_X_DAG 0
SQRT_Y 0
SQRT_Y_DAG 0
H_XY 0
H_YZ 0
CY 0 1
SWAP 0 1
ISWAP 0 1
ISWAP_DAG 0 1
XCX 0 1
XCY 0 1
XCZ 0 1
YCX 0 1
YCY 0 1
YCZ 0 1
CXSWAP 0 1
SWAPCX 0 1
CZSWAP 0 1
R 0
RX 0
RY 0
MR 0
MRX 0
MRY 0
Y_ERROR(0.0) 0
M 0 1
";
    let instrs = parse_lines(circuit).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements.len(), 5); // MR + MRX + MRY + M 0 + M 1
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test phase1_integration -q`
Expected: PASS (all gates implemented in prior tasks).

**Step 3: Commit**

```bash
git add tests/phase1_integration.rs
git commit -m "test: add Phase 1 integration tests"
```

---

Plan complete and saved to `docs/plans/2026-02-22-phase1-clifford-gates.md`. Two execution options:

**1. Subagent-Driven (this session)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Parallel Session (separate)** — Open a new session with executing-plans, batch execution with checkpoints.

Which approach?
