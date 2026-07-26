# Hyperbolic {5,5} Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an implementation-ready deferred contract for future pure-Rust hyperbolic `{5,5}` quotient support without adding a callable constructor.

**Architecture:** Keep the normative contract in `qec-code/doc/hyperbolic_5_5_contract.md`. Add one Rust integration test in `qec-code/tests/deferred_contracts.rs` that includes the document, checks the exact required markers and fixture values, and confirms `hyperbolic_5_5` is not listed as a callable family.

**Tech Stack:** Rust 2024 integration tests, `qec-code`, Markdown contract documentation, Cargo workspace tests.

## Global Constraints

- Contract document path is exactly `qec-code/doc/hyperbolic_5_5_contract.md`.
- Test file path is exactly `qec-code/tests/deferred_contracts.rs`.
- The contract must define `schema_version = 1`.
- The contract must define `construction = "hyperbolic_5_5_quotient"`.
- The future accepted v1 input is a supplied permutation quotient, not a subgroup enumerator.
- The contract must compare supplied permutation-quotient input and subgroup input.
- The contract must define flag-orbit enumeration using the Coxeter generators `r0`, `r1`, and `r2`.
- The Coxeter presentation is exactly `r0^2 = r1^2 = r2^2 = 1`, `(r0 r1)^5 = 1`, `(r1 r2)^5 = 1`, and `(r0 r2)^2 = 1`.
- Canonical ordering must be independent of hash-map iteration.
- The contract must define reconstruction of vertices, edges, faces, and boundary maps from quotient flag orbits.
- The contract must define validation for Coxeter relations, quotient transitivity, manifold incidence, orientability, torsion, and `boundary * boundary = 0`.
- The contract must define typed failure mode `InvalidCoxeterQuotient` with `failed_relation`.
- The negative quotient fixture must violate `(r0 r1)^5 = 1` and return `InvalidCoxeterQuotient`.
- The small stellated dodecahedron fixture must pin `V=12`, `E=30`, `F=12`, `[[30,8,3]]`, `m_x=m_z=12`, `rank_x=rank_z=11`, and check weights 5.
- The family cannot move to `supported` until fixture reconstruction passes under 5 seconds and 512 MiB in the standard test environment.
- No callable runtime stub, constructor, CLI route, or public `hyperbolic_5_5` API is added by this issue.
- Required verification commands:
  - `cargo test -p qec-code --test deferred_contracts hyperbolic_5_5_contract_is_complete_and_deferred -- --exact`
  - `cargo test -p qec-code`
  - `cargo test`

---

## File Structure

- Create `qec-code/tests/deferred_contracts.rs`: source-level deferred contract test, document marker checks, exact fixture checks, negative-control checks, and callable-runtime absence checks.
- Create `qec-code/doc/hyperbolic_5_5_contract.md`: normative research contract for future implementation issues.
- Keep this plan in `docs/superpowers/plans/2026-07-27-issue-571-hyperbolic-55-contract.md`.

### Task 1: Add the Deferred Contract Test

**Files:**
- Create: `qec-code/tests/deferred_contracts.rs`

**Interfaces:**
- Consumes: planned document at `qec-code/doc/hyperbolic_5_5_contract.md`.
- Consumes: `qec_code::family_contract::{CssFamilySpec, RequestedFamilyId}`.
- Produces: integration test `hyperbolic_5_5_contract_is_complete_and_deferred`.

- [ ] **Step 1: Write the failing integration test**

Create `qec-code/tests/deferred_contracts.rs`:

```rust
use std::fs;
use std::path::Path;

use qec_code::family_contract::{CssFamilySpec, RequestedFamilyId};

const CONTRACT: &str = include_str!("../doc/hyperbolic_5_5_contract.md");

fn assert_contains(haystack: &str, needle: &str) {
    assert!(haystack.contains(needle), "missing contract marker: {needle}");
}

fn assert_src_tree_does_not_define_callable_hyperbolic_5_5() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "fn hyperbolic_5_5",
        "pub fn hyperbolic_5_5",
        "hyperbolic_5_5_css_checks",
        "construct_hyperbolic_5_5",
        "Hyperbolic55Spec",
    ];
    for path in rust_sources(&src) {
        let text = fs::read_to_string(&path).expect("Rust source should be readable");
        for marker in forbidden {
            assert!(
                !text.contains(marker),
                "{} must not define callable hyperbolic_5_5 runtime surface via {marker}",
                path.display()
            );
        }
    }
}

fn rust_sources(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    collect_rust_sources(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_sources(root: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("source directory should be readable") {
        let entry = entry.expect("source directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}

#[test]
fn hyperbolic_5_5_contract_is_complete_and_deferred() {
    assert_contains(CONTRACT, "# Hyperbolic {5,5} Quotient Contract");
    assert_contains(CONTRACT, "contract_version: 1");
    assert_contains(CONTRACT, "schema_version = 1");
    assert_contains(CONTRACT, "construction = \"hyperbolic_5_5_quotient\"");
    assert_contains(CONTRACT, "## Input Contract");
    assert_contains(CONTRACT, "## Quotient Input Choices");
    assert_contains(CONTRACT, "permutation quotient");
    assert_contains(CONTRACT, "subgroup");
    assert_contains(CONTRACT, "## Coxeter Presentation");
    assert_contains(CONTRACT, "r0^2 = r1^2 = r2^2 = 1");
    assert_contains(CONTRACT, "(r0 r1)^5 = 1");
    assert_contains(CONTRACT, "(r1 r2)^5 = 1");
    assert_contains(CONTRACT, "(r0 r2)^2 = 1");
    assert_contains(CONTRACT, "## Flag-Orbit Enumeration");
    assert_contains(CONTRACT, "vertices = orbits of <r1, r2>");
    assert_contains(CONTRACT, "edges = orbits of <r0, r2>");
    assert_contains(CONTRACT, "faces = orbits of <r0, r1>");
    assert_contains(CONTRACT, "## Canonical Ordering");
    assert_contains(CONTRACT, "independent of hash-map iteration");
    assert_contains(CONTRACT, "## Boundary Maps");
    assert_contains(CONTRACT, "H_X = boundary_1");
    assert_contains(CONTRACT, "H_Z = transpose(boundary_2)");
    assert_contains(CONTRACT, "## Validation");
    assert_contains(CONTRACT, "quotient transitivity");
    assert_contains(CONTRACT, "manifold incidence");
    assert_contains(CONTRACT, "orientability");
    assert_contains(CONTRACT, "torsion");
    assert_contains(CONTRACT, "boundary * boundary = 0");
    assert_contains(CONTRACT, "## Typed Failure Modes");
    assert_contains(CONTRACT, "InvalidCoxeterQuotient");
    assert_contains(CONTRACT, "failed_relation");
    assert_contains(CONTRACT, "## Pure-Rust Algorithms");
    assert_contains(CONTRACT, "union-find");
    assert_contains(CONTRACT, "Todd-Coxeter");
    assert_contains(CONTRACT, "## Resource Limits");
    assert_contains(CONTRACT, "max_flags = 200000");
    assert_contains(CONTRACT, "5 seconds");
    assert_contains(CONTRACT, "512 MiB");
    assert_contains(CONTRACT, "## Fixture: Small Stellated Dodecahedron");
    assert_contains(CONTRACT, "V = 12");
    assert_contains(CONTRACT, "E = 30");
    assert_contains(CONTRACT, "F = 12");
    assert_contains(CONTRACT, "code = [[30,8,3]]");
    assert_contains(CONTRACT, "m_x = 12");
    assert_contains(CONTRACT, "m_z = 12");
    assert_contains(CONTRACT, "rank_x = 11");
    assert_contains(CONTRACT, "rank_z = 11");
    assert_contains(CONTRACT, "x_check_weight = 5");
    assert_contains(CONTRACT, "z_check_weight = 5");
    assert_contains(CONTRACT, "## Negative Quotient Fixture");
    assert_contains(CONTRACT, "violates `(r0 r1)^5 = 1`");
    assert_contains(CONTRACT, "failed_relation = \"(r0 r1)^5 = 1\"");
    assert_contains(CONTRACT, "## Split Decision");
    assert_contains(CONTRACT, "one implementation issue");
    assert_contains(CONTRACT, "quotient enumeration");
    assert_contains(CONTRACT, "cellulation");
    assert_contains(CONTRACT, "## Deferred Runtime Status");
    assert_contains(CONTRACT, "No callable runtime stub");

    assert!(
        !CssFamilySpec::callable_requested_family_ids().contains(&RequestedFamilyId::Hyperbolic55),
        "hyperbolic_5_5 must remain absent from callable family IDs"
    );
    assert_src_tree_does_not_define_callable_hyperbolic_5_5();
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test -p qec-code --test deferred_contracts hyperbolic_5_5_contract_is_complete_and_deferred -- --exact
```

Expected: FAIL because `qec-code/doc/hyperbolic_5_5_contract.md` does not exist.

- [ ] **Step 3: Commit the red test**

Run:

```bash
git add qec-code/tests/deferred_contracts.rs
git commit -m "test: add hyperbolic 5-5 deferred contract check"
```

### Task 2: Write the Hyperbolic {5,5} Quotient Contract

**Files:**
- Create: `qec-code/doc/hyperbolic_5_5_contract.md`

**Interfaces:**
- Consumes: test markers from `qec-code/tests/deferred_contracts.rs`.
- Produces: implementation-ready research contract for future supplied permutation quotient support.

- [ ] **Step 1: Add the contract document**

Create `qec-code/doc/hyperbolic_5_5_contract.md` with these required sections:

```markdown
# Hyperbolic {5,5} Quotient Contract

contract_version: 1

## Scope
## Input Contract
## Quotient Input Choices
## Coxeter Presentation
## Flag-Orbit Enumeration
## Canonical Ordering
## Boundary Maps
## Validation
## Typed Failure Modes
## Pure-Rust Algorithms
## Resource Limits
## Fixture: Small Stellated Dodecahedron
## Negative Quotient Fixture
## Split Decision
## Deferred Runtime Status
## References
```

The content must spell out every global constraint above using the exact
fixture numbers and negative-control failure relation.

- [ ] **Step 2: Run the focused test and verify GREEN**

Run:

```bash
cargo test -p qec-code --test deferred_contracts hyperbolic_5_5_contract_is_complete_and_deferred -- --exact
```

Expected: PASS.

- [ ] **Step 3: Commit the contract document**

Run:

```bash
git add qec-code/doc/hyperbolic_5_5_contract.md
git commit -m "docs: define hyperbolic 5-5 quotient contract"
```

### Task 3: Verify the Deferred Contract Branch

**Files:**
- Modify: `docs/superpowers/plans/2026-07-27-issue-571-hyperbolic-55-contract.md`

**Interfaces:**
- Consumes: completed contract document and test from Tasks 1 and 2.
- Produces: verified branch ready for review and PR.

- [ ] **Step 1: Run the focused acceptance command**

Run:

```bash
cargo test -p qec-code --test deferred_contracts hyperbolic_5_5_contract_is_complete_and_deferred -- --exact
```

Expected: PASS.

- [ ] **Step 2: Run the qec-code crate tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS.

- [ ] **Step 3: Run the workspace tests**

Run:

```bash
cargo test
```

Expected: PASS. If unrelated pre-existing warnings appear, record them without
changing unrelated code.

- [ ] **Step 4: Check the diff**

Run:

```bash
git diff --check origin/master..HEAD
git status --short
```

Expected: no whitespace errors and no unintended files.

- [ ] **Step 5: Commit plan checklist updates if needed**

Run:

```bash
git add docs/superpowers/plans/2026-07-27-issue-571-hyperbolic-55-contract.md
git commit -m "docs: record hyperbolic 5-5 implementation plan"
```

Only run this step if the plan checklist changed after execution and the
changes should be retained.

## Self-Review

Spec coverage: every issue acceptance criterion is mapped to Task 1 test
markers and Task 2 contract content. Placeholder scan: passed. Type
consistency: no public Rust types are added by this plan.
