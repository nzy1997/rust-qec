# Task 1 Report: Rust BB72 Selector and Shared Case Export

## Scope

Implemented Task 1 exactly in the requested ownership surface:

- `rsinter/src/bb_circuit_memory.rs`
- `rsinter/src/bin/rsinter.rs`
- `rsinter/tests/bb_circuit_memory.rs`
- `rsinter/tests/bench_cli.rs`

No other tracked source files were modified.

## TDD Record

### Red

Added the required failing tests:

- `build_code_supports_bb72_smoke_shape`
- `comparison_case_export_contains_models_samples_and_profile`
- `rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle`

Ran the focused commands from the brief before implementation:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

Observed expected failure at the missing symbol seam:

- unresolved import `export_comparison_case_for_code`

### Green

Implemented:

- `bb72` selector support in `build_code`
- `BivariateBicycleParams::bb72()`
- `Serialize` derives for:
  - `SimulationConfig`
  - `SimulationResult`
  - `BbCircuitBposdProfile`
- comparison export structs:
  - `BbCircuitBposdComparisonExport`
  - `ComparisonModelExport`
  - `ComparisonTrialExport`
- helpers:
  - `comparison_model_export`
  - `comparison_trial_export`
- `export_comparison_case_for_code(code_id, config)`
- CLI flag:
  - `rsinter bb-circuit-bposd-memory --json-compare-case`

Implementation detail:

- `run_simulation_for_code` now delegates through `export_comparison_case_for_code` and returns `rust_result`, which keeps the legacy result path and the new JSON export path aligned on one execution flow.

## Verification

Ran the required focused tests after implementation:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

All passed.

Ran one additional regression check on the legacy CLI output path:

```bash
cargo test -p rsinter rsinter_bb90_circuit_bposd_memory_prints_four_column_result_line -q
```

Passed.

Formatted the workspace with:

```bash
cargo fmt --all
```

Then re-ran the three focused Task 1 tests and confirmed they still passed.

## Self-Review

What I checked:

- `bb72` support is confined to the requested selector path and supported-id error message.
- The JSON compare-case path serializes shared data needed by downstream comparison tooling:
  - code/config metadata
  - rust result/profile bundle
  - sparse model rows and augmented columns
  - sampled trial syndromes/logicals
- The non-JSON CLI path still prints the existing four-column result line.
- Export generation uses the same simulation and decode loop as the normal run path, avoiding drift between the two interfaces.

## Concerns

None from the task scope.

## Fix Report: Review Follow-Up

### What Changed

- Split the shared execution path so `run_simulation_for_code` now calls an internal helper with trial export collection disabled.
- Kept `export_comparison_case_for_code` on the same decode loop, but made it opt into collecting `ComparisonTrialExport` values.
- Added a regression unit test that checks the internal helper returns `None` for retained trials on the legacy path and `Some(...)` only for the export path.
- Strengthened JSON export coverage to assert `max_bp_iterations`, `osd_order`, `z_model.first_logical_row`, `x_model.first_logical_row`, and logical vector lengths.

### Test Command / Output

Red:

```bash
cargo test -p rsinter simulation_case_collection_is_export_only -q
```

Output:

```text
error[E0432]: unresolved import `super::run_simulation_case_for_code`
```

Green and required coverage:

```bash
cargo fmt --all
cargo test -p rsinter simulation_case_collection_is_export_only -q
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
cargo test -p rsinter rsinter_bb90_circuit_bposd_memory_prints_four_column_result_line -q
```

Output summary:

- `cargo fmt --all`: exit 0
- all five test commands: exit 0, targeted test passed in each run

### Files Changed

- `rsinter/src/bb_circuit_memory.rs`
- `rsinter/tests/bb_circuit_memory.rs`
- `rsinter/tests/bench_cli.rs`
- `.sdd/task-1-report.md`

### Self-Review

- The legacy result path no longer allocates or retains exported trial payloads.
- JSON export still uses the same sampling and decode logic, so result/profile behavior stays aligned across both interfaces.
- The added regression test covers the specific review concern rather than only rechecking JSON output shape.
