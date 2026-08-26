# #303 Investigation: BB bposd Full-Comparison Logical Error Rates

## Conclusion

The current checked-in full comparison is **correct**: the LER unit is the
per-Monte-Carlo-trial (per-shot) failure rate under the upstream Bravyi
failure predicate, and the Rust `rbposd` / Python `ldpc_bposd` rows now agree
on identical exported batches within binomial sampling noise. The
unexpectedly high `rbposd` values quoted in #303 were produced before the
hard-replay logical-prediction mismatch was traced in #306 and fixed in
#307; the full CSV was regenerated after that fix. The remaining gap between
these numbers and the paper/reference curve is a **unit convention**
difference, not a counting bug.

## 1. Intended LER unit for this benchmark

`logical_error_rate = logical_errors / shots_used`, where one shot is one
Monte Carlo trial of the full `num_cycles`-cycle circuit, and one logical
error is one failed trial under the Bravyi predicate: decode Z first; a
trial fails if the predicted Z logicals differ from the sampled logicals
(any of the k=12 Z observables), and X is decoded only if Z succeeds
(`z_first_x_only_if_z_succeeds`).

Evidence:

- `run_compare.py::_batched_row`: `logical_error_rate = logical_errors / shots_used`.
- `run_compare.py::_bravyi_trial_failed` and `_python_row`: Python replay
  applies the Z-first / X-only-if-Z-succeeds predicate per exported trial.
- `rsinter/src/bb_circuit_memory.rs::run_bb_p_point_case` (around lines
  1119-1183): Rust counts `num_failed_trials` with the same predicate and
  deliberately skips the X decode after a Z failure.
- `reference/bravyi_contract.md` pins this to upstream
  `sbravyi/BivariateBicycleCodes@fa77e333` `decoder_run.py` lines 364-415:
  the upstream failure unit is one Monte Carlo trial, not one cycle, not one
  observable.

So the LER is **per shot, any-observable**: it is *not* divided by
`num_cycles` (6 for bb72, 12 for bb144) and *not* divided by k=12 logical
observables. Both decoders count failures on the same exported trial batches
with the same predicate, so there is no observable-convention mismatch
between the Rust bundle and the Python replay.

## 2. What caused the #303 anomaly

The values quoted in #303 (e.g. bb144 p=0.003: rbposd 200/40000 = 0.005 vs
ldpc_bposd 138/40000 = 0.00345) came from the pre-#307 CSV. The pinned BB90
hard-syndrome replay (#306) demonstrated a real Rust logical-prediction
mismatch on identical syndromes, fixed in #307. The full CSV at
`results/full/results.csv` was regenerated after #307; the current rows:

| code_id | p | rbposd | ldpc_bposd | delta |
| --- | ---: | --- | --- | --- |
| bb144 | 0.003 | 204/56000 = 0.003643 | 204/56000 = 0.003643 | 0 |
| bb144 | 0.006 | 238/500 = 0.476 | 238/500 = 0.476 | 0 |
| bb72 | 0.003 | 216/8000 = 0.027 | 217/8000 = 0.027125 | 1 shot |
| bb72 | 0.006 | 384/1000 = 0.384 | 383/1000 = 0.383 | 1 shot |

Six of eight pairs are exactly equal; the other two differ by one shot,
consistent with tie-breaking differences inside BP+OSD on identical trials.
The regression guard
`tests/test_bravyi_ler_normalization.py::test_checked_in_full_results_paired_decoders_agree`
now pins this agreement.

## 3. Why the LER still looks higher than the reference trend

Unit conventions, in order of importance:

1. **Per shot vs per syndrome cycle.** The reference/paper curve plots
   logical error rate *per syndrome cycle*. This benchmark's CSV stores the
   per-shot rate over the whole `num_cycles`-cycle experiment. The plot
   pipeline applies the conversion: `plot.toml` sets
   `logical_rate_unit = "per_round"` and `rsinter/src/bench/plot.rs`
   divides the per-shot LER by `params.rounds` (= `num_cycles`) for the
   "Logical Error Rate per Syndrome Cycle" panel. Example: bb144 p=0.003
   per-shot 0.003643 -> per-cycle 3.04e-4, i.e. a factor of 12.
2. **Any-observable vs per-observable.** A failed trial is any of the k=12
   logical observables failing (Z first, then X). A per-observable
   normalization would divide by up to another factor of ~k. The plot
   adapter currently hardcodes `logical_observable_count = 1` in
   `rsinter/src/bench/bb_compare_csv.rs`, so selecting a per-observable unit
   for this benchmark would be wrong; the checked-in plot uses `per_round`.
3. **Error-budget early stopping.** `max_errors=200` stops a point after
   the batch in which either decoder reaches 200 errors
   (`run_compare.py::run_batched_suite`). Both decoders decode every shot of
   every completed batch and share `shots_used`, so the stop does not bias
   either decoder's rate; it only shortens high-p runs (bb144 p=0.006 has
   500 shots, so the 95% binomial half-width is about 0.045) and makes
   point-to-point jitter at high p look larger than a fixed-shot reference
   campaign. `verify_batched_accounting` enforces the shared-shot pairing.

## 4. Plot adapter check (#303 item 5)

`rsinter/src/bench/bb_compare_csv.rs::into_benchmark_row` copies the CSV
`logical_error_rate` verbatim into `metrics.logical_error_rate`; the only
transformation is the declared `logical_rate_unit` division in
`bench/plot.rs`. `verify_bravyi_ler` rejects any CSV row whose stored rate
is not exactly `logical_errors / shots_used`, with dedicated diagnostics for
per-cycle or otherwise rescaled values.

## 5. Verification commands

```bash
# Smoke pipeline (paired Rust/Python rows accepted):
python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke \
  --output-dir /tmp/rstim-issue303-smoke --rust-binary target/release/rsinter
python -m benchmarks.bb_circuit_bposd_compare.verify_smoke /tmp/rstim-issue303-smoke/results.csv

# Checked-in full CSV passes the LER and paired-accounting gates:
python -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
python -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv

# Regression tests (synthetic known-LER rows + checked-in CSV pins):
python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py -q
```

All of the above pass on this branch.

## 6. Impact and follow-ups

- No counting/normalization bug remains in the current artifacts; no full
  rerun is required by this investigation. (A fresh full rerun, if ever
  needed, is `make bb-circuit-bposd-compare-full`.)
- The stale regression pin (`("0.003", 12, 40000, 200)`) referenced the
  pre-#307 CSV and is updated to the regenerated tuple
  `("0.003", 12, 56000, 204)`.
- Suggested follow-up (not done here): surface the per-cycle conversion and
  the error-budget stopping in the plot panel subtitle so readers do not
  compare the per-shot CSV numbers against per-cycle reference curves
  directly.
