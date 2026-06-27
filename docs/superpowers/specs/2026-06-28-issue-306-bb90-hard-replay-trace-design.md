# Issue 306 BB90 Hard-Replay Correction Trace Design

## Context

Issue #306 is a diagnosis task for the pinned
`bb90-p006-c10-seed12345-order7-hard-syndrome` replay. The existing
`hard-replay` tier exports paired Rust `rbposd` and Python `ldpc_bposd`
CSV rows and the current CSV verifier fails when final logical predictions
diverge. That failure is useful but too coarse: the artifact does not preserve
correction support, correction weight, or residual-syndrome evidence for both
decoders.

The dependency from issue #305 is present on `master`: the Bravyi source
contract pins the upstream settings to `bp_method=ms`, `max_iter=10000`,
`osd_method=osd_cs`, `osd_order=7`, and `ms_scaling_factor=0`.

## Scope

This change adds a deterministic diagnostic artifact for the existing
hard-replay tier:

```text
<output-dir>/hard_replay_trace.json
```

The existing `results.csv`, `summary.md`, and `verify_replay` behavior remain
intact. This PR records the known mismatch; it does not change decoding
behavior or try to make Rust match Python.

## Chosen Approach

Add a small trace writer inside `benchmarks.bb_circuit_bposd_compare.run_compare`
and a companion verifier module named
`benchmarks.bb_circuit_bposd_compare.verify_replay_trace`.

The Rust side extends the comparison export schema with the Z and X correction
bit vectors returned by `rbposd` for collected trials. The hard replay consumes
only the Z correction because the pinned fixture basis is `Z`. The Python side
already receives the correction vector from `BpOsdDecoder.decode(...)`; the
trace writer converts both corrections to sorted support lists and weights.

## Alternatives Considered

1. Add more columns to `results.csv`.
   This would make the existing CSV verifier and readiness gate carry
   diagnosis-only fields. It is a wider compatibility surface than the issue
   needs.

2. Write only a Python-side trace.
   This would prove the Python replay path but would not expose the Rust
   correction-level data needed to locate the divergence.

3. Add a separate one-case JSON trace artifact.
   This keeps the current CSV contract stable, preserves symmetric decoder
   correction data, and matches the issue's requested output. This is the
   selected approach.

## Trace Shape

The artifact is one JSON object:

```json
{
  "schema_version": 1,
  "case_id": "bb90-p006-c10-seed12345-order7-hard-syndrome",
  "basis": "Z",
  "syndrome_support": [0, 2, 3],
  "syndrome_weight": 3,
  "expected_sampled_logical": [false, true],
  "classification": "logical_prediction_mismatch",
  "decoders": [
    {
      "decoder_impl": "rbposd",
      "bp_osd_settings": {
        "bp_method": "ms",
        "max_iter": 10000,
        "osd_method": "osd_cs",
        "osd_order": 7
      },
      "correction_support": [0, 2, 3],
      "correction_weight": 3,
      "residual_syndrome_weight": 0,
      "residual_syndrome_support": [],
      "residual_syndrome_matches": true,
      "predicted_logical": [false, true],
      "profile": {
        "decode_call_count": 1,
        "bp_iteration_count": 10000,
        "osd_use_count": 1,
        "osd_candidate_count": 4100,
        "gf2_solve_count": 4101,
        "gf2_full_elimination_count": 1
      }
    },
    {
      "decoder_impl": "ldpc_bposd",
      "bp_osd_settings": {
        "bp_method": "ms",
        "max_iter": 10000,
        "osd_method": "osd_cs",
        "osd_order": 7,
        "ms_scaling_factor": 0,
        "input_vector_type": "syndrome"
      },
      "correction_support": [0, 2, 3],
      "correction_weight": 3,
      "residual_syndrome_weight": 0,
      "residual_syndrome_support": [],
      "residual_syndrome_matches": true,
      "predicted_logical": [true, false]
    }
  ]
}
```

The example arrays are illustrative; tests use small fake exports, and the
real replay determines the concrete supports.

## Classification Rules

The trace classification is:

- `logical_prediction_mismatch` when both decoder entries are present,
  paired on case, basis, syndrome support, and expected sampled logical, but
  their predicted logical vectors differ.
- `matched` when both predicted logical vectors are identical.
- `incomplete` when a decoder entry is skipped or unavailable.

For issue #306, the expected real hard replay classification is
`logical_prediction_mismatch`.

## Verifier

`verify_replay_trace` validates one JSON trace file. It requires:

- top-level `schema_version`, `case_id`, `basis`, `syndrome_support`,
  `syndrome_weight`, `expected_sampled_logical`, `classification`, and
  `decoders`;
- exactly one `rbposd` entry and one `ldpc_bposd` entry unless the trace is
  explicitly `incomplete`;
- both completed entries include non-empty `correction_support`,
  `correction_weight`, `residual_syndrome_matches`,
  `residual_syndrome_weight`, `residual_syndrome_support`, and
  `predicted_logical`;
- correction weights equal support lengths;
- residual status is boolean and residual supports have the declared weight;
- both entries are paired on the top-level syndrome and expected logical;
- the recorded classification matches the two logical predictions.

The CLI prints a compact success line containing `case_id=...`, `basis=Z`,
and `classification=...`. On failure it exits nonzero and names the missing or
unpaired field.

## Testing

Tests cover:

- `run_hard_replay_suite` writes `hard_replay_trace.json` next to the existing
  CSV for fake paired Rust/Python hard replay data;
- the trace records both decoder correction supports, weights, residual
  checks, predicted logical vectors, and Rust profile counters;
- `verify_replay_trace` accepts a valid logical mismatch trace;
- the verifier rejects a missing Python correction support field;
- the verifier rejects unpaired decoder syndrome metadata;
- Rust comparison export JSON includes `z_correction` for collected Z replay
  trials.

The full issue verification remains:

```bash
cargo build --release -p rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-trace \
  --rust-binary target/release/rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay_trace \
  /tmp/rstim-bb90-hard-trace/hard_replay_trace.json
```

The existing `cargo test` gate must also pass.
