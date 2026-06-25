# Quantum Tanner CSS Distance Design

## Context

Issue #185 extends the quantum Tanner path that now exists in `qec-code`.
Issue #184 added `qec-code code css quantum-tanner --spec <path> hx|hz`, and
the current CLI already has exact and randomized CSS distance commands that
accept either a built-in `--code-id` or an `--hx`/`--hz` sparse-row file pair.

The quantum Tanner constructor from the earlier issues returns ordinary CSS
checks. The distance command should therefore only add a new input source: read
and validate the spec, construct `Hx` and `Hz`, convert them to the existing
`CssCode`, then call the current distance implementation.

## Chosen Approach

Add `--quantum-tanner-spec <path>` to `qec-code code css-distance exact` and
to `qec-code code css-distance randomized-upper-bound`. The exact command is
the issue's required interface; the randomized command uses the same input
source helper so both CSS distance routes stay consistent.

Introduce a small CLI input-selection helper in `qec-code/src/cli.rs` that
accepts exactly one source:

- `--code-id <id>`
- `--quantum-tanner-spec <path>`
- `--hx <path> --hz <path>`

The helper keeps the existing file-pair validation behavior and updates the
missing/conflicting input messages to mention the new source.

## Data Flow

The new source path reads JSON with the existing `read_quantum_tanner_spec`
helper, calls `quantum_tanner_css_checks`, validates sparse supports through
`SparseRowsMatrix::new`, and creates a `CssCode` with `CssCode::from_hx_hz`.

For exact distance JSON, add an `ExactCssDistanceInput::QuantumTannerSpec`
variant so the existing result shape records the input provenance while still
returning the same status, distance, method, witness, and provenance fields.
Randomized output currently records algorithm options rather than input
provenance, so it only needs the source routing.

## Error Handling

Invalid quantum Tanner specs fail while building the CSS checks, before
`compute_distance` or `randomized_css_upper_bound` is called. This preserves the
issue's negative-control requirement and surfaces the constructor validation
message, such as the non-symmetric generator set error.

Missing spec files reuse the current CSS matrix read error type for consistency
with the existing file-based CLI path.

## Tests

Add CLI tests for the exact path:

- `toric_d4.json` returns completed exact JSON with `distance == 4`, a weight-4
  witness, and `options.input == "quantum_tanner_spec"`.
- `invalid_non_symmetric_a.json` exits non-zero, writes no stdout JSON, and
  reports the quantum Tanner validation failure.

Add randomized coverage for the same input source with a pinned seed and target
weight so the shared route is exercised without adding a new algorithm.

These tests cite the qLDPC toric Tanner fixture context from
`drafts/qLDPC/src/qldpc/codes/quantum_test.py`: `Z_d x Z_d` yields a rotated
toric code with distance `d`.

## Out Of Scope

Do not add new distance algorithms, matrix formats, qLDPC importers, group
search, `rsinter` benchmark flows, or decoder integrations.
