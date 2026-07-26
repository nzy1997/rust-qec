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

## Construction Routing

`qec-code` now routes compact and structured CSS constructors through one typed
construction layer before matrix generation.

Compact CLI inputs use the documented inline syntax already accepted by
`code css export`, such as `surface_rotated:d=3`, `bb72`, or
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

The JSON request above lowers to the same typed surface specification as the
inline `surface_rotated:d=3` route and the Rust API value
`CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 })`. Unsupported schema
versions are rejected before construction.

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
