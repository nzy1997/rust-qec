# Phase 3: Remaining Noise Channels — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the noise model with identity errors, correlated errors, general Pauli channels, and heralded erasure/Pauli channels.

**Architecture:** `I_ERROR`/`II_ERROR` are no-ops (recognize and skip). `CORRELATED_ERROR`/`ELSE_CORRELATED_ERROR` use a persistent boolean flag in the executor to implement mutually-exclusive error chains over Pauli targets. `PAULI_CHANNEL_1`/`PAULI_CHANNEL_2` sample from explicit probability distributions over Pauli operators. `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` push herald bits into the measurement record, then conditionally apply Pauli errors. Parser gains `[tag]` support for instruction annotations.

**Tech Stack:** Rust, `rand` crate, `cargo test`

---

### Task 1: Parser Tag Support + I_ERROR / II_ERROR

**Files:**
- Modify: `src/parser.rs` (add tag parsing in `split_name_and_args`)
- Modify: `src/executor.rs` (add `I_ERROR`, `II_ERROR` match arms)
- Create: `tests/noise_phase3.rs`

**Context:**

The IR already has `tag: Option<String>` in `StimInstr::Op` but the parser never populates it. Stim syntax allows `INSTR_NAME[tag_text](args) targets`, e.g. `I_ERROR[LEAKAGE:0.1](0.05) 0`. The tag is metadata for external tools; rstim stores it but doesn't act on it.

`I_ERROR` and `II_ERROR` are no-ops: they don't modify quantum state. `II_ERROR` takes qubit pairs (even number of targets). They exist so external tools can attach metadata via tags.

**Step 1: Write failing tests**

In `tests/noise_phase3.rs`:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

fn run(program: &str) -> Vec<bool> {
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    ex.run(&mut rng).unwrap().measurements
}

#[test]
fn i_error_is_noop() {
    let m = run("I_ERROR(0.1) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn ii_error_is_noop() {
    let m = run("II_ERROR(0.1) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn parser_tag_round_trip() {
    let instrs = parse_lines("I_ERROR[LEAKAGE:0.1](0.05) 0\n").unwrap();
    match &instrs[0] {
        rstim::ir::StimInstr::Op { name, tag, args, .. } => {
            assert_eq!(name, "I_ERROR");
            assert_eq!(tag.as_deref(), Some("LEAKAGE:0.1"));
            assert_eq!(args, &[0.05]);
        }
        _ => panic!("expected Op"),
    }
}

#[test]
fn parser_tag_no_args() {
    let instrs = parse_lines("I_ERROR[TAG] 0\n").unwrap();
    match &instrs[0] {
        rstim::ir::StimInstr::Op { name, tag, args, .. } => {
            assert_eq!(name, "I_ERROR");
            assert_eq!(tag.as_deref(), Some("TAG"));
            assert!(args.is_empty());
        }
        _ => panic!("expected Op"),
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: compilation or assertion errors

**Step 3: Implement tag parsing**

In `src/parser.rs`, modify `split_name_and_args` to return `(&str, Option<String>, Vec<f64>)`:

```rust
fn split_name_and_args(token: &str) -> Result<(&str, Option<String>, Vec<f64>), String> {
    let (name_part, rest) = if let Some(bracket_start) = token.find('[') {
        let bracket_end = token.find(']').ok_or_else(|| format!("unclosed tag bracket in {token}"))?;
        let tag = &token[bracket_start + 1..bracket_end];
        let name = &token[..bracket_start];
        let remainder = &token[bracket_end + 1..];
        (name, Some((tag.to_string(), remainder)))
    } else {
        (token, None)
    };

    let (name, tag, args_part) = match rest {
        Some((tag, remainder)) => (name_part, Some(tag), remainder),
        None => (name_part, None, token.get(name_part.len()..).unwrap_or("")),
    };

    // Now parse args from the remaining part (could be "(0.1, 0.2)" or empty)
    let args_str = if !args_part.is_empty() {
        args_part
    } else if tag.is_none() {
        // Original path: no tag, check for args in full token
        token.get(name.len()..).unwrap_or("")
    } else {
        ""
    };

    let args = if let Some(idx) = args_str.find('(') {
        if !args_str.ends_with(')') {
            return Err(format!("bad args {token}"));
        }
        let inner = args_str[idx + 1..args_str.len() - 1].trim();
        if inner.is_empty() {
            vec![]
        } else {
            inner
                .split(',')
                .map(|s| s.trim().parse::<f64>().map_err(|_| format!("bad arg {s}")))
                .collect::<Result<Vec<_>, _>>()?
        }
    } else {
        vec![]
    };

    Ok((name, tag, args))
}
```

Update the call site in `parse_lines` that calls `split_name_and_args`:

```rust
let (name, tag, args) = split_name_and_args(name_token)?;
let name = name.to_ascii_uppercase();
// ...
let mut instr = StimInstr::Op {
    name: name.clone(),
    tag,
    args,
    targets: vec![],
};
```

Note: `StimInstr::new` doesn't accept a tag, so construct `StimInstr::Op` directly instead of using `StimInstr::new`.

**Step 4: Implement I_ERROR / II_ERROR in executor**

In `src/executor.rs`, add these match arms after the existing noise channels:

```rust
"I_ERROR" => {}
"II_ERROR" => {}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: all 4 tests pass

Run: `cargo test 2>&1 | tail -5`
Expected: all existing tests still pass

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: parser tag support + I_ERROR/II_ERROR no-ops"
```

---

### Task 2: CORRELATED_ERROR / ELSE_CORRELATED_ERROR

**Files:**
- Modify: `src/executor.rs` (add flag + match arms + helper)
- Modify: `tests/noise_phase3.rs` (add tests)

**Context:**

`CORRELATED_ERROR(p) X0 Y1 Z2` means: with probability `p`, simultaneously apply X to qubit 0, Y to qubit 1, and Z to qubit 2. It also manages a boolean flag:

- `CORRELATED_ERROR(p)`: reset flag. Sample. If fires: apply Paulis, flag = true. If not: flag = false.
- `ELSE_CORRELATED_ERROR(p)`: if flag is already true, skip. Otherwise: sample. If fires: apply Paulis, flag = true.

This creates mutually exclusive error chains: at most one in a chain fires.

Targets are Pauli targets (`StimTarget::Pauli { qubit, basis, .. }`) — same type used by MPP, but without combiners. Each target specifies which Pauli to apply to which qubit.

**Step 1: Write failing tests**

Append to `tests/noise_phase3.rs`:

```rust
#[test]
fn correlated_error_deterministic() {
    // probability 1 → always fires
    let m = run("CORRELATED_ERROR(1) X0\nM 0\n");
    assert_eq!(m, vec![true]);
}

#[test]
fn correlated_error_zero_prob() {
    let m = run("CORRELATED_ERROR(0) X0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn correlated_error_multi_pauli() {
    // Apply X0 Y1 Z2 deterministically
    let m = run("H 0 1 2\nCORRELATED_ERROR(1) X0 Y1 Z2\nH 0 1 2\nM 0 1 2\n");
    // X0 on |+> → |+>, so after H → |0> → false — wait, need to think through
    // Simpler: start in |000>, apply X0, measure → 1
    let m = run("CORRELATED_ERROR(1) X0\nM 0 1 2\n");
    assert_eq!(m, vec![true, false, false]);
}

#[test]
fn else_correlated_error_skipped_when_first_fires() {
    // First fires (p=1), second is skipped
    let m = run("CORRELATED_ERROR(1) X0\nELSE_CORRELATED_ERROR(1) X1\nM 0 1\n");
    assert_eq!(m, vec![true, false]);
}

#[test]
fn else_correlated_error_fires_when_first_doesnt() {
    // First doesn't fire (p=0), second fires (p=1)
    let m = run("CORRELATED_ERROR(0) X0\nELSE_CORRELATED_ERROR(1) X1\nM 0 1\n");
    assert_eq!(m, vec![false, true]);
}

#[test]
fn correlated_error_chain_three() {
    // Only second fires: first p=0, second p=1, third skipped
    let m = run("CORRELATED_ERROR(0) X0\nELSE_CORRELATED_ERROR(1) X1\nELSE_CORRELATED_ERROR(1) X2\nM 0 1 2\n");
    assert_eq!(m, vec![false, true, false]);
}

#[test]
fn correlated_error_resets_flag() {
    // Two independent chains
    let m = run(
        "CORRELATED_ERROR(1) X0\n\
         ELSE_CORRELATED_ERROR(1) X1\n\
         CORRELATED_ERROR(1) X2\n\
         ELSE_CORRELATED_ERROR(1) X3\n\
         M 0 1 2 3\n"
    );
    // Chain 1: X0 fires, X1 skipped. Chain 2: X2 fires, X3 skipped.
    assert_eq!(m, vec![true, false, true, false]);
}

#[test]
fn correlated_error_multi_qubit_pauli() {
    // Apply Y on qubit 0 and Z on qubit 1
    // Y|0⟩ = i|1⟩, so M gives 1. Z|0⟩ = |0⟩, M gives 0.
    let m = run("CORRELATED_ERROR(1) Y0 Z1\nM 0 1\n");
    assert_eq!(m, vec![true, false]);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: "unsupported instruction CORRELATED_ERROR" errors

**Step 3: Implement**

In `src/executor.rs`, add a `last_correlated_error_occurred` local variable in `run()`:

```rust
let mut last_correlated_error_occurred = false;
```

Add a helper function (outside `run`):

```rust
fn apply_pauli_targets(state: &mut StabilizerState, targets: &[StimTarget]) -> Result<(), String> {
    for t in targets {
        match t {
            StimTarget::Pauli { qubit, basis, .. } => {
                let q = *qubit as usize;
                match basis {
                    PauliBasis::X => state.x_gate(q),
                    PauliBasis::Y => state.y_gate(q),
                    PauliBasis::Z => state.z_gate(q),
                }
            }
            _ => return Err("CORRELATED_ERROR targets must be Pauli".to_string()),
        }
    }
    Ok(())
}
```

Add match arms in the executor:

```rust
"CORRELATED_ERROR" | "E" => {
    let p = args.first().copied().unwrap_or(0.0);
    if p > 0.0 && rng.gen::<f64>() < p {
        apply_pauli_targets(&mut state, targets)?;
        last_correlated_error_occurred = true;
    } else {
        last_correlated_error_occurred = false;
    }
}
"ELSE_CORRELATED_ERROR" => {
    if !last_correlated_error_occurred {
        let p = args.first().copied().unwrap_or(0.0);
        if p > 0.0 && rng.gen::<f64>() < p {
            apply_pauli_targets(&mut state, targets)?;
            last_correlated_error_occurred = true;
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: all tests pass

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: CORRELATED_ERROR / ELSE_CORRELATED_ERROR"
```

---

### Task 3: PAULI_CHANNEL_1 / PAULI_CHANNEL_2

**Files:**
- Modify: `src/executor.rs`
- Modify: `tests/noise_phase3.rs`

**Context:**

`PAULI_CHANNEL_1(px, py, pz) q` applies X with prob `px`, Y with prob `py`, Z with prob `pz`, I with prob `1-px-py-pz`. Applied independently to each target qubit.

`PAULI_CHANNEL_2(p_ix, p_iy, p_iz, p_xi, p_xx, p_xy, p_xz, p_yi, p_yx, p_yy, p_yz, p_zi, p_zx, p_zy, p_zz) q0 q1` applies the corresponding two-qubit Pauli with the given probability. The remaining probability `1 - sum` gives II. Targets are qubit pairs.

The 15-probability order for `PAULI_CHANNEL_2` follows this enumeration of non-II Pauli pairs (first Pauli on qubit a, second on qubit b):
IX, IY, IZ, XI, XX, XY, XZ, YI, YX, YY, YZ, ZI, ZX, ZY, ZZ.

**Step 1: Write failing tests**

Append to `tests/noise_phase3.rs`:

```rust
#[test]
fn pauli_channel_1_deterministic_x() {
    // px=1, py=0, pz=0 → always X
    let m = run("PAULI_CHANNEL_1(1,0,0) 0\nM 0\n");
    assert_eq!(m, vec![true]);
}

#[test]
fn pauli_channel_1_deterministic_z() {
    // px=0, py=0, pz=1 → Z on |0⟩ = |0⟩
    let m = run("PAULI_CHANNEL_1(0,0,1) 0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn pauli_channel_1_no_error() {
    let m = run("PAULI_CHANNEL_1(0,0,0) 0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn pauli_channel_1_multi_qubit() {
    // Apply X to each qubit independently with p=1
    let m = run("PAULI_CHANNEL_1(1,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![true, true]);
}

#[test]
fn pauli_channel_2_deterministic_xx() {
    // p_xx=1 (index 4 in the 15 probs, 0-indexed), all others 0
    // Order: IX IY IZ XI XX XY XZ YI YX YY YZ ZI ZX ZY ZZ
    let m = run("PAULI_CHANNEL_2(0,0,0,0,1,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![true, true]);
}

#[test]
fn pauli_channel_2_deterministic_zi() {
    // p_zi=1 (index 11): Z on first qubit, I on second
    let m = run("PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,1,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]); // Z|0⟩=|0⟩
}

#[test]
fn pauli_channel_2_no_error() {
    let m = run("PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test noise_phase3 pauli_channel 2>&1 | tail -20`
Expected: "unsupported instruction" errors

**Step 3: Implement**

Add to `src/executor.rs`:

```rust
"PAULI_CHANNEL_1" => {
    let px = args.get(0).copied().unwrap_or(0.0);
    let py = args.get(1).copied().unwrap_or(0.0);
    let pz = args.get(2).copied().unwrap_or(0.0);
    for q in qubits(targets)? {
        let r: f64 = rng.gen();
        if r < px {
            state.x_gate(q);
        } else if r < px + py {
            state.y_gate(q);
        } else if r < px + py + pz {
            state.z_gate(q);
        }
    }
}
"PAULI_CHANNEL_2" => {
    let probs: Vec<f64> = (0..15).map(|i| args.get(i).copied().unwrap_or(0.0)).collect();
    let pairs = qubit_pairs(targets)?;
    for (a, b) in pairs {
        let r: f64 = rng.gen();
        let mut cumulative = 0.0;
        let mut chosen = None;
        // Order: IX IY IZ XI XX XY XZ YI YX YY YZ ZI ZX ZY ZZ
        let paulis: [(u8, u8); 15] = [
            (0, 1), (0, 2), (0, 3), // IX IY IZ
            (1, 0), (1, 1), (1, 2), (1, 3), // XI XX XY XZ
            (2, 0), (2, 1), (2, 2), (2, 3), // YI YX YY YZ
            (3, 0), (3, 1), (3, 2), (3, 3), // ZI ZX ZY ZZ
        ];
        for (i, &(pa, pb)) in paulis.iter().enumerate() {
            cumulative += probs[i];
            if r < cumulative {
                chosen = Some((pa, pb));
                break;
            }
        }
        if let Some((pa, pb)) = chosen {
            apply_pauli(&mut state, a, pa);
            apply_pauli(&mut state, b, pb);
        }
    }
}
```

The `apply_pauli` helper already exists (from DEPOLARIZE2 implementation).

**Step 4: Run tests**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: all tests pass

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: PAULI_CHANNEL_1 / PAULI_CHANNEL_2"
```

---

### Task 4: HERALDED_ERASE / HERALDED_PAULI_CHANNEL_1

**Files:**
- Modify: `src/executor.rs`
- Modify: `tests/noise_phase3.rs`

**Context:**

These channels push a **herald bit** to the measurement record (via `recorder.push()`), indicating whether an error was detected. This bit can then be referenced by `DETECTOR rec[-N]`.

**`HERALDED_ERASE(p) q`:** For each target qubit:
- With prob `1-p`: record `false` (no erasure), apply I.
- With prob `p`: record `true` (erasure detected), then apply a uniformly random Pauli from {I, X, Y, Z} (each with probability `p/4` of the total, or equivalently, given that erasure occurred, each Pauli is equally likely at 25%).

Equivalently: sample whether erasure occurs. If yes, push `true` and apply `X_ERROR(0.5)` + `Z_ERROR(0.5)` (which is the same as uniformly random I/X/Y/Z). If no, push `false`.

**`HERALDED_PAULI_CHANNEL_1(pi, px, py, pz) q`:** For each target qubit:
- With prob `1-pi-px-py-pz`: record `false`, apply I.
- With prob `pi`: record `true`, apply I (false positive herald).
- With prob `px`: record `true`, apply X.
- With prob `py`: record `true`, apply Y.
- With prob `pz`: record `true`, apply Z.

**Step 1: Write failing tests**

Append to `tests/noise_phase3.rs`:

```rust
#[test]
fn heralded_erase_no_noise() {
    // p=0 → never erases, herald is always false
    let instrs = parse_lines("HERALDED_ERASE(0) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // measurements: [herald=false, M=false]
    assert_eq!(out.measurements, vec![false, false]);
}

#[test]
fn heralded_erase_always() {
    // p=1 → always erases, herald is always true
    // Qubit gets random Pauli; measure might be 0 or 1
    let instrs = parse_lines("HERALDED_ERASE(1) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements[0], true); // herald bit
    // out.measurements[1] is random, just check length
    assert_eq!(out.measurements.len(), 2);
}

#[test]
fn heralded_erase_detector_sees_herald() {
    // Herald bit at rec[-2], measurement at rec[-1]
    let instrs = parse_lines("HERALDED_ERASE(1) 0\nM 0\nDETECTOR rec[-2]\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.detectors[0], true); // detector sees herald=true
}

#[test]
fn heralded_erase_multi_qubit() {
    // Each qubit gets its own herald bit
    let instrs = parse_lines("HERALDED_ERASE(1) 0 1\nM 0 1\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // 4 measurement bits: herald0, herald1, M0, M1
    assert_eq!(out.measurements.len(), 4);
    assert_eq!(out.measurements[0], true); // herald for qubit 0
    assert_eq!(out.measurements[1], true); // herald for qubit 1
}

#[test]
fn heralded_pauli_channel_1_no_noise() {
    // All probs 0 → no herald, no error
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0,0,0,0) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, false]);
}

#[test]
fn heralded_pauli_channel_1_deterministic_x() {
    // pi=0, px=1, py=0, pz=0 → herald + X
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0,1,0,0) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true, true]); // herald=true, X|0⟩→|1⟩
}

#[test]
fn heralded_pauli_channel_1_false_positive() {
    // pi=1 → herald fires but no Pauli applied
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(1,0,0,0) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true, false]); // herald=true, I|0⟩=|0⟩
}

#[test]
fn heralded_pauli_channel_1_deterministic_z() {
    // pz=1 → herald + Z (Z|0⟩=|0⟩)
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0,0,0,1) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true, false]); // herald=true, Z|0⟩=|0⟩
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test noise_phase3 heralded 2>&1 | tail -20`
Expected: "unsupported instruction" errors

**Step 3: Implement**

In `src/executor.rs`, add match arms:

```rust
"HERALDED_ERASE" => {
    let p = args.first().copied().unwrap_or(0.0);
    for q in qubits(targets)? {
        if p > 0.0 && rng.gen::<f64>() < p {
            recorder.push(true);
            match rng.gen_range(0u8..4) {
                1 => state.x_gate(q),
                2 => state.y_gate(q),
                3 => state.z_gate(q),
                _ => {} // I
            }
        } else {
            recorder.push(false);
        }
    }
}
"HERALDED_PAULI_CHANNEL_1" => {
    let pi = args.get(0).copied().unwrap_or(0.0);
    let px = args.get(1).copied().unwrap_or(0.0);
    let py = args.get(2).copied().unwrap_or(0.0);
    let pz = args.get(3).copied().unwrap_or(0.0);
    let total = pi + px + py + pz;
    for q in qubits(targets)? {
        let r: f64 = rng.gen();
        if r < total {
            recorder.push(true);
            let inner = r;
            if inner < pi {
                // I — false positive
            } else if inner < pi + px {
                state.x_gate(q);
            } else if inner < pi + px + py {
                state.y_gate(q);
            } else {
                state.z_gate(q);
            }
        } else {
            recorder.push(false);
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test noise_phase3 2>&1 | tail -20`
Expected: all tests pass

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass (including all Phase 1/2 tests)

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: HERALDED_ERASE / HERALDED_PAULI_CHANNEL_1"
```
