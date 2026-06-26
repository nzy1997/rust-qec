# Issue 285 BB Diagnostic Compare Design

Issue: #285 Add paired high-p diagnostic compare rows without running 50k trials

Date: 2026-06-26

## Context

The compare harness already has three relevant surfaces:

- `smoke`, which runs intentionally easy low-p paired Rust/Python rows.
- `small_ldpc_catalog`, which validates the full #209 target manifest without
  running the 50,000-trial campaign.
- `hard-replay`, which replays one checked-in BB90 hard-syndrome fixture and
  verifies per-basis replay metadata and Rust OSD/GF(2) counters.

Issue #285 asks for the missing middle tier: high-p BB diagnostic rows that are
paired across Rust `rbposd` and Python `ldpc_bposd`, but still cheap enough for
routine local verification.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because this is a CLI/CSV verifier change.
- The design is approved from the issue text, merged #279/#282 local context,
  and existing compare-harness patterns.
- Use a dedicated `diagnostic` tier rather than broadening `smoke`, because
  smoke remains a low-p quick check and diagnostic should intentionally cover
  harder high-p points.
- Use the exact issue trial budget, `num_trials = 1`, for both diagnostic
  points. The case catalog and verifier both assert that value.
- Use the existing full-trial `_python_row` path rather than the fixture-only
  hard replay path, because the diagnostic cases are selected configurations,
  not checked-in syndrome fixtures.
- Keep missing Python dependencies as skipped rows from the runner and verifier
  failures by default. Add a verifier `--allow-missing-python` mode matching
  `verify_replay.py` for local Rust-only diagnostics.

## Approaches Considered

1. Add `DIAGNOSTIC_CASES`, a `--tier diagnostic` runner path, and a dedicated
   `verify_diagnostic.py`.
   This is recommended because it follows the existing package structure,
   keeps the case list machine-readable, and gives the new tier exact
   negative-control tests without changing smoke semantics.
2. Add the high-p points to `SMOKE_CASES` and extend `verify_smoke.py`.
   This would reuse the current runner path, but it changes the meaning and
   runtime profile of smoke, which is explicitly out of scope for #285.
3. Represent the high-p rows only in the `small_ldpc_catalog` dry run.
   This documents the cases but never exercises Rust/Python pairing, counters,
   or missing-Python behavior.

## Design

`benchmarks/bb_circuit_bposd_compare/cases.py` remains the case source of truth.
It adds:

- `DIAGNOSTIC_TRIALS = 1`
- `DIAGNOSTIC_SEED = 12345`
- `DIAGNOSTIC_BP_METHOD = "ms"`
- `DIAGNOSTIC_MAX_ITER = 10000`
- `DIAGNOSTIC_OSD_METHOD = "osd_cs"`
- `DIAGNOSTIC_OSD_ORDER = 7`
- `DIAGNOSTIC_CASES` with exactly:
  - BB90 at `p = 0.006`, `num_cycles = 10`, `num_trials = 1`,
    `seed = 12345`
  - BB144 at `p = 0.006`, `num_cycles = 12`, `num_trials = 1`,
    `seed = 12345`
- `validate_diagnostic_cases()` to assert the exact case set, trial budget,
  seed, decoder settings, and stable `case_id` values.

`run_compare.py` adds `--tier diagnostic`. The tier validates
`DIAGNOSTIC_CASES`, runs the existing paired Rust/Python suite over those
cases, writes `results.csv` and `summary.md`, and preserves current skipped
Python dependency behavior:

- skipped Python rows make the runner exit nonzero unless
  `--allow-missing-python` is passed,
- skipped Python rows remain visible in the CSV with `status=skipped` and an
  explicit dependency error.

`verify_diagnostic.py` verifies the diagnostic CSV contract:

- required CSV columns are present,
- each required diagnostic point has exactly one Rust row and exactly one
  Python row sharing `case_id`,
- each pair has matching `code_id`, `p`, `num_cycles`, `num_trials`, `seed`,
  `bp_method`, `max_iter`, `osd_method`, and `osd_order`,
- completed rows carry timing/status/logical fields,
- completed Rust rows carry numeric nonnegative OSD/GF(2) counter fields and
  integer count fields,
- Python skipped rows are rejected by default and allowed only with
  `--allow-missing-python` plus a nonempty dependency error,
- wrong or missing BB144 `p = 0.006`, `num_cycles = 12`, or `num_trials = 1`
  is a verifier failure.

The README gains a `Diagnostic Tier` section with the release-binary commands,
the exact case table, and the skipped-Python semantics.

## Data Flow

For each diagnostic case:

1. `run_compare.py --tier diagnostic` validates the diagnostic case list.
2. The runner invokes `rsinter bb-circuit-bposd-memory --json-compare-case`
   through the release binary when `--rust-binary target/release/rsinter` is
   supplied.
3. The Rust row uses `_rust_row()` with the existing profile/timing fields and
   additionally records aggregate counters from the Rust export when present.
4. The Python row uses `_python_row()` to replay the exported Z/X effective
   models and sampled trial through `ldpc.BpOsdDecoder`.
5. `verify_diagnostic.py` checks exact coverage, pair identity, completed-row
   fields, and Rust counter coverage.

## Error Handling

Catalog validation errors are printed by the diagnostic runner and make the
tier fail before decoders run. Rust exporter failures are recorded as Rust
error rows and make the runner fail. Missing Python dependencies produce skipped
Python rows and a nonzero runner exit unless the runner is called with
`--allow-missing-python`; the verifier still rejects those skipped rows unless
its own `--allow-missing-python` flag is passed.

The verifier reports exact messages for missing diagnostic cases, duplicate
rows, mismatched pair fields, wrong pinned settings, missing Rust counters, and
skipped Python rows. This gives the issue's negative controls an observable
failure reason.

## Testing

Use TDD:

1. Add case-catalog tests for exact diagnostic coverage, settings, stable case
   IDs, and negative controls for missing BB144 or wrong BB144 p/cycle values.
2. Add runner tests that `--tier diagnostic` writes paired rows for fake BB90
   and BB144 exports, records Rust counters, and records skipped Python rows
   with nonzero default status.
3. Add `verify_diagnostic.py` tests that accept valid paired diagnostic rows,
   reject mismatched `case_id` or config values, reject missing/wrong BB144
   points, reject missing Rust counters, and allow skipped Python rows only
   with the explicit flag.
4. Update README documentation.

Required verification:

```bash
cargo build --release -p rsinter
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier diagnostic \
  --output-dir /tmp/rstim-bb-diagnostic \
  --rust-binary target/release/rsinter
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.verify_diagnostic \
  /tmp/rstim-bb-diagnostic/results.csv
cargo test
```

Out of scope: full 50,000-trial campaign execution, publication plots, decoder
semantic changes, and expanding the low-p smoke tier.
