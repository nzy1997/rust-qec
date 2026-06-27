# Issue 310 BB Batched Accounting Verifier Design

Issue: #310 Validate paired early-stop accounting for BB batched comparisons

Date: 2026-06-28

## Context

The BB72/BB144 full comparison writes one aggregate CSV row per decoder after a
batched run. Early stopping happens between batches when either decoder reaches
the per-case logical-error budget. Issue #303 identified a risk that bad
early-stop accounting could make the Rust and Python logical-error rates appear
better or worse even when decoder behavior is unchanged.

Issue #309 is already merged as PR #315 and owns the trial-level logical error
rate contract through `verify_bravyi_ler.py`. This issue adds the paired-row
semantics that #309 intentionally leaves out: one comparable Rust row and one
comparable Python row per `(case_id, code_id, p, num_cycles)` group, except for
explicit missing-Python dependency skips.

Live GitHub issue reads are blocked in the Agent Desk sandbox by the configured
proxy, so this design uses the manager-supplied issue body plus local merged
#309 docs, tests, and code as authoritative context.

## Automatic Answers

This run is non-interactive. The standing answer policy chooses these answers:

- No visual companion is needed because the output is a CLI verifier and pytest
  coverage, not a visual design.
- Use a dedicated module,
  `benchmarks.bb_circuit_bposd_compare.verify_batched_accounting`, instead of
  extending `verify_bravyi_ler.py`; #309 remains focused on normalization.
- Accept `ldpc_bposd` absence only when an explicit skipped Python row exists
  with `status=skipped` and `stop_reason=python_dependency_missing`.
- Treat completed paired rows as rows with `status in {"ok", "partial"}` and
  `decoder_impl in {"rbposd", "ldpc_bposd"}`. Skipped Python rows are not
  completed rows and are checked by their skip semantics.
- Use tight float comparison for `logical_error_rate == logical_errors /
  shots_used`, matching #309's tolerance.
- Validate the checked-in full CSV as a real fixture and add small synthetic CSV
  rows for edge cases and negative controls.

## Approaches Considered

### Recommended: Dedicated Paired-Accounting Verifier

Add `verify_batched_accounting.py` as a pure CSV inspector. It groups accepted
batched rows by `(case_id, code_id, p, num_cycles)`, checks exact Rust/Python
pairing and shared early-stop metadata, verifies per-row logical-error-rate
math, and prints PASS lines for accepted groups.

This is the best fit because it gives reviewers a deterministic command for
checked-in artifacts without rerunning decoders, and it keeps paired semantics
separate from the #309 normalization verifier.

### Alternative: Extend `verify_bravyi_ler.py`

This would reduce one file but would mix two contracts: trial-level
normalization and paired early-stop comparability. The issue explicitly asks to
keep #309 focused, so this is rejected.

### Alternative: Add Checks To `run_compare.py`

The runner could enforce some invariants while producing CSVs, but the issue
asks for an independent verifier with a negative control. Runtime assertions do
not prove that checked-in or externally copied CSV artifacts are still valid.

## Design

### Verifier Module

`benchmarks.bb_circuit_bposd_compare.verify_batched_accounting` will expose:

- `load_rows(csv_path: Path) -> list[dict[str, str]]`
- `verify_rows(rows: list[dict[str, str]]) -> list[VerifiedPair | VerificationError]`
- CLI entry point:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting <results.csv>
```

The verifier only inspects CSV rows. It must not import matplotlib, render
plots, run Rust, or run Python `ldpc`.

### Row Grouping

The group key is exactly `(case_id, code_id, p, num_cycles)`. For each group:

- require exactly one `rbposd` row with `runner=batched_compare`;
- require either exactly one completed `ldpc_bposd` row or exactly one skipped
  `ldpc_bposd` row with `stop_reason=python_dependency_missing`;
- reject duplicate Rust or Python rows;
- reject any group that has no accepted batched rows.

### Comparable Pair Checks

For completed `rbposd`/`ldpc_bposd` pairs, require identical:

- `shots_used`
- `batch_size`
- `batches_completed`
- `stop_reason`
- `seed`
- `bp_method`
- `max_iter`
- `osd_method`
- `osd_order`

This proves each pair used the same exported batches and decoder settings.

### Early-Stop Checks

`stop_reason=errors_budget_reached` is valid only when `errors_budget` parses as
a positive integer and at least one row in the completed pair has
`logical_errors >= errors_budget`.

`status=partial` rows must use one of the explicit partial reasons:
`wall_budget_exhausted` or `python_dependency_missing`. A completed Python skip
is not allowed; skipped Python rows must be `status=skipped`,
`decoder_impl=ldpc_bposd`, and `stop_reason=python_dependency_missing`.

`wall_budget_exhausted` requires completed Rust and Python rows so both decoders
remain comparable for the partial point.

### Logical Error Rate Checks

For every accepted completed row, parse `shots_used`, `logical_errors`, and
`logical_error_rate`. Require:

- `shots_used > 0`
- `0 <= logical_errors <= shots_used`
- `logical_error_rate` is finite
- `logical_error_rate == logical_errors / shots_used` within `1e-12`

Skipped Python rows may have `shots_used` from the batch that triggered the
missing dependency, but they are not treated as completed LER rows.

### Output

On success, the CLI prints:

- one header line: `PASS BB batched paired accounting`
- one PASS line per accepted group, including `case_id`, `code_id`, `p`,
  `num_cycles`, `shots_used`, `batches_completed`, `batch_size`, `stop_reason`,
  and Rust/Python logical-error counts when both rows completed;
- skipped Python groups clearly report `python_dependency_missing`.

On failure, the CLI exits nonzero and prints each error to stderr. Pair
mismatch errors must name that the Rust/Python pair is no longer comparable so
the negative control is obvious.

## Tests

Add
`benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py` with TDD
coverage for:

1. A synthetic valid `errors_budget_reached` pair.
2. The checked-in full CSV, requiring BB72 and BB144 PASS coverage for
   `errors_budget_reached`.
3. Negative control: mismatched Python `shots_used` fails and says the pair is
   no longer comparable.
4. Negative control: mismatched Python `batches_completed` fails similarly.
5. `errors_budget_reached` fails if neither decoder reaches `errors_budget`.
6. Partial `wall_budget_exhausted` requires completed paired rows and matching
   metadata.
7. `python_dependency_missing` is accepted only as an explicit skipped Python
   row and printed distinctly.
8. Per-row LER mismatch fails even when the pair metadata matches.
9. CLI success and failure exit codes.

Required verification:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
cargo test
```

Negative control:

```bash
cp benchmarks/bb_circuit_bposd_compare/results/full/results.csv /tmp/bb_batched_unpaired_bad.csv
python3 - <<'PY'
import csv
from pathlib import Path
path = Path("/tmp/bb_batched_unpaired_bad.csv")
rows = list(csv.DictReader(path.open()))
for row in rows:
    if row["decoder_impl"] == "ldpc_bposd":
        row["shots_used"] = str(int(row["shots_used"]) + 1)
        break
with path.open("w", newline="") as handle:
    writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows(rows)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting /tmp/bb_batched_unpaired_bad.csv
```

Expected: the verifier exits nonzero and reports that the Rust/Python pair is
no longer comparable.

## Out Of Scope

- Re-running any BB full campaign.
- Changing the physical sampling distribution.
- Deciding whether early stopping is sufficient for a paper comparison.
- Fixing Rust/Python hard replay semantic mismatches.
