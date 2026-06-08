# Surface Decoder Partial Rerun Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a partial rerun merge mode to the surface decoder comparison benchmark so selected decoders can refresh `full/results.csv` in place and the existing plot command can regenerate the full figure from the merged table.

**Architecture:** Keep benchmark execution in `run_suite(...)` unchanged and add merge-specific helpers in `benchmarks/surface_decoder_compare/run_compare.py`. The CLI will expose `--merge-into-existing`, validate that it is only used with `--decoders`, load the canonical tier CSV if present, replace matching rows by benchmark identity, and rewrite the canonical file in deterministic order.

**Tech Stack:** Python 3, `argparse`, `csv`, `pathlib`, `unittest`, existing benchmark schema helpers

---

### Task 1: Add merge-focused tests for `run_compare`

**Files:**
- Modify: `benchmarks/surface_decoder_compare/tests/test_run_compare.py`
- Verify: `benchmarks/surface_decoder_compare/run_compare.py`

- [ ] **Step 1: Write the failing merge helper tests**

Add these tests near the end of `RunCompareTest` in `benchmarks/surface_decoder_compare/tests/test_run_compare.py`:

```python
    def test_merge_rows_replaces_matching_keys_and_preserves_others(self) -> None:
        existing_rows = [
            {
                "tier": "full",
                "decoder": "ldpc",
                "backend": "native",
                "distance": "3",
                "rounds": "3",
                "p": "0.002",
                "seed": "12345",
                "num_dets": "24",
                "num_obs": "1",
                "shots_budget": "10000",
                "errors_budget": "200",
                "shots_used": "10000",
                "logical_errors": "11",
                "logical_error_rate": "0.0011",
                "compile_us": "10.0",
                "total_decode_us": "20.0",
                "decode_us_per_shot": "0.002",
                "status": "ok",
                "error": "",
            },
            {
                "tier": "full",
                "decoder": "pymatching",
                "backend": "native",
                "distance": "3",
                "rounds": "3",
                "p": "0.002",
                "seed": "12345",
                "num_dets": "24",
                "num_obs": "1",
                "shots_budget": "10000",
                "errors_budget": "200",
                "shots_used": "10000",
                "logical_errors": "2",
                "logical_error_rate": "0.0002",
                "compile_us": "5.0",
                "total_decode_us": "6.0",
                "decode_us_per_shot": "0.0006",
                "status": "ok",
                "error": "",
            },
        ]
        new_rows = [
            ResultRow(
                tier="full",
                decoder="ldpc",
                backend="gurobi",
                distance=3,
                rounds=3,
                p=0.002,
                seed=12345,
                num_dets=24,
                num_obs=1,
                shots_budget=10000,
                errors_budget=200,
                shots_used=8192,
                logical_errors=7,
                logical_error_rate=7 / 8192,
                compile_us=12.0,
                total_decode_us=24.0,
                decode_us_per_shot=24.0 / 8192,
                status="error",
                error="solver failed",
            )
        ]

        merged = _merge_rows(existing_rows, new_rows)

        self.assertEqual(len(merged), 2)
        self.assertEqual([row["decoder"] for row in merged], ["ldpc", "pymatching"])
        self.assertEqual(merged[0]["backend"], "gurobi")
        self.assertEqual(merged[0]["status"], "error")
        self.assertEqual(merged[0]["error"], "solver failed")
        self.assertEqual(merged[1]["decoder"], "pymatching")

    def test_load_existing_rows_returns_empty_when_file_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            missing = Path(tmpdir) / "missing.csv"
            self.assertEqual(_load_existing_rows(missing), [])
```

- [ ] **Step 2: Write the failing CLI validation and merge dispatch tests**

Add these tests in the same file:

```python
    def test_main_rejects_merge_without_decoders(self) -> None:
        with self.assertRaises(SystemExit) as ctx:
            main(["--tier", "full", "--merge-into-existing"])
        self.assertNotEqual(ctx.exception.code, 0)

    @mock.patch("benchmarks.surface_decoder_compare.run_compare.write_results")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare._merge_rows")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare._load_existing_rows")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.run_suite")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_case_specs")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_driver_registry")
    def test_main_merges_into_existing_results_when_requested(
        self,
        registry_mock: mock.Mock,
        case_specs_mock: mock.Mock,
        run_suite_mock: mock.Mock,
        load_existing_rows_mock: mock.Mock,
        merge_rows_mock: mock.Mock,
        write_results_mock: mock.Mock,
    ) -> None:
        registry_mock.return_value = {"ldpc": object(), "rbposd": object()}
        case_specs_mock.return_value = [CaseSpec(distance=3, rounds=3, p=0.002)]
        run_suite_mock.return_value = [
            ResultRow(
                tier="full",
                decoder="ldpc",
                backend="native",
                distance=3,
                rounds=3,
                p=0.002,
                seed=12345,
                num_dets=24,
                num_obs=1,
                shots_budget=10000,
                errors_budget=200,
                shots_used=10000,
                logical_errors=3,
                logical_error_rate=0.0003,
                compile_us=1.0,
                total_decode_us=2.0,
                decode_us_per_shot=0.0002,
                status="ok",
                error="",
            )
        ]
        load_existing_rows_mock.return_value = [{"decoder": "pymatching"}]
        merge_rows_mock.return_value = [{"decoder": "ldpc"}, {"decoder": "pymatching"}]

        exit_code = main(
            [
                "--tier",
                "full",
                "--decoders",
                "ldpc",
                "--merge-into-existing",
            ]
        )

        self.assertEqual(exit_code, 0)
        write_results_mock.assert_called_once_with(
            merge_rows_mock.return_value,
            Path("benchmarks/surface_decoder_compare/results/full/results.csv"),
        )
        load_existing_rows_mock.assert_called_once_with(
            Path("benchmarks/surface_decoder_compare/results/full/results.csv")
        )
        merge_rows_mock.assert_called_once_with(
            load_existing_rows_mock.return_value,
            run_suite_mock.return_value,
        )
```

- [ ] **Step 3: Run the focused tests and verify they fail**

Run:

```bash
.venv-surface-decoder/bin/python -m unittest benchmarks.surface_decoder_compare.tests.test_run_compare -v
```

Expected: FAIL with import or attribute errors for `_load_existing_rows` / `_merge_rows`, plus CLI validation mismatch because `--merge-into-existing` is not implemented yet.

- [ ] **Step 4: Commit the red test state if working in a dedicated feature branch**

```bash
git add benchmarks/surface_decoder_compare/tests/test_run_compare.py
git commit -m "test: cover partial rerun merge mode"
```

If the branch is being kept dirty for interactive development, skip this commit and proceed.

### Task 2: Implement merge helpers and CLI behavior in `run_compare.py`

**Files:**
- Modify: `benchmarks/surface_decoder_compare/run_compare.py`
- Verify: `benchmarks/surface_decoder_compare/schema.py`
- Test: `benchmarks/surface_decoder_compare/tests/test_run_compare.py`

- [ ] **Step 1: Add merge helpers and deterministic row ordering**

Update `benchmarks/surface_decoder_compare/run_compare.py` imports and helper section to include CSV row loading and merge logic:

```python
from __future__ import annotations

import argparse
import csv
from pathlib import Path

from .cases import build_case_specs, materialize_case_bundle
from .drivers import build_driver_registry
from .schema import CSV_HEADER, DEFAULT_BATCH_SIZE, ResultRow, TIER_CONFIGS


def write_results(rows: list[dict[str, object]] | list[ResultRow], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    normalized_rows: list[dict[str, object]] = []
    for row in rows:
        if isinstance(row, ResultRow):
            normalized_rows.append(row.to_csv_row())
        else:
            normalized_rows.append(dict(row))
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        for row in normalized_rows:
            writer.writerow(row)


def _load_existing_rows(path: Path) -> list[dict[str, str]]:
    if not path.exists():
        return []
    with path.open() as handle:
        return list(csv.DictReader(handle))


def _row_identity(row: dict[str, object]) -> tuple[str, str, str, str, str, str]:
    return (
        str(row["tier"]),
        str(row["decoder"]),
        str(row["distance"]),
        str(row["rounds"]),
        str(row["p"]),
        str(row["seed"]),
    )


def _sort_key(row: dict[str, object]) -> tuple[str, float, float, int]:
    return (
        str(row["decoder"]),
        float(row["distance"]),
        float(row["p"]),
        int(row["seed"]),
    )


def _merge_rows(
    existing_rows: list[dict[str, str]],
    new_rows: list[ResultRow],
) -> list[dict[str, object]]:
    normalized_new_rows = [row.to_csv_row() for row in new_rows]
    replacement_keys = {_row_identity(row) for row in normalized_new_rows}
    kept_rows = [
        row for row in existing_rows if _row_identity(row) not in replacement_keys
    ]
    merged = kept_rows + normalized_new_rows
    return sorted(merged, key=_sort_key)
```

- [ ] **Step 2: Add the new CLI flag and merge write path**

In the `main(...)` function of `benchmarks/surface_decoder_compare/run_compare.py`, add:

```python
    parser.add_argument(
        "--merge-into-existing",
        action="store_true",
        help="Replace matching rows inside the canonical tier results.csv",
    )
```

Then replace the current `run_suite(...)` dispatch block with:

```python
    if args.merge_into_existing and not args.decoders:
        parser.error("--merge-into-existing requires --decoders")

    rows = run_suite(
        tier_name=args.tier,
        output_dir=args.output_dir,
        seed=args.seed,
        batch_size=args.batch_size,
        drivers=registry,
        case_specs=case_specs,
    )

    if args.merge_into_existing:
        results_path = args.output_dir / args.tier / "results.csv"
        merged_rows = _merge_rows(_load_existing_rows(results_path), rows)
        write_results(merged_rows, results_path)
```

and keep `return 0` at the end.

- [ ] **Step 3: Run the focused tests and verify they pass**

Run:

```bash
.venv-surface-decoder/bin/python -m unittest benchmarks.surface_decoder_compare.tests.test_run_compare -v
```

Expected: PASS for the new merge tests and the existing CLI filter tests.

- [ ] **Step 4: Commit the implementation**

```bash
git add benchmarks/surface_decoder_compare/run_compare.py benchmarks/surface_decoder_compare/tests/test_run_compare.py
git commit -m "feat: merge partial surface benchmark reruns"
```

### Task 3: Verify end-to-end merge mode and rerender the plot

**Files:**
- Modify: `benchmarks/surface_decoder_compare/results/full/results.csv`
- Modify: `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`
- Verify: `benchmarks/surface_decoder_compare/plot_compare.py`

- [ ] **Step 1: Run the selected full-tier decoder rerun with merge mode**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare \
  --tier full \
  --decoders ldpc,rbposd \
  --merge-into-existing
```

Expected: benchmark output for only `ldpc` and `rbposd`, followed by a rewritten `benchmarks/surface_decoder_compare/results/full/results.csv` that still contains the other decoders.

- [ ] **Step 2: Regenerate the full-tier plot from the merged table**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier full
```

Expected: `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png` updated with the merged full table.

- [ ] **Step 3: Sanity-check the merged result table**

Run:

```bash
.venv-surface-decoder/bin/python - <<'PY'
import csv
from collections import Counter
from pathlib import Path

path = Path("benchmarks/surface_decoder_compare/results/full/results.csv")
rows = list(csv.DictReader(path.open()))
print("rows", len(rows))
print("decoders", Counter(row["decoder"] for row in rows))
PY
```

Expected: all six decoders still appear; `ldpc` and `rbposd` counts match the full-tier case grid; total row count matches the full full-tier table.

- [ ] **Step 4: Run the plot-related and benchmark CLI tests**

Run:

```bash
.venv-surface-decoder/bin/python -m unittest \
  benchmarks.surface_decoder_compare.tests.test_run_compare \
  benchmarks.surface_decoder_compare.tests.test_plot_compare -v
```

Expected: PASS.

- [ ] **Step 5: Commit the updated benchmark artifacts and code**

```bash
git add \
  benchmarks/surface_decoder_compare/run_compare.py \
  benchmarks/surface_decoder_compare/tests/test_run_compare.py \
  benchmarks/surface_decoder_compare/results/full/results.csv \
  benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png \
  docs/superpowers/specs/2026-06-07-surface-decoder-partial-rerun-merge-design.md \
  docs/superpowers/plans/2026-06-07-surface-decoder-partial-rerun-merge-plan.md
git commit -m "feat: support merged partial surface benchmark reruns"
```
