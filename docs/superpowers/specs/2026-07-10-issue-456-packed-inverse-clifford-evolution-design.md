# Issue 456 Packed Inverse Clifford Evolution Design

Issue: #456
Date: 2026-07-10

## Context

Issue #455 added `PackedInverseTableau` as packed storage only: separate X/Z `Vec<u64>` planes, packed sign bits, identity construction, row copy, and plane XOR. Issue #456 extends that type with inverse-tableau Clifford evolution for `H`, `S`, `S_DAG`, `X`, `Y`, `Z`, and directed `CX`.

The legacy oracle is `rstim/src/sim/tableau.rs` at audited commit `47ffef302a8a471475a5b954a418880cd192c475`. This work may add one read-only snapshot accessor to `StabilizerState`, but must not alter the existing gate bodies.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves the Superpowers gates:

- Visual companion: not used because the design is a packed Rust data-structure change with no visual question.
- Clarifying questions: answered from the issue text and dependency #455.
- Recommended design: keep evolution inside `PackedInverseTableau`, add only a test-visible canonical snapshot adapter, and leave production sampler routing out of scope.
- Design approval: accepted automatically because the issue gives exact gates, tests, negative controls, and exclusions.

## Alternatives Considered

1. Packed inverse rows with packed row evaluation and Gaussian snapshot inversion. This is the chosen approach. It preserves the inverse-tableau storage contract and compares to the legacy oracle without expanding rows during gate evolution.
2. Convert packed rows to a legacy `StabilizerState` after each gate. This is simpler but violates the objective that packed inverse evolution must not expand rows into Boolean vectors.
3. Store a parallel forward tableau for snapshot output. This would make the adapter a cache read instead of an inversion of raw inverse rows, so it would not satisfy the normalization requirement.

## Chosen Design

`PackedInverseTableau` continues to store raw inverse images:

- row `q` is the packed image of `U^\dagger X_q U`;
- row `n + q` is the packed image of `U^\dagger Z_q U`;
- signs use the canonical tableau convention where sign bit `1` means phase `2`.

Gate methods update those inverse rows directly:

- `h(q)` swaps rows `q` and `n + q`;
- `s(q)` replaces row `q` with the packed image of `-Y_q` under the current inverse tableau;
- `s_dag(q)` replaces row `q` with the packed image of `Y_q`;
- `x_gate(q)` toggles row `n + q`;
- `z_gate(q)` toggles row `q`;
- `y_gate(q)` toggles both row `q` and row `n + q`;
- `cx(c, t)` replaces row `c` with the packed image of `X_c X_t` and row `n + t` with the packed image of `Z_c Z_t`.

The row evaluator uses Aaronson-Gottesman tableau phase accounting, matching the legacy gate formulas:

- a row represents `(-1)^r i^(x dot z) X^x Z^z`;
- multiplying packed rows accumulates `2 * (acc_z dot src_x)` for anticommutation crossings;
- evaluating a canonical input Pauli adds `2 * input_sign + (input_x dot input_z)` before converting the result back to a sign bit.

This keeps evolution packed: row products operate on `u64` words and only allocate temporary packed rows for affected outputs.

## Snapshot Interface

Add:

```rust
pub struct CanonicalTableauSnapshot {
    pub num_qubits: usize,
    pub x: Vec<Vec<bool>>,
    pub z: Vec<Vec<bool>>,
    pub phase: Vec<u8>,
}
```

`PackedInverseTableau::canonical_snapshot()` converts inverse rows into the legacy row-major representation by inverting the packed binary basis:

1. Treat the raw inverse rows as a symplectic matrix in `[X | Z]` row order.
2. Use the identity `M^{-1} = J M^T J` to read each forward row's coefficients directly from raw inverse columns, avoiding per-snapshot Gaussian elimination in the 4,096-gate acceptance loop.
3. Use those coefficients as the canonical forward row X/Z bits.
4. Evaluate the coefficient Pauli through the raw inverse tableau with zero input sign; the resulting basis sign is the forward canonical phase.

This is normalization and inversion, not raw row relabeling.

`StabilizerState` gets one `#[doc(hidden)]` read-only `canonical_snapshot()` accessor that clones the existing X/Z/phase vectors into the same snapshot struct. No legacy gate body changes.

## Testing

Add `rstim/tests/packed_inverse_tableau_clifford.rs` with the issue-required tests:

- `each_supported_gate_matches_pinned_legacy`
- `directed_cx_0_to_1_is_not_cx_1_to_0`
- `fixed_seed_sequences_match_after_every_gate`
- `packed_evolution_crosses_words_63_64_129`

The tests compare snapshots after every instruction, cover the 130-qubit word-boundary sequence, run 4,096 gates for seeds `0x455`, `0xC0FFEE`, and `0x5EED5EED`, and print `PASS packed inverse Clifford evolution`.

The oracle-integrity assertion strips the marked snapshot-accessor block and compares the current `tableau.rs` byte length plus FNV-1a checksum against the audited source at `47ffef302a8a471475a5b954a418880cd192c475`. This avoids requiring CI to fetch historical git objects and still fails if a legacy gate body changes.

## Out Of Scope

This design does not add measurement, reset, non-listed Clifford gates, production routing, or changes to legacy gate semantics.
