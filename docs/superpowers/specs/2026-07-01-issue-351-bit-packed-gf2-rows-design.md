# Issue 351 Bit-Packed GF(2) Rows Design

## Context

Issue #351 asks for crate-private bit-packed GF(2) row primitives that preserve
the dense `Vec<u8>` row semantics already used by `qec-code/src/gf2.rs`.
Adjacent issue #346 introduced `RandomWindowKernelWorkspace`, so this issue
should only add the row layer needed by later random-window kernel and span
work. It must not change random-window sampling, dense helper APIs, CLI output,
or benchmark behavior.

Live GitHub issue lookup is blocked in the Agent Desk sandbox by the configured
proxy, so this design uses the complete issue body supplied by Agent Desk and
the merged local #346 Superpowers design/plan as context.

## Design Options

1. Add one crate-private `BitPackedRow` helper in `qec-code/src/gf2.rs`.
   Store bits in little-endian `u64` words with explicit logical width, validate
   dense input before packing, and mask the final word after construction and
   XOR. This keeps the first implementation small, dependency-free, and close
   to the existing dense helpers and tests.

2. Add a separate `qec-code/src/gf2_bitpacked.rs` module. This would isolate
   code physically, but the current GF(2) surface is compact and the tests need
   existing private validation helpers. A new module would add wiring without a
   clear ownership benefit yet.

3. Replace random-window kernel generation with bit-packed rows now. This may
   be the eventual optimization, but issue #351 explicitly keeps algorithm
   changes out of scope.

Chosen approach: option 1. It gives later random-window work a tested row
primitive while preserving dense behavior and keeping the public API unchanged.

## Row Contract

Add `pub(crate) struct BitPackedRow` with private fields:

- `width: usize`, the logical bit width;
- `words: Vec<u64>`, enough storage for `width` bits.

Bits use little-endian word layout: logical bit `i` is stored in
`words[i / 64]` at bit offset `i % 64`. Width `0` uses an empty word vector.
All constructors and operations maintain the invariant that padding bits beyond
the logical width are zero. Operations that compare, count, dot, or unpack rows
therefore do not observe padding storage.

Expose only crate-private methods needed by the issue:

- `try_from_dense(row: &[u8], width: usize) -> Result<Self>`;
- `zeros(width: usize) -> Self`;
- `width(&self) -> usize`;
- `to_dense(&self) -> Vec<u8>`;
- `xor_assign(&mut self, rhs: &Self) -> Result<()>`;
- `dot_parity(&self, rhs: &Self) -> Result<u8>`;
- `weight(&self) -> usize`;
- `eq_logical(&self, rhs: &Self) -> Result<bool>`;
- `is_zero(&self) -> bool`;
- test-only `set_storage_padding_for_test(&mut self)` behind `#[cfg(test)]` so
  tail-bit behavior can be exercised without weakening production invariants.

`try_from_dense` must call `validate_target(row)` and then reject a row length
that differs from `width` with `QecError::RowWidthMismatch`, matching existing
dense helper style. Incompatible-width operations return
`QecError::RowWidthMismatch { expected: self.width, actual: rhs.width }`.

## Tail Bits

Define a small `tail_mask(width)` helper:

- width `0` has mask `0`;
- widths divisible by `64` have mask `u64::MAX`;
- otherwise the mask is `(1 << (width % 64)) - 1`.

Packing, `zeros`, and `xor_assign` apply the mask to the final word. `weight`,
`eq_logical`, `dot_parity`, `is_zero`, and `to_dense` use the stored words after
that invariant, and tests deliberately dirty final-word padding through the
test-only helper to prove logical operations still ignore it.

## Tests

Add focused unit tests in `qec-code/src/gf2.rs` with the issue-required names:

- `gf2_bitpacked_rows_match_dense_binary_rows` packs and unpacks dense fixtures
  at widths `0`, `1`, `63`, `64`, `65`, and `144`, verifying exact logical
  round trips.
- `gf2_bitpacked_row_ops_match_dense_ops` compares bit-packed XOR/add,
  dot/parity, popcount, equality, and zero checks with straightforward dense
  calculations on non-trivial fixtures.
- `gf2_bitpacked_row_ops_handle_tail_bits` dirties storage padding for widths
  crossing word boundaries and proves padding bits do not affect equality,
  popcount, parity, zero checks, or unpacked output.
- `gf2_bitpacked_rows_reject_invalid_binary_inputs` verifies non-binary dense
  values, row-width mismatches, and incompatible-width operations return
  `QecError` instead of silently masking input.

## Out Of Scope

Do not integrate `BitPackedRow` into random-window kernel basis generation or
span filtering in this issue. Do not add SIMD, unsafe code, external GF(2)
libraries, M4RI, `dist-m4ri`, `QDistRnd`, or `codeDistancePYPI` dependencies.
Do not change dense GF(2) helper behavior, random-window sampling semantics, CLI
output, or benchmark manifests.

## Self-Review

- Placeholder scan: no TBD or TODO entries.
- Consistency: the chosen single-file design matches the existing dense helper
  location and the issue's suggested API shape.
- Scope: only row primitives and tests are included; hot-path integration is
  deferred.
- Ambiguity: incompatible-width errors use the existing row-width mismatch
  variant with `expected` equal to the left operand width.
