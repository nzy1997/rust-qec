# CSS Distance Workflow

This note covers the user-facing CSS distance commands in `qec-code`. Run the
commands from the repository root.

Use `random-window-upper-bound` when you need a fast randomized CSS distance
upper-bound search with the current windowed sampler. It is useful for checking
known CSS instances, reproducing issue-225 ladder evidence, or comparing a new
code against a pinned upper-bound target.

Use `randomized-upper-bound` when you want the older simple sampler as a
baseline comparison. It remains available unchanged and is still useful as a
small negative-control style baseline, but it may return looser bounds on cases
where the windowed search succeeds.

## Built-In Code Example

The built-in path accepts any registered built-in CSS code ID.

<!-- css_distance:random_window_builtin -->
```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --code-id steane --iterations 500 --restarts 4 --seed 7 --target-weight 3 --json
```

The command should print one JSON object to stdout and nothing to stderr.

## Sparse-Row File Example

The file path accepts explicit `sparse_rows` `Hx` and `Hz` JSON matrices. This
example uses committed sparse-row fixtures.

<!-- css_distance:random_window_files -->
```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --hx qec-code/tests/fixtures/css/steane_hx.json --hz qec-code/tests/fixtures/css/steane_hz.json --iterations 500 --restarts 4 --seed 7 --target-weight 3 --json
```

Use the same shape with your own files:

```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --hx path/to/hx.json --hz path/to/hz.json --iterations 5000 --restarts 8 --seed 7 --target-weight 5 --json
```

## JSON Result Fields

A completed random-window run has this shape:

```json
{
  "status": "completed",
  "method": "random-window-upper-bound",
  "bound_type": "upper",
  "upper_bound": 3,
  "logical_class": "x_like",
  "witness": {
    "x": [1, 1, 1, 0, 0, 0, 0],
    "z": [0, 0, 0, 0, 0, 0, 0],
    "weight": 3
  },
  "options": {
    "iterations": 500,
    "restarts": 4,
    "seed": 7,
    "target_weight": 3
  },
  "provenance": {
    "tool": "qec-code",
    "tool_version": "0.1.0",
    "method_revision": 1
  },
  "search_stats": {
    "permutations_sampled": 1,
    "kernel_basis_generations": 1,
    "component_candidates_generated": 1,
    "zero_candidates_rejected": 0,
    "weight_pruned_candidates": 0,
    "stabilizer_span_candidates_rejected": 0,
    "witness_validation_candidates_rejected": 0,
    "valid_witnesses_found": 1,
    "best_witness_updates": 1,
    "target_reached": true
  }
}
```

- `status`: `completed` means the search returned a validated logical witness.
- `method`: identifies `random-window-upper-bound`; use it to distinguish the
  windowed search from `randomized-upper-bound`.
- `bound_type`: `upper` means the result is an upper bound.
- `upper_bound`: the returned witness weight.
- `logical_class`: the logical class of the witness, such as `x_like` or
  `z_like`.
- `witness`: the Pauli support and its `weight`.
- `options`: the effective randomized-search options.
- `provenance`: the emitting tool version and method revision.
- `search_stats`: random-window diagnostic counters for sampled permutations,
  kernel basis generations, generated component candidates, rejection reasons,
  current-best weight pruning, valid witnesses, best-witness updates, and
  whether `target_weight` ended the run early.

When `bound_type: "upper"` appears in this JSON, the value is an upper bound from a randomized search, not a certified exact distance. Treat `upper_bound` as evidence that a logical operator of that weight was found; do not treat it as a proof that no lower-weight logical operator exists.

## Issue-225 Ladder Evidence

Issue #234 added the issue-225 ladder evidence tests for the windowed method.
The smoke command is intended for normal local checks:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
```

The full ladder is intentionally ignored by default. Run it explicitly when you
need the full acceptance evidence:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture
```

The old sampler remains available as a simple baseline. Issue #234 also keeps a
negative-control check showing that `randomized-upper-bound` is rejected by the
issue-225 ladder verifier on a known loose case:

```bash
cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q
```
