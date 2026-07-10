# Issue 457 Packed Inverse Measurement And Reset Design

Issue: #457
Date: 2026-07-10

## Context

Issue #456 added `PackedInverseTableau` Clifford evolution for `H`, `S`, `S_DAG`, `X`, `Y`, `Z`, and directed `CX`, with canonical snapshots used only as an oracle-facing adapter. Issue #457 extends that packed inverse tableau with biased measurement and reset operations in Z, X, and Y bases.

The existing legacy oracle is `rstim::sim::tableau::StabilizerState`. Its noiseless reference measurement chooses `false` for genuinely random measurements, preserves deterministic `true` outcomes, collapses the tableau, and prepares positive eigenstates for reset operations. The packed implementation must match that behavior without materializing the full Boolean tableau.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves the Superpowers gates:

- Visual companion: not used because this is a backend Rust data-structure change.
- Clarifying questions: answered from issue #457 and the existing issue #456 implementation.
- Recommended design: add packed methods directly to `PackedInverseTableau`, test them with a new focused integration test, and leave general sampler routing out of scope.
- Design approval: accepted automatically because the issue gives exact operations, expected bits, negative controls, and verification commands.
- Spec review: this document is considered approved for planning under the non-interactive run policy after the placeholder, consistency, and scope checks pass.

## Alternatives Considered

1. Add packed measurement/reset methods to `PackedInverseTableau` and implement collapse through packed row algebra. This is the chosen approach because it satisfies the interface, keeps the packed backend self-contained, and avoids Boolean tableau materialization.
2. Route existing reference sampling through a packed backend. This is rejected because the issue explicitly says not to route general sampling through this backend.
3. Convert `PackedInverseTableau::canonical_snapshot()` into `Vec<Vec<bool>>`, run the legacy measurement algorithm, then rebuild packed storage. This is rejected because it materializes the full Boolean tableau and would weaken the negative controls.

## Chosen Design

`PackedInverseTableau` gets public biased operations:

- `measure_z_biased(q, inverted) -> bool`
- `measure_x_biased(q, inverted) -> bool`
- `measure_y_biased(q, inverted) -> bool`
- `measure_reset_z_biased(q, inverted) -> bool`
- `measure_reset_x_biased(q, inverted) -> bool`
- `measure_reset_y_biased(q, inverted) -> bool`
- `reset_z_biased(q)`
- `reset_x_biased(q)`
- `reset_y_biased(q)`

The boolean argument is the Stim `!` target inversion flag. It flips only the returned measurement bit and never changes the collapsed or reset state.

Z-basis measurement uses a packed equivalent of the legacy Aaronson-Gottesman collapse:

1. Build packed canonical rows from the raw inverse rows using the existing symplectic inversion relationship.
2. Find a stabilizer row `p` in `n..2n` whose packed X word has bit `q` set.
3. If such a row exists, the measurement is random. Use raw result `false`, multiply row `p` into every other packed row that has X on `q`, copy row `p` into its matching destabilizer row, and replace row `p` with positive `Z_q`.
4. If no such row exists, compute the deterministic result by multiplying stabilizer rows selected by destabilizer rows with X on `q` into a temporary packed `Z_q` row. A negative temporary row returns raw result `true`.
5. Convert the updated packed canonical rows back into raw inverse packed rows by symplectic transpose and packed phase evaluation.

X and Y measurements reuse the existing packed Clifford evolution for the same basis changes as the legacy reference:

- X basis: `H; measure Z; H`.
- Y basis: `S_DAG; H; measure Z; H; S`.

Measure-reset operations append the measurement bit first, then prepare the positive eigenstate in the corresponding basis. Reset-only operations prepare the positive eigenstate and append no bit. Reset state preparation uses the raw measurement bit before target inversion:

- `MR` / `R`: after Z measurement, apply `X` when the raw bit is `true`.
- `MRX` / `RX`: use the X-basis basis-change wrapper and apply the equivalent Z-basis correction inside that wrapper.
- `MRY` / `RY`: use the Y-basis basis-change wrapper and apply the equivalent Z-basis correction inside that wrapper.

## Packed Row Helpers

The implementation will keep row work in `u64` planes:

- A private packed canonical scratch representation holds X words, Z words, and sign bits for `2n` rows.
- Packed row multiplication uses the same phase convention as the existing inverse evaluator: `(-1)^r i^(x dot z) X^x Z^z`.
- Multiplication adds the source row exponent and the anticommutation crossing term `2 * (acc_z dot src_x)`, then XORs X/Z words.
- The scratch representation can set basis rows, copy rows, multiply one row into another, and evaluate a row from coefficient words.
- Conversion from inverse to canonical and canonical to inverse uses `M^{-1} = J M^T J` for binary rows, plus packed phase evaluation to set signs.

This allocates packed scratch rows but never allocates `Vec<Vec<bool>>` or a legacy `StabilizerState` inside the operation implementations.

## Testing

Add `rstim/tests/packed_inverse_tableau_measurement.rs`.

The primary test asserts the issue's known-answer cases exactly:

- `M 0` -> `[false]`
- `X 0; M 0` -> `[true]`
- `H 0; MX 0` -> `[false]`
- `H 0; Z 0; MX 0` -> `[true]`
- `H 0; S 0; MY 0` -> `[false]`
- `H 0; S_DAG 0; MY 0` -> `[true]`
- `X 0; MR 0; M 0` -> `[true, false]`
- `H 0; Z 0; MRX 0; MX 0` -> `[true, false]`
- `H 0; S_DAG 0; MRY 0; MY 0` -> `[true, false]`
- `X 0; R 0; M 0; RX 1; MX 1; RY 2; MY 2` -> `[false, false, false]`
- `H 0; CX 0 1; M 0 1` -> `[false, false]`
- `H 63; CX 63 64; M 63 64; H 64; CX 64 129; M 64 129` on 130 qubits -> four `false` bits

The supplementary differential test compares packed and legacy results plus canonical snapshots using seed `0x457`, 130 qubits, and a deterministic sequence containing every operation in scope: `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`, and `RY`, along with Clifford setup gates.

The acceptance test prints:

```text
PASS packed inverse measurement and reset
```

## Negative Controls

The tests must fail if:

- measurement methods return `false` for every result, because `X 0; M 0` expects `[true]`;
- `MR` skips reset, because `X 0; MR 0; M 0` would return `[true, true]`;
- final storage words are ignored, because the qubit-129 collapse case depends on the third word in 130-qubit storage;
- target inversion mutates the post-reset state instead of only the reported bit.

## Verification

Focused acceptance:

```sh
cargo test -p rstim --test packed_inverse_tableau_measurement -- --nocapture
```

Final Agent Desk verification:

```sh
cargo test
```

## Out Of Scope

This design does not route general sampling through the packed backend, implement noisy frame evolution, add loss-visible measurement variants, or replace the legacy reference sampler.
