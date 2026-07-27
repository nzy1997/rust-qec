# Issue 572 Perturbed-HGP Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add a test-enforced source-grounding decision record for `perturbed_hgp` that leaves the family explicitly unsupported until maintainers approve a unique construction.

**Architecture:** Keep the runtime crate unchanged. Add a Markdown decision record under `qec-code/doc/` and extend the existing deferred-contract integration test with string markers and source-tree guards that prevent callable perturbed-HGP stubs.

**Tech Stack:** Rust integration tests, Markdown documentation, existing `qec-code` family contract APIs, `cargo test`.

## Global Constraints

- Do not add callable runtime stubs, constructors, public specs, CLI routes, or source markers for `perturbed_hgp`.
- The document must record searched terminology, sources, candidate definitions, dispositions, provenance, license compatibility, and the final unsupported decision.
- The test must fail when the document omits the source grounding, perturbation-rule disposition, positive-fixture disposition, negative-fixture disposition, or final disposition decision.
- The verification command required by issue #572 is `cargo test -p qec-code --test deferred_contracts perturbed_hgp_contract_is_grounded_or_explicitly_unsupported -- --exact`.
- The Agent Desk run also requires `cargo test`.

---

## File Structure

- Modify `qec-code/tests/deferred_contracts.rs`: add the focused `perturbed_hgp` deferred-contract test and no-runtime-stub guard.
- Create `qec-code/doc/perturbed_hgp_contract.md`: source-grounding decision record with explicit unsupported status.
- Keep `qec-code/src/**` unchanged except for incidental formatting from tools; no runtime implementation belongs in this issue.

### Task 1: Add The Failing Deferred-Contract Test

**Files:**
- Modify: `qec-code/tests/deferred_contracts.rs`
- Test: `qec-code/tests/deferred_contracts.rs`

**Interfaces:**
- Consumes: existing `assert_contains`, `rust_sources`, `parse_css_construction_json`, `CssFamilySpec::callable_requested_family_ids`, and `RequestedFamilyId::PerturbedHgp`.
- Produces: `perturbed_hgp_contract_is_grounded_or_explicitly_unsupported`, which Task 2 must satisfy with the document.

- [x] **Step 1: Add the missing include constant**

Add this constant below the existing `CONTRACT` constant:

```rust
const PERTURBED_HGP_CONTRACT: &str = include_str!("../doc/perturbed_hgp_contract.md");
```

- [x] **Step 2: Add the source-tree guard helper**

Add this function after `assert_src_tree_does_not_define_callable_hyperbolic_5_5`:

```rust
fn assert_src_tree_does_not_define_callable_perturbed_hgp() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "fn perturbed_hgp",
        "pub fn perturbed_hgp",
        "perturbed_hgp_css_checks",
        "construct_perturbed_hgp",
        "PerturbedHgpSpec",
    ];
    for path in rust_sources(&src) {
        let text = fs::read_to_string(&path).expect("Rust source should be readable");
        for marker in forbidden {
            assert!(
                !text.contains(marker),
                "{} must not define callable perturbed_hgp runtime surface via {marker}",
                path.display()
            );
        }
    }
}
```

- [x] **Step 3: Add the focused test**

Add this test at the end of `qec-code/tests/deferred_contracts.rs`:

```rust
#[test]
fn perturbed_hgp_contract_is_grounded_or_explicitly_unsupported() {
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "# Perturbed HGP Source-Grounding Decision Record",
    );
    assert_contains(PERTURBED_HGP_CONTRACT, "contract_version: 1");
    assert_contains(PERTURBED_HGP_CONTRACT, "family_id = \"perturbed_hgp\"");
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "selection_status = \"explicitly_unsupported\"",
    );
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "disposition_decision = \"remain_deferred_unsupported\"",
    );
    assert_contains(PERTURBED_HGP_CONTRACT, "## Searched Terminology");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed HGP\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed_hgp\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed hypergraph product\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"cross swap\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Source Log");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:0903.0566");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2511.04634");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2501.09622");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2409.02193");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2601.08824");
    assert_contains(PERTURBED_HGP_CONTRACT, "Error Correction Zoo");
    assert_contains(PERTURBED_HGP_CONTRACT, "GitHub code search");
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "## Candidate Definitions And Dispositions",
    );
    assert_contains(PERTURBED_HGP_CONTRACT, "Standard hypergraph product");
    assert_contains(PERTURBED_HGP_CONTRACT, "Okada-Kasai cross-swap repair");
    assert_contains(PERTURBED_HGP_CONTRACT, "HGP optimization by random walks");
    assert_contains(PERTURBED_HGP_CONTRACT, "weight-reduced HGP");
    assert_contains(PERTURBED_HGP_CONTRACT, "active-orthogonality APM-LDPC");
    assert_contains(PERTURBED_HGP_CONTRACT, "Rejected");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Disposition Decision");
    assert_contains(PERTURBED_HGP_CONTRACT, "No construction is selected");
    assert_contains(PERTURBED_HGP_CONTRACT, "## No Selected Construction");
    assert_contains(PERTURBED_HGP_CONTRACT, "selected_primary_source = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "perturbation_rule = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "positive_fixture = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "negative_fixture = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "orthogonality_preservation_rule = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "pure_rust_input_schema = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Would-Be Selected Contract Requirements");
    assert_contains(PERTURBED_HGP_CONTRACT, "versioned pure-Rust input schema");
    assert_contains(PERTURBED_HGP_CONTRACT, "orthogonality-preservation rule");
    assert_contains(PERTURBED_HGP_CONTRACT, "one exact positive fixture");
    assert_contains(PERTURBED_HGP_CONTRACT, "one deliberately nonorthogonal negative fixture");
    assert_contains(PERTURBED_HGP_CONTRACT, "resource limits");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Provenance And License Compatibility");
    assert_contains(PERTURBED_HGP_CONTRACT, "Apache-2.0");
    assert_contains(PERTURBED_HGP_CONTRACT, "Creative Commons");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Follow-Up Scope");
    assert_contains(PERTURBED_HGP_CONTRACT, "No implementation issue is filed");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Deferred Runtime Status");
    assert_contains(PERTURBED_HGP_CONTRACT, "No callable runtime stub");

    assert!(
        !CssFamilySpec::callable_requested_family_ids().contains(&RequestedFamilyId::PerturbedHgp),
        "perturbed_hgp must remain absent from callable family IDs"
    );
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"perturbed_hgp","base":{"schema_version":1,"construction":"hypergraph_product","left":{"num_cols":2,"rows":[[0,1]]},"right":{"num_cols":2,"rows":[[0,1]]}},"operations":[]}"#
        ),
        Err(QecError::UnknownCssConstruction {
            construction: "perturbed_hgp".to_owned(),
        }),
        "perturbed_hgp inputs must remain non-callable until maintainers approve a unique construction"
    );
    assert_src_tree_does_not_define_callable_perturbed_hgp();
}
```

- [x] **Step 4: Run the focused test and verify RED**

Run:

```text
cargo test -p qec-code --test deferred_contracts perturbed_hgp_contract_is_grounded_or_explicitly_unsupported -- --exact
```

Expected before Task 2: FAIL because `qec-code/doc/perturbed_hgp_contract.md` does not exist or lacks required markers.

### Task 2: Add The Source-Grounding Decision Record

**Files:**
- Create: `qec-code/doc/perturbed_hgp_contract.md`
- Test: `qec-code/tests/deferred_contracts.rs`

**Interfaces:**
- Consumes: the exact marker strings from Task 1.
- Produces: a complete decision record that satisfies the deferred-contract test without adding runtime code.

- [x] **Step 1: Create `qec-code/doc/perturbed_hgp_contract.md`**

Write a Markdown document with:

```text
# Perturbed HGP Source-Grounding Decision Record

contract_version: 1
family_id = "perturbed_hgp"
selection_status = "explicitly_unsupported"
disposition_decision = "remain_deferred_unsupported"
```

Then include sections for searched terminology, source log, candidate
definitions and dispositions, disposition decision, no selected construction,
would-be selected contract requirements, provenance/license compatibility,
follow-up scope, and deferred runtime status.

- [x] **Step 2: Fill the search and source log**

Record at least these searched terms:

```text
"perturbed HGP"
"perturbed_hgp"
"perturbed hypergraph product"
"perturbation hypergraph product quantum LDPC"
"cross swap"
"H_X H_Z^T local repair"
"GitHub code search"
```

Record at least these sources:

```text
Error Correction Zoo, Hypergraph product (HGP) code, https://errorcorrectionzoo.org/c/hypergraph_product
Tillich and Zemor, Quantum LDPC codes with positive rate and minimum distance proportional to n^{1/2}, arXiv:0903.0566
Okada and Kasai, Random Construction of Quantum LDPC Codes, arXiv:2511.04634
Freire, Delfosse, and Leverrier, Optimizing hypergraph product codes with random walks, simulated annealing and reinforcement learning, arXiv:2501.09622
Tan and Stambler, Effective Distance of Higher Dimensional HGPs and Weight-Reduced Quantum LDPC Codes, arXiv:2409.02193
Kasai, Breaking the Orthogonality Barrier in Quantum LDPC Codes, arXiv:2601.08824
GitHub code search results for "perturbed_hgp", "perturbed HGP", and "cross swap" with "H_X" and "H_Z"
```

- [x] **Step 3: Fill candidate dispositions**

Include a table with each candidate and this disposition:

```text
Standard hypergraph product | Rejected | already implemented by #556; not perturbed
Okada-Kasai cross-swap repair | Rejected | generic CSS pair perturbation, not uniquely HGP-specific and not named perturbed HGP
HGP optimization by random walks | Rejected | optimizer/search over HGP instances, not a versioned perturbation constructor
weight-reduced HGP | Rejected | check-weight transformation/syndrome-extraction analysis, not a two-input perturbed-HGP constructor
active-orthogonality APM-LDPC | Rejected | APM-LDPC construction, not an HGP construction
```

- [x] **Step 4: Fill the unsupported decision and would-be requirements**

State exactly:

```text
No construction is selected.
selected_primary_source = none
perturbation_rule = none
positive_fixture = none
negative_fixture = none
orthogonality_preservation_rule = none
pure_rust_input_schema = none
No implementation issue is filed
No callable runtime stub
```

Also state that a future selected construction must provide:

```text
versioned pure-Rust input schema
orthogonality-preservation rule
one exact positive fixture
one deliberately nonorthogonal negative fixture
resource limits
```

- [x] **Step 5: Run the focused test and verify GREEN**

Run:

```text
cargo test -p qec-code --test deferred_contracts perturbed_hgp_contract_is_grounded_or_explicitly_unsupported -- --exact
```

Expected after Task 2: PASS.

### Task 3: Verify, Review, And Commit

**Files:**
- Modify: `qec-code/tests/deferred_contracts.rs`
- Create: `qec-code/doc/perturbed_hgp_contract.md`
- Modify: `docs/superpowers/plans/2026-07-27-issue-572-perturbed-hgp-contract.md`

**Interfaces:**
- Consumes: completed Tasks 1 and 2.
- Produces: verified branch commit ready for PR.

- [x] **Step 1: Run the required focused verification**

Run:

```text
cargo test -p qec-code --test deferred_contracts perturbed_hgp_contract_is_grounded_or_explicitly_unsupported -- --exact
```

Expected: PASS.

- [x] **Step 2: Run broader Agent Desk verification**

Run:

```text
cargo test
```

Expected: PASS.

- [x] **Step 3: Check no runtime files were modified**

Run:

```text
git diff --name-only HEAD
```

Expected files only:

```text
docs/superpowers/plans/2026-07-27-issue-572-perturbed-hgp-contract.md
qec-code/doc/perturbed_hgp_contract.md
qec-code/tests/deferred_contracts.rs
```

- [x] **Step 4: Commit the implementation**

Run:

```text
git add docs/superpowers/plans/2026-07-27-issue-572-perturbed-hgp-contract.md qec-code/doc/perturbed_hgp_contract.md qec-code/tests/deferred_contracts.rs
git commit -m "docs: record perturbed hgp contract decision"
```

Expected: commit succeeds.

- [x] **Step 5: Finish through Pull Request**

Use `superpowers:verification-before-completion` and
`superpowers:finishing-a-development-branch`. Choose "Push and create a Pull
Request" when asked.

## Plan Self-Review

Spec coverage: Task 1 covers the negative-control deferred-contract test and
no-runtime guard. Task 2 covers the source-grounding document, searched terms,
candidate dispositions, unsupported decision, provenance/license notes, and
future selected-contract requirements. Task 3 covers focused and broad
verification, commit, and PR creation.

Placeholder scan: passed. The plan contains no placeholders.

Type consistency: passed. The new test uses only existing imports already
available in `qec-code/tests/deferred_contracts.rs`.
