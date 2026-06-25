# BB Circuit BP-OSD Compare Smoke Design

## Context

Issue #217 asks for a minimal Rust `rbposd` versus Python upstream `ldpc`/`bposd`
comparison smoke for bivariate-bicycle circuit BP-OSD diagnostics. The dependency
work is present on `master`: the BB90 hard fixture (#210/#239), timing/counter
profile surface (#212/#259), and optimized order-7 OSD path (#214/#267) are
landed. The existing BB circuit memory builder supports BB90 and BB144, while
BB72 parameters exist in the CSS catalog and need a small selector entry in the
circuit-memory builder.

This smoke stays intentionally thin. It produces one CSV and one summary
artifact under `benchmarks/bb_circuit_bposd_compare/results/smoke/`, and it does
not grow into the full small-LDPC campaign.

## Considered Approaches

1. Add a pure Python BB circuit model builder and run both decoders from Python.
   This keeps the comparison package self-contained, but it duplicates the Rust
   circuit/effective-model implementation and risks comparing subtly different
   case definitions.

2. Add a Rust-exported shared case bundle, then let Python `ldpc` replay the same
   effective models and sampled syndromes. This ties both rows to the same
   `case_id`, model, seed, and trial data while keeping the Python path focused
   on the upstream decoder.

3. Shell out only to the existing four-column `rsinter bb-circuit-bposd-memory`
   CLI and estimate Python independently. This is the smallest change, but it
   lacks setup/decode timing split and cannot prove the Python rows replay the
   same sampled diagnostic cases.

Chosen approach: option 2. It is the smallest reliable way to make paired rows
reviewable without broadening the benchmark framework.

## Architecture

Add a new package at `benchmarks/bb_circuit_bposd_compare/`:

- `cases.py` defines a tiny smoke manifest with stable `case_id` values for one
  BB72 paired diagnostic and one BB90 paired diagnostic. Both use seed `12345`,
  min-sum BP, `max_iter = 10000`, `osd_method = osd_cs`, and `osd_order = 7`.
- `run_compare.py` calls the Rust exporter for each case, writes an `rbposd`
  row from the Rust profile, and runs Python upstream `ldpc.BpOsdDecoder` on
  the exported Z/X effective models and sampled syndromes to write a
  `ldpc_bposd` row.
- `summary.py` writes a compact Markdown timing summary so the Rust-vs-Python
  setup/decode/runtime comparison is visible without reading code.
- `verify_smoke.py` validates the CSV contract and negative-control behavior.

Add a narrow Rust JSON export path to the existing `rsinter
bb-circuit-bposd-memory` command. The default four-column output remains
unchanged. With a new JSON flag, the command emits the selected BB code id,
case parameters, Rust profile, effective decoder models, and sampled trials.
This is comparison infrastructure, not a new broad public benchmark API.

Add BB72 to `rsinter::bb_circuit_memory::build_code` using the fixed
`[[72,12,6]]` bivariate-bicycle parameters already present in the CSS catalog:
`ell = 6`, `m = 6`, `a = x^3 + y + y^2`, and `b = y^3 + x + x^2`.

## Data Flow

For each smoke case:

1. `run_compare.py` invokes `cargo run -q -p rsinter --bin rsinter --
   bb-circuit-bposd-memory ... --json-compare-case`.
2. The Rust exporter builds the production BB effective models, samples the tiny
   trial set once, decodes with `rbposd`, and returns JSON containing the profile
   and sampled syndromes/logicals.
3. The Python runner imports `ldpc.BpOsdDecoder`, constructs upstream decoders
   from the exported sparse rows and channel probabilities, decodes the same
   Z/X syndromes, and computes logical failure rate from the exported logical
   observables.
4. The runner writes `results.csv` and `summary.md`.
5. The verifier checks required columns, required BB72/BB90 coverage, completed
   timing fields, logical error rate fields, and at least one paired case with
   both `rbposd` and `ldpc_bposd` rows for matching diagnostic metadata.

## Error Handling

Missing Python `ldpc` or `bposd` dependencies are explicit. By default the run
records skipped Python rows with a dependency error and exits nonzero, so a
Rust-only CSV cannot be mistaken for a successful comparison. Passing
`--allow-missing-python` allows artifact generation for local Rust-only
inspection, but the verifier still rejects the CSV as a completed comparison.

Rust exporter failures are recorded as error rows for the affected Rust case and
cause the smoke command to fail.

## Testing

Add Python unit tests for:

- verifier acceptance of a valid paired BB72/BB90 CSV,
- verifier rejection when the Python upstream row is removed,
- verifier rejection when Rust and Python rows are present but no `case_id` is
  shared,
- summary generation from completed rows,
- run_compare dependency-missing behavior with explicit skipped Python rows.

Add Rust tests for BB72 circuit selector support and JSON comparison export
shape while preserving the existing four-column CLI default.

Required manual verification remains:

- `make bb-circuit-bposd-compare-smoke`
- `python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv`
- negative controls against copied CSVs with removed Python rows and unpaired
  case IDs.
