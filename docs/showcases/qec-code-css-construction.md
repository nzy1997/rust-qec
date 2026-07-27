# QEC-Code CSS Construction

Use `qec-code` to inspect the built-in CSS catalog, export parity-check
matrices as sparse-row JSON, and run a small exact-distance check against the
Steane fixture.

## What This Shows

This showcase follows the stable CSS construction paths that are already
covered by `qec-code` tests. It demonstrates the CLI-facing workflow for:

- listing built-in CSS code identifiers
- exporting `Hx` and `Hz` matrices for fixed built-ins and APM/Kasai presets
- exporting `Hx` and `Hz` matrices from an explicit quantum Tanner spec
- checking a small exact distance through the same CLI family

The examples use `steane`, `bb72`, `apm_kasai:p=96`, and the committed
`toric_d4` quantum Tanner fixture because those fixtures are pinned in the
repository today.

## Run It

Run these commands from the repository root:

```sh
cargo run -q -p qec-code -- code css list
cargo run -q -p qec-code -- code css export steane hx
cargo run -q -p qec-code -- code css export steane hz
cargo run -q -p qec-code -- code css export bb72 hx
cargo run -q -p qec-code -- code css export bb72 hz
cargo run -q -p qec-code -- code css export apm_kasai:p=96 hx > /tmp/apm_p96_hx.json
cargo run -q -p qec-code -- code css export apm_kasai:p=96 hz > /tmp/apm_p96_hz.json
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx
cargo run -q -p qec-code -- code css verify-families
cargo run -q -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json
cargo run -q -p qec-code -- code css-distance exact --code-id steane --json
```

## Expected Result

The list command prints the current built-in CSS catalog. This is the set of
code IDs and parameterized family shapes that `qec-code` can generate through
`code css export` today:

```text
Built-in CSS codes:
  steane                                                          fixed [[7,1,3]] CSS code
  bb72                                                            fixed [[72,12,6]] bivariate-bicycle CSS code
  apm_kasai:p=96                                                  fixed Table A1 P=96 APM-CSS code
  apm_kasai:p=192                                                 fixed Table A1 P=192 APM-CSS code
  bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...  bivariate-bicycle CSS family over periodic lattice
  repetition_x:d=<distance>                                       X-check chain, distance >= 2
  repetition_z:d=<distance>                                       Z-check chain, distance >= 2
  surface_rotated:d=<distance>                                    rotated surface CSS code, distance >= 2
  color_666:d=<distance>                                          triangular 6.6.6 color CSS code, odd distance >= 3
  toric:d=<distance>                                              periodic square-lattice toric CSS code, distance >= 2
```

Quantum Tanner is a separate file-driven generation path, not a named built-in
catalog entry. Given an explicit spec such as
`qec-code/tests/fixtures/quantum_tanner/toric_d4.json`, `qec-code` can generate
ordinary `sparse_rows` `Hx` and `Hz` matrices with:

```sh
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz
```

The committed `toric_d4` fixture has `num_cols` 16 and exact distance 4 through
the direct spec path:

```sh
cargo run -q -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json
```

Each export command prints a JSON object with `"format":"sparse_rows"`.
The Steane exports use `num_cols` 7, the `bb72` exports use `num_cols` 72, and
the `apm_kasai:p=96` exports use `num_cols` 1152. The APM/Kasai `p=96`
exports are redirected to `/tmp` above because their sparse-row output is much
larger than the Steane and `bb72` examples.

The exact-distance command returns JSON with `"status":"completed"` and
`"distance":3` for the Steane code.

## Family Verifier

`code css verify-families` is the offline end-to-end catalog check for the 14
requested CSS families. It reads
`qec-code/tests/fixtures/family_manifest/manifest.v1.json`, constructs the
positive fixture for each available family in-process, validates the metadata,
and prints one stable line per family in manifest order.

The success transcript ends with:

```text
SUMMARY PASS supported=12 deferred=2 failed=0
```

A `PASS` line means the manifest entry is `disposition=supported`,
`availability=available`, its positive fixture parsed and constructed through
`construct_css`, its dimensions, ranks, row weights, requested-family ID,
orthogonality, and provenance matched the fixture, and no subprocess or network
was used.

`DEFERRED` is intentional for `hyperbolic_5_5` and `perturbed_hgp`. Those lines
include the tracking issue and the research contract path, and they do not imply
a callable constructor:

```text
DEFERRED hyperbolic_5_5 tracking_issue=#571 contract=qec-code/doc/hyperbolic_5_5_contract.md
DEFERRED perturbed_hgp tracking_issue=#572 contract=qec-code/doc/perturbed_hgp_contract.md
```

A `FAIL` line means the checked-in catalog and the constructor result no longer
agree, or the catalog state is internally inconsistent. Examples include invalid
manifest JSON, duplicate or missing requested family IDs, a supported family
marked unavailable, missing deferred contract metadata, construction failures,
orthogonality failures, and expected-stat, rank, row-weight, requested-family,
or provenance mismatches. Any `FAIL` line changes the summary to
`SUMMARY FAIL supported=12 deferred=2 failed=N` and the CLI exits nonzero.

Parameterized Rust usage stays on the typed constructor path:

```rust
use qec_code::family_contract::{
    construct_css, CssFamilySpec, SurfaceFamilySpec,
};

let result = construct_css(
    CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 }).into(),
)?;
assert_eq!(result.stats.n, 9);
```

Parameterized CLI usage stays on the existing export path:

```sh
cargo run -q -p qec-code -- code css export surface_rotated:d=3 hx
cargo run -q -p qec-code -- code css export color_666:d=5 hz
cargo run -q -p qec-code -- code css export toric_3d:lx=3,ly=3,lz=3 hx
```

When adding a new supported fixture, update exactly one manifest entry with
normalized inputs, expected stats, row weights, distance-verification class,
provenance, and executable positive and negative cases. Then run
`code css verify-families`, the `family_catalog` tests, and the showcase checker
before treating the fixture as documented support. Keep deferred families
deferred until their contract path names an implementation-ready construction
and the constructor exists.

## Construction Routing

`qec-code` now routes compact and structured CSS constructors through one typed
construction layer before matrix generation.

Compact CLI inputs use the documented inline syntax already accepted by
`code css export`, such as `surface_rotated:d=3`, `color_666:d=5`, `bb72`, or
`bb:lx=6,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0`. The CLI lowers each inline string
with `CssConstructionSpec::from_inline`, constructs through `construct_css`,
then serializes the selected `Hx` or `Hz` matrix with the existing
`sparse_rows` JSON format. This preserves legacy byte output while giving the
Rust API one normalized result shape.

Structured constructor requests use versioned JSON with `schema_version = 1`
and a `construction` field. They are exported with
`code css construct --spec <path> hx` or `code css construct --spec <path> hz`.
For example:

```json
{"schema_version":1,"construction":"surface","distance":3}
```

```json
{"schema_version":1,"construction":"color_666","distance":5}
```

The legacy JSON request above remains the square rotated adapter: it lowers to
the same typed surface specification as the inline `surface_rotated:d=3` route
and the Rust API value `CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 })`.
Use `SurfaceSpec::rotated_square(3)` or a fully specified `SurfaceSpec` for
the generalized route.
Structured surface requests can select a layout and independent row and column
distances, for example:

```json
{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":3,"column_distance":5}
```

Color-code requests lower to
`CssFamilySpec::Color666(Color666FamilySpec { distance: 5, layout:
Color666Layout::Triangular })`. Unsupported schema versions are rejected before
construction.

## Code

Primary implementation and CLI coverage:

- [`qec-code/src/cli.rs`](qec-code/src/cli.rs)
- [`qec-code/src/codes/built_in_css.rs`](qec-code/src/codes/built_in_css.rs)
- [`qec-code/src/family_contract.rs`](qec-code/src/family_contract.rs)
- [`qec-code/tests/cli.rs`](qec-code/tests/cli.rs)
- [`qec-code/tests/code.rs`](qec-code/tests/code.rs)
- [`qec-code/tests/family_contract.rs`](qec-code/tests/family_contract.rs)

Construction notes and contracts:

- [`qec-code/doc/apm_css.md`](qec-code/doc/apm_css.md)
- [`qec-code/doc/quantum_tanner.md`](qec-code/doc/quantum_tanner.md)
- [`qec-code/doc/quantum_tanner_cli.md`](qec-code/doc/quantum_tanner_cli.md)

Fixtures used by the documented examples and focused tests:

- [`qec-code/tests/fixtures/css/steane_hx.json`](qec-code/tests/fixtures/css/steane_hx.json)
- [`qec-code/tests/fixtures/css/steane_hz.json`](qec-code/tests/fixtures/css/steane_hz.json)
- [`qec-code/tests/fixtures/css/bb72_hx.json`](qec-code/tests/fixtures/css/bb72_hx.json)
- [`qec-code/tests/fixtures/css/bb72_hz.json`](qec-code/tests/fixtures/css/bb72_hz.json)
- [`qec-code/tests/fixtures/apm/table_a1_manifest.json`](qec-code/tests/fixtures/apm/table_a1_manifest.json)
- [`qec-code/tests/fixtures/apm/p96_hx.json`](qec-code/tests/fixtures/apm/p96_hx.json)
- [`qec-code/tests/fixtures/apm/p96_hz.json`](qec-code/tests/fixtures/apm/p96_hz.json)
- [`qec-code/tests/fixtures/quantum_tanner/toric_d4.json`](qec-code/tests/fixtures/quantum_tanner/toric_d4.json)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
```

Run the CLI coverage tied to this page:

```sh
cargo test -p qec-code --test cli -q
```

That integration test covers the documented list/export/distance-facing command
family, including `code css list`, `steane` and `bb72` sparse-row exports,
`apm_kasai:p=96` exports, Steane exact-distance JSON, and the unsupported
`apm_kasai:p=128` rejection path. It also covers quantum Tanner exact,
randomized-upper-bound, and random-window-upper-bound paths from the committed
`toric_d4` spec.

Run the focused APM contract checks:

```sh
cargo test -p qec-code apm_contract_doc_examples_compile -q
cargo test -p qec-code apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values -q
```

Those tests keep the APM construction contract examples compiling, verify the
`apm_kasai:p=96` sparse-row shape, and keep the negative control for
`apm_kasai:p=128` in place.

## Limits

This page documents existing CLI behavior and pinned fixtures only. It does not
claim new distances for `bb72` or APM/Kasai codes, and it treats APM Table A1
distance values as fixture metadata rather than as newly verified exact
minimum-distance results.

Quantum Tanner generation is limited to explicit finite-data specs accepted by
the current contract. It does not search for groups, call GAP or Oscar, or
import qLDPC/qTanner/Julia construction code at runtime. Use
[`qec-code/doc/quantum_tanner.md`](qec-code/doc/quantum_tanner.md) for the
accepted input contract and
[`qec-code/doc/quantum_tanner_cli.md`](qec-code/doc/quantum_tanner_cli.md) for
the focused CLI workflow.
