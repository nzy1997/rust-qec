# Near-Clifford tableau row-operation campaign

This campaign compares merged #770 (`68903574`) with the row-product implementation
at `8438bd9e`. It reuses the immutable 43-configuration scale and 28-configuration
entangled timing drivers, fixture generators, unified dependency lock, six timing
modes, warmup policy and untimed diagnostic overlay. Historical inputs and evidence
are unchanged. No cache, RNG, probability reduction or public API change is involved.

Use fresh scratch directories:

```sh
python3 benchmarks/near_clifford/row_ops/run.py --suite entangled \
  --baseline 68903574abcb70f1c74e4581fc676704f2ed770d \
  --candidate 8438bd9e5dcf4549ff0e1a8235277c0659848210 \
  --scratch drafts/row-ops-entangled-reproduction \
  --output drafts/row-ops-entangled-reproduction/results.json
python3 benchmarks/near_clifford/row_ops/scale.py \
  --baseline 68903574abcb70f1c74e4581fc676704f2ed770d \
  --candidate 8438bd9e5dcf4549ff0e1a8235277c0659848210 \
  --scratch drafts/row-ops-scale-reproduction \
  --output drafts/row-ops-scale-reproduction/results.json
python3 benchmarks/near_clifford/row_ops/verify.py RESULTS.json \
  --git-sources --binaries CAMPAIGN_SCRATCH
python3 benchmarks/near_clifford/row_ops/test_contract.py
python3 -O benchmarks/near_clifford/row_ops/test_contract.py
```

Use `scale.py` for the complete scale campaign: it retains all four pristine and
diagnostic semantic payload sets. The lower-level `run.py --suite scale` entry
only provides inherited timing/output checks and does not satisfy the final
semantic-payload verifier. Smoke/partial campaigns are also rejected.

The new wrapper binds `tableau.rs` separately in both pristine and diagnostic
builds. Verification checks exact source/overlay/oracle hashes, binary hashes,
full semantic payload equality, RNG continuation, historical circuit identity,
raw medians, alternating pair order and cache counters/budgets. If a source commit
is unavailable in a clone after squash merging, `--git-sources` accepts a selected
source input only when its bound hash matches that exact file in HEAD; a present
source commit is always checked directly. This does not reproduce historical timing.

The entangled dense-oracle coverage remains 14 exact-width and 14 reduced family
witnesses, with 2,682 conditional-probability comparisons per revision. Reduced
witnesses do not validate the entire wide circuit distribution. The added Rust
integration test provides a separate full-width magic-GHZ analytic witness through
193 qubits, including mixed X/Y measurements and reset. It does not establish
frequency accuracy for every synthetic workload.

After timing and other CPU-intensive work finish:

```sh
python3 benchmarks/near_clifford/row_ops/profile.py ENTANGLED_RESULTS.json \
  --scratch ENTANGLED_SCRATCH --output drafts/row-ops-profiles
python3 benchmarks/near_clifford/row_ops/plot.py RESULTS.json --output drafts/row-ops-plot
```

Profiles use a separate debug/frame-pointer release build and bind its tableau,
near-Clifford source, lock, driver, binary and exact circuit hashes. Inclusive
profile percentages can overlap and must not be added. Plots show the range of
three paired process ratios, not confidence intervals. RSS includes validation,
multiple samplers and outputs, not an isolated cache's memory use. Timing is
Apple M4-only; x86 has not been measured.

See the [retained analysis](../results/apple-m4-row-ops-analysis-2026-10-03.md).
