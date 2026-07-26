# Directional CSS Constructor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a route-generated directional CSS family constructor for rectangular hardware tori.

**Architecture:** Put route parsing, torus reduction, layout validation, connectivity validation, and matrix generation in `qec-code/src/codes/directional.rs`. Route the public typed spec through `qec-code/src/family_contract.rs` so Rust API and `code css construct --spec` JSON use the same construction path.

**Tech Stack:** Rust 2024, serde, existing `qec-code` family contract, existing sparse-row CSS fixtures, existing GF(2) rank helpers.

## Global Constraints

- `NE2N` route support must use `P=[(0,1),(1,2),(3,2),(4,3)]`.
- The `8 x 6` square `NE2N` fixture must have `H_X[0]=[4,9,10,14]`, `H_Z[0]=[8,12,13,18]`, `n=24`, `m_x=m_z=12`, `rank_x=rank_z=11`, `k=2`, and exact component distances `d_x=d_z=3`.
- The `18 x 4` hex-compatible `NE3N` fixture must have `n=36`, `m_x=m_z=18`, `rank_x=rank_z=16`, `k=4`, and exact component distances `d_x=d_z=4`.
- Full checks must be stored in reviewed fixtures and verified orthogonal.
- Route parsing, coordinate reduction, qubit ordering, and metadata must be deterministic.
- Reject `NE2N` when `connectivity=hex`, a route/layout odd-overlap conflict, and a torus too small to avoid support collisions.
- This issue does not add circuits.

---

### Task 1: Red Tests For Directional Fixtures And Negative Controls

**Files:**
- Create: `qec-code/tests/directional.rs`
- Create: `qec-code/tests/fixtures/directional/square_ne2n_8x6.json`
- Create: `qec-code/tests/fixtures/directional/hex_ne3n_18x4.json`

**Interfaces:**
- Consumes: `construct_css`, `parse_css_construction_json`, `verify_css_orthogonality`, `CssFamilySpec`.
- Produces: the exact public behavior that the constructor must satisfy.

- [ ] **Step 1: Write the failing tests**

Add tests named exactly:

```rust
#[test]
fn directional_square_ne2n_matches_fixture() { /* load square fixture, construct, compare */ }

#[test]
fn directional_hex_ne3n_matches_fixture() { /* load hex fixture, construct, compare */ }

#[test]
fn directional_rejects_incompatible_routes() { /* hex NE2N, odd-overlap NE, small torus */ }
```

The helper must compute exact CSS component distance by enumerating binary
supports up to the fixture's expected distance and checking
`kernel(H_Z) \ row_span(H_X)` for `d_x` and `kernel(H_X) \ row_span(H_Z)` for
`d_z` with `qec_code::binary::try_in_row_span`.

- [ ] **Step 2: Run RED verification**

Run:

```bash
cargo test -p qec-code --test directional directional_square_ne2n_matches_fixture -- --exact
cargo test -p qec-code --test directional directional_hex_ne3n_matches_fixture -- --exact
cargo test -p qec-code --test directional directional_rejects_incompatible_routes -- --exact
```

Expected: fail to compile because the directional spec/types are not available.

- [ ] **Step 3: Commit red tests**

```bash
git add qec-code/tests/directional.rs qec-code/tests/fixtures/directional
git commit -m "test: pin directional css fixtures"
```

### Task 2: Directional Constructor Module

**Files:**
- Create: `qec-code/src/codes/directional.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/src/error.rs`

**Interfaces:**
- Consumes: tests from Task 1.
- Produces: `DirectionalCssSpec`, `DirectionalTorusSpec`, `DirectionalLayoutSpec`, `DirectionalAncillaCoset`, `DirectionalConnectivity`, `DirectionalCssChecks`, `build_directional_css_checks`, and `parse_directional_route_support`.

- [ ] **Step 1: Implement route and spec types**

Define serde-compatible public spec types and defaults. The default torus
shift is `0`, default layout is `X=(odd,even)` and `Z=(even,odd)`, and default
connectivity is `square`.

- [ ] **Step 2: Implement route parsing**

Parse `N`, `E`, `S`, `W` with positive decimal repetition suffixes and compute
support offsets with `Q_j = 2 * sum(previous displacements) + d_j`.

- [ ] **Step 3: Implement validation**

Reject invalid periods, parity-breaking vertical shifts, same-coset layouts,
malformed routes, duplicate infinite route supports, odd-overlap conflicts,
finite support collisions, conservative delta-vector torus collisions, and
hex requests whose normalized route is not in the paper's hex-compatible table.

- [ ] **Step 4: Implement matrix generation**

Order data qubits row-major over the fundamental hardware window, reducing
coordinates by `(period_x,0)` and `(vertical_period_x_shift,period_y)`. Generate
`H_X` and `H_Z` by row-major ancilla coset translation.

- [ ] **Step 5: Run GREEN verification for constructor tests**

Run the three Task 1 test commands. Expected: pass.

- [ ] **Step 6: Commit constructor**

```bash
git add qec-code/src/codes/directional.rs qec-code/src/codes/mod.rs qec-code/src/error.rs
git commit -m "feat: add directional css constructor"
```

### Task 3: Family Contract And CLI JSON Routing

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: `build_directional_css_checks` and directional spec types from Task 2.
- Produces: `CssFamilySpec::Directional`, JSON construction `"directional"`, deterministic normalized parameters, requested family id `directional`, and CLI construct coverage.

- [ ] **Step 1: Extend the family contract**

Add `Directional(DirectionalCssSpec)` to `CssFamilySpec`, include
`RequestedFamilyId::Directional` in `callable_requested_family_ids`, parse
versioned JSON construction `"directional"`, and route construction through
`build_directional_css_checks`.

- [ ] **Step 2: Add contract tests**

Update `planned_families_have_no_callable_stub` and add a deterministic
metadata/JSON lowering test for the `NE2N` square fixture spec.

- [ ] **Step 3: Add CLI test**

Use a temp JSON file with `construction:"directional"` and verify
`code css construct --spec <file> hx` returns the fixture `H_X` JSON.

- [ ] **Step 4: Run family and CLI verification**

Run:

```bash
cargo test -p qec-code --test family_contract directional
cargo test -p qec-code --test cli directional
```

Expected: pass.

- [ ] **Step 5: Commit routing**

```bash
git add qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/cli.rs
git commit -m "feat: route directional css family"
```

### Task 4: Final Verification And Review

**Files:**
- Modify only files needed for fixes found during verification or review.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run required focused tests**

```bash
cargo test -p qec-code --test directional directional_square_ne2n_matches_fixture -- --exact
cargo test -p qec-code --test directional directional_hex_ne3n_matches_fixture -- --exact
cargo test -p qec-code --test directional directional_rejects_incompatible_routes -- --exact
```

Expected: pass.

- [ ] **Step 2: Run crate and workspace verification**

```bash
cargo test -p qec-code
cargo test
```

Expected: pass.

- [ ] **Step 3: Request final code review**

Use `superpowers:requesting-code-review` with the merge-base against `origin/master`.
Fix Critical and Important findings, then rerun the affected tests.

- [ ] **Step 4: Finish the branch**

Use `superpowers:verification-before-completion` and
`superpowers:finishing-a-development-branch`. Choose "Push and create a Pull
Request" under the non-interactive Standing Answer Policy.

## Self-Review

Spec coverage: all issue deliverables and negative controls map to Tasks 1-4.
Placeholder scan: no placeholder markers remain. Type consistency: the
directional type and function names are introduced in Task 2 and consumed by
Task 3.
