# Issue 181 Quantum Tanner Cayley Faces Design

Scope: GitHub issue #181, deterministic Cayley-complex face and local incidence
enumeration for validated explicit quantum Tanner group data.

## Context

`qec-code` already has the quantum Tanner parser, local-code helper, and
validated finite-group table from issues #178, #182, and #180. The construction
contract from #177 defines the v1 mode vocabulary and the no-cover face
canonicalization convention. This issue adds the middle layer between validated
group data and later CSS sparse-row generation: enumerate physical face ids and
local X/Z incidence coordinates, but do not build `Hx` or `Hz`.

The relevant qLDPC behavior is:

- `CayleyComplex` requires symmetric generator subsets and defines faces from
  vertices `{g, a*g, g*b, a*g*b}`.
- `QTCode.get_subgraphs` labels X incidence by `(a, b)` at source `g` and Z
  incidence by `(a^-1, b)` at source `a*g`.
- `test_toric_tanner_code` gives the `Z_d x Z_d` toric Tanner known answer
  `[[d^2, 2, d]]`; for `d = 4`, physical qubits are the 16 canonical faces, not
  the 64 oriented `(g, a, b)` records.

## Selected Approach

Extend `qec-code/src/codes/quantum_tanner.rs` with a library-only enumerator:

```rust
pub fn enumerate_quantum_tanner_cayley_faces(
    construction_mode: QuantumTannerConstructionMode,
    group: &ValidatedFiniteGroup,
) -> Result<QuantumTannerCayleyComplex>
```

The function consumes the already validated finite group and its in-range
generator arrays. It validates the generator arrays as construction sets, then
enumerates deterministic face and incidence records. The public output will be a
plain Rust struct, not JSON and not sparse CSS matrices.

Rejected alternatives:

- Add a CLI or JSON output now. This would widen public surface beyond the issue
  and conflict with the explicit out-of-scope list.
- Fold enumeration into CSS generation. Later local-code tensor rows need this
  metadata, but this issue should stop before matrix construction.
- Create a separate module file. The existing parser, validator, and local-code
  helpers are already in `quantum_tanner.rs`; keeping the first enumerator there
  follows the current layout and avoids premature module churn.

## Data Model

Add these public records:

```rust
pub struct QuantumTannerCayleyComplex {
    pub faces: Vec<QuantumTannerCayleyFace>,
    pub oriented_faces: Vec<QuantumTannerOrientedFace>,
    pub x_incidence: Vec<QuantumTannerLocalIncidence>,
    pub z_incidence: Vec<QuantumTannerLocalIncidence>,
}

pub struct QuantumTannerCayleyFace {
    pub id: usize,
    pub vertices: [usize; 4],
}

pub struct QuantumTannerOrientedFace {
    pub root_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub vertices: [usize; 4],
    pub face_id: usize,
}

pub struct QuantumTannerLocalIncidence {
    pub source_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub face_id: usize,
}
```

`faces` are physical-qubit records. Their `id` is the index in `faces`, and
`vertices` is the canonical sorted face key. `oriented_faces` are ordered by
`root_vertex`, caller-provided `A` coordinate order, then caller-provided `B`
coordinate order. `x_incidence` and `z_incidence` are sorted by
`source_vertex`, `a_index`, `b_index`, then `face_id`, so future sparse-row
generation can read each local neighborhood reproducibly.

## Validation

The enumerator will accept only `LeftRightCayleyNoCoverV1`. The current enum has
only that supported value; unsupported strings remain rejected by
`quantum_tanner_spec_from_json_str` with `UnsupportedQuantumTannerConstructionMode`.

Before enumeration, validate both generator arrays:

- nonempty
- duplicate-free in caller coordinate order
- symmetric under the validated inverse table

Add typed errors for construction-set validation and degenerate faces:

```rust
InvalidQuantumTannerGeneratorSet { set: &'static str, reason: String }
DegenerateQuantumTannerFace { root: usize, a: usize, b: usize, vertices: Vec<usize> }
```

Malformed tables and out-of-range generators remain handled by the existing
group validator. Degenerate faces are checked after computing
`g`, `a*g`, `g*b`, and `a*g*b`; a canonical face key with fewer than four
distinct vertices fails immediately.

## Enumeration Semantics

For each `g` in `0..group.order()`, each `a` in `group.a_generators()` in caller
order, and each `b` in `group.b_generators()` in caller order:

1. compute `ag = a * g`, `gb = g * b`, and `agb = ag * b`
2. canonicalize `[g, ag, gb, agb]` by sorting the four vertex ids
3. collect all canonical keys, sort them lexicographically, and assign face ids
4. emit the oriented record with the physical `face_id`
5. emit X incidence at source `g` with local coordinate `(a, b)`
6. emit Z incidence at source `ag` with local coordinate `(a^-1, b)`

The Z coordinate uses the caller-provided index of `a^-1` in the `A` array. This
is why symmetry and duplicate checks happen before enumeration.

For `toric_d4`, the output oracle is:

- `faces.len() == 16`
- `oriented_faces.len() == 64`
- `x_incidence.len() == z_incidence.len() == 64`
- each source vertex has four X and four Z local coordinates
- X source `0` coordinates `(4,1)`, `(4,3)`, `(12,1)`, `(12,3)` map to face ids
  `[0, 2, 1, 3]` under lexicographic canonical-face ordering
- the matching Z relationship holds: the X record at source `0` with `(4,1)`
  has the same face id as the Z record at source `4` with `(12,1)`, and
  similarly for the other identity-based coordinates

## Tests

Add a focused integration test in `qec-code/tests/code.rs`:

- parse the `toric_d4` fixture
- validate the group
- enumerate faces
- assert exact face count, oriented count, incidence count, first canonical
  faces, source-neighborhood coordinate order, and X/Z inverse-label
  relationship

Add negative controls:

- parse `invalid_non_symmetric_a.json`, validate the finite group, and require
  enumeration to reject the non-symmetric `A` set with the typed generator-set
  error
- mutate `toric_d4` to the reserved quadripartite mode and require the existing
  parser to reject it with `UnsupportedQuantumTannerConstructionMode`

Run the issue command:

```bash
cargo test -p qec-code quantum_tanner_cayley_faces_match_toric_d4_counts -q
```

Before creating the pull request, also run:

```bash
cargo test
```

## Out Of Scope

No CSS matrix generation, local-code tensor product application, distance
computation, CLI support, cover-mode implementation, GAP/Oscar/SmallGroup
generation, or external qLDPC runtime calls.

## Self-Review

- No placeholders remain.
- The selected API consumes validated finite-group data and keeps parser
  unsupported-mode handling intact.
- Face ids, oriented records, and X/Z incidence order are specified exactly
  enough for reproducible sparse-row generation later.
- The toric `d=4` oracle covers counts and one exact identity-based local
  neighborhood, including the X `(a,b)` and Z `(a^-1,b)` relationship.
