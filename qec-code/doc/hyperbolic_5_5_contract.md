# Hyperbolic {5,5} Quotient Contract

contract_version: 1

This document is the implementation-ready research contract for a future
pure-Rust `hyperbolic_5_5` family in `qec-code`. It deliberately specifies
validation, ordering, and reconstruction semantics without adding a callable
runtime stub.

## Scope

The v1 target is a homological CSS code built from a finite quotient of the
regular hyperbolic `{5,5}` tiling. Qubits live on edges. X checks are vertex
stars and Z checks are face boundaries. A future implementation must reconstruct
the cellular chain complex and then use the binary chain-complex contract from
`qec_code::binary_chain_complex`:

```text
H_X = boundary_1
H_Z = transpose(boundary_2)
```

The accepted v1 runtime input is a supplied permutation quotient. Subgroup and
coset enumeration inputs are research workflow inputs until a separate quotient
enumerator exists in pure Rust.

## Input Contract

The future serializable input is JSON with these fields:

```text
schema_version = 1
construction = "hyperbolic_5_5_quotient"
quotient_kind = "permutation_action"
num_flags: usize
r0: permutation over 0..num_flags
r1: permutation over 0..num_flags
r2: permutation over 0..num_flags
metadata: optional object
expected: optional fixture object
limits: optional object
```

Each generator is encoded as an array `p` of length `num_flags`, where `p[i]`
is the image of flag `i`. Arrays must be bijections over `0..num_flags`.
Implementations may accept a compact cycle notation later, but the normalized
contract is always the dense zero-based permutation array so serialization and
diffs are deterministic.

`metadata` may record provenance such as a subgroup name, coset action source,
paper citation, or fixture id. Metadata must not affect reconstruction.

`expected` is fixture-only. It may pin counts, ranks, check weights, and
distance for known examples. It must not be used as a constructor shortcut.

`limits` may lower default resource caps. A request cannot raise the caps above
the defaults in this document without an explicit feature gate in a later issue.

## Quotient Input Choices

### Supplied permutation quotient

A supplied permutation quotient gives the action of Coxeter generators on the
finite flag set directly. This is the minimal pure-Rust implementation target:
validate the permutations, reconstruct cells from flag orbits, build boundary
maps, and report CSS checks. It does not require GAP, Magma, Sage, Oscar, or
subgroup search.

Use one implementation issue if this input is sufficient:

- parse and normalize the dense permutation arrays;
- validate the Coxeter presentation and quotient transitivity;
- enumerate cells from flag orbits;
- build and validate binary boundaries;
- expose a constructor only after the small stellated dodecahedron fixture meets
  the time and memory gates.

### Subgroup input

A subgroup input gives words, generators, or a finite-index subgroup of the
abstract Coxeter group. That is not a v1 runtime input because `qec-code` would
also need coset enumeration, quotient action extraction, and torsion checks for
the subgroup itself.

If subgroup input is required, split the work into separate issues:

- quotient enumeration: pure-Rust finite-index coset enumeration or a validated
  importer for externally enumerated coset actions;
- cellulation: the supplied permutation quotient consumer described above.

Subgroup names, GAP ids, Magma scripts, or presentation words may be retained in
fixture metadata, but the normalized constructor input remains the supplied
permutation quotient.

## Coxeter Presentation

The `{5,5}` Coxeter presentation for flag adjacency uses involutions `r0`,
`r1`, and `r2`:

```text
r0^2 = r1^2 = r2^2 = 1
(r0 r1)^5 = 1
(r1 r2)^5 = 1
(r0 r2)^2 = 1
```

The implementation must evaluate each relation as a permutation identity over
the full flag set. Composition order must be documented in code and tests; the
contract only requires consistency. A failed relation returns
`InvalidCoxeterQuotient` and includes `failed_relation`.

## Flag-Orbit Enumeration

Flags are chambers of the quotient. The generator `r_i` crosses the codimension
one face opposite the `i`th element in the flag. Cells are reconstructed as
orbits under parabolic subgroups:

```text
vertices = orbits of <r1, r2>
edges = orbits of <r0, r2>
faces = orbits of <r0, r1>
```

For every flag, compute the three orbit ids containing that flag. These ids
define incidence:

```text
flag f -> vertex_id[f], edge_id[f], face_id[f]
edge e incident to vertex v if any flag f has edge_id[f] = e and vertex_id[f] = v
face a incident to edge e if any flag f has face_id[f] = a and edge_id[f] = e
```

Orbit enumeration must use an explicit stack or queue and deterministic sorted
seed order `0..num_flags`. The adjacency generator order inside each orbit is
the ascending generator label order shown above.

The expected `{5,5}` local incidence is:

- each edge orbit is incident to exactly 2 distinct vertex orbits;
- each edge orbit is incident to exactly 2 distinct face orbits;
- each vertex orbit is incident to exactly 5 edge orbits;
- each face orbit is incident to exactly 5 edge orbits.

An explicit incidence list may be included only as a fixture. It must not be
accepted as the general constructor because it bypasses Coxeter quotient
validation.

## Canonical Ordering

All canonical ordering must be independent of hash-map iteration.

Use these deterministic keys:

- flag order: numeric flag id `0..num_flags`;
- orbit key: sorted ascending list of member flags;
- vertex order: lexicographic order of vertex orbit keys;
- edge order: lexicographic order of edge orbit keys;
- face order: lexicographic order of face orbit keys;
- boundary row supports: sorted ascending canonical cell ids.

Never expose order derived from `HashMap`, `HashSet`, pointer addresses, or
parallel iteration scheduling. If hash tables are used internally, copy keys to
a vector and sort before assigning ids or serializing output.

When two normalized inputs describe isomorphic quotients with different flag
labels, they may produce different ids. The required invariant is that one
normalized input produces byte-stable output on repeated runs and across
platforms.

## Boundary Maps

After canonical ordering, build sparse GF(2) cellular boundary maps with rows as
codomain cells and columns as domain cells:

```text
boundary_1: vertices x edges
boundary_2: edges x faces
```

For `boundary_1`, column `edge_id` has support equal to the two canonical
vertices incident to that edge.

For `boundary_2`, column `face_id` has support equal to the five canonical
edges incident to that face. The boundary is binary for CSS commutation; the
orientability check below is still required because signed orientation is the
topological witness that the cellulation is an orientable closed surface.

The CSS matrices are:

```text
H_X = boundary_1
H_Z = transpose(boundary_2)
```

The future implementation must construct a `BinaryChainComplex` from
`boundary_1` and `boundary_2` or apply identical validation. In either route,
`boundary * boundary = 0` is mandatory before CSS checks are returned.

## Validation

Validation must run before any CSS result is exposed.

1. Parse and shape-check `schema_version = 1`,
   `construction = "hyperbolic_5_5_quotient"`, `num_flags`, and the three dense
   permutations.
2. Validate each permutation is a bijection over `0..num_flags`.
3. Validate Coxeter relations exactly:
   `r0^2 = r1^2 = r2^2 = 1`, `(r0 r1)^5 = 1`, `(r1 r2)^5 = 1`, and
   `(r0 r2)^2 = 1`.
4. Validate quotient transitivity: the group generated by `r0`, `r1`, and `r2`
   has one orbit on the flag set. Disconnected quotients must not silently
   produce disjoint-code direct sums.
5. Enumerate vertex, edge, and face orbits.
6. Validate manifold incidence:
   each edge has exactly two endpoint vertices and exactly two incident faces;
   every vertex has five incident edges; every face has five incident edges.
7. Validate torsion: no non-identity local stabilizer fixes a flag or collapses
   a required local orbit. In v1 this means all vertex, edge, and face links
   have the expected `{5,5}` sizes above; a later subgroup enumerator must also
   reject torsion before it emits a permutation quotient.
8. Validate orientability by attempting to assign a sign `+1` or `-1` to every
   flag so each generator edge flips sign. A contradiction means
   `NonOrientableQuotient`.
9. Build `boundary_1` and `boundary_2`, then validate
   `boundary * boundary = 0` over GF(2). A failure must identify a nonzero
   composed row and support.
10. Validate expected fixture metadata, if supplied, after reconstruction. A
    mismatch is a fixture mismatch, not a license to trust metadata.

## Typed Failure Modes

Future runtime errors must be typed. Minimum v1 names:

- `UnsupportedHyperbolic55SchemaVersion { version }`
- `InvalidHyperbolic55Construction { reason }`
- `InvalidPermutation { generator, reason }`
- `InvalidCoxeterQuotient { failed_relation, witness_flag }`
- `DisconnectedQuotient { components }`
- `InvalidFlagOrbit { orbit_kind, reason }`
- `InvalidManifoldIncidence { cell_kind, cell_id, reason }`
- `NonOrientableQuotient { witness_flag }`
- `TorsionDetected { orbit_kind, witness_flag, reason }`
- `NonzeroBoundaryComposition { lower_dimension, upper_dimension, row, support }`
- `ResourceLimitExceeded { limit, requested_or_observed }`
- `FixtureMismatch { field, expected, observed }`

The negative control in this document must return `InvalidCoxeterQuotient` with
`failed_relation = "(r0 r1)^5 = 1"`.

## Pure-Rust Algorithms

The supplied permutation quotient path needs only bounded pure-Rust graph and
permutation algorithms:

- dense permutation validation with a `Vec<bool>` seen set;
- permutation composition and exponentiation by repeated composition for the
  short Coxeter relations;
- breadth-first or depth-first orbit enumeration over deterministic generator
  lists;
- union-find as an equivalent orbit-building strategy when it gives cleaner
  incidence assembly;
- sorted-vector canonicalization for orbit keys and sparse supports;
- bipartite sign propagation for orientability;
- sparse GF(2) boundary composition through `SparseGf2Matrix` or
  `BinaryChainComplex`.

Todd-Coxeter coset enumeration is viable pure Rust for subgroup inputs, but it
is not part of the first supplied-permutation implementation. If added later,
it must have separate tests for deduction queues, coincidence handling,
standardization of coset numbering, relation-table validation, and resource
cutoffs. Low-index subgroup enumeration is larger still and should remain a
separate research implementation issue.

No runtime implementation may shell out to GAP, Magma, Sage, Oscar, Python, or
Julia. External tools may generate fixture permutation quotients, but Rust owns
validation.

## Resource Limits

Default v1 limits:

```text
max_flags = 200000
max_vertices = 50000
max_edges = 100000
max_faces = 50000
max_relation_checks = 6
max_orbit_generators = 2
max_fixture_seconds = 5 seconds
max_fixture_memory = 512 MiB
```

The supplied-permutation path is linear in `num_flags` up to sorting orbit
member lists. Expected memory is `O(num_flags + V + E + F + incidence)`.

Before any family promotion, the small stellated dodecahedron fixture must
reconstruct in under 5 seconds and 512 MiB in the standard test environment.
The family cannot move to `supported` until that performance gate and all
validation gates pass from the supplied permutation quotient.

Subgroup enumeration limits must be stricter and separate. A Todd-Coxeter issue
must define maximum cosets, maximum table entries, maximum deductions, and
timeout behavior before it is callable.

## Fixture: Small Stellated Dodecahedron

The required positive fixture is the small stellated dodecahedron cellulation of
the closed `{5,5}` quotient.

Expected reconstructed fields:

```text
fixture_id = "small_stellated_dodecahedron_v1"
tiling = "{5,5}"
V = 12
E = 30
F = 12
code = [[30,8,3]]
n = 30
k = 8
d = 3
m_x = 12
m_z = 12
rank_x = 11
rank_z = 11
x_check_weight = 5
z_check_weight = 5
genus = 4
euler_characteristic = -6
```

The fixture should also assert:

- every edge qubit is incident to two X checks and two Z checks;
- every X row has weight 5;
- every Z row has weight 5;
- `rank_x + rank_z = 22`;
- `k = E - rank_x - rank_z = 8`;
- the exact distance metadata is `d = 3`.

An explicit incidence list may be placed below the fixture in a future issue
for debugging, but it is only a fixture. The constructor must reconstruct
incidence from flag orbits of the supplied permutation quotient.

## Negative Quotient Fixture

The required negative control is a transitive four-flag assignment whose
generators are involutions and whose `(r1 r2)^5 = 1` and `(r0 r2)^2 = 1`
relations pass, but which violates `(r0 r1)^5 = 1`.

Use dense permutation arrays:

```text
num_flags = 4
r0 = [1, 0, 3, 2]
r1 = [2, 3, 0, 1]
r2 = [2, 3, 0, 1]
```

This assignment violates `(r0 r1)^5 = 1` because `r0 r1` has order 2, so its
fifth power is not identity. The future validator must return:

```text
error = InvalidCoxeterQuotient
failed_relation = "(r0 r1)^5 = 1"
```

This negative control is intentionally not a cellulation fixture. Validation
must fail at the Coxeter-relation stage before orbit enumeration or incidence
checks.

## Split Decision

Create one implementation issue if a supplied permutation quotient is
sufficient. That issue should implement parser, quotient validation,
flag-orbit cellulation, boundary maps, fixture reconstruction, and the deferred
runtime promotion gate together.

If caller-facing subgroup input is required, split the work:

- one quotient enumeration issue for Todd-Coxeter or another pure-Rust coset
  action algorithm;
- one cellulation issue for the supplied permutation quotient consumer.

Do not combine subgroup enumeration, low-index subgroup search, and CSS
cellulation in one issue. The failure modes, resource limits, and verification
fixtures are different enough to need separate review.

## Deferred Runtime Status

No callable runtime stub is added by this contract. `hyperbolic_5_5` remains a
deferred family in the manifest and must stay absent from
`CssFamilySpec::callable_requested_family_ids()`.

The family cannot move to `supported` until all of these are true:

- the supplied permutation quotient parser and validator exist in pure Rust;
- the small stellated dodecahedron fixture reconstructs from the quotient, not
  from a hand-written incidence list;
- reconstruction finishes under 5 seconds and 512 MiB in the standard test
  environment;
- `boundary * boundary = 0`, ranks, check weights, and `[[30,8,3]]` metadata
  are verified by tests;
- the negative quotient fixture returns `InvalidCoxeterQuotient` with the
  failed relation.

## References

- GitHub issue #571, Roadmap ID M4-01.
- GitHub issue #552, family manifest deferring `hyperbolic_5_5`.
- GitHub issue #565, binary cellular boundary maps.
- Conrad, Chamberland, Breuckmann, and Terhal, "The small stellated
  dodecahedron code and friends", Philos. Trans. A 376:20170323, 2018,
  PMCID PMC5990658.
