# Quantum Tanner CSS Checks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build sparse CSS `Hx`/`Hz` row supports for validated quantum Tanner specs and verify the `toric_d4` acceptance fixture.

**Architecture:** Keep the implementation inside `qec-code/src/codes/quantum_tanner.rs`. Add one typed result struct, one validated-parts constructor, and one convenience constructor that composes existing spec, group, Cayley, and local-code helpers. Reuse `SparseRowsMatrix`, `CssCode::from_hx_hz`, and `compute_distance` rather than adding new formats or code paths.

**Tech Stack:** Rust 2024, existing `qec-code` GF(2), CSS, distance, and quantum Tanner helpers; `cargo test`.

## Global Constraints

- Keep the result compatible with existing `qec-code::css::SparseRowsMatrix` and `CssCode::from_hx_hz` paths.
- Avoid adding a new matrix serialization format.
- First acceptance target is `toric_d4` with `n=16`, `k=2`, exact distance `4`, and stabilizer weight `4`.
- Negative control is the invalid non-symmetric `A` fixture from `qec-code/tests/fixtures/quantum_tanner`.
- Do not add CLI flags, file import commands, `rsinter` integration, qTanner/qLDPC importers, random search, SmallGroup, GAP/Oscar, or Morgenstern/Ramanujan generation.

---

## File Structure

- Modify `qec-code/src/error.rs`: add `InvalidQuantumTannerCssConstruction { reason: String }`.
- Modify `qec-code/src/codes/quantum_tanner.rs`: add `QuantumTannerCssChecks`, public builder functions, incidence-to-sparse-row helpers, and consistency validation.
- Modify `qec-code/src/distance.rs`: make the non-ILP exact search enumerate Pauli candidates by increasing weight so `n=16`, `d=4` completes under the required test command.
- Modify `qec-code/tests/code.rs`: add the issue acceptance test and helper assertions.
- Modify `qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json`: make the negative fixture parseable by matching `h_a` width to the one-element invalid A set, so rejection reaches the intended non-symmetric generator check.

---

### Task 1: Acceptance Test And Fixture Negative Control

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json`

**Interfaces:**
- Consumes: planned `quantum_tanner_css_checks(&QuantumTannerSpec) -> Result<QuantumTannerCssChecks>`.
- Produces: failing acceptance coverage for CSS matrix generation, CSS orthogonality, distance, and catalog negative rejection.

- [ ] **Step 1: Make `invalid_non_symmetric_a` parseable for the intended rejection**

Change `qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json`:

```json
"h_a": [[1]]
```

Keep `h_b` as `[[1, 1]]`.

- [ ] **Step 2: Adjust catalog validation for the parseable negative fixture**

In `validate_quantum_tanner_local_codes`, keep the existing exact `[[1, 1]]` expectation only when `expected_widths` is `Some((2, 2))`. For the non-symmetric rejection path with `expected_widths == None`, only require nonempty GF(2) rows. This preserves strict positive fixture validation while allowing the catalog negative to reach generator symmetry validation.

- [ ] **Step 3: Add test imports**

Extend the quantum Tanner import list in `qec-code/tests/code.rs` to include:

```rust
quantum_tanner_css_checks,
```

Also import:

```rust
use qec_code::distance::compute_distance;
```

- [ ] **Step 4: Add CSS orthogonality helper**

Add this helper near `toric_d4_json_with`:

```rust
fn assert_sparse_css_orthogonal(num_cols: usize, hx: &[Vec<usize>], hz: &[Vec<usize>]) {
    for (x_index, x_row) in hx.iter().enumerate() {
        let x_support = x_row.iter().copied().collect::<std::collections::BTreeSet<_>>();
        for (z_index, z_row) in hz.iter().enumerate() {
            let overlap = z_row
                .iter()
                .filter(|support| x_support.contains(support))
                .count();
            assert_eq!(
                overlap % 2,
                0,
                "Hx row {x_index} and Hz row {z_index} have odd overlap in width {num_cols}"
            );
        }
    }
}
```

- [ ] **Step 5: Add failing acceptance test**

Add:

```rust
#[test]
fn quantum_tanner_toric_d4_generates_css_checks() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    let checks = quantum_tanner_css_checks(&spec).unwrap();

    assert_eq!(checks.num_cols, 16);
    assert_eq!(checks.hx.len(), 16);
    assert_eq!(checks.hz.len(), 16);
    for row in checks.hx.iter().chain(&checks.hz) {
        if !row.is_empty() {
            assert_eq!(row.len(), 4, "expected weight-4 stabilizer row, got {row:?}");
        }
    }
    assert_sparse_css_orthogonal(checks.num_cols, &checks.hx, &checks.hz);

    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz.clone())
        .unwrap()
        .to_dense_rows();
    let css = CssCode::from_hx_hz(hx, hz).unwrap();
    assert_eq!(css.code().num_logical_qubits(), 2);

    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 4);
    assert_eq!(distance.witness.weight(), 4);

    let invalid_non_symmetric_a = quantum_tanner_spec_from_json_str(include_str!(
        "fixtures/quantum_tanner/invalid_non_symmetric_a.json"
    ))
    .unwrap();
    assert!(matches!(
        quantum_tanner_css_checks(&invalid_non_symmetric_a).unwrap_err(),
        QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. }
    ));
}
```

- [ ] **Step 6: Verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_toric_d4_generates_css_checks -q
```

Expected: compile failure or unresolved import for `quantum_tanner_css_checks`.

---

### Task 2: Quantum Tanner CSS Constructor

**Files:**
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/codes/quantum_tanner.rs`

**Interfaces:**
- Consumes: `QuantumTannerSpec`, `ValidatedFiniteGroup`, `QuantumTannerCayleyComplex`, `QuantumTannerLocalCodeTensorDual`.
- Produces:

```rust
pub struct QuantumTannerCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

pub fn quantum_tanner_css_checks(spec: &QuantumTannerSpec) -> Result<QuantumTannerCssChecks>;

pub fn quantum_tanner_css_checks_from_validated_parts(
    spec: &QuantumTannerSpec,
    group: &ValidatedFiniteGroup,
    complex: &QuantumTannerCayleyComplex,
    local: &QuantumTannerLocalCodeTensorDual,
) -> Result<QuantumTannerCssChecks>;
```

- [ ] **Step 1: Add typed construction error**

In `QecError`, add:

```rust
#[error("invalid quantum Tanner CSS construction: {reason}")]
InvalidQuantumTannerCssConstruction { reason: String },
```

- [ ] **Step 2: Add imports**

In `qec-code/src/codes/quantum_tanner.rs`, add:

```rust
use crate::css::{CssCode, SparseRowsMatrix};
```

- [ ] **Step 3: Add result struct and public builders after Cayley structs**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}
```

Then add the two public functions before `parse_construction_mode`.

- [ ] **Step 4: Implement convenience builder**

`quantum_tanner_css_checks` should call, in order:

```rust
let group = validate_quantum_tanner_group_table(spec)?;
let complex = enumerate_quantum_tanner_cayley_faces(spec.construction_mode, &group)?;
let local = quantum_tanner_local_code_tensor_dual(spec)?;
quantum_tanner_css_checks_from_validated_parts(spec, &group, &complex, &local)
```

- [ ] **Step 5: Implement validated-parts builder**

The function should:

1. Match `spec.construction_mode`.
2. Validate local widths against `group.a_generators().len()` and `group.b_generators().len()`.
3. Validate every local sector row length equals `a_width * b_width`.
4. Build `hx` from `complex.x_incidence` and `local.x_sector_rows`.
5. Build `hz` from `complex.z_incidence` and `local.z_sector_rows`.
6. Validate each sparse matrix with `SparseRowsMatrix::new`.
7. Validate orthogonality with `CssCode::from_hx_hz`.
8. Return `QuantumTannerCssChecks { num_cols, hx, hz }`.

- [ ] **Step 6: Implement incidence helpers**

Add helpers:

```rust
fn css_construction_error(reason: impl Into<String>) -> QecError;

fn validate_local_tensor_rows(
    sector: &'static str,
    rows: &[Vec<u8>],
    expected_width: usize,
) -> Result<()>;

fn sparse_rows_from_local_incidence(
    sector: &'static str,
    group: &ValidatedFiniteGroup,
    local_rows: &[Vec<u8>],
    incidence: &[QuantumTannerLocalIncidence],
    num_cols: usize,
) -> Result<Vec<Vec<usize>>>;

fn local_incidence_grid_for_source(
    sector: &'static str,
    group: &ValidatedFiniteGroup,
    incidence: &[QuantumTannerLocalIncidence],
    source_vertex: usize,
    num_cols: usize,
) -> Result<Vec<Option<usize>>>;
```

Use coordinate index `a_index * group.b_generators().len() + b_index`, validate duplicate/missing coordinates, expected generator values, and face bounds.

- [ ] **Step 7: Run targeted test**

Run:

```bash
cargo test -p qec-code quantum_tanner_toric_d4_generates_css_checks -q
```

Expected: test reaches distance assertion; it may still be too slow until Task 3.

---

### Task 3: Practical Exact Distance For The Acceptance Fixture

**Files:**
- Modify: `qec-code/src/distance.rs`

**Interfaces:**
- Consumes: existing `compute_distance(&StabilizerCode)`.
- Produces: same API, but non-ILP exhaustive mode searches by increasing Pauli weight and returns the first non-stabilizer normalizer witness.

- [ ] **Step 1: Replace all-candidate collection with increasing-weight search**

In `compute_distance_via_exhaustive_search`, keep the zero-logical check in `compute_distance`, then call a new helper:

```rust
for weight in 1..=code.n() {
    if let Some(witness) = find_normalizer_witness_of_weight(code, weight)? {
        return Ok(DistanceResult {
            distance: weight,
            logical_class: classify_logical(&witness),
            witness,
        });
    }
}
Err(QecError::DistanceWitnessNotFound)
```

- [ ] **Step 2: Preserve current unsupported behavior**

Before the loop, keep a `usize` mask-width capability check:

```rust
let symplectic_bits = code.n().checked_mul(2).ok_or(...)?;
let _ = 1usize.checked_shl(symplectic_bits as u32).ok_or(...)?;
```

This preserves the current `n=32` unsupported tests.

- [ ] **Step 3: Add combination recursion helpers**

Implement:

```rust
fn find_normalizer_witness_of_weight(code: &StabilizerCode, weight: usize) -> Result<Option<Pauli>>;
fn search_supports(...);
fn search_pauli_assignments(...);
fn is_nontrivial_normalizer_witness(code: &StabilizerCode, candidate: &Pauli) -> Result<bool>;
```

For each chosen qubit support, assign one of `(x,z) = (1,0), (0,1), (1,1)` to each selected qubit.

- [ ] **Step 4: Run existing distance tests**

Run:

```bash
cargo test -p qec-code --test logical_distance -q
```

Expected: all tests pass, including the no-ILP `n=32` unsupported test.

- [ ] **Step 5: Run acceptance test**

Run:

```bash
cargo test -p qec-code quantum_tanner_toric_d4_generates_css_checks -q
```

Expected: all assertions pass with exact distance `4`.

---

### Task 4: Verification And Review

**Files:**
- No planned source edits unless verification finds issues.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

- [ ] **Step 2: Required targeted verification**

Run:

```bash
cargo test -p qec-code quantum_tanner_toric_d4_generates_css_checks -q
```

- [ ] **Step 3: Required broad verification**

Run:

```bash
cargo test
```

- [ ] **Step 4: Inspect diff**

Run:

```bash
git diff --stat
git diff -- qec-code/src/codes/quantum_tanner.rs qec-code/src/distance.rs qec-code/src/error.rs qec-code/tests/code.rs qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json
```

Confirm the diff stays inside the issue scope.

- [ ] **Step 5: Commit implementation**

Run:

```bash
git add docs/superpowers/specs/2026-06-25-quantum-tanner-css-checks-design.md docs/superpowers/plans/2026-06-25-quantum-tanner-css-checks.md qec-code/src/codes/quantum_tanner.rs qec-code/src/distance.rs qec-code/src/error.rs qec-code/tests/code.rs qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json
git commit -m "feat: build quantum tanner css checks"
```

- [ ] **Step 6: Create PR**

Use the finishing-a-development-branch workflow. Choose "Push and create a Pull Request" because it is the required non-interactive Agent Desk policy.
