# Issue #312 BB72/BB144 Reference-Gap Report Design

Date: 2026-06-28
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #312, BB72/BB144 comparison evidence and #303 verdict

## Context

Issue #312 asks for the BB72/BB144 comparison to be regenerated after the
Bravyi contract, effective-model audit, LER accounting, batched accounting, and
hard-replay parity work landed. The current branch starts at
`6e3d5a9c66e69c5c210c84bad298ca7593db0867`, which includes:

- #305 / PR #313: pinned Bravyi source contract at
  `sbravyi/BivariateBicycleCodes@fa77e3333d3ec44c79d8f914dd24c040d1da471b`;
- #308 / PR #314: deterministic BB72 effective-model audit machinery and
  checked-in expected fixture;
- #309 / PR #315: Bravyi-style trial-level LER verifier;
- #306 / PR #316: hard-replay correction trace classification;
- #310 / PR #317: paired batched early-stop accounting verifier;
- #307 / PR #318: `rbposd` `ldpc_osd_cs` parity fix for the pinned BB90 hard
  replay.

The checked-in full BB72/BB144 artifact already has 16 paired rows under
`benchmarks/bb_circuit_bposd_compare/results/full/results.csv`. Those rows are
not cheap to regenerate in a normal PR: the BB144 `p=0.003` row alone records
roughly an hour of paired decode time, and the full tier can run until either
one million shots or 200 logical errors per point. A local Python environment
does exist at `/private/tmp/rstim-ldpc-venv/bin/python` with `ldpc 2.4.1`,
`bposd 2.1`, and `numpy 2.5.0`, so a small controlled paired rerun is possible
without network installation.

## Goals

- Preserve the existing paired full CSV unless a full rerun is explicitly
  completed.
- Run and check a controlled BB72/BB144 paired rerun using the existing
  `run_compare --tier bb72-bb144-plot-smoke` path and the local `ldpc` venv.
- Refresh generated summary/plot artifacts from the committed CSV and add the
  controlled rerun artifacts under a separate results directory.
- Add a source-backed `reference_gap_report.md` under
  `benchmarks/bb_circuit_bposd_compare/results/full/`.
- Add a validator CLI,
  `benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report`, that
  rejects reports missing the Bravyi commit, audit statuses, row counts,
  per-code/per-p LER table, Rust/Python delta table, or final #303 verdict.
- Include exact commands and environment facts: Python binary, `ldpc` version,
  `bposd` version, Rust binary path, and Rust source commit used for the
  controlled rerun.

## Non-Goals

- Do not overwrite the full CSV with Python-skipped or wall-budget partial rows.
- Do not support BB108, BB288, or new BB constructors.
- Do not weaken the existing hard-replay, Bravyi LER, or batched accounting
  verifiers.
- Do not attempt to reproduce every curve from the Bravyi paper.
- Do not file a follow-up issue unless the report identifies a specific
  remaining implementation bug rather than a comparability gap.

## Approach Options

### Recommended: Preserve Full CSV, Add Controlled Rerun And Auditable Report

Keep `results/full/results.csv` as the paired full evidence, run a small paired
plot-smoke rerun into `results/controlled/`, regenerate the plot/summary from
the preserved full CSV, and write a report that explicitly separates old full
evidence from new controlled evidence. The validator recomputes the row counts,
LER rows, and Rust/Python deltas from the CSV and checks required report
sections.

This is the safest PR-sized option because it uses the existing compare path,
does not destroy the only paired full data, and still gives reviewers fresh
post-#307 execution evidence.

### Alternative: Full Regeneration

Run `make bb-circuit-bposd-compare-full` with the local `ldpc` venv and commit
the resulting CSV, summary, and plot. This is the cleanest scientific refresh
but is too expensive for a normal Agent Desk PR based on the existing row
timings.

### Alternative: Report Only

Add only a Markdown report around the current full CSV. This is cheaper, but it
would not satisfy the issue's request to rerun the comparison surface after the
hard-replay and accounting fixes.

## Design

### Report Writer

Add `benchmarks.bb_circuit_bposd_compare.write_reference_gap_report`. It reads:

- `--results benchmarks/bb_circuit_bposd_compare/results/full/results.csv`;
- `--contract benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json`;
- optional `--controlled-results` for the plot-smoke rerun;
- optional environment strings for Python, `ldpc`, `bposd`, Rust binary, Rust
  source commit, and commands run.

The writer imports the existing Bravyi contract validator, Bravyi LER verifier,
and batched accounting verifier. It fails if any of those checks fail. On
success, it writes:

- source contract commit and source URLs;
- audit status table for Bravyi contract, Bravyi LER, batched accounting,
  Bravyi model audit, and hard replay parity evidence;
- regeneration evidence that states the full CSV was preserved and controlled
  rerun rows were generated separately;
- per-row LER table with code, p, cycles, decoder, shots, logical errors, LER,
  status, and stop reason;
- Rust/Python delta table by code and p;
- a final verdict for #303.

The intended #303 verdict is the third outcome from issue #312: implementation
checks pass on the current artifacts, but the preserved full run is not directly
comparable to the paper/reference target. The full rows are batched
error-budget-stopped rows, not a fresh fixed-shot reproduction of the pinned
Bravyi curve, and several high-p points stop after small shot counts once one
decoder reaches 200 logical errors.

### Report Validator

Add `benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report`. It
recomputes the same model from the CSV and contract and then checks the report
body for:

- the exact Bravyi upstream commit;
- required section headings;
- visible PASS status lines for Bravyi contract, Bravyi LER, batched
  accounting, Bravyi model audit, and hard replay parity;
- exact `Full results rows: 16` and `Paired comparison groups: 8` counts for
  the current full CSV;
- every per-row LER table line derived from the CSV;
- every Rust/Python delta line derived from the CSV;
- a visible final verdict line for #303 containing one of the allowed outcomes:
  expected trend reached, remaining implementation gap, or not directly
  comparable.

The validator prints `PASS reference gap report validated` on success and exits
nonzero with named missing evidence on failure. The negative controls remove the
Bravyi commit and the final verdict line from a copied report.

### Controlled Rerun Artifacts

Use the existing batched compare path:

```bash
cargo build --release -p rsinter
MPLCONFIGDIR=/tmp/rstim-mplconfig /private/tmp/rstim-ldpc-venv/bin/python \
  -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier bb72-bb144-plot-smoke \
  --output-dir benchmarks/bb_circuit_bposd_compare/results/controlled \
  --rust-binary target/release/rsinter \
  --batch-size 10
```

This writes paired controlled `results.csv`, `summary.md`, and
`bb_circuit_bposd_compare.png` without replacing the full CSV.

## Tests

Add `benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py`
with:

- writer output includes the source-backed sections, Bravyi commit, row counts,
  representative LER row, delta row, and final #303 verdict;
- validator accepts the generated report;
- validator rejects a report with the Bravyi commit removed and names the
  missing contract evidence;
- validator rejects a report with the final verdict line removed and names the
  missing verdict evidence;
- validator rejects a tampered per-row LER table.

## Verification

Required commands:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py
cargo build --release -p rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract \
  benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
cargo test
```

Negative control:

```bash
cp benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md \
  /tmp/reference_gap_report_bad.md
python3 - <<'PY'
from pathlib import Path
path = Path("/tmp/reference_gap_report_bad.md")
text = path.read_text()
text = text.replace("fa77e3333d3ec44c79d8f914dd24c040d1da471b", "", 1)
path.write_text(text)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report /tmp/reference_gap_report_bad.md
```

Expected: validator exits nonzero and names the missing Bravyi contract commit.

## Approval

The run is non-interactive. The standing answer policy chooses the recommended
preserve-full-plus-controlled-rerun approach because it keeps the checked-in
paired full evidence valid, produces fresh post-fix evidence with the available
local `ldpc` environment, and creates an auditable report for #303.
