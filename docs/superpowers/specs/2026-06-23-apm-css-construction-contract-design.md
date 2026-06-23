# APM CSS Construction Contract Design

Scope: GitHub issue #134, developer-facing construction contract for the
APM-CSS Table A1 instances used by `qec-code`.

## Context

Issue #132 added `qec-code/tests/fixtures/apm/table_a1_manifest.json` with the
source-grounded `P=96` and `P=192` Table A1 entries. Issue #133 will use that
manifest as the known-answer target for generated P=96 sparse matrices. Issue
#134 fills the gap between those fixture fields and the future Rust generator:
a cold implementer needs the algebra, dimensions, validation vocabulary, and
negative controls in one crate-facing note before adding construction code.

The local `drafts/construct_apm_css_code` reference clone named by the issue is
not present in this worktree. The implementation will therefore cite the
expected paths as provenance, but the concrete checked values come from the
merged #132 manifest and the arXiv pages for 2604.16209 and 2601.08824.

## Chosen Approach

Add the actual construction contract at `qec-code/doc/apm_css.md` and keep it
implementation-oriented:

- define `AffineMap { a, b, modulus }` as the Rust-side data shape;
- spell out `L2 = L / 2`, active block rows, `Delta`, and `Gamma`;
- define the affine commutation residual used by validator tests;
- derive `n`, `mx`, and `mz` for `P=96,J=3,L=12`;
- describe sparse-row output expectations for later `Hx` and `Hz` fixtures;
- link #132 as the manifest source and #133 as the known-answer fixture target;
- include a validation checklist and a documented invalid non-unit slope case.

Add a paired test named `apm_contract_doc_examples_compile` in
`qec-code/tests/code.rs`. The test will exercise the same smallest examples the
doc describes without introducing a public parser or generator API.

## Alternatives Considered

1. Put the note only under `docs/superpowers/specs/`.

   This satisfies the workflow artifact requirement, but it is too far from
   `qec-code` and its cargo test vocabulary. Future generator implementers
   would have to discover a process document instead of a crate note.

2. Put the note under `qec-code/doc/` and verify it with test-local helpers.

   This is the selected approach. It keeps the contract close to the crate,
   keeps examples synchronized with the merged manifest, and avoids exposing
   unstable production APIs.

3. Add production `AffineMap` and manifest parser types now.

   That would overstep #134. The issue asks for the construction contract and
   validation vocabulary only, not a generator or public parser surface.

## Test Contract

The focused verification command is:

```bash
cargo test -p qec-code apm_contract_doc_examples_compile -q
```

The test should pass only when:

- `P=96,J=3,L=12` maps to `n=1152,mx=288,mz=288`;
- at least one manifest `required_commuting_pairs` Gamma example has zero
  affine residual modulo the documented column-component modulus;
- at least one manifest `required_noncommuting_pairs` example has nonzero
  affine residual modulo the full Table A1 modulus;
- a non-unit slope such as `a=2` under `P=96` is rejected.

The broader Agent Desk gate remains:

```bash
cargo test
```

## Out Of Scope

- Native APM generator implementation.
- Sparse matrix fixture generation for #133.
- Public Rust API for APM manifest parsing.
- Regenerating or changing the #132 manifest schema.
