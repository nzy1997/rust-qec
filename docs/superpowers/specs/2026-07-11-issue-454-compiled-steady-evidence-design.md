# Issue 454 Compiled Steady Evidence Design

Issue: #454 Publish compiled steady-state sampling evidence

## Context

Issue #453 added `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py`
and long-lived Stim/rstim workers. The runner emits one `ready`, nine
`sample`, and one `final` raw record per worker, then writes a summary,
report, and environment metadata. Issue #454 asks for a durable checked bundle
under `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/`
with an independent checker that derives lifecycle and measured timing claims
from `raw.jsonl` before trusting any summary, report, environment, or hash
manifest.

## Approach

The checker will treat `raw.jsonl` as the source of truth. It will validate the
two #453 raw worker variants (`stim` and `rstim`) and expose them as the release
evidence variants `stim-compiled-steady-b8` and `rstim-compiled-steady-b8` in
derived summaries, reports, and error messages. This keeps the raw #453
telemetry shape intact while giving #454 a stable release-evidence label.

Alternative approaches considered:

1. Modify the #453 runner and workers to emit the full #454 labels directly.
   This would satisfy the release labels at the source, but it would widen the
   change into already-closed runner behavior and force churn in existing #453
   tests.
2. Add a separate post-processing script that rewrites raw records into a new
   schema. This would make the bundle labels explicit, but it would weaken the
   "raw telemetry" requirement by introducing a transformation layer.
3. Add a checker that consumes the #453 raw schema and derives the #454 release
   view. This is the chosen approach because lifecycle counters, request IDs,
   response byte counts, and timing remain anchored in raw records.

## Files

- Add `tools/check_rstim_vs_stim_compiled_steady_evidence.py`.
- Add `tools/test_check_rstim_vs_stim_compiled_steady_evidence.py`.
- Add the checked bundle files under
  `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/`.

No site manifest or showcase page will be updated.

## Checker Behavior

The checker validates in this order:

1. Required bundle files exist.
2. `raw.jsonl` parses and has exactly two variants, each with one `ready`, nine
   `sample`, and one `final` record.
3. Per-variant semantics are valid before any artifact hash check:
   - ready telemetry has compile count 1, reference-build count 1, and
     sample-call count 0;
   - sample records have request IDs 0 through 8, two warmups, seven measured
     records, cumulative response sample-call counts 1 through 9, 1024 shots,
     12,121 measurements, `b8`, 1,516 bytes/shot, and 1,552,384 response bytes;
   - final telemetry has compile count 1, reference-build count 1, and
     sample-call count 9;
   - ready, sample, response, final, fixture, and environment values agree.
4. The canonical `summary.json` and `report.md` are regenerated from measured
   raw records only and must match byte-for-byte.
5. Environment provenance is checked against raw-derived lifecycle values and
   #453 provenance fields, including fixture, source manifest, fair manifest,
   worker argv, Python executable, loaded Stim extension, rstim worker binary,
   protocol version, seed policy, warmup/measurement counts, and path hashes.
6. `artifact-sha256.json` is verified last and must map exactly the other four
   files to lowercase SHA-256 digests.

The negative-control final `compile_count` mutation must fail with
`rstim-compiled-steady-b8 final compile_count must be 1, got 9` before any hash
error.

## Evidence Bundle

The release bundle will be generated with the #453 command:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_compiled_steady \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 --profile release \
  --warmup-rounds 2 --measure-rounds 7 --seed 0 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
```

After generation, `artifact-sha256.json` will be written over `raw.jsonl`,
`summary.json`, `report.md`, and `environment.json`.

## Testing

Unit tests will create small valid bundles using the same raw schema and the
same checker derivation functions, then mutate them to prove:

- removing a raw sample request is detected while environment still claims
  lifecycle `1/1/9`;
- duplicating a request ID is detected;
- changing a cumulative sample-call count is detected;
- changing the rstim final compile count fails semantically before hash checks;
- a rehashed but altered `summary.json` is rejected as not derived from raw;
- removing `artifact-sha256.json` fails.

Final verification:

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
cargo test
```
