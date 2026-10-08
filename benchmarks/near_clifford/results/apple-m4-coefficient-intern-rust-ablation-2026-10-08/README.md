# Apple M4 coefficient-intern selected and rejected scouts

Eleven closed Rust-only experiments retain all 24 cells, without peer/SOTA claims.
Each warm experiment has 288 events: 48 four-seed full-record/counts/RNG validations
and 240 timing processes, five rotated/reversed rounds and seven observations of
at least 50 ms. Each cold experiment has 240 fresh processes × 32 fixed seeds
739..770 (7680 observations), separately retaining compile, prepare, first-call
and their phase sum. Every paired cold observation checks identical counts and
sixteen subsequent RNG words; cold observations have no 50 ms minimum.

## Selection and regressions

| Variant | Evidence directories | Decision |
| --- | --- | --- |
| Full hash including closed admission | `full-hash-warm` | Rejected: all six MSC d5 cells regress by roughly 2–9%. |
| Hash only during cache admission | `admission-warm`, `admission-confirmation`, `admission-cold` | Rejected: warm bulk gains do not compensate first-call MSC d5 throughput falling to 0.29–0.50×. |
| Sparse fingerprint, exact full-bit comparison | `sparse-warm`, `sparse-cold` | Rejected: eager index construction lowers MSC single-shot cold throughput to 0.57–0.73× and regresses MSC d3 bulk. |
| Sparse index after 128 retained states | `deferred-warm`, `deferred-cold` | Rejected: MSC d3 cold bulk remains about 5–11% slower. Cold source adds only a correctness activation test to the warm source. |
| Deferred index only for 32..4096 coefficients | `large-warm`, `large-confirmation`, `large-cold` | Selected for full evaluation; modest benefits and regressions remain. |

For the selected variant, MSC d5 warm Strict/Fused 64-shot throughput ratios are
1.0430/1.0136; the confirmation gives 1.0413/1.0250. All four paired ranges exceed
one. Warm 1024-shot paired ranges cross one. Cold first-call Strict/Fused d5
1024-shot ratios are 1.0317/1.0275, both paired ranges above one; Fused 64-shot
is 0.9804, all pairs below one. MSC d3 Strict cold 64/1024-shot ratios are
0.9576/0.9745, all pairs below one. A 2.15% MSC d3 Strict warm single-shot
regression in the first run does not recur in confirmation; confirmation instead
records a 2.23% surface d9 Fused 1024-shot regression. All other cells and all
compile/prepare phase statistics remain in their original `summary.json` files.

Results compare each candidate to the original compact-counts-replay source,
not to the previous interner variant. This archive preserves candidate and
baseline source histories, including failures, rather than estimating effects
from unrelated peer campaigns. Later runs explicitly used native CPU flags;
early headers record no RUSTFLAGS. Each paired run uses its recorded builds.
The host is shared and unpinned; small effects and paired ranges are descriptive,
not confidence intervals. The selected candidate's full [M4](../apple-m4-coefficient-intern-2026-10-08/README.md)
and [x86](../linux-vm-x86-coefficient-intern-2026-10-08/README.md) comparisons
retain important remaining single-shot and x86 gaps.

## Reproduction and verification

`bindings.json` maps every original untracked probe path to the retained bytes
and every clean producer to its original commit. Header and closure identities,
driver digests and raw event bytes are unchanged. Producer commits are retained
by `benchmark-source/coefficient-intern-{scout,admission-scout,sparse-scout,deferred-scout,deferred-validation,large-scout}-2026-10-08` tags.
Original drivers preserve their historical checkout paths; to recollect in fresh
checkouts, place probe files at the mapped original paths, adjust only driver
checkout/output paths, build their isolated Cargo manifests and record a new
campaign identity. Relocated probe manifests are preserved source artifacts,
not directly runnable at their archive location. Measured binaries are not vendored.

```sh
python3 benchmarks/near_clifford/verify_coefficient_intern_scouts.py
python3 -O benchmarks/near_clifford/verify_coefficient_intern_scouts.py
python3 benchmarks/near_clifford/test_coefficient_intern_scouts.py
python3 -O benchmarks/near_clifford/test_coefficient_intern_scouts.py
```

The verifier checks original source/probe/driver hashes, before/after closure,
the rotated paired schedule, validation coverage, timing arithmetic, fresh seeds,
cross-source cold counts and RNG, cache cap and every derived summary cell.
It uses explicit checks in both modes. The archived collection drivers are
historical evidence; their assertions are not relied on for verification.
