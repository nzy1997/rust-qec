# Issue 437 Expanded rstim-vs-Stim Evidence Checker Design

Issue: #437

## Context

The dependency chain is complete on `master`:

- issue #432 publishes the source-grounded distribution evidence and the
  expanded correctness rollup under `results/distributions/`;
- issue #416 publishes the selected d11/r100 release sample evidence under
  `results/release/`, separately from the old issue #406 debug artifact;
- issues #434 and #435 publish release evidence for
  `rep-sample-d13-r13` and `surface-detect-d13-r13`;
- issue #436 publishes checked DEM sampling evidence under
  `results/release-dem-sample/`.

Each area already has a focused checker. Issue #437 adds the umbrella contract
that prevents any required area or case from silently disappearing. It does not
publish or rerun benchmarks and does not turn recorded timings into thresholds.

## Approaches Considered

1. Add a thin Python composition layer around the existing focused validators,
   with an internal manifest for the required speed cases and a preflight case
   coverage check. This is selected because it preserves the stronger existing
   correctness and DEM provenance checks, gives the umbrella exact missing-case
   diagnostics, and minimizes duplicated validation logic.

2. Run each existing checker as a subprocess. This would reuse their public
   commands, but composing stdout, stderr, and exit codes would make the umbrella
   noisy and would not support an order-independent `--speed-dirs` list cleanly.

3. Reimplement every correctness, speed, and DEM validation inside one new
   script. This would provide one self-contained file but would duplicate large
   provenance contracts and allow the focused and umbrella checkers to drift.

## Command Interface

Add `tools/check_rstim_vs_stim_expanded_evidence.py` with this interface:

```sh
python3 tools/check_rstim_vs_stim_expanded_evidence.py \
  --correctness-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-correctness benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json \
  --speed-dirs benchmarks/rstim_vs_stim_simulator/results/release,benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample,benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --dem-speed-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
```

`--catalog` is optional and defaults to
`benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`. The distribution
catalog is the correctness case manifest. The circuit and DEM speed case lists
are stable constants in the checker so callers cannot weaken required coverage
through command-line options.

On success the checker prints exactly:

```text
PASS expanded rstim-vs-Stim evidence
```

On failure it prints one validation message to stderr and exits nonzero.

## Required Evidence Manifest

The three comma-separated `--speed-dirs` are order-independent. Across those
directories the checker requires each of these cases exactly once:

| Case | Workload | Required variants | Release provenance |
| --- | --- | --- | --- |
| `stim-style-surface-sample-d11-r100-b1024` | `sample` | `stim-cli`, `rstim-interpreted`, `rstim-compiled` | issue 416 post-optimization published evidence |
| `rep-sample-d13-r13` | `sample` | `stim-cli`, `rstim-interpreted`, `rstim-compiled` | issue 434 published evidence |
| `surface-detect-d13-r13` | `detect` | `stim-cli`, `rstim-interpreted`, `rstim-compiled` | issue 435 published evidence |

The separate `--dem-speed-dir` must contain
`stim-style-surface-dem-sample-d11-r100-b1024` with variants
`stim-sample-dem` and `rstim-sample-dem`.

Missing cases use the stable diagnostic:

```text
missing required evidence case <case-label>
```

This diagnostic also applies when the DEM directory has no `summary.json`, as
required by the issue's negative control. Duplicate required cases are rejected
so one evidence area cannot be ambiguously sourced from multiple directories.

## Validation Flow

### Correctness

Load `summary.json`, `expanded-correctness.json`, and `report.md` from
`--correctness-dir`, then call the validation functions from
`tools/check_rstim_vs_stim_expanded_correctness.py`. This preserves the catalog
hash, complete distribution case set, environment provenance, rollup artifact
hashes, report references, and full d11/r100 top-level pass checks.

### Circuit speed evidence

First scan every supplied speed summary and map required case labels to their
directories. Reject missing and duplicate coverage before running detailed
checks. For each mapped case, reuse
`tools/check_rstim_vs_stim_release_speed_case.py` to validate the exact artifact
set, workload, completed variants, release profile, required toolchain fields,
case labels in the environment, and case label in the report.

The umbrella additionally requires `published_artifact = true`, the expected
`source_issue`, and the expected evidence-kind wording. For the selected
d11/r100 release case, it compares the parsed summary with
`results/full/speed-summary.json` and rejects equality with
`release evidence reuses old #406 debug summary`. This semantic comparison
also catches a byte-for-byte copy that has merely been reformatted.

No wall-time, throughput, or Stim ratio field is used as a pass/fail threshold.

### DEM speed evidence

Preflight `summary.json` for the required DEM case and stable missing-case
diagnostic. Then call the issue #436 checker functions to validate pinned DEM
metadata, `raw.jsonl`, summary workload/tier/variant completion, release rounds,
fixture hashes, and environment provenance. Add the umbrella-level checks that
the report mentions the required case and the environment records non-empty
`rstim_binary_path`, `rustc_version`, `cargo_version`, and `stim_cli_status`.

## Tests and Documentation

Add `tools/test_check_rstim_vs_stim_expanded_evidence.py` using subprocesses and
temporary copied speed directories. Cover:

- the committed full evidence pack and exact PASS line;
- removal of `surface-detect-d13-r13`, expecting the required missing-case
  diagnostic;
- a DEM directory without `summary.json`;
- a DEM summary without the required case;
- a missing required speed variant;
- missing environment metadata;
- reuse of the checked issue #406 debug summary as selected release evidence.

Add the umbrella command and expected PASS line to
`benchmarks/rstim_vs_stim_simulator/README.md`. Run the issue checker command,
its unit tests, and `cargo test` before completion.

## Scope Limits

- Do not generate, refresh, or otherwise modify benchmark result artifacts.
- Do not add speed ratio or wall-clock thresholds.
- Do not change sampler, detector, or DEM semantics.
- Do not update the public benchmark site in this issue.
- Keep focused checkers independently usable; the umbrella imports their
  validation functions but does not replace their commands.

## Self-Review

- The command and PASS line match issue #437.
- Every dependency evidence area maps to a concrete validation step.
- Both required negative-control messages are specified exactly.
- The design rejects issue #406 debug summary reuse without relying on timing
  thresholds.
- The expected case manifest cannot be weakened through caller arguments.
- No placeholders, contradictory requirements, benchmark publication, or broad
  performance claims remain.
