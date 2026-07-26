# Issue 566 Directional CSS Design

Scope: GitHub issue #566, route-generated directional CSS codes on rectangular
tori in `qec-code`.

## Requirements

Add a callable `directional` requested family through the existing CSS family
contract. The Rust API and versioned JSON CLI path must lower to the same
typed specification before generating matrices.

The specification will expose:

- torus periods `period_x`, `period_y`, and an explicit
  `vertical_period_x_shift`;
- a route word using `N`, `E`, `S`, `W`, and decimal repetition suffixes;
- a two-coset layout assigning `X` and `Z` checks to the two checkerboard
  ancilla cosets; and
- a connectivity choice, `square` or `hex`.

The constructor will parse route symbols and compute route offsets with the
paper convention
`Q_j = 2 * sum(previous displacements) + d_j`. Data qubits are ordered
deterministically by row-major hardware coordinates over checkerboard data
sites. Check rows are ordered row-major over the selected `X` and `Z` ancilla
cosets.

## Approach Options

### Recommended: Paper-Backed Generic Constructor

Implement a small `codes::directional` module that owns route parsing, torus
coordinate reduction, two-coset layout validation, connectivity validation, and
matrix generation. `family_contract` will only route the typed spec and convert
the generated checks into the common `CssConstructionResult`.

This keeps the public family layer small, keeps constructor-specific validation
out of the contract router, and supports the required fixtures without adding
circuit-generation scope.

### Alternative: Fixtures Only

Hard-code the `NE2N` and `NE3N` matrices as fixed cases. This is too narrow for
the issue because layout and connectivity choices would not be exposed and route
parsing would not be tested as behavior.

### Alternative: Full Parallelogram Constructor

Support arbitrary two-vector tori immediately. This is more general than the
issue requires and would require a broader coordinate-ordering contract.

## Detailed Design

`DirectionalCssSpec` will contain a `DirectionalTorusSpec`, route string,
`DirectionalLayoutSpec`, and `DirectionalConnectivity`. The torus shape is a
rectangular hardware window with period vectors `(period_x, 0)` and
`(vertical_period_x_shift, period_y)`. The shift defaults to `0`; the
`8 x 6` `NE2N` fixture sets it to `4`, matching the rectangular toric-code
specialization while preserving the exact leading rows requested in the issue.

The two supported ancilla cosets are `(x odd, y even)` and `(x even, y odd)`.
The default layout assigns `X` to the first and `Z` to the second, which gives
`H_X[0]=[4,9,10,14]` and `H_Z[0]=[8,12,13,18]` for the `8 x 6` `NE2N` route.

Validation will reject:

- odd or zero torus periods;
- period vectors that do not preserve the checkerboard bipartition;
- malformed route words and route support self-collisions;
- two-coset layouts that assign the same coset to both Pauli types;
- route/layout odd-overlap conflicts using the paper's `Delta_odd` rule;
- finite tori that identify route support offsets or violate the conservative
  delta-vector collision checks; and
- `hex` connectivity for route words not listed as hex-grid compatible in the
  cited construction table.

Full `H_X` and `H_Z` matrices for the square `NE2N` and hex-compatible `NE3N`
fixtures will be stored as JSON fixtures. Tests will compare generated rows,
metadata, ranks, `k`, orthogonality, and exact component distances up to the
fixture distance.

## Out Of Scope

This PR will not add syndrome extraction circuits, arbitrary twisted
parallelogram ordering, decoder integration, or distance computation to the
runtime construction result.

## Self-Review

No placeholders remain. The design is scoped to one constructor and its family
contract wiring. The optional period shift is explicit in the spec rather than
hidden behind route-specific behavior.
