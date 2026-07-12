# Issue 488 Reference Sample Tree Design

Issue: #488
Date: 2026-07-12

## Context

Issue #487 is closed and merged into `master` by PR #504, so this work can
build on the current packed reference-sample path without changing it.
Issue #488 asks for an independently tested compressed representation of
reference measurement output, equivalent to Stim v1.15.0's
`ReferenceSampleTree`.

The source behavior to port is structural compression only: prefix bits,
suffix children, repetitions, deterministic structural equality,
decompression, size calculation, simplification, and factorization by periods
2, 3, and 5. Circuit execution, tableau comparison, repeat-cycle detection,
and public flat reference-sample API changes are out of scope.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because this is a backend Rust data-structure
  change.
- Clarifying questions: answered from issue #488, the merged #487 context, and
  the Stim v1.15.0 `reference_sample_tree` source.
- Recommended design: add a standalone `rstim::reference_sample_tree` module
  with the requested tree type and operations, plus focused integration tests.
- Design approval: accepted automatically because the issue gives exact method
  names, required fixtures, negative controls, and out-of-scope constraints.
- Spec review: this document is approved for planning after placeholder,
  consistency, and scope checks pass.

## Alternatives Considered

1. Add the tree inside `rstim/src/data_path.rs` and use it immediately during
   packed reference sampling. This is rejected because the issue explicitly
   keeps public flat reference-sample API and circuit execution changes out of
   scope.
2. Put the tree under `rstim/src/sim/` as simulator internals. This is rejected
   because the requested type is a compression data structure, not a tableau or
   frame simulator component.
3. Add a standalone `rstim::reference_sample_tree` module and integration test
   it independently. This is the chosen approach because it exposes the exact
   requested representation without disturbing sampler behavior.

## Chosen Design

Create `rstim/src/reference_sample_tree.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSampleTree {
    pub prefix_bits: Vec<bool>,
    pub suffix_children: Vec<ReferenceSampleTree>,
    pub repetitions: u64,
}
```

`Default` returns the empty tree with zero repetitions. Structural equality is
deterministic through derived `PartialEq` and `Eq`; equality is intentionally
structural, not decompressed-content equality.

The public methods are:

- `empty(&self) -> bool`
- `size(&self) -> usize`
- `decompress_into(&self, output: &mut Vec<bool>)`
- `simplified(&self) -> ReferenceSampleTree`
- `try_factorize(&mut self, period_factor: usize)`

`size` recursively sums prefix and child sizes for one body and multiplies by
`repetitions`, matching Stim's semantics. The implementation will use checked
conversion and multiplication so oversized `u64` repetitions fail loudly
instead of silently truncating on `usize` platforms.

`decompress_into` emits, for each repetition, the node prefix first and then
each child in original order. This is the fixture that catches reversed child
order.

`simplified` ports Stim v1.15.0's flatten/fuse logic:

- zero-repetition nodes disappear;
- non-empty prefixes become simple one-repetition leaf nodes during flattening;
- adjacent structurally identical children merge by adding repetitions;
- adjacent unrepeated leaf prefixes concatenate;
- repeated nodes with one fused child multiply that child's repetitions;
- repeated nodes with multiple fused children take the first leaf prefix as the
  parent payload when possible.

`try_factorize` only acts on nodes with no prefix whose child count is divisible
by the requested factor. It compares child slices structurally, shrinks to one
period, and multiplies `repetitions` by the factor. The tests call it with
2, 3, and 5 as requested.

## Tests

Add `rstim/tests/reference_sample_tree.rs` with fixtures that port the relevant
Stim v1.15.0 cases and the issue's required names:

- `decompresses_prefix_and_children_in_order`
- `identical_children_factor_into_repetitions`
- `nested_repetitions_preserve_flat_bits`
- `factorization_matches_stim_v1_15_cases`
- `size_matches_decompressed_length`

Additional tests cover structural equality, empty/default behavior, and
Stim-style simplification edge cases. The `(10)` repeated 50 times fixture must
have `size() == 100` and decompress to exactly fifty `10` pairs.

Focused verification:

```sh
cargo test -p rstim --test reference_sample_tree -- --nocapture
```

Final verification also runs:

```sh
cargo test
```

## Out Of Scope

This design does not execute circuits, compare tableau states, add compressed
reference sampling to `data_path`, or change the public flat reference-sample
API.

## Self-Review

The spec has no placeholders or contradictory requirements. It is scoped to one
data structure, one module export, and one integration test file. The method
list matches the issue interface, and the negative controls map directly to
the child-order decompression and non-identical-child factorization tests.
