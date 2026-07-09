# Issue 430 Distribution Verifier Design

## Objective

Add an executable statistical verifier for the source-grounded small-circuit
catalog from issue #429. The verifier runs `stim sample` and `rstim sample` on
each inline catalog circuit, compares each tool's observed output distribution
against the catalog's expected probabilities, and writes stable JSON evidence
that reviewers can inspect.

The issue requires the checked verifier command to pass on the catalog. During
context exploration, `rstim sample` currently emits deterministic results for
`H 0; M 0` unless an explicit reset precedes the Hadamard. Stim treats qubits as
initially reset to `|0>`, so the verifier would correctly fail the catalog
without a narrow sampler fix. This design includes that fix because it is needed
for the verifier to produce honest passing evidence.

## Explored Approaches

1. Extend `verify_correctness.py`. This reuses runner helpers, but that module
   compares `rstim` and Stim against each other through selected marginals and
   pairs. Issue #430 needs whole-line distributions compared independently to
   expected probabilities, so extending it would blur two verification
   contracts.
2. Add `verify_distributions.py` beside the existing validators and share only
   small helper patterns. This keeps the catalog distribution verifier focused,
   preserves the existing correctness verifier, and matches the requested module
   path.
3. Add unit tests that call sampler APIs directly instead of CLI tools. This
   would be cheap but would not record commands, tool status, stderr, or
   reviewer-readable evidence from the public binaries.

Selected approach: option 2. Add a dedicated Python verifier CLI and focused
tests. Also add the minimal `rstim` frame-sampler initialization correction
needed for Stim-compatible initial-qubit sampling.

## Verifier CLI

Create `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`.

The CLI accepts:

- `--cases`, defaulting to no implicit path and required by argparse;
- `--stim`, default `"stim"`;
- `--rstim`, defaulting to the same binary-or-Cargo fallback pattern used by
  `verify_correctness.py`;
- `--shots`, required positive integer;
- `--seeds`, comma-separated list with default `12345`;
- `--out`, required JSON output path;
- `--inject-rstim-bitflip-rate`, default `0.0`, for the required negative
  control.

For each case, send the inline circuit on stdin and invoke:

```text
<tool> sample --shots <shots> --seed <seed> --out_format 01
```

The command list, exit code, success flag, stderr, and `stdin_source =
"catalog:circuit"` marker are recorded per run. This avoids machine-specific
temporary paths in the JSON. The verifier does not include elapsed timings.

## Distribution Check

For every successful tool run, parse `01` output into bitstrings and count
observed frequencies across all requested seeds. Compare each observed outcome
to the catalog's `expected_distribution` with a five-standard-deviation
tolerance:

```text
tolerance = 5 * sqrt(p * (1 - p) / sample_count) + floor
```

Use a small numeric floor, `1e-12`, only to avoid floating-point edge issues for
probabilities exactly 0 or 1. Include zero-count expected outcomes and unexpected
observed outcomes in the comparison. Unexpected outcomes use expected
probability 0 and therefore fail unless they never appear.

Per tool status is:

- `pass` if every outcome is within tolerance;
- `statistical_mismatch` if parsing succeeds but any outcome exceeds tolerance;
- `tool_failed` if the subprocess exits nonzero or the output is malformed.

Per case status is `pass`, `statistical_mismatch`, `stim_failed`, or
`rstim_failed`, with tool failures taking priority over statistical mismatches.
The suite status uses the same priority. A passing suite prints:

```text
PASS distribution correctness cases=8 mismatch=0
```

A statistical negative control prints:

```text
FAIL statistical mismatch cases=8 mismatch=<nonzero>
```

## JSON Evidence

Write stable, sorted, indented JSON with:

- manifest path, suite, status, case count, shots, seeds, commands, and injection
  rate;
- counts for pass, statistical mismatch, Stim failure, and `rstim` failure;
- per-case `case_id`, `source_url`, `source_commit`, source lines, expected
  probabilities, tolerance metadata, sample count, status, and failure reasons;
- per-tool observed counts, observed frequencies, per-outcome deltas,
  tolerances, status, and recorded run metadata.

The JSON is written even on statistical mismatch so the negative control leaves
reviewable evidence. Invalid CLI arguments or invalid manifests fail before
writing JSON.

## Rstim Sampling Fix

The frame sampler should model Stim's implicit initial `|0>` state. In a Pauli
frame simulator, a fresh Z-basis reset clears the X frame and randomizes the
conjugate Z frame so future non-commuting measurements produce random outcomes.
Initial qubits need the same conjugate-frame randomization. Add a
`FrameSimulator` method that randomizes every qubit's Z frame once at the start
of frame-based sampling, and call it from both interpreted and compiled sampler
paths before running the circuit.

Add focused regression tests that `sample_batch` observes both `0` and `1` for
`H 0; M 0` and only `00`/`11` outcomes for the Bell case. Existing deterministic
Z-basis tests should still pass because measurement uses the X frame, which
remains zero until a non-commuting operation exposes the randomized conjugate
frame.

## Tests

Add `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`
covering:

- sample-output parsing and distribution counting;
- expected-probability tolerance pass and mismatch behavior;
- command construction and tool failure evidence;
- per-case JSON shape with expected probabilities, observed frequencies,
  source provenance, status, sample count, tolerance, and stderr;
- CLI success output and sorted JSON with mocked tool runs;
- negative-control injection returning nonzero and creating at least one
  mismatching case.

Add Rust sampler tests for the initial-state frame fix.

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --shots 100000 \
  --out /tmp/rstim-vs-stim-distributions.json
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --shots 100000 \
  --inject-rstim-bitflip-rate 0.20 \
  --out /tmp/rstim-vs-stim-distributions-bad.json
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
cargo test
```

## Scope Limits

Do not publish checked evidence, update the benchmark site, or add performance
timing fields. Do not change the source-grounded catalog probabilities from
issue #429. Keep the simulator fix scoped to the implicit initial-state
randomness needed by Stim-compatible sampling.

## Self-Review

- No unresolved marker text remains.
- The selected CLI path matches the issue request.
- The verifier compares each tool to expected probabilities instead of comparing
  Stim and `rstim` shot-by-shot.
- The JSON evidence includes commands, tool status, stderr, expected
  probabilities, observed frequencies, tolerance, sample count, status, and
  provenance URL.
- The negative-control injection is designed to fail statistically and still
  write evidence.
- The sampler fix is limited to the initial Pauli-frame state needed for Stim
  semantics and does not add timing or benchmark-site output.
