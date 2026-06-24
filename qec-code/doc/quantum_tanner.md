# Quantum Tanner Construction Contract

This note is the v1 implementation contract for future quantum Tanner support in
`qec-code`. It is a consumer contract: external tools may generate finite-group
data, generator sets, local binary codes, or known-answer fixtures, but
`qec-code` only consumes validated explicit data and emits deterministic CSS
sparse-row matrices.

The contract follows the left-right Cayley-complex vocabulary used by quantum
Tanner references, while deliberately keeping Rust out of group search and
computer-algebra orchestration.

## References

Use these sources for vocabulary and algorithm checks, not for mechanical code
copying:

- Local qLDPC reference: `drafts/qLDPC/src/qldpc/codes/quantum.py`, especially
  `QTCode`, `QTCode.get_subgraphs`, and `QTCode.get_subcodes`.
- Local qLDPC Cayley-complex reference: `drafts/qLDPC/src/qldpc/objects.py`,
  especially `CayleyComplex`, cover-mode handling, total no-conjugacy, symmetric
  generator validation, and face semantics.
- Local qLDPC test reference: `drafts/qLDPC/src/qldpc/codes/quantum_test.py`,
  especially `test_toric_tanner_code`, where `Z_d x Z_d` with the repetition
  seed code gives `[[d^2, 2, d]]`.
- Upstream qLDPC: <https://github.com/qLDPCOrg/qLDPC>.
- QuantumExpanders.jl vocabulary and explicit quantum Tanner construction
  examples: <https://github.com/QuantumSavory/QuantumExpanders.jl>.

## Scope Boundary

`qec-code` v1 accepts explicit finite data. It must not call GAP, Oscar,
SmallGroup, GroupNames, Morgenstern constructors, Ramanujan graph search,
random-code search, qLDPC Python code, or Julia/Oscar code at runtime.

External tools may produce a multiplication table, generator sets, local code
matrices, or fixture metadata. The Rust implementation owns validation of that
explicit data and the deterministic conversion to CSS sparse rows.

Out of scope for this contract issue: parser implementation, group validation
implementation, Cayley-complex enumeration implementation, CLI flags, search
workflows, and matrix generation.

## Input Object

The future input object should contain these fields:

```text
construction_mode: string
base_group: { order, identity, multiplication_table }
a_generator_indices: [usize]
b_generator_indices: [usize]
local_codes: { matrix_role, field, h_a, h_b }
```

### Base Group

The accepted finite group is an explicit multiplication table:

- `order` is the number of base-group elements.
- `identity` is the identity element index. v1 requires `identity == 0`.
- `multiplication_table` is an `order x order` rectangular array.
- `multiplication_table[left][right]` is the product `left * right`.
- Every table entry is a zero-based element index `< order`.

The implementation must reject malformed tables before construction. A complete
validator should check rectangular shape, range, identity laws, inverses, and
associativity, but this issue only documents that contract.

### Generator Sets A And B

`a_generator_indices` and `b_generator_indices` are arrays of base-group element
indices. They are not covered-element ids in v1. The arrays must be nonempty,
in-range, duplicate-free after canonicalization, and symmetric: if an element is
listed, its inverse under the multiplication table must also be listed.

The ordering of each generator array is semantically meaningful for local-code
coordinates. A future implementation may canonicalize duplicates for validation,
but it must either preserve the caller's coordinate order or return a typed
coordinate-order error. Silent reordering before local-code interpretation is
not allowed.

### Local Binary Codes

`local_codes.field` is exactly `GF(2)` in v1. `h_a` and `h_b` are binary
parity-check matrices:

- entries are `0` or `1`
- every row in `h_a` has width `|A|`
- every row in `h_b` has width `|B|`
- empty row sets are allowed only if the future implementation explicitly
  defines the corresponding local code as unconstrained

The qLDPC reference constructs quantum Tanner CSS sectors from tensor/dual local
codes. This Rust contract only fixes the accepted local-code input shape; later
implementation issues can define the internal tensor-dual derivation in code.

## Construction Modes

The v1 supported construction mode is:

- `lr_cayley_no_cover_v1`: build the left-right Cayley complex directly over the
  validated base group.

Reserved mode names that must be rejected in v1:

- `lr_cayley_bipartite_double_cover_v1`
- `lr_cayley_quadripartite_cover_v1`

Future code must reject any unsupported string with a typed
`UnsupportedConstructionMode` error. It must not silently interpret
`bipartite`, `quadripartite`, or a missing mode by folklore.

### Base Group Versus Cover Group

In `lr_cayley_no_cover_v1`, there is no cover group. All vertex ids, generator
indices, and face records refer to the input base group.

If a later issue adds a cover mode, the input `A`/`B` arrays should still refer
to base-group element indices unless that issue explicitly changes the contract.
The covered group would then be derived internally from the base group and the
mode. Covered vertices must not leak into v1 input examples.

## Face Records And Physical Qubits

For `lr_cayley_no_cover_v1`, an oriented face record is `(g, a, b)` with `g` in
the base group, `a` in `A`, and `b` in `B`. Its vertices are:

```text
g
a*g
g*b
a*g*b
```

using the multiplication table. The canonical face key is the sorted,
duplicate-free list of those four vertex ids. If the list has fewer than four
vertices, the face is degenerate and construction must fail with `DegenerateFace`
or a more specific typed error.

Physical-qubit ids are assigned by sorting all distinct canonical face keys
lexicographically and numbering them from `0`. This makes output independent of
hash-map iteration order and independent of which orientation first discovered a
face.

<!-- quantum_tanner_contract:toric_d4_counting_convention -->
For the `Z4 x Z4` toric Tanner example below, the mode is
`lr_cayley_no_cover_v1`, so no construction-mode cover changes the vertex set.
There are `|G| * |A| * |B| = 16 * 2 * 2 = 64` oriented face records. Each square
is reached from four orientations, so the physical-qubit count is
`n = |G| * |A| * |B| / 4 = 16 * 2 * 2 / 4 = 16`.

## CSS Row Assembly Semantics

Interpret `h_a` and `h_b` as parity-check matrices whose kernels are the local
binary codes `C_A` and `C_B`. All derived local spaces are over GF(2). The
future implementation must use deterministic row-basis choices when deriving
these local Tanner rows:

```text
X local rows: basis of dual(C_A tensor C_B)
Z local rows: basis of dual(dual(C_A) tensor dual(C_B))
```

For each oriented face record `(g, a, b)`, the X-sector incidence is attached to
source vertex `g` with local coordinate `(a, b)`. The Z-sector incidence is
attached to source vertex `a*g` with local coordinate `(a^-1, b)`. This matches
the qLDPC `QTCode.get_subgraphs` convention and keeps the local coordinate order
consistent when the same canonical face is reached from another orientation.

For each source vertex and each derived local row, emit one sparse CSS row by
mapping local coordinates with bit `1` to canonical physical-qubit ids. Zero
local rows should be rejected or omitted by an explicitly documented future
implementation rule; v1 should not silently emit ambiguous zero stabilizers.

## Sparse CSS Output

The future generator should return two matrices compatible with the existing
`SparseRowsMatrix` JSON contract in `qec-code/src/css.rs`:

```json
{
  "format": "sparse_rows",
  "num_cols": 16,
  "rows": [[0, 1, 4, 5]]
}
```

The output contract is:

- `Hx.format == "sparse_rows"` and `Hz.format == "sparse_rows"`
- `Hx.num_cols == Hz.num_cols == n`
- every row support is sorted, unique, and in range
- row order is deterministic from base source-vertex order and local-code row
  order
- column order is deterministic from canonical face ids
- `Hx * Hz^T == 0 mod 2`

The construction result should also report expected or computed metadata needed
by tests, including `n`, rank-derived `k` when computed, and any known-answer
expected distance supplied by fixtures.

## Error Vocabulary

Future implementation errors should be typed enough for CLI and tests to match
causes without parsing prose. Minimum v1 names:

- `InvalidGroupTable`
- `InvalidGeneratorIndex`
- `NonSymmetricGeneratorSet`
- `InvalidLocalCodeMatrix`
- `UnsupportedConstructionMode`
- `DegenerateFace`
- `NonOrthogonalCssOutput`

## Example: `toric_d4`

The element order is `id = 4*x + y` for `(x, y) in Z4 x Z4`; multiplication is
component-wise addition modulo `4`. The generators are `A = {(+1,0), (-1,0)}`
and `B = {(0,+1), (0,-1)}`, encoded as base-group indices.

<!-- quantum_tanner_contract:toric_d4 -->
```json
{
  "example_id": "toric_d4",
  "construction_mode": "lr_cayley_no_cover_v1",
  "base_group": {
    "name": "Z4xZ4",
    "element_order": "id = 4*x + y for (x,y) in Z4 x Z4",
    "order": 16,
    "identity": 0,
    "multiplication_table": [
      [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15
      ],
      [
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12
      ],
      [
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13
      ],
      [
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14
      ],
      [
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3
      ],
      [
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0
      ],
      [
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1
      ],
      [
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2
      ],
      [
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7
      ],
      [
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4
      ],
      [
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5
      ],
      [
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6
      ],
      [
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11
      ],
      [
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8
      ],
      [
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9
      ],
      [
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10
      ]
    ]
  },
  "a_generator_indices": [
    4,
    12
  ],
  "b_generator_indices": [
    1,
    3
  ],
  "local_codes": {
    "matrix_role": "parity_check",
    "field": "GF(2)",
    "h_a": [
      [
        1,
        1
      ]
    ],
    "h_b": [
      [
        1,
        1
      ]
    ]
  },
  "expected_css": {
    "n": 16,
    "k": 2,
    "expected_distance": 4
  }
}
```

The known-answer parameters match the qLDPC `test_toric_tanner_code` convention
for `d = 4`: `[[16, 2, 4]]`. The count is `16`, not `64`, because physical
qubits are canonical faces, not oriented `(g, a, b)` records.

## Bad Example: Non-Symmetric Generator Set

This example removes the inverse of generator `4`, so `A` is not symmetric and
must be rejected before face enumeration.

<!-- quantum_tanner_contract:bad_non_symmetric_generator -->
```json
{
  "example_id": "bad_non_symmetric_generator",
  "construction_mode": "lr_cayley_no_cover_v1",
  "base_group": "same as toric_d4",
  "a_generator_indices": [
    4
  ],
  "b_generator_indices": [
    1,
    3
  ],
  "expected_error": "NonSymmetricGeneratorSet"
}
```

## Future Implementation Checklist

- Validate the base multiplication table before reading generator semantics.
- Validate `A` and `B` as in-range symmetric base-group index arrays.
- Validate local matrices as GF(2) parity-check matrices with widths matching
  `|A|` and `|B|`.
- Reject unsupported construction modes with `UnsupportedConstructionMode`.
- Enumerate face records only after all input validation passes.
- Canonicalize face keys before assigning physical-qubit ids.
- Emit deterministic `sparse_rows` `Hx` and `Hz` matrices.
- Check CSS orthogonality before returning output.
