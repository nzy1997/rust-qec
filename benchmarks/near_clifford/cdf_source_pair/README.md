# Near-Clifford CDF source-pair scout

This fixed exploratory screen measures a source change against a baseline and an
identical baseline binary. It covers 24 structural cells and 12 application-counts
cells under both Strict and Fused arithmetic. It supplies source-effect evidence;
it does not compare peers or certify a SOTA result or a production admission.

| Workload | Shots per public call | Purpose |
| --- | --- | --- |
| Depth 32 | 1, 32, 63, 64, 65, 1024 | Deep coherent work and packet boundary/tail behavior |
| Full rank 4, 12; projected rank 8, 16 | 1 | Small-call controls and rank sensitivity |
| Projected rank 12 | 64, 1024 | Bulk rank control |
| MSC distance 3, 5, original noisy circuits | 1, 64, 1024 | Native postselection counts |

Each of six rounds measures all three roles. Every role occupies each execution
position twice per cell. Each child produces seven adaptive observations of at
least 50 ms. The complete ledger has 72 finite children, 648 timing children and
4536 timing observations. No failed or slow observation is dropped.

Structural witnesses compare 129 scalar rows against a flat call within each
source, including 16 literal RNG continuation words. Counts witnesses cover all
12 name/shot/policy combinations, independently replay raw records through the
original detector and observable annotations, and require the probe's native
chronological RNG self-check. Counts probes do not emit literal continuation
words. Different compiler revisions need not generate identical seeded rows.

Fixture SHA256 values are fixed in `manifest.json`. Structural inputs originate
in `../diagnostics/corpus.py`; MSC and surface fixtures preserve the original
circuit bytes and Apache-2.0 provenance in `../application_counts/manifest.json`.
The 13-file fixture inventory also retains unused rank and surface controls.
Schema fixtures contain real historical probe output, with an explicitly
relocated structural circuit path; their provenance is recorded separately and
they are used only in contract tests, never as timing evidence. The rotation
name fixture is actual local Mac ARM debug output, bound to its exact modified
source bytes; it does not qualify Linux AVX/Release execution. The protocol
rejects the earlier projection group as a different candidate preflight. The complete
720-event regression fixture uses toy binaries and fabricated process receipts to
exercise the standalone verifier; it is explicitly synthetic and supplies no
production or performance evidence.

## Run on a quiet CPU host

Use a clean committed protocol checkout, Python 3.10 or later, Rust toolchain
1.93.1, and two locally available exact 40-character source SHAs. The candidate
must contain the `phase_specialized_cdf_tests`, wide-packet raw-record/RNG
test, the exact
`highest_rotation_gather_tests::both_policies_highest_pairs_match_snapshot_gather_at_all_mask_boundaries`
test, and the exact
`near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage`
test. This preflight revision is for the rotation array-group candidate; it rejects
missing, ignored, duplicated or failed named candidate tests. All four checks
use native Release. The diagonal projection preflight remains frozen at
`4e69bddd8921e3caa257eb2f334ef9efc4cbf8c9`; its originals and historical native
stdout fixture are unchanged, and that old group cannot qualify this rotation
candidate. The zero-noise sign protocol remains frozen at
`3ee3f528ad2e260834e5b7300068649b69c76cac`, and the earlier replay-cache protocol
at `02ad5e9993a032b73f64f4950ae96e5a4bd0b8de`; their artifacts use their own verifiers.
Preparation still rejects an old filter that executes zero tests. Both sources build
the same public probe implementation with `-C target-cpu=native` before timings.
Each source's complete Rust/Cargo inventory, probe inputs, actual binaries,
compiler version, build logs and actual process receipts are retained.

```sh
python3 -I benchmarks/near_clifford/cdf_source_pair/test_contract.py
python3 -I -O benchmarks/near_clifford/cdf_source_pair/test_contract.py
mkdir -p drafts/cdf-screen
python3 -I benchmarks/near_clifford/cdf_source_pair/prepare.py \
  --baseline-ref "$BASELINE_SHA" --candidate-ref "$CANDIDATE_SHA" \
  --out drafts/cdf-screen/preparation
python3 -I benchmarks/near_clifford/cdf_source_pair/run_retained.py \
  --preparation drafts/cdf-screen/preparation \
  --out drafts/cdf-screen/output --control drafts/cdf-screen/control
python3 -I benchmarks/near_clifford/cdf_source_pair/verify.py --seal drafts/cdf-screen
python3 -I benchmarks/near_clifford/cdf_source_pair/verify.py drafts/cdf-screen --git-sources
```

Pin the controller to one logical CPU with `taskset -c` on Linux. Other busy CPU
jobs can invalidate a performance conclusion even when all semantic checks pass.
The outer controller requires preparation PIDs to be absent. It records actual
producer exit and cancellation, and preserves failed/timeout worker streams and
the ledger. Summaries stay in files until the original bundle is sealed; observe
only the validation/round/closure progress lines while it runs.

To run on an independent Ubuntu VM, dispatch the existing **Near-Clifford x86
application counts** workflow at the protocol branch with `cdf_source_pair=true`,
`baseline_ref` and `candidate_ref`. Leave `profile_only=false`. Its other benchmark
modes remain available. The source-pair job builds both sources, pins collection
to one logical CPU, seals original bytes, verifies the complete protocol in normal
and optimized Python, and uploads actual binaries and all original streams.
Partial failures are uploaded by the final step too.

## Verify a downloaded artifact

Locate the uploaded `x86-cdf` directory containing `original-seal.json` and pass it
to `verify.py`. Seals use relative paths, so the bundle can move to another host.
The verifier binds exact worker commands and configs, compiler and native build/test
commands, complete historical PID coverage, retained source and binary bytes, preparation and
producer closure, raw-output hashes, all finite witnesses, the fixed 720-child
schedule, and an independent reconstruction of every summary. Add `--git-sources`
when both source refs are available locally. Historical PIDs are verified using
the original host's process-absence receipt, not by probing unrelated PIDs on the
reviewer's machine. Keep any later interpretation under a separate `analysis/`
directory; do not edit the original bundle or overwrite its seal.
