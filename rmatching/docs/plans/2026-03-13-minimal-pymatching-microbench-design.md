# Minimal Benchmark Design: rmatching vs PyMatching

## Goal

Evaluate the current repository against PyMatching starting from the smallest possible decoding examples, with emphasis on:

- decode time
- prediction accuracy

This benchmark is intentionally narrower than the existing surface-code benchmark. It removes circuit sampling, DEM generation, and file parsing from the main comparison so the first results isolate decoder behavior.

## Scope

The benchmark compares `rmatching` and `PyMatching` on hand-written DEM cases only. Each decoder receives the same DEM and the same precomputed syndrome set.

The suite is split into three minimal tiers:

1. `boundary-2`
   Two detectors with boundary edges. This measures the lowest-complexity decode path.
2. `square-4`
   Four detectors with a small logical edge. This adds observable prediction while staying easy to inspect.
3. `blossom`
   A minimal odd-cycle case that can exercise blossom-specific behavior.

These tiers must be reported separately. They are not collapsed into a single score.

## Metrics

Accuracy is defined as agreement with PyMatching on a per-syndrome basis, not logical error rate. For each syndrome:

- decode with PyMatching
- decode with rmatching
- compare the full observable prediction vector

Reported metrics per `case + decoder` row:

- `case_name`
- `num_detectors`
- `num_edges`
- `num_syndromes_tested`
- `prediction_match_rate`
- `mismatch_cases`
- `mean_decode_us`
- `median_decode_us`
- `p95_decode_us`

Optional secondary metric:

- `build_us`

`build_us` is recorded separately and not folded into the main performance conclusion.

## Data Flow

Each case is defined once as:

- a DEM string
- metadata such as detector count and edge count
- a complete syndrome set

The syndrome set should be exhaustive for these tiny graphs. Exhaustive enumeration is preferred over random sampling because it guarantees coverage of corner cases and makes mismatches reproducible.

The runtime flow is:

1. construct a PyMatching decoder from the DEM
2. construct an rmatching decoder from the same DEM
3. enumerate all syndromes for the case
4. warm up both decoders
5. run repeated decode passes over the same syndrome list
6. collect timing summaries and exact prediction mismatches

The primary timing scope is decode only. Graph construction is measured separately if needed.

## Implementation Shape

Use Python as the benchmark driver because PyMatching is already exposed there. Add a small Rust CLI that accepts a DEM and a batch of syndromes, decodes them with `rmatching`, and prints predictions plus timing data.

The Python driver owns:

- case definitions
- syndrome enumeration
- PyMatching decode
- invocation of the Rust CLI
- result aggregation
- CSV and mismatch artifact generation

This keeps both decoders on one source of truth for inputs and prevents the two sides from drifting.

## Outputs

Produce two artifacts:

- a summary CSV for performance and agreement metrics
- a mismatch JSON for detailed failing syndromes, if any

If a case fails to build on either side, mark it as a build failure and continue with the remaining cases.

## Verification

Verification should happen in three layers:

1. unit tests for case definitions and syndrome enumeration
2. golden comparisons on known tiny cases
3. a benchmark smoke test that generates the summary artifacts

Success means:

- all benchmark cases build in both decoders
- `prediction_match_rate` is 100% for all cases
- timing numbers are produced for all cases
