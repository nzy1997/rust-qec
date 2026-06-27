# Issue 307 BB90 LDPC OSD-CS Parity Design

## Context

Issue #307 asks the Rust `rbposd` `ldpc_osd_cs` / `osd_cs` path to match
Python `ldpc.BpOsdDecoder` on the pinned
`bb90-p006-c10-seed12345-order7-hard-syndrome` replay. The #306 trace artifact
shows both decoders return residual-zero Z-basis corrections, but the
correction supports and projected logical predictions differ.

Local replay with Python `ldpc==2.4.1` reproduced the failure:

- Rust prediction: `[true, true, false, true, true, false, false, true]`
- Python prediction: `[true, true, false, true, true, false, false, false]`
- `verify_replay` rejects the CSV with
  `Rust/Python logical predictions do not match`

Both Rust and Python project correction bits through the same exported
`augmented_columns` / `first_logical_row` model, so the mismatch belongs in
the decoder path rather than the BB logical projection.

## Chosen Approach

Change only the `OsdVariant::LdpcCombinationSweep` branch to match the Python
`ldpc` OSD handoff:

1. Keep BP as the first decode stage and continue returning the BP correction
   when BP converges.
2. When BP does not converge and `ldpc_osd_cs` is selected, run OSD against
   the original syndrome with an all-zero base correction instead of solving
   the residual around the BP hard decision.
3. Order OSD columns using the signed BP posterior log-probability ratios,
   matching the upstream C++ `soft_decision_col_sort(log_prob_ratios, ...)`
   input, instead of ordering by absolute reliability.
4. Score LDPC OSD-CS candidates with `log(1 / p_i)` channel-probability
   weights, matching the upstream OSD candidate-weight objective.
5. Leave legacy combination sweep and explicit OSD-0 behavior unchanged.

This keeps `osd_cs` and `ldpc_osd_cs` aliases on the bounded LDPC-compatible
path and preserves the single-elimination candidate-count constraints from
#280/#281/#282.

## Alternatives Considered

1. Adjust the BB logical projection.
   This is rejected because the trace shows the correction supports differ and
   both Python and Rust apply the same projection to the exported model.

2. Change all OSD variants to solve original syndromes.
   This is too broad: existing Rust legacy OSD and OSD-0 fixtures intentionally
   lock the current residual-around-BP semantics.

3. Change only LDPC OSD-CS handoff, ordering, and objective weights.
   This is the narrowest behavior change that matches the upstream `ldpc`
   path observed in the pinned replay. This is the selected approach.

## Testing

Add a focused `rsinter` regression beside the BB90 fixture that runs the
checked-in hard replay through `export_comparison_case_for_code_with_osd_variant`
with `OsdVariant::LdpcCombinationSweep` and asserts the Z logical prediction is
the Python-pinned vector:

```text
[true, true, false, true, true, false, false, false]
```

The existing hard replay CSV verifier remains the end-to-end parity gate.

Required verification:

```bash
cargo build --release -p rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-replay-fixed \
  --rust-binary target/release/rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay \
  /tmp/rstim-bb90-hard-replay-fixed/results.csv
cargo test --release -p rsinter bb90_hard_syndrome
cargo test -p rbposd osd
cargo test
```

Also run the requested negative control by flipping one Rust
`logical_prediction` bit in a copied fixed CSV and confirming
`verify_replay` exits nonzero with
`Rust/Python logical predictions do not match`.

## Self-Review

- No placeholders remain.
- Scope is one decoder semantic boundary and one fixture-level regression.
- The design keeps legacy OSD paths and performance counters intact.
- The design explicitly chooses the decoder fix over a projection change based
  on trace evidence.
