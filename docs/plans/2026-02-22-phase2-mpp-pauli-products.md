# Phase 2: MPP and Pauli Product Targets — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add multi-Pauli-product measurement (MPP), pair measurements (MXX/MYY/MZZ), Pauli product phase gates (SPP/SPP_DAG), and measurement padding (MPAD) to rstim.

**Architecture:** Extend the IR with Pauli target types (`PauliX`, `PauliY`, `PauliZ`) and a `Combiner` target for the `*` operator. The parser splits `X0*X1` tokens by `*` and emits Pauli+Combiner sequences. MPP is implemented by decomposing each Pauli product into basis changes (H for X, H_YZ for Y), a CX fold onto an anchor qubit, a Z-basis measurement, and uncomputation. SPP/SPP_DAG use the same decomposition but apply S/S_DAG instead of measuring. MXX/MYY/MZZ desugar to the same helper.

**Tech Stack:** Rust, `rand` crate, `cargo test`

---

### Task 1: IR and Parser — Pauli Target Types and Combiner

**Files:**
- Modify: `src/ir.rs`
- Modify: `src/parser.rs`
- Modify: `src/executor.rs` (only `max_qubit`)
- Test: `tests/parser_pauli.rs`

**Step 1: Write the failing test**

Create `tests/parser_pauli.rs`:

```rust
use rstim::ir::{StimInstr, StimTarget, PauliBasis};
use rstim::parser::parse_lines;

#[test]
fn parse_mpp_single_product() {
    let prog = "MPP X0*Z1";
    let instrs = parse_lines(prog).unwrap();
    assert_eq!(instrs.len(), 1);
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, false));
        assert_eq!(targets[1], StimTarget::Combiner);
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_multiple_products() {
    let prog = "MPP X0*X1 Z2*Z3";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        // X0 * X1  Z2 * Z3
        assert_eq!(targets.len(), 6);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, false));
        assert_eq!(targets[1], StimTarget::Combiner);
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::X, false));
        assert_eq!(targets[3], StimTarget::pauli(2, PauliBasis::Z, false));
        assert_eq!(targets[4], StimTarget::Combiner);
        assert_eq!(targets[5], StimTarget::pauli(3, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_inverted() {
    let prog = "MPP !Y0*Z1";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::Y, true));
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_with_args() {
    let prog = "MPP(0.01) Z0*Z1";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { args, targets, .. } = &instrs[0] {
        assert_eq!(args, &[0.01]);
        assert_eq!(targets.len(), 3);
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpad() {
    let prog = "MPAD 0 1 0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], StimTarget::Qubit(0));
        assert_eq!(targets[1], StimTarget::Qubit(1));
        assert_eq!(targets[2], StimTarget::Qubit(0));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_spp_single_qubit() {
    let prog = "SPP Z0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_spp_inverted() {
    let prog = "SPP !X0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, true));
    } else {
        panic!("expected Op");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test parser_pauli -- --nocapture`
Expected: compile errors (PauliBasis, Combiner, pauli() don't exist yet)

**Step 3: Implement IR changes**

In `src/ir.rs`, add before `StimTarget`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PauliBasis {
    X,
    Y,
    Z,
}
```

Add to `StimTarget` enum:

```rust
Pauli { qubit: u32, basis: PauliBasis, inverted: bool },
Combiner,
```

Add helper to `StimTarget`:

```rust
impl StimTarget {
    pub fn pauli(qubit: u32, basis: PauliBasis, inverted: bool) -> Self {
        StimTarget::Pauli { qubit, basis, inverted }
    }
}
```

**Step 4: Implement parser changes**

In `src/parser.rs`, modify the target-parsing loop in `parse_lines` (lines 57–61). Replace:

```rust
for token in parts {
    if let Some(t) = parse_target(token)? {
        targets.push(t);
    }
}
```

With:

```rust
for token in parts {
    let subs: Vec<&str> = token.split('*').collect();
    if subs.len() > 1 {
        for (j, sub) in subs.iter().enumerate() {
            if sub.is_empty() {
                return Err(format!("line {}: empty target around *", line_no + 1));
            }
            if j > 0 {
                targets.push(StimTarget::Combiner);
            }
            if let Some(t) = parse_target(sub)? {
                targets.push(t);
            }
        }
    } else if let Some(t) = parse_target(token)? {
        targets.push(t);
    }
}
```

In `parse_target`, add Pauli parsing before the integer qubit fallback:

```rust
fn parse_target(token: &str) -> Result<Option<StimTarget>, String> {
    if token.starts_with("rec[") && token.ends_with(']') {
        let inner = &token[4..token.len() - 1];
        let val: i32 = inner.parse().map_err(|_| format!("bad rec target {token}"))?;
        if val >= 0 {
            return Err("rec must be negative".to_string());
        }
        return Ok(Some(StimTarget::Rec(val)));
    }
    let (negated, raw) = if let Some(rest) = token.strip_prefix('!') {
        (true, rest)
    } else {
        (false, token)
    };

    // Pauli targets: X5, Y2, Z7
    if raw.len() > 1 {
        let first = raw.as_bytes()[0];
        if matches!(first, b'X' | b'Y' | b'Z') {
            if let Ok(q) = raw[1..].parse::<u32>() {
                let basis = match first {
                    b'X' => PauliBasis::X,
                    b'Y' => PauliBasis::Y,
                    _ => PauliBasis::Z,
                };
                return Ok(Some(StimTarget::pauli(q, basis, negated)));
            }
        }
    }

    if let Ok(q) = raw.parse::<u32>() {
        if negated {
            return Ok(Some(StimTarget::QubitInv(q)));
        }
        return Ok(Some(StimTarget::Qubit(q)));
    }
    Err(format!("unsupported target {token}"))
}
```

Add `use crate::ir::PauliBasis;` at the top of parser.rs.

**Step 5: Update max_qubit in executor**

In `src/executor.rs` function `max_qubit`, add handling for Pauli and Combiner targets alongside the existing Qubit/QubitInv arms:

```rust
StimTarget::Pauli { qubit: q, .. } => {
    max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
}
StimTarget::Combiner => {}
```

**Step 6: Run tests to verify they pass**

Run: `cargo test --test parser_pauli -- --nocapture`
Expected: all 7 tests PASS

**Step 7: Commit**

```bash
git add -A && git commit -m "feat: add Pauli target types, Combiner, and parser support"
```

---

### Task 2: MPAD — Measurement Record Padding

**Files:**
- Modify: `src/executor.rs`
- Test: `tests/executor_mpad.rs`

**Step 1: Write the failing test**

Create `tests/executor_mpad.rs`:

```rust
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn run(prog: &str) -> Vec<bool> {
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    exec.run(&mut rng).unwrap().measurements
}

#[test]
fn mpad_pushes_fixed_bits() {
    let m = run("MPAD 0 1 0 1 1");
    assert_eq!(m, vec![false, true, false, true, true]);
}

#[test]
fn mpad_before_measurement() {
    // MPAD 1 then M on |0> gives [true, false]
    let m = run("MPAD 1\nM 0");
    assert_eq!(m, vec![true, false]);
}

#[test]
fn mpad_noisy() {
    // With p=1.0, all bits flip
    let m = run("MPAD(1.0) 0 1 0");
    assert_eq!(m, vec![true, false, true]);
}

#[test]
fn mpad_interacts_with_detector() {
    // MPAD 0 then DETECTOR rec[-1] should give detector=false
    let prog = "MPAD 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = exec.run(&mut rng).unwrap();
    assert_eq!(out.detectors, vec![false]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test executor_mpad -- --nocapture`
Expected: FAIL ("unsupported instruction MPAD")

**Step 3: Implement MPAD**

In `src/executor.rs`, add match arm (near the M/MX/MY block):

```rust
"MPAD" => {
    let p = args.first().copied().unwrap_or(0.0);
    for t in targets {
        let q = expect_qubit(t)?;
        let mut bit = q != 0;
        if p > 0.0 && rng.r#gen::<f64>() < p {
            bit = !bit;
        }
        recorder.push(bit);
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test executor_mpad -- --nocapture`
Expected: all 4 tests PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: implement MPAD (measurement record padding)"
```

---

### Task 3: MPP — Multi-Pauli-Product Measurement

**Files:**
- Modify: `src/executor.rs`
- Test: `tests/executor_mpp.rs`

**Step 1: Write the failing test**

Create `tests/executor_mpp.rs`:

```rust
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn run(prog: &str) -> Vec<bool> {
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    exec.run(&mut rng).unwrap().measurements
}

fn run_many(prog: &str, n: usize) -> Vec<Vec<bool>> {
    (0..n).map(|seed| {
        let instrs = parse_lines(prog).unwrap();
        let mut exec = Executor::from_instrs(instrs).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        exec.run(&mut rng).unwrap().measurements
    }).collect()
}

#[test]
fn mpp_zz_bell_deterministic() {
    // Bell state |Φ+> has Z0⊗Z1 = +1, so MPP Z0*Z1 → 0
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_xx_bell_deterministic() {
    // Bell state |Φ+> has X0⊗X1 = +1, so MPP X0*X1 → 0
    let m = run("H 0\nCX 0 1\nMPP X0*X1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_yy_bell_deterministic() {
    // Bell state |Φ+> has Y0⊗Y1 = -1, so MPP Y0*Y1 → 1
    let m = run("H 0\nCX 0 1\nMPP Y0*Y1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mpp_single_qubit_z() {
    // MPP Z0 on |0> is deterministic 0
    let m = run("MPP Z0");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_single_qubit_x_random() {
    // MPP X0 on |0> is random
    let results = run_many("MPP X0", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mpp_inverted() {
    // !Z0*Z1 on Bell state: inverts the result (0→1)
    let m = run("H 0\nCX 0 1\nMPP !Z0*Z1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mpp_multiple_products() {
    // Two products in one MPP: Z0*Z1 and X0*X1 on Bell state
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1 X0*X1");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mpp_three_qubit_product() {
    // Prepare GHZ: (|000>+|111>)/√2, then MPP Z0*Z1*Z2 → 0
    let m = run("H 0\nCX 0 1\nCX 0 2\nMPP Z0*Z1*Z2");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_preserves_state() {
    // MPP should not disturb the state (uncomputes). Two consecutive MPP Z0*Z1 on Bell state → same result.
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1\nMPP Z0*Z1");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mpp_mixed_xyz_product() {
    // Prepare |0,+,0>, measure X0*X1*Z2
    // X0 on |0> is random, X1 on |+> is +1, Z2 on |0> is +1
    // Product = X0⊗X1⊗Z2 on |0,+,0>
    // Since |0,+,0> is not an eigenstate of this product, result is random.
    let results = run_many("H 1\nMPP X0*X1*Z2", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mpp_noisy() {
    // With p=1.0, deterministic result flips
    let m = run("H 0\nCX 0 1\nMPP(1.0) Z0*Z1");
    assert_eq!(m, vec![true]); // flipped from false
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test executor_mpp -- --nocapture`
Expected: FAIL ("unsupported instruction MPP")

**Step 3: Implement MPP**

In `src/executor.rs`, add a helper function to split Pauli product targets:

```rust
use crate::ir::PauliBasis;

struct PauliProduct {
    terms: Vec<(usize, PauliBasis)>,
    inverted: bool,
}

fn split_pauli_products(targets: &[StimTarget]) -> Result<Vec<PauliProduct>, String> {
    let mut products = Vec::new();
    let mut current_terms: Vec<(usize, PauliBasis)> = Vec::new();
    let mut inverted = false;
    let mut after_combiner = false;

    for target in targets {
        match target {
            StimTarget::Pauli { qubit, basis, inverted: inv } => {
                if !after_combiner && !current_terms.is_empty() {
                    products.push(PauliProduct { terms: std::mem::take(&mut current_terms), inverted });
                    inverted = false;
                }
                if current_terms.is_empty() && *inv {
                    inverted = true;
                }
                current_terms.push((*qubit as usize, *basis));
                after_combiner = false;
            }
            StimTarget::Combiner => {
                after_combiner = true;
            }
            _ => return Err("MPP targets must be Pauli targets".to_string()),
        }
    }
    if !current_terms.is_empty() {
        products.push(PauliProduct { terms: current_terms, inverted });
    }
    Ok(products)
}
```

Add a helper to measure a single Pauli product:

```rust
fn measure_pauli_product(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    rng: &mut impl Rng,
) -> bool {
    if terms.is_empty() {
        return inverted;
    }

    // Basis change: X→Z via H, Y→Z via H_YZ
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    // CX fold: chain all qubits' Z parity onto anchor (last qubit)
    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms.iter().map(|&(q, _)| q).filter(|&q| q != anchor).collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    // Measure anchor in Z basis
    let (bit, _) = state.measure_z(anchor, rng);
    let result = (bit == 1) ^ inverted;

    // Uncompute CX (reverse order, CX is self-inverse)
    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    // Undo basis change (H and H_YZ are self-inverse)
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    result
}
```

Add the MPP match arm in the executor:

```rust
"MPP" => {
    let p = args.first().copied().unwrap_or(0.0);
    let products = split_pauli_products(targets)?;
    for product in &products {
        let mut bit = measure_pauli_product(&mut state, &product.terms, product.inverted, rng);
        if p > 0.0 && rng.r#gen::<f64>() < p {
            bit = !bit;
        }
        recorder.push(bit);
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test executor_mpp -- --nocapture`
Expected: all 11 tests PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: implement MPP (multi-Pauli-product measurement)"
```

---

### Task 4: MXX, MYY, MZZ — Pair Measurements

**Files:**
- Modify: `src/executor.rs`
- Test: `tests/executor_pair_measure.rs`

**Step 1: Write the failing test**

Create `tests/executor_pair_measure.rs`:

```rust
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn run(prog: &str) -> Vec<bool> {
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    exec.run(&mut rng).unwrap().measurements
}

fn run_many(prog: &str, n: usize) -> Vec<Vec<bool>> {
    (0..n).map(|seed| {
        let instrs = parse_lines(prog).unwrap();
        let mut exec = Executor::from_instrs(instrs).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        exec.run(&mut rng).unwrap().measurements
    }).collect()
}

#[test]
fn mxx_bell_deterministic() {
    // Bell state X0⊗X1 = +1
    let m = run("H 0\nCX 0 1\nMXX 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn myy_bell_deterministic() {
    // Bell state Y0⊗Y1 = -1
    let m = run("H 0\nCX 0 1\nMYY 0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mzz_bell_deterministic() {
    // Bell state Z0⊗Z1 = +1
    let m = run("H 0\nCX 0 1\nMZZ 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mxx_random_on_product_state() {
    // |00> is not an XX eigenstate → random
    let results = run_many("MXX 0 1", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mzz_product_state_deterministic() {
    // |00> is a ZZ eigenstate with eigenvalue +1
    let m = run("MZZ 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mxx_inverted() {
    // Bell state X0⊗X1 = +1, inverted → 1
    let m = run("H 0\nCX 0 1\nMXX !0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mxx_multiple_pairs() {
    // MXX on two pairs
    let m = run("H 0\nCX 0 1\nH 2\nCX 2 3\nMXX 0 1 2 3");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mzz_noisy() {
    // p=1.0 flips the deterministic result
    let m = run("MZZ(1.0) 0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mxx_equivalent_to_mpp() {
    // MXX 0 1 should give same result as MPP X0*X1
    let m1 = run("H 0\nCX 0 1\nMXX 0 1");
    let m2 = run("H 0\nCX 0 1\nMPP X0*X1");
    assert_eq!(m1, m2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test executor_pair_measure -- --nocapture`
Expected: FAIL ("unsupported instruction MXX")

**Step 3: Implement MXX, MYY, MZZ**

In `src/executor.rs`, add a helper for pair measurements:

```rust
fn pair_measure(
    state: &mut StabilizerState,
    targets: &[StimTarget],
    basis: PauliBasis,
    noise_p: f64,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> Result<(), String> {
    let pairs = qubits_with_inversion_pairs(targets)?;
    for ((a, inv_a), (b, _inv_b)) in pairs {
        let terms = vec![(a, basis), (b, basis)];
        let mut bit = measure_pauli_product(state, &terms, inv_a, rng);
        if noise_p > 0.0 && rng.r#gen::<f64>() < noise_p {
            bit = !bit;
        }
        recorder.push(bit);
    }
    Ok(())
}
```

Add helper to extract pairs with inversion:

```rust
fn qubits_with_inversion_pairs(targets: &[StimTarget]) -> Result<Vec<((usize, bool), (usize, bool))>, String> {
    let flat = qubits_with_inversion(targets)?;
    if flat.len() % 2 != 0 {
        return Err("odd number of targets for pair measurement".to_string());
    }
    Ok(flat.chunks(2).map(|c| (c[0], c[1])).collect())
}
```

Add match arms in the executor:

```rust
"MXX" => {
    let p = args.first().copied().unwrap_or(0.0);
    pair_measure(&mut state, targets, PauliBasis::X, p, rng, &mut recorder)?;
}
"MYY" => {
    let p = args.first().copied().unwrap_or(0.0);
    pair_measure(&mut state, targets, PauliBasis::Y, p, rng, &mut recorder)?;
}
"MZZ" => {
    let p = args.first().copied().unwrap_or(0.0);
    pair_measure(&mut state, targets, PauliBasis::Z, p, rng, &mut recorder)?;
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test executor_pair_measure -- --nocapture`
Expected: all 9 tests PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: implement MXX, MYY, MZZ pair measurements"
```

---

### Task 5: SPP and SPP_DAG — Pauli Product Phase Gates

**Files:**
- Modify: `src/executor.rs`
- Test: `tests/executor_spp.rs`

**Step 1: Write the failing test**

Create `tests/executor_spp.rs`:

```rust
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn run(prog: &str) -> Vec<bool> {
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    exec.run(&mut rng).unwrap().measurements
}

fn run_many(prog: &str, n: usize) -> Vec<Vec<bool>> {
    (0..n).map(|seed| {
        let instrs = parse_lines(prog).unwrap();
        let mut exec = Executor::from_instrs(instrs).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        exec.run(&mut rng).unwrap().measurements
    }).collect()
}

#[test]
fn spp_z_equals_s() {
    // SPP Z0 on |+> then measure Z should match S 0 on |+> then measure Z
    // |+> → S → (|0>+i|1>)/√2 → measure Z is random
    // |+> → SPP Z0 → same thing
    // Compare via: prepare |+>, apply gate, measure X (should be deterministic)
    // S|+> = (|0>+i|1>)/√2 → X expectation = Re(<+|S†X S|+>) = 0
    // Actually: SPP Z0 = S. Apply S then H then M:
    // |0> → H → |+> → S → (|0>+i|1>)/√2 → H → ((1+i)|0>+(1-i)|1>)/2
    // M(Z) prob(0) = |1+i|²/4 = 2/4 = 1/2 — random.
    // Better test: |0> → SPP Z0 → M → must be 0 (S|0>=|0>)
    let m1 = run("SPP Z0\nM 0");
    let m2 = run("S 0\nM 0");
    assert_eq!(m1, m2);
    assert_eq!(m1, vec![false]); // S|0>=|0>, measure→0
}

#[test]
fn spp_z_on_one_state() {
    // S|1> = i|1>, phase doesn't affect Z measurement
    let m = run("X 0\nSPP Z0\nM 0");
    assert_eq!(m, vec![true]);
}

#[test]
fn spp_x_equals_sqrt_x() {
    // SPP X0 should be equivalent to SQRT_X
    // |0> → SQRT_X → measure Y should be deterministic
    // Test: |0> → SPP X0 → H_YZ → M → compare with SQRT_X path
    let m1 = run("SPP X0\nH_YZ 0\nM 0");
    let m2 = run("SQRT_X 0\nH_YZ 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_dag_z_equals_s_dag() {
    let m1 = run("X 0\nSPP_DAG Z0\nM 0");
    let m2 = run("X 0\nS_DAG 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_xx_phase() {
    // SPP X0*X1 phases the -1 eigenspace of X0⊗X1 by i.
    // On |00>: XX has eigenvalues ±1. |00> = (|++>+|-->)/√2.
    // SPP XX: |++> → |++>, |--> → i|-->
    // Then MPP X0*X1: result depends on whether |00> → eigenstate after SPP
    // After SPP XX: (|++> + i|-->)/√2 — not an eigenstate of XX, so random.
    let results = run_many("SPP X0*X1\nMPP X0*X1", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected random after SPP XX on |00>");
}

#[test]
fn spp_inverted_equals_spp_dag() {
    // SPP !Z0 should equal SPP_DAG Z0
    let m1 = run("H 0\nSPP !Z0\nH 0\nM 0");
    let m2 = run("H 0\nSPP_DAG Z0\nH 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_preserves_stabilizer() {
    // SPP Z0*Z1 on Bell state (which has Z0Z1=+1) should leave it unchanged
    // since the +1 eigenspace gets no phase.
    let m = run("H 0\nCX 0 1\nSPP Z0*Z1\nMPP Z0*Z1");
    assert_eq!(m, vec![false]);
}

#[test]
fn spp_multiple_products() {
    // SPP Z0 Z1 = S(0) followed by S(1)
    let m1 = run("SPP Z0 Z1\nM 0 1");
    let m2 = run("S 0\nS 1\nM 0 1");
    assert_eq!(m1, m2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test executor_spp -- --nocapture`
Expected: FAIL ("unsupported instruction SPP")

**Step 3: Implement SPP and SPP_DAG**

Add a helper for applying Pauli product phase:

```rust
fn apply_spp(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    dag: bool,
) {
    if terms.is_empty() {
        return;
    }

    // Basis change
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    // CX fold
    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms.iter().map(|&(q, _)| q).filter(|&q| q != anchor).collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    // Apply S or S_DAG (inversion swaps them)
    if dag ^ inverted {
        state.s_dag(anchor);
    } else {
        state.s(anchor);
    }

    // Uncompute
    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }
}
```

Add match arms:

```rust
"SPP" => {
    let products = split_pauli_products(targets)?;
    for product in &products {
        apply_spp(&mut state, &product.terms, product.inverted, false);
    }
}
"SPP_DAG" => {
    let products = split_pauli_products(targets)?;
    for product in &products {
        apply_spp(&mut state, &product.terms, product.inverted, true);
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test executor_spp -- --nocapture`
Expected: all 8 tests PASS

**Step 5: Run all tests**

Run: `cargo test`
Expected: all tests PASS (parser_pauli + executor_mpad + executor_mpp + executor_pair_measure + executor_spp + all existing Phase 1 tests)

**Step 6: Commit**

```bash
git add -A && git commit -m "feat: implement SPP and SPP_DAG (Pauli product phase gates)"
```

---

### Summary

| Task | What | Tests |
|------|------|-------|
| 1 | IR + Parser: PauliBasis, Combiner, Pauli target parsing | 7 |
| 2 | MPAD: measurement record padding | 4 |
| 3 | MPP: multi-Pauli-product measurement | 11 |
| 4 | MXX, MYY, MZZ: pair measurements | 9 |
| 5 | SPP, SPP_DAG: Pauli product phase gates | 8 |
| **Total** | | **39** |

After all tasks, run `cargo test` to confirm no regressions in Phase 1 tests.
