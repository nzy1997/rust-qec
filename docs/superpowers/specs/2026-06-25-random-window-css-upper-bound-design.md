# Random-Window CSS Upper-Bound Search Design

## Context

Issue #232 adds the library search path for `random_window_css_upper_bound`.
The merged dependency work already provides:

- `RandomWindowUpperBoundOptions` and a `random-window-upper-bound` result contract.
- Method-aware validation through `validate_random_window_upper_bound_result`.
- A GF(2) random-window kernel-basis helper that applies and reverses a column permutation.

`CssCode` currently stores only the derived `StabilizerCode`, so the search cannot inspect the original CSS component checks without reparsing external inputs. The issue explicitly calls out preserving validated `H_X` and `H_Z` rows as an acceptable implementation route.

## Considered Approaches

1. Preserve validated dense `H_X` and `H_Z` rows inside `CssCode`.
   This is the selected approach because `CssCode::from_hx_hz` is already the validated construction boundary, and the search can consume component rows without any CLI-specific parsing.
2. Introduce a separate internal descriptor passed only to the new search.
   This would work, but every existing construction path would need to build and carry a second representation, increasing API surface and risk.
3. Reconstruct component matrices from CLI or built-in input sources.
   This is rejected because the issue explicitly says to avoid reparsing CLI JSON inside the algorithm, and not every `CssCode` necessarily has a CLI source.

## Design

`CssCode` will retain validated dense component rows:

- Store `hx: Vec<Vec<u8>>` and `hz: Vec<Vec<u8>>` alongside the derived stabilizer code.
- Preserve the existing `code()` accessor.
- Add accessors for the dense component rows so `distance_bound` can run CSS-specific checks.
- Clone the rows only at construction time after width, binary-entry, and orthogonality validation.

`random_window_css_upper_bound(css, options)` will be a library-only entry point parallel to `randomized_css_upper_bound`:

- Validate options and reject zero-logical-qubit codes before search.
- For X-like candidates, generate kernel candidates from `H_Z`, reject candidates in `row_span(H_X)`, convert to a Pauli with X support and empty Z support, and run the existing witness validator.
- For Z-like candidates, generate kernel candidates from `H_X`, reject candidates in `row_span(H_Z)`, convert to a Pauli with Z support and empty X support, and run the existing witness validator.
- Track the smallest valid witness found across both components and return early when `target_weight` is reached or beaten.
- Construct results with `DistanceBoundResult::completed_random_window_upper_bound`, set `logical_class` from the witness support, and validate through `validate_random_window_upper_bound_result`.

The current full-symplectic `randomized_css_upper_bound` implementation remains unchanged except for sharing existing helpers where appropriate.

## Search Order And Determinism

Use the existing `SplitMix64` deterministic RNG. For each restart and iteration, generate a fresh Fisher-Yates column permutation for the CSS code width, then evaluate both component searches in a fixed order. Candidate iteration follows the GF(2) helper output order. Because all randomness comes from `options.seed` and the traversal order is fixed, repeated runs with the same input and seed return identical results.

## Error Handling

The new path reuses existing error types where possible:

- Invalid options return `InvalidDistanceBoundOption`.
- Codes with no logical qubits return `DistanceWitnessNotFound`.
- Exhausted searches with no valid candidate return `RandomizedUpperBoundWitnessNotFound`, matching the existing upper-bound search failure behavior.
- Invalid generated candidates are skipped only after explicit validation failure.

## Tests

Add focused integration tests in `qec-code/tests/distance_bound.rs`:

- Surface rotated `d=5` and toric `d=5` return upper bound `5` under pinned `iterations = 5000`, `restarts = 8`, `seed = 7`, and `target_weight = 5`.
- The returned witness validates against the full CSS stabilizer code and repeating the run returns the same result.
- A low-weight component vector in the relevant stabilizer row span is rejected even when it satisfies the kernel equation.

Run the issue-specific positive and negative tests, the dependency contract tests where useful, and the full workspace `cargo test` before opening the PR.
