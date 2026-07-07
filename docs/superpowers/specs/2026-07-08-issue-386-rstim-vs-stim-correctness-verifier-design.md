# Issue 386 rstim-vs-Stim Correctness Verifier Design

## Objective

Add a statistical correctness verifier for the shared
`benchmarks/rstim_vs_stim_simulator/` fixture catalog. The verifier runs Stim
and `rstim` against the same checked `.stim` circuit text, compares observable
sample properties instead of shot-by-shot RNG streams, and writes reviewer
evidence even when one tool fails or the statistical comparison fails.

## Selected Approach

Implement a standalone Python module:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness
```

The module reuses `validate_cases.load_manifest`, `validate_cases.validate_manifest`,
and the catalog path-resolution convention from #385. It does not change the
fixture manifest schema. By default it executes `smoke` and `full` cases and
skips `documentation-only` entries, so the existing smoke manifest can document
the full d=11/r=100 case without making the smoke verifier run it.

CLI inputs:

- `--cases`: TOML fixture manifest.
- `--stim`: Stim CLI path, defaulting to `stim`.
- `--rstim`: `rstim` binary path, defaulting to `target/debug/rstim` when it
  exists and `rstim` otherwise.
- `--shots`: positive shot count overriding manifest shots.
- `--seeds`: comma-separated seed list, defaulting to one deterministic seed.
- `--out`: JSON summary path.
- `--inject-rstim-bitflip-rate`: verifier-only negative control that flips
  `rstim` sample bits after command execution.

## Sampling Model

Each executable catalog case has detectors and observables, so the verifier
uses detector-output sampling:

```sh
stim detect --shots N --seed S --append_observables --out_format 01 < circuit.stim
rstim detect --shots N --seed S --append_observables --out_format 01 --in circuit.stim
```

If a future case has no detectors, the verifier can fall back to
`stim sample` / `rstim sample` with `out_format 01`. The expected bit count is
`expected_detectors + expected_observables` for detector cases and
`expected_measurements` for measurement cases.

The verifier parses `01` output as a rectangular matrix with one row per shot.
Malformed output is recorded as a tool failure instead of crashing before the
JSON report is written.

## Statistics

For each tool, case, and seed, the verifier computes:

- selected marginal firing rates;
- selected pair co-firing rates;
- sample counts for every compared statistic.

Selected columns are deterministic and bounded for readable JSON. The default
selection includes the first few columns, the final observable columns, and a
small middle sample when available. Pair checks use adjacent selected columns
and detector-observable pairs when observables exist.

Each statistic compares two independent binomial estimates with a conservative
normal confidence bound:

```text
tolerance = z * sqrt(pooled * (1 - pooled) * (1/n_stim + 1/n_rstim)) + floor
```

where `z = 6.0` by default and `floor = 0.01`. This is intentionally generous
for smoke evidence and avoids hiding low-rate noise by still reporting the
observed deltas. A case status is `statistical_mismatch` when any selected
statistic exceeds its tolerance.

## Evidence JSON

The output JSON is a stable evidence summary with:

- manifest path, command-line configuration, overall status, and verdict;
- one case object per manifest entry;
- tool command, exit code, stderr, elapsed seconds, and parsing status;
- per-case selected marginal and pair statistics with Stim rate, `rstim` rate,
  delta, tolerance, sample count, and pass/fail;
- skipped documentation-only cases with status `skipped`.

Status values include:

- `pass`
- `statistical_mismatch`
- `stim_failed`
- `rstim_failed`
- `skipped`

Stim or `rstim` command failures take precedence over statistical comparison
for the affected case, but all cases still appear in the JSON report.

## Text Report

The CLI prints a compact reviewer-readable report:

- `PASS correctness smoke` when all executable cases pass.
- `FAIL statistical mismatch` when any executable case has a statistical
  mismatch.
- `FAIL tool failure` when any executable case has a Stim or `rstim` failure.
- per-case lines showing case ID, status, sample counts, selected column count,
  selected pair count, maximum delta, and failure reason.

The exit code is 0 only for an all-pass executable set. Negative-control
bit-flip injection should therefore exit nonzero and print
`FAIL statistical mismatch`.

## Tests

Add Python unit tests under
`benchmarks/rstim_vs_stim_simulator/tests/` for:

- statistical helper behavior for clear pass and mismatch cases;
- CLI behavior with mocked subprocess results so pass, mismatch, and tool
  failure paths write JSON evidence;
- negative-control bit flipping on parsed `rstim` samples.

Run the issue's explicit smoke and negative-control commands, the package unit
tests, the fixture validator tests, and `cargo test`.

## Scope Limits

This change does not optimize `rstim`, fix simulator discrepancies, or require
the full catalog to pass quickly. Failed or slow tool runs are evidence and are
recorded in the summary.

## Self-Review

- No unresolved marker text remains.
- The design consumes #385 manifests without schema churn.
- Tool failures and statistical failures both produce JSON evidence.
- The negative control exercises the verifier rather than modifying fixtures or
  simulator code.
