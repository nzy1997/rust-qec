# Random-Window Upper-Bound CLI Design

## Context

Issue #233 asks for a CLI surface for the library path merged in #232. The current `qec-code code css-distance` CLI exposes `exact` and `randomized-upper-bound`. The library already provides `random_window_css_upper_bound`, `RandomWindowUpperBoundOptions`, and JSON serialization with `method = "random-window-upper-bound"` and `bound_type = "upper"`.

## Considered Approaches

1. Add `random-window-upper-bound` as a sibling CLI subcommand to `randomized-upper-bound`. This is selected because it keeps method names explicit, preserves the old command unchanged, and reuses the existing CSS input selection path.
2. Add a flag to `randomized-upper-bound` to choose the random-window engine. This is rejected because the issue requires an explicit command name and unchanged old command behavior.
3. Make random-window the default randomized CLI implementation. This is rejected because the issue explicitly keeps default replacement out of scope.

## Design

Add `CssDistanceCommands::RandomWindowUpperBound` with CLI arguments matching `randomized-upper-bound`: `--code-id` or `--hx --hz` or `--quantum-tanner-spec`, `--iterations`, optional `--restarts`, `--seed`, optional `--target-weight`, and required `--json`. The `--restarts` default remains `1`, matching the current randomized command and the shared upper-bound options validation contract.

Route the new command through the existing `css_distance_input_selection`, `css_code_from_built_in`, `css_code_from_files`, and `css_code_from_quantum_tanner_spec` helpers. Construct `RandomWindowUpperBoundOptions` from the CLI values and call `random_window_css_upper_bound`.

Missing `--json` returns `QecError::JsonOutputRequired { command: "code css-distance random-window-upper-bound" }`, so the binary exits nonzero with no stdout and a command-specific stderr message. Other invalid input errors should mirror the existing randomized path because they use the same input selection and matrix readers.

The existing `randomized-upper-bound` subcommand stays in place and continues to call `randomized_css_upper_bound`.

## Tests

Add a CLI contract test in `qec-code/tests/cli.rs` that covers built-in code, `--hx/--hz` files, `--quantum-tanner-spec`, and the missing `--json` negative control for `random-window-upper-bound`. The successful cases assert completed JSON, `method = "random-window-upper-bound"`, `bound_type = "upper"`, and target-bounded witness weights under small pinned fixtures.

Run the issue command, the new focused CLI test, the missing-JSON negative control, and the full workspace `cargo test` before opening the PR.
