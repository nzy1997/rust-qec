# Issue 489 Fold Periodic Reference Sampling Repeats Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect exact packed-tableau repeat cycles during reference sampling, compress repeated output with `ReferenceSampleTree`, and still return the existing flat `Vec<bool>`.

**Architecture:** Change the packed reference builder from direct flat accumulation to tree accumulation plus final decompression. Long repeats record exact cloned `PackedInverseTableau` loop-boundary states, wrap detected period output in `ReferenceSampleTree`, skip whole cycles, and execute any remainder normally.

**Tech Stack:** Rust 2024 Cargo workspace, `rstim` integration tests, existing `ReferenceSampleTree`, existing Python `profile_reference_build.py`, no new dependencies.

## Global Constraints

- Repeats below 10 execute normally and must not skip iterations.
- Longer supported repeats compare exact packed inverse-tableau state at loop boundaries; any hash acceleration must be followed by exact equality.
- Measurement output for the detected period is stored in `ReferenceSampleTree` and decompressed only at the final flat `Vec<bool>` API boundary.
- Nested supported repeats fold recursively.
- Existing packed-path fallback rules remain unchanged.
- Do not fold legacy-fallback circuits or add measurement-record feedback support.
- Negative control `REPEAT 99 { X 0 } M 0` must return final bit `1`.
- The surface fixture reference must remain 12,121 zero bits with packed-byte digest `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`.
- Expected profile output is exactly `PASS reference phase profile batches=5 canonical=0 transposed=2 pivots=120 executed_repeats=1 skipped_repeats=98 bits=12121`.

---

### Task 1: Add Repeat-Aware Tests And Telemetry Expectations

**Files:**
- Create: `rstim/tests/repeat_aware_reference_sample.rs`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`

**Interfaces:**
- Consumes: `rstim::data_path::build_reference_sample_with_decision`, `ReferenceBuildPhaseCounters`, and `rstim::parser::parse_lines`.
- Produces: red tests requiring `executed_repeat_iterations`, `skipped_repeat_iterations`, folded surface fixture counters, and the final profile pass line.

- [ ] **Step 1: Write the failing Rust integration test**

Create `rstim/tests/repeat_aware_reference_sample.rs`:

```rust
use rstim::data_path::{ReferenceBuildPhaseCounters, ReferenceSampleDecision, build_reference_sample_with_decision};
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use sha2::{Digest, Sha256};

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);
const SURFACE_DIGEST: &str = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d";

fn parse_circuit(source: &str) -> Vec<StimInstr> {
    parse_lines(source).expect("test circuit parses")
}

fn build(source: &str) -> (Vec<bool>, ReferenceBuildPhaseCounters) {
    let result = build_reference_sample_with_decision(&parse_circuit(source))
        .expect("reference sample builds");
    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    (result.bits, result.phase_counters)
}

fn pack_b8(bits: &[bool]) -> Vec<u8> {
    let mut packed = vec![0_u8; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().enumerate() {
        if *bit {
            packed[index / 8] |= 1 << (index % 8);
        }
    }
    packed
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn period_one_repeat_executes_once_and_skips_rest() {
    let (bits, counters) = build("REPEAT 12 {\n  M 0\n}\n");
    assert_eq!(bits, vec![false; 12]);
    assert_eq!(counters.measurement_reset_batches, 1);
    assert_eq!(counters.expanded_repeat_iterations, 12);
    assert_eq!(counters.executed_repeat_iterations, 1);
    assert_eq!(counters.skipped_repeat_iterations, 11);
}

#[test]
fn period_two_repeat_stores_period_output_before_skipping() {
    let (bits, counters) = build("REPEAT 12 {\n  X 0\n  M 0\n}\n");
    let expected: Vec<bool> = (0..12).map(|index| index % 2 == 0).collect();
    assert_eq!(bits, expected);
    assert_eq!(counters.measurement_reset_batches, 2);
    assert_eq!(counters.expanded_repeat_iterations, 12);
    assert_eq!(counters.executed_repeat_iterations, 2);
    assert_eq!(counters.skipped_repeat_iterations, 10);
}

#[test]
fn short_repeats_below_ten_execute_normally() {
    let (bits, counters) = build("REPEAT 9 {\n  M 0\n}\n");
    assert_eq!(bits, vec![false; 9]);
    assert_eq!(counters.measurement_reset_batches, 9);
    assert_eq!(counters.expanded_repeat_iterations, 9);
    assert_eq!(counters.executed_repeat_iterations, 9);
    assert_eq!(counters.skipped_repeat_iterations, 0);
}

#[test]
fn nested_long_repeats_fold_recursively_inside_short_parent() {
    let (bits, counters) = build("REPEAT 2 {\n  REPEAT 12 {\n    M 0\n  }\n}\n");
    assert_eq!(bits, vec![false; 24]);
    assert_eq!(counters.measurement_reset_batches, 2);
    assert_eq!(counters.expanded_repeat_iterations, 26);
    assert_eq!(counters.executed_repeat_iterations, 4);
    assert_eq!(counters.skipped_repeat_iterations, 22);
}

#[test]
fn state_alternating_empty_period_is_not_folded_by_bits_only() {
    let (bits, counters) = build("REPEAT 99 {\n  X 0\n}\nM 0\n");
    assert_eq!(bits, vec![true]);
    assert_eq!(counters.measurement_reset_batches, 1);
    assert_eq!(counters.expanded_repeat_iterations, 99);
    assert_eq!(counters.executed_repeat_iterations, 3);
    assert_eq!(counters.skipped_repeat_iterations, 96);
}

#[test]
fn surface_fixture_skips_periodic_reference_rounds_and_preserves_digest() {
    let result = build_reference_sample_with_decision(&parse_circuit(SURFACE_D11_R100))
        .expect("surface reference sample builds");
    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    assert_eq!(result.bits.len(), 12_121);
    assert!(result.bits.iter().all(|bit| !*bit));
    assert_eq!(sha256_hex(&pack_b8(&result.bits)), SURFACE_DIGEST);

    let counters = result.phase_counters;
    assert_eq!(counters.measurement_reset_batches, 5);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.direct_inverse_batches, 5);
    assert_eq!(counters.transposed_collapse_batches, 2);
    assert_eq!(counters.collapse_pivots, 120);
    assert_eq!(counters.expanded_repeat_iterations, 99);
    assert_eq!(counters.executed_repeat_iterations, 1);
    assert_eq!(counters.skipped_repeat_iterations, 98);
    assert_eq!(counters.measurement_bits, 12_121);
}
```

- [ ] **Step 2: Update Python profile tests to expect new counters**

In `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`,
change `DEFAULT_COUNTERS` to include:

```python
"executed_repeat_iterations": 1,
"skipped_repeat_iterations": 98,
```

Change the pass-line assertion to:

```python
"PASS reference phase profile batches=103 canonical=0 transposed=2 pivots=120 executed_repeats=1 skipped_repeats=98 bits=12121"
```

The fake worker keeps `measurement_reset_batches=103` because this unit test
only validates formatting and schema, not the real surface fixture.

- [ ] **Step 3: Run red tests**

Run:

```sh
cargo test -p rstim --test repeat_aware_reference_sample -- --nocapture
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Expected: Rust fails to compile because `executed_repeat_iterations` and
`skipped_repeat_iterations` do not exist. Python fails because the profile
schema does not validate or print the new fields.

### Task 2: Implement Tree Accumulation, Exact Cycle Detection, And Counters

**Files:**
- Modify: `rstim/src/data_path.rs`
- Modify: `rstim/tests/packed_reference_routing.rs`
- Modify: `rstim/tests/rstim_reference_build_worker.rs`

**Interfaces:**
- Consumes: `ReferenceSampleTree` from #488 and `PackedInverseTableau: Clone + PartialEq + Eq`.
- Produces: packed-reference construction that skips long exact cycles and exposes new serialized counter fields.

- [ ] **Step 1: Add counter fields and helper functions**

In `ReferenceBuildPhaseCounters`, add the fields before `measurement_bits`:

```rust
pub executed_repeat_iterations: usize,
pub skipped_repeat_iterations: usize,
```

Add helper constants and functions in `rstim/src/data_path.rs` near
`packed_reference_instrs`:

```rust
const REPEAT_FOLD_THRESHOLD: u64 = 10;

fn saturating_usize_from_u64(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn add_repeat_counter(slot: &mut usize, value: u64) {
    *slot = slot.saturating_add(saturating_usize_from_u64(value));
}

fn append_tree_bits(tree: &mut ReferenceSampleTree, bits: Vec<bool>) {
    if bits.is_empty() {
        return;
    }
    if tree.suffix_children.is_empty() {
        tree.prefix_bits.extend(bits);
    } else {
        tree.suffix_children.push(ReferenceSampleTree {
            prefix_bits: bits,
            suffix_children: Vec::new(),
            repetitions: 1,
        });
    }
}

fn append_tree_child(tree: &mut ReferenceSampleTree, child: ReferenceSampleTree) {
    if !child.empty() {
        tree.suffix_children.push(child);
    }
}
```

Also import the tree:

```rust
use crate::reference_sample_tree::ReferenceSampleTree;
```

- [ ] **Step 2: Decompress the tree at the public API boundary**

Change `build_packed_reference_sample` so it builds a tree and then
decompresses:

```rust
let tree = packed_reference_instrs(tableau: ..., instrs: ..., counters: ...)?;
let mut measurements = Vec::with_capacity(tree.size());
tree.decompress_into(&mut measurements);
Ok((measurements, counters))
```

The exact code should construct `tableau` and `counters` as today, call the
new tree-returning helper, and return the decompressed vector.

- [ ] **Step 3: Replace flat instruction recursion with tree recursion**

Replace `packed_reference_instrs` with:

```rust
fn packed_reference_instrs(
    tableau: &mut PackedInverseTableau,
    instrs: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                let mut bits = Vec::new();
                packed_reference_op(tableau, &mut bits, name, targets, counters)?;
                append_tree_bits(&mut tree, bits);
            }
            StimInstr::Repeat { count, body } => {
                add_repeat_counter(&mut counters.expanded_repeat_iterations, *count);
                let child = packed_reference_repeat(tableau, *count, body, counters)?;
                append_tree_child(&mut tree, child);
            }
        }
    }
    Ok(tree.simplified())
}
```

The existing `packed_reference_op` signature can remain unchanged because it
already writes only the bits produced by one operation into a `Vec<bool>`.

- [ ] **Step 4: Add exact repeat-cycle detection**

Add:

```rust
fn packed_reference_repeat(
    tableau: &mut PackedInverseTableau,
    count: u64,
    body: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    if count < REPEAT_FOLD_THRESHOLD {
        return packed_reference_repeat_without_skip(tableau, count, body, counters);
    }

    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    let mut seen = vec![(tableau.clone(), 0_u64, 0_usize)];
    let mut iteration = 0_u64;

    while iteration < count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
        iteration += 1;

        if let Some((_, previous_iteration, child_start)) =
            seen.iter().find(|(state, _, _)| state == tableau).cloned()
        {
            let period = iteration - previous_iteration;
            let remaining = count - iteration;
            let whole_cycles = remaining / period;
            if whole_cycles > 0 {
                let total_period_repetitions = whole_cycles + 1;
                let period_children = tree.suffix_children[child_start..].to_vec();
                tree.suffix_children.truncate(child_start);
                append_tree_child(
                    &mut tree,
                    ReferenceSampleTree {
                        prefix_bits: Vec::new(),
                        suffix_children: period_children,
                        repetitions: total_period_repetitions,
                    }
                    .simplified(),
                );
                let skipped = whole_cycles * period;
                add_repeat_counter(&mut counters.skipped_repeat_iterations, skipped);
                iteration += skipped;
            }
            break;
        }

        seen.push((tableau.clone(), iteration, tree.suffix_children.len()));
    }

    while iteration < count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
        iteration += 1;
    }

    Ok(tree.simplified())
}

fn packed_reference_repeat_without_skip(
    tableau: &mut PackedInverseTableau,
    count: u64,
    body: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    for _ in 0..count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
    }
    Ok(tree.simplified())
}
```

- [ ] **Step 5: Update existing counter expectations**

In `rstim/tests/packed_reference_routing.rs`, update the surface fixture
expectations to:

```rust
assert_eq!(counters.measurement_reset_batches, 5);
assert_eq!(counters.direct_inverse_batches, 5);
assert_eq!(counters.expanded_repeat_iterations, 99);
assert_eq!(counters.executed_repeat_iterations, 1);
assert_eq!(counters.skipped_repeat_iterations, 98);
```

In `phase_counters_distinguish_deterministic_and_collapsing_measurements`,
assert both new fields are `0` for non-repeat circuits.

In `rstim/tests/rstim_reference_build_worker.rs`, add assertions for the new
fields. The single-measurement fixture has both `0`; the surface fixture has
`executed_repeat_iterations == 1` and `skipped_repeat_iterations == 98`.

- [ ] **Step 6: Run focused Rust tests**

Run:

```sh
cargo test -p rstim --test repeat_aware_reference_sample -- --nocapture
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
```

Expected: all pass.

### Task 3: Update Profile Schema, Run Issue Verification, Commit

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/profile_reference_build.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`

**Interfaces:**
- Consumes: serialized `ReferenceBuildPhaseCounters` from the worker.
- Produces: profile validation and output compatible with issue #489.

- [ ] **Step 1: Validate the new counter fields**

In `profile_reference_build.py`, update `COUNTER_KEYS` to include:

```python
"executed_repeat_iterations",
"skipped_repeat_iterations",
```

Keep `expanded_repeat_iterations` in the JSON schema for compatibility.

- [ ] **Step 2: Print the issue's expected pass line**

Replace the final `print` format with:

```python
print(
    "PASS reference phase profile "
    f"batches={counters['measurement_reset_batches']} "
    f"canonical={counters['canonical_materializations']} "
    f"transposed={counters['transposed_collapse_batches']} "
    f"pivots={counters['collapse_pivots']} "
    f"executed_repeats={counters['executed_repeat_iterations']} "
    f"skipped_repeats={counters['skipped_repeat_iterations']} "
    f"bits={counters['measurement_bits']}"
)
```

Do not print `canonical_writebacks` or `expanded_repeat_iterations` in the pass
line.

- [ ] **Step 3: Run issue verification**

Run:

```sh
cargo test -p rstim --test repeat_aware_reference_sample -- --nocapture
cargo build --release -p rstim --bin rstim_reference_build_worker
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --worker target/release/rstim_reference_build_worker \
  --out /tmp/rstim-repeat-aware-profile.json
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
cargo test
```

Expected profile stdout:

```text
PASS reference phase profile batches=5 canonical=0 transposed=2 pivots=120 executed_repeats=1 skipped_repeats=98 bits=12121
```

- [ ] **Step 4: Commit implementation**

Run:

```sh
git add docs/superpowers/plans/2026-07-12-issue-489-fold-periodic-reference-sampling-repeats.md \
  rstim/src/data_path.rs \
  rstim/tests/repeat_aware_reference_sample.rs \
  rstim/tests/packed_reference_routing.rs \
  rstim/tests/rstim_reference_build_worker.rs \
  benchmarks/rstim_vs_stim_simulator/profile_reference_build.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py
git commit -m "feat: fold periodic reference sampling repeats"
```

## Self-Review

Spec coverage: all issue requirements map to Task 1 tests, Task 2 packed
repeat folding, or Task 3 profile verification. Placeholder scan: no `TBD`,
`TODO`, or unspecified "handle edge cases" instructions remain. Type
consistency: new counter fields are named `executed_repeat_iterations` and
`skipped_repeat_iterations` consistently across Rust serialization, Python
validation, and profile formatting.
