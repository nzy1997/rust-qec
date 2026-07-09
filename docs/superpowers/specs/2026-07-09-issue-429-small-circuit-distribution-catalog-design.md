# Issue 429 Small-Circuit Distribution Catalog Design

## Objective

Add a shared, validated small-circuit output distribution catalog under
`benchmarks/rstim_vs_stim_simulator/`. The catalog records source-grounded
expected sample probabilities from Stim's upstream `command_sample.test.cc` at
commit `9e225958f9ae1f9c33d1b9a012b7ec4392b43aef` so later correctness checks
can consume one reviewed source of truth.

## Explored Approaches

1. Extend `cases.full.toml` with distribution fields. This would reuse the
   existing validator but mix generated fixture metadata with inline
   small-circuit probability cases.
2. Add a separate `distribution_cases.toml` and validator. This matches the
   issue interface, keeps the small distribution schema focused, and avoids
   changing the fixture manifest contract from earlier benchmark work.
3. Add only unit-test-local expected distributions. This is smallest but would
   scatter the source-grounded probabilities across tests instead of creating
   the shared catalog requested by the issue.

Selected approach: option 2. Use a dedicated catalog and CLI validator, keeping
the implementation in the benchmark package beside `validate_cases.py`.

## Catalog Schema

Create `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml` with:

- `manifest_version = 1`
- `suite = "rstim_vs_stim_simulator"`
- `description`
- `distribution_tolerance = 1e-9`
- `[[cases]]` entries using flat review-friendly fields.

Each case has:

- `case_id`
- `source_url`
- `source_commit`
- `source_line_start`
- `source_line_end`
- `circuit`
- `shots`
- `tolerance`
- `expected_distribution`
- optional `source_expression` preserving upstream probability formulas.

The expected distribution is a TOML table mapping sample bitstrings to numeric
probabilities that sum to 1.0 within `tolerance`.

## Source-Grounded Cases

Include exactly the eight issue-recommended cases so the validator prints
`PASS 8 distribution cases`:

- `stim_bell_pair_basic_distribution`, lines 160-169:
  `H 0; CNOT 0 1; M 0 1` with `00 = 0.5`, `11 = 0.5`.
- `stim_sqrt_x_transformed_pair`, lines 171-180:
  `H 0; CNOT 0 1; SQRT_X 0 1; M 0 1` with `10 = 0.5`, `01 = 0.5`.
- `stim_sqrt_y_transformed_pair`, lines 182-191:
  `H 0; CNOT 0 1; SQRT_Y 0 1; M 0 1` with `00 = 0.5`, `11 = 0.5`.
- `stim_x_error_two_measured_qubits`, lines 194-202:
  `X_ERROR(0.1) 0 1; M 0 1` with probabilities
  `00 = 0.81`, `01 = 0.09`, `10 = 0.09`, `11 = 0.01`.
- `stim_z_error_h_conjugated_pair`, lines 216-226:
  `H 0 1; Z_ERROR(0.1) 0 1; H 0 1; M 0 1` with the same
  `0.81/0.09/0.09/0.01` distribution.
- `stim_y_error_two_measured_qubits`, lines 238-246:
  `Y_ERROR(0.1) 0 1; M 0 1` with the same
  `0.81/0.09/0.09/0.01` distribution.
- `stim_depolarize1_two_measured_qubits`, lines 260-268:
  `DEPOLARIZE1(0.3) 0 1; M 0 1` with per-qubit measurement flip rate
  `0.2`, giving `00 = 0.64`, `01 = 0.16`, `10 = 0.16`, `11 = 0.04`.
- `stim_depolarize2_two_measured_qubits`, lines 293-301:
  `DEPOLARIZE2(0.1) 0 1; M 0 1` with `00 = 0.92` and
  `01 = 10 = 11 = 0.02666666666666667`.

The source helper at lines 37-70 motivates the catalog's documented
probability-sum tolerance.

## Validator

Add `benchmarks/rstim_vs_stim_simulator/validate_distribution_cases.py`.
It loads TOML with `tomllib`, validates top-level manifest metadata, then
checks every case:

- all required fields are present;
- `case_id`, `source_url`, `source_commit`, and `circuit` are non-empty strings;
- `source_commit` equals the pinned 40-hex Stim commit;
- `source_line_start` and `source_line_end` are positive integers and ordered;
- `shots` is a positive integer;
- expected distribution keys are non-empty bitstrings containing only `0` and
  `1`, and all keys in a case have the same width;
- expected probabilities are numeric, finite, and between 0 and 1;
- expected probabilities sum to 1.0 within the case tolerance or manifest
  default tolerance.

On success the CLI prints exactly:

```text
PASS 8 distribution cases
```

On failure it prints actionable errors to stderr and exits nonzero. The
negative-control sum failure must contain:

```text
expected distribution probabilities must sum to 1
```

## Tests

Add `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py`
covering:

- CLI success output for the catalog;
- required case IDs and representative expected probabilities;
- rejection of missing pinned `source_commit`;
- rejection of missing source line metadata;
- rejection of invalid probability sums via
  `tests/fixtures/bad_distribution_sum.toml`.

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_distribution_cases \
  benchmarks/rstim_vs_stim_simulator/distribution_cases.toml
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
cargo test
```

## Scope Limits

Do not run Stim or `rstim` for this issue. Do not add benchmark evidence,
statistical correctness runs, generated outputs, or simulator behavior changes.

## Self-Review

- No placeholder or unresolved marker text remains.
- The selected schema contains every issue-requested field.
- The eight cases are tied to exact source line ranges from the pinned Stim
  file.
- The validator enforces pinned provenance and probability sums.
- Verification does not invoke Stim or `rstim`.
