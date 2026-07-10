# Packed Inverse Tableau Storage Design

Issue: #455
Date: 2026-07-10

## Context

`StabilizerState` currently stores tableau X and Z bits as `Vec<Vec<bool>>` and phases as `Vec<u8>`. Issue #455 asks for a standalone packed inverse-tableau storage primitive under `rstim/src/sim/packed_inverse_tableau.rs`. This is not a request to route production sampling through the new representation.

The relevant local patterns are:

- simulator primitives live under `rstim/src/sim/` and are exported from `rstim/src/sim/mod.rs`;
- integration tests for simulator storage contracts live under `rstim/tests/`;
- `BitTable` already uses contiguous `Vec<u64>` row storage, but this issue needs two separate X/Z planes plus a packed sign plane and inverse-tableau identity semantics.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves the brainstorming approval gates:

- Visual companion: not offered because this is a storage layout change with no visual design question.
- Clarifying question answer: use the conservative public API that exposes read-only raw plane slices for testability and logical accessors for normal use.
- Approach approval: choose the standalone type approach because it matches the issue and avoids unrelated production routing.
- Spec approval: approved automatically because the issue body gives exact acceptance tests, out-of-scope boundaries, and layout formulas.

## Alternatives Considered

1. Standalone `PackedInverseTableau` with its own storage and tests. This is the chosen approach. It is narrow, matches the requested file path, and keeps this PR independent from Clifford evolution.
2. Generalize `BitTable` and compose two tables plus a packed sign helper. This reuses storage mechanics, but it spreads the inverse-tableau contract across multiple types and makes sign packing less explicit.
3. Replace `StabilizerState` internals immediately. This could yield performance benefits later, but it is out of scope and would risk behavioral regressions in measurement, reset, and Clifford evolution.

## Chosen Design

Add `PackedInverseTableau` in `rstim/src/sim/packed_inverse_tableau.rs` and export it through `rstim/src/sim/mod.rs`.

The type owns:

- `num_qubits: usize`
- `words_per_row: usize`, computed as `ceil(num_qubits / 64)`
- `x_plane: Vec<u64>` of length `2 * num_qubits * words_per_row`
- `z_plane: Vec<u64>` of length `2 * num_qubits * words_per_row`
- `signs: Vec<u64>` of length `ceil(2 * num_qubits / 64)`

`PackedInverseTableau::identity(num_qubits)` constructs the inverse-tableau identity:

- rows `0..num_qubits` contain X basis rows with `x(i, i) = true`;
- rows `num_qubits..2*num_qubits` contain Z basis rows with `z(num_qubits + i, i) = true`;
- all other X, Z, sign, and padding bits are zero;
- `num_qubits = 0` yields `words_per_row = 0`, `num_rows = 0`, and empty planes.

## Public API

Expose shape and raw storage accessors:

- `num_qubits(&self) -> usize`
- `num_rows(&self) -> usize`
- `words_per_row(&self) -> usize`
- `x_plane_words(&self) -> &[u64]`
- `z_plane_words(&self) -> &[u64]`
- `sign_words(&self) -> &[u64]`

Expose logical accessors:

- `x(&self, row: usize, qubit: usize) -> bool`
- `z(&self, row: usize, qubit: usize) -> bool`
- `sign_bit(&self, row: usize) -> bool`
- `canonical_phase(&self, row: usize) -> u8`, returning `0` for sign bit `0` and `2` for sign bit `1`

Expose sign mutation needed by the storage primitive tests and later users:

- `set_sign_bit(&mut self, row: usize, negative: bool)`
- `set_canonical_phase(&mut self, row: usize, phase: u8)`, accepting only phases `0` and `2`

Expose row storage primitives:

- `copy_row(&mut self, src: usize, dst: usize)`, copying X, Z, and sign from `src` to `dst`
- `xor_pauli_planes(&mut self, src: usize, dst: usize)`, applying `dst.x ^= src.x` and `dst.z ^= src.z` while leaving `dst` sign unchanged

Document `xor_pauli_planes` explicitly as a storage primitive, not phase-aware Pauli multiplication.

## Invariants And Errors

Index validation is centralized:

- row indexes must be `< 2 * num_qubits`;
- qubit indexes must be `< num_qubits`;
- invalid indexes panic through assertions with consistent messages.

Padding is deterministic:

- `final_word_mask()` returns the valid low-bit mask for the final row word;
- identity construction never sets padding bits;
- `copy_row` masks the destination row after copying;
- `xor_pauli_planes` masks the destination row after XORing.

For `num_qubits` divisible by 64, the padding mask is `u64::MAX`. For `num_qubits = 0`, no row operation is valid because there are no rows.

## Testing

Add `rstim/tests/packed_inverse_tableau_storage.rs` with the required tests:

- `identity_and_lengths_are_exact_for_0_1_64_65_130`
- `boundary_bits_63_64_129_map_to_expected_words`
- `packed_signs_round_trip_positive_and_negative`
- `row_copy_and_plane_xor_obey_contract`
- `unused_padding_bits_stay_zero`

The acceptance print line is `PASS packed inverse-tableau storage`.

The tests assert:

- exact plane lengths for `n = 0, 1, 64, 65, 130`;
- exact packed sign length, including `ceil(2*n/64)`;
- every identity X/Z/logical phase bit for the covered sizes;
- raw word mapping for qubits 63, 64, and 129;
- sign bit and canonical phase round trips for positive and negative rows;
- row copy copies sign and planes;
- plane XOR changes only X/Z planes and preserves the destination sign;
- all X/Z padding bits above qubit 129 remain zero for `n = 130`.

## Out Of Scope

This design does not implement Clifford evolution, phase-aware row multiplication, measurement/reset behavior, conversion from `StabilizerState`, or production sampler routing.
