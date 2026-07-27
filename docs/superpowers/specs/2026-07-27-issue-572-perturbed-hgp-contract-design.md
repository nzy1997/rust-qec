# Issue 572 Perturbed-HGP Contract Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #572, Roadmap ID M4-02

## Summary

Issue #572 is a source-grounding task for the deferred `perturbed_hgp` family.
The family name is ambiguous: repository-local search, GitHub issue context,
and primary literature searches show a standard hypergraph-product
construction, several HGP variants or optimizers, and one closely related
cross-swap plus local-repair framework for generic CSS check pairs, but no
primary source that uniquely defines a construction named `Perturbed HGP`.

The selected design is therefore a conservative unsupported decision record.
Create `qec-code/doc/perturbed_hgp_contract.md` with searched terminology,
sources, candidate definitions, candidate dispositions, and an explicit
`selection_status = "explicitly_unsupported"` decision. Add a focused
deferred-contract test that accepts either a future complete selected contract
or this explicit unsupported decision, and proves no callable runtime surface is
introduced.

## Context

- Issue #552 closed with `perturbed_hgp` marked `disposition = deferred` and
  `availability = not_applicable` in the family manifest.
- Issue #556 closed with the standard generic hypergraph-product constructor
  from two classical binary parity-check matrices.
- The issue reference
  `docs/design/2026-07-26-qec-code-family-support.md` is absent in this
  checkout, matching prior local design notes that used checked-in
  `docs/superpowers/specs/2026-07-26-issue-552-qec-family-manifest-design.md`
  and `docs/superpowers/specs/2026-07-27-issue-556-hypergraph-product-css-design.md`
  as the local source of truth.
- `qec-code/tests/deferred_contracts.rs` already has the pattern for a deferred
  research contract plus no-runtime-stub guard.

## Approaches Considered

### 1. Explicit unsupported decision record plus negative-control test

Create the document as a decision record and add a test that requires the
search log, candidate disposition table, explicit unsupported status, follow-up
scope, license/provenance notes, and absence of callable runtime code.

Benefits:

- matches the issue warning against inventing incompatible mathematics;
- preserves the manifest's deferred status from #552;
- does not create a public API or runtime stub;
- leaves a test-enforced paper trail for a future maintainer to reopen with a
  precise source.

Cost:

- no implementation issue is filed because no unique construction is selected.

This is the selected approach.

### 2. Select the cross-swap plus local-repair CSS framework

Use Okada and Kasai's 2025 arXiv construction as a "perturbed HGP" by
initializing it from HGP checks and defining a pure-Rust ILP-free or ILP-backed
repair contract.

Benefits:

- has a concrete perturbation operation and orthogonality-repair rule;
- can start from HGP output.

Costs:

- the primary source describes generic orthogonal CSS check pairs, not a
  uniquely named HGP-specific construction;
- selecting an HGP initialization would be a repository invention;
- the source algorithm uses OR-Tools CP-SAT, which would need separate license
  and pure-Rust solver decisions.

This is rejected for issue #572.

### 3. Treat HGP optimizers or weight-reduction variants as perturbations

Use random-walk, simulated-annealing, reinforcement-learning, improved
HGP, cyclic HGP, or weight-reduced HGP literature as the intended
`perturbed_hgp`.

Benefits:

- these papers are HGP-adjacent and sometimes modify seeds, checks, or
  syndrome-extraction structure.

Costs:

- none provides the exact construction name or a single versioned input schema
  for `perturbed_hgp`;
- several are already separate family targets or operational optimizers rather
  than constructors.

This is rejected for issue #572.

## Contract Document

Create `qec-code/doc/perturbed_hgp_contract.md`.

Required markers:

- `# Perturbed HGP Source-Grounding Decision Record`
- `contract_version: 1`
- `family_id = "perturbed_hgp"`
- `selection_status = "explicitly_unsupported"`
- `disposition_decision = "remain_deferred_unsupported"`
- `## Searched Terminology`
- `## Source Log`
- `## Candidate Definitions And Dispositions`
- `## Disposition Decision`
- `## No Selected Construction`
- `## Would-Be Selected Contract Requirements`
- `## Provenance And License Compatibility`
- `## Follow-Up Scope`
- `## Deferred Runtime Status`

The document must record the exact literature and implementation-search terms,
cite the standard HGP definition, cite the closest perturbation source, explain
why each plausible candidate is accepted or rejected, and state that no
positive fixture, negative fixture, perturbation operation, pure-Rust input
schema, or follow-up implementation issue exists until maintainers approve a
unique construction.

## Test Contract

Modify `qec-code/tests/deferred_contracts.rs`.

Add:

- `const PERTURBED_HGP_CONTRACT: &str =
  include_str!("../doc/perturbed_hgp_contract.md");`
- `perturbed_hgp_contract_is_grounded_or_explicitly_unsupported`
- a source-tree guard that rejects callable perturbed-HGP runtime markers such
  as `construct_perturbed_hgp`, `PerturbedHgpSpec`,
  `perturbed_hgp_css_checks`, and `fn perturbed_hgp`.

The test should require the unsupported markers listed above. It should also
require the candidate source names and the explicit strings that prove a future
selected construction would need a primary source, perturbation rule, positive
fixture, negative fixture, orthogonality-preservation rule, versioned
pure-Rust input schema, resource limits, provenance/license review, and a
separately filed implementation issue.

## Verification

Run the required focused verification:

```text
cargo test -p qec-code --test deferred_contracts perturbed_hgp_contract_is_grounded_or_explicitly_unsupported -- --exact
```

Run the required broader verification from the Agent Desk instructions:

```text
cargo test
```

## Self-Review

Placeholder scan: passed. The design has no `TBD` or `TODO` placeholders.

Consistency check: passed. The design deliberately chooses unsupported status
because selecting a construction would guess public API and mathematics.

Scope check: passed. The change is one source-grounding document and one
deferred-contract test; no runtime code or public constructor surface is in
scope.
