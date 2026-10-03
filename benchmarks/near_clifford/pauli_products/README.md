# Near-Clifford Pauli reconstruction controls

This follows #771's tableau-row improvement with a fused `Pauli::multiply_tableau_row`
traversal. The two kernels use different phase conventions: this product stores
`i^phase X^x Z^z`, while tableau rows use canonical local Paulis. Keep their tests
and implementations separate.

The direct-query controls prepare the same full-width magic-GHZ state using star
or chain CX frames. A Y probability query at qubit zero selects width+1 tableau
rows in the star frame and three rows in the chain frame. The separate diagnostic
overlay counts actual row products and processed entries, checking these expected
values. Every qubit participates; there is no idle padding. Rank remains one and
the direct ActiveState API does not use the terminal cache.

Widths are 63/64/65/127/128/129/193. Each pristine process makes 32 warmup queries
then three repetitions of 1024 queries. Three baseline/candidate process pairs
alternate order per fixture. Preparation, full-width conditional Born checks and
diagnostic counters are outside timing. This isolates probability-query cost,
including coordinate conversion and allocation; it is not end-to-end shot sampling
or an isolated function-level speedup. Paired ranges are not confidence intervals.

```sh
python3 benchmarks/near_clifford/pauli_products/run.py \
  --baseline 68b907dc9f6c3e938a98a07624592b0865ed6852 \
  --candidate 2538142f96c9009795aa1f1348ba45f3a16b4fc0 \
  --scratch drafts/pauli-support-reproduction \
  --output drafts/pauli-support-reproduction/results.json
python3 benchmarks/near_clifford/pauli_products/verify.py RESULTS.json \
  --git-sources --binaries CAMPAIGN_SCRATCH
python3 benchmarks/near_clifford/pauli_products/test_contract.py
python3 -O benchmarks/near_clifford/pauli_products/test_contract.py
python3 benchmarks/near_clifford/pauli_products/test_report.py
python3 -O benchmarks/near_clifford/pauli_products/test_report.py
```

All four pristine/diagnostic semantic payload sets are retained, including RNG
continuation and two seeded full-width GHZ conditional-probability witnesses per
frame. Verification recomputes the final physical phase from intermediate outcomes,
checks complete fixtures, timing medians, alternating order, work counters,
source/overlay/binary hashes and the retained unified dependency lock. Source
identity uses the shared exact-byte HEAD-ancestry fallback only for unavailable
original commits; it cannot establish that lost commit's full build tree.

The existing 43-case scale and 28-case entangled campaigns are rerun separately
with the unchanged [row-operation harness](../row_ops/README.md), using the same
baseline/candidate revisions above. Historic timing drivers and fixtures remain
unchanged. Those campaigns retain rank 11/12 cache-boundary controls, compact
high-rank cases, measurement/reset/feedback and all six timing modes. Independent
entangled dense coverage remains 14 exact-width and 14 reduced witnesses; GHZ
controls do not prove every synthetic wide family's full distribution.

These measurements are Apple M4-only. Regressions and post-change profile limits
are reported in the retained analysis.
