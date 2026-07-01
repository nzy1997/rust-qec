# Multi-Seed No-Target Stability Reporting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add multi-seed no-target stability reporting for qec-code random-window smoke benchmarks while preserving one JSONL row per case/seed run.

**Architecture:** Keep `run_local.py` as the per-seed execution layer and implement aggregation validation/reporting in `summarize.py`. Add a Make target that invokes the existing runner with `--seeds 7 11 17`, then summarize the resulting JSONL.

**Tech Stack:** Python standard library `unittest`, existing qec-code benchmark Python modules, Make, Markdown docs, Rust/Cargo verification.

## Global Constraints

- Preserve exact per-seed JSONL rows; aggregate only in summary outputs.
- Treat `target_upper_bound` as a reporting threshold, not as a CLI `--target-weight`.
- No-target runs must omit `--target-weight`; `target_weight` remains unset/null in JSONL and visibly unset in summaries.
- Avoid external dependencies and do not require `codeDistancePYPI`, `QDistRnd`, `dist-m4ri`, Gurobi, or any external reference implementation.
- Do not change random-window sampling semantics.
- Do not add statistical significance claims or hard performance pass/fail thresholds.
- Use the issue verification seeds `7 11 17` for the multi-seed smoke path.
- The negative-control unittest entrypoint must be `benchmarks.qec_code_random_window.tests.test_multiseed_summary.MultiSeedSummaryTest.test_rejects_missing_seed_or_mixed_build_profile`.

---

## File Structure

- Create `benchmarks/qec_code_random_window/tests/test_multiseed_summary.py` for the issue #339 positive and negative-control summary tests.
- Modify `benchmarks/qec_code_random_window/summarize.py` to validate run settings, expose `run_build_profile_values`, and make Markdown tables explicit about observed seeds, target weights, build profiles, target hits, and elapsed distributions.
- Modify `Makefile` to add `qec-code-random-window-bench-no-target-multiseed-smoke` and variables for the output directory.
- Modify `benchmarks/qec_code_random_window/README.md`, `docs/showcases/qec-code-random-window-benchmark.md`, and `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py` so docs and Make target tests cover the new entrypoint.

---

### Task 1: Add Multi-Seed Summary Tests

**Files:**
- Create: `benchmarks/qec_code_random_window/tests/test_multiseed_summary.py`

**Interfaces:**
- Consumes: `benchmarks.qec_code_random_window.summarize` CLI invoked with `python3 -m benchmarks.qec_code_random_window.summarize`.
- Produces: `MultiSeedSummaryTest` with positive summary assertions and the required negative-control test method.

- [ ] **Step 1: Write the failing test file**

Create `benchmarks/qec_code_random_window/tests/test_multiseed_summary.py` with this content:

```python
from __future__ import annotations

import csv
import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def _write_manifest(path: Path) -> None:
    path.write_text(
        textwrap.dedent(
            """
            manifest_version = 1
            suite = "qec_code_random_window"

            [[cases]]
            case_id = "bb72_no_target_smoke"
            code_id = "bb72"
            distance_side = "any"
            iterations = 500
            restarts = 1
            seed = 7
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_no_target_smoke"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 500
            restarts = 1
            seed = 7
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """
        ).lstrip(),
        encoding="utf-8",
    )


def _row(
    case_id: str,
    seed: int,
    status: str,
    *,
    upper_bound: int | None,
    elapsed_s: float,
    build_profile: str = "release",
    target_weight: int | None = None,
    target_upper_bound: int,
) -> dict[str, object]:
    row: dict[str, object] = {
        "case_id": case_id,
        "status": status,
        "seed": seed,
        "iterations": 500,
        "restarts": 1,
        "target_weight": target_weight,
        "target_upper_bound": target_upper_bound,
        "elapsed_s": elapsed_s,
        "build_profile": build_profile,
        "command": [
            "target/release/qec-code",
            "code",
            "css-distance",
            "random-window-upper-bound",
            "--seed",
            str(seed),
            "--json",
        ],
    }
    if status == "ok":
        assert upper_bound is not None
        row["upper_bound"] = upper_bound
    else:
        row["upper_bound"] = upper_bound
        row["stderr_context"] = "fixture failure"
    return row


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def _read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class MultiSeedSummaryTest(unittest.TestCase):
    def run_summarizer(
        self,
        manifest: Path,
        runs: Path,
        out_dir: Path,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.summarize",
                "--cases",
                str(manifest),
                "--runs",
                str(runs),
                "--out-dir",
                str(out_dir),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_multiseed_no_target_summary_reports_seed_stability_fields(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            _write_manifest(manifest)
            _write_jsonl(
                runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "cli_error", upper_bound=None, elapsed_s=0.5, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 17, "ok", upper_bound=12, elapsed_s=6.0, target_upper_bound=12),
                ],
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            rows = {row["case_id"]: row for row in _read_csv_rows(out_dir / "summary.csv")}
            self.assertEqual(rows["bb72_no_target_smoke"]["attempted_seed_rows"], "3")
            self.assertEqual(rows["bb72_no_target_smoke"]["successful_seed_rows"], "2")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_seed_values"], "7;11;17")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_target_weight_values"], "")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_build_profile_values"], "release")
            self.assertEqual(rows["bb72_no_target_smoke"]["best_upper_bound"], "6")
            self.assertEqual(rows["bb72_no_target_smoke"]["target_hit_count"], "1")
            self.assertEqual(rows["bb72_no_target_smoke"]["target_hit_rate"], "0.500000")
            self.assertEqual(rows["bb72_no_target_smoke"]["median_elapsed_s"], "2.0")
            self.assertEqual(rows["bb72_no_target_smoke"]["min_elapsed_s"], "1.0")
            self.assertEqual(rows["bb72_no_target_smoke"]["max_elapsed_s"], "3.0")
            self.assertEqual(rows["bb144_no_target_smoke"]["attempted_seed_rows"], "3")
            self.assertEqual(rows["bb144_no_target_smoke"]["successful_seed_rows"], "3")
            self.assertEqual(rows["bb144_no_target_smoke"]["target_hit_count"], "2")
            self.assertEqual(rows["bb144_no_target_smoke"]["target_hit_rate"], "0.666667")
            self.assertEqual(rows["bb144_no_target_smoke"]["median_elapsed_s"], "6.0")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("observed_seeds", markdown)
            self.assertIn("target_hits", markdown)
            self.assertIn("target_weight", markdown)
            self.assertIn("build_profile", markdown)
            self.assertIn("| bb72_no_target_smoke |", markdown)
            self.assertIn("7;11;17", markdown)
            self.assertIn("1/2 (0.500000)", markdown)
            self.assertIn("none", markdown)
            self.assertNotIn("--target-weight", markdown)

    def test_rejects_missing_seed_or_mixed_build_profile(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            _write_manifest(manifest)

            missing_seed_runs = tmp_path / "missing-seed.jsonl"
            _write_jsonl(
                missing_seed_runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "ok", upper_bound=6, elapsed_s=2.0, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12),
                ],
            )

            missing_result = self.run_summarizer(
                manifest,
                missing_seed_runs,
                tmp_path / "missing-summary",
            )

            self.assertNotEqual(missing_result.returncode, 0)
            self.assertIn("bb144_no_target_smoke", missing_result.stderr)
            self.assertIn("seed", missing_result.stderr)
            self.assertIn("7;11;17", missing_result.stderr)

            mixed_profile_runs = tmp_path / "mixed-profile.jsonl"
            _write_jsonl(
                mixed_profile_runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "ok", upper_bound=6, elapsed_s=2.0, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12, build_profile="debug"),
                    _row("bb144_no_target_smoke", 17, "ok", upper_bound=12, elapsed_s=6.0, target_upper_bound=12),
                ],
            )

            mixed_result = self.run_summarizer(
                manifest,
                mixed_profile_runs,
                tmp_path / "mixed-summary",
            )

            self.assertNotEqual(mixed_result.returncode, 0)
            self.assertIn("bb144_no_target_smoke", mixed_result.stderr)
            self.assertIn("build_profile", mixed_result.stderr)
            self.assertIn("debug;release", mixed_result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the new test and verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_multiseed_summary -q
```

Expected: FAIL because `summary.csv` does not yet include `run_build_profile_values` and the missing-seed/mixed-profile validation does not yet reject invalid input.

- [ ] **Step 3: Commit the failing test**

Run:

```bash
git add benchmarks/qec_code_random_window/tests/test_multiseed_summary.py
git commit -m "test: cover multi-seed no-target summaries"
```

---

### Task 2: Implement Summary Validation and Markdown Clarity

**Files:**
- Modify: `benchmarks/qec_code_random_window/summarize.py`
- Test: `benchmarks/qec_code_random_window/tests/test_multiseed_summary.py`
- Test: `benchmarks/qec_code_random_window/tests/test_summarize.py`
- Test: `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py`

**Interfaces:**
- Consumes: validated JSONL rows from `load_run_rows`.
- Produces: `run_build_profile_values` in `CSV_FIELDS`; validation errors from `summarize_cases`; Markdown rows that expose observed seeds, target-hit count/rate, target-weight status, and build profile.

- [ ] **Step 1: Add validation helpers and build-profile field**

In `benchmarks/qec_code_random_window/summarize.py`:

- Add `"run_build_profile_values"` after `"run_target_weight_values"` in `CSV_FIELDS`.
- Validate optional row field `build_profile` as a non-empty string when present.
- Store normalized `build_profile` in validated rows.
- Add a helper that validates per-case settings before summary rows are produced:

```python
def _require_optional_str(row: dict[str, Any], field: str, location: str) -> str | None:
    value = row.get(field)
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise _fail(location, f'field "{field}" must be a non-empty string when present')
    return value


def _format_seed_set(seeds: set[int]) -> str:
    return _join_sorted(seeds)


def _validate_case_rows(case: dict[str, Any], rows: list[dict[str, Any]]) -> set[int]:
    case_id = case["case_id"]
    seen_seeds: set[int] = set()
    duplicate_seeds: set[int] = set()
    iterations_values = {row["iterations"] for row in rows}
    restarts_values = {row["restarts"] for row in rows}
    target_weight_values = {row["target_weight"] for row in rows}
    build_profile_values = {row["build_profile"] for row in rows if row.get("build_profile") is not None}
    target_upper_bound_values = {
        row["target_upper_bound"]
        for row in rows
        if "target_upper_bound" in row and row["target_upper_bound"] is not None
    }

    for row in rows:
        seed = row["seed"]
        if seed in seen_seeds:
            duplicate_seeds.add(seed)
        seen_seeds.add(seed)

    if duplicate_seeds:
        raise SummaryError(
            f'case "{case_id}" field "seed" has duplicate attempted row(s): '
            f"{_format_seed_set(duplicate_seeds)}"
        )
    if iterations_values != {case["iterations"]}:
        raise SummaryError(
            f'case "{case_id}" field "iterations" must match manifest value '
            f'{case["iterations"]}; observed {_join_sorted(iterations_values)}'
        )
    if restarts_values != {case["restarts"]}:
        raise SummaryError(
            f'case "{case_id}" field "restarts" must match manifest value '
            f'{case["restarts"]}; observed {_join_sorted(restarts_values)}'
        )
    expected_target_weight = case.get("target_weight")
    if target_weight_values != {expected_target_weight}:
        expected = "none" if expected_target_weight is None else str(expected_target_weight)
        observed = _join_sorted({value for value in target_weight_values if value is not None}) or "none"
        raise SummaryError(
            f'case "{case_id}" field "target_weight" must match manifest value '
            f"{expected}; observed {observed}"
        )
    expected_target_upper_bound = case.get("target_upper_bound")
    if target_upper_bound_values and target_upper_bound_values != {expected_target_upper_bound}:
        expected = "none" if expected_target_upper_bound is None else str(expected_target_upper_bound)
        raise SummaryError(
            f'case "{case_id}" field "target_upper_bound" must match manifest value '
            f"{expected}; observed {_join_sorted(target_upper_bound_values)}"
        )
    if len(build_profile_values) > 1:
        raise SummaryError(
            f'case "{case_id}" field "build_profile" must be homogeneous; '
            f"observed {_join_sorted(build_profile_values)}"
        )
    return seen_seeds
```

- [ ] **Step 2: Validate no-target multi-seed completeness**

Still in `summarize.py`, update `summarize_cases` so it calls `_validate_case_rows` before `_summarize_case`. Then enforce that all attempted no-target cases share the same seed set whenever any no-target case has more than one observed seed:

```python
def _validate_multiseed_no_target_seed_sets(
    cases: list[dict[str, Any]],
    seed_sets_by_case: dict[str, set[int]],
) -> None:
    no_target_seed_sets = {
        case["case_id"]: seed_sets_by_case[case["case_id"]]
        for case in cases
        if case.get("target_weight") is None and seed_sets_by_case[case["case_id"]]
    }
    if not no_target_seed_sets:
        return
    expected = max(no_target_seed_sets.values(), key=lambda values: (len(values), sorted(values)))
    if len(expected) <= 1:
        return
    for case_id, observed in no_target_seed_sets.items():
        if observed != expected:
            missing = expected - observed
            extra = observed - expected
            details = []
            if missing:
                details.append(f"missing {_format_seed_set(missing)}")
            if extra:
                details.append(f"extra {_format_seed_set(extra)}")
            raise SummaryError(
                f'case "{case_id}" field "seed" must include observed multi-seed set '
                f"{_format_seed_set(expected)}; observed {_format_seed_set(observed)} "
                f"({', '.join(details)})"
            )
```

Wrap `summarize_cases(cases, rows)` in `run()` with `except SummaryError` and return exit code `1`, matching row-validation failures.

- [ ] **Step 3: Populate the new CSV and Markdown fields**

In `_summarize_case`, add:

```python
"run_build_profile_values": _join_sorted(
    {row["build_profile"] for row in rows if row.get("build_profile") is not None}
),
```

In `write_summary_md`, replace the case table header with columns for observed seeds, target hits, target weights, and build profiles:

```python
"| case_id | code_id | status | attempted | successful | observed_seeds | best_upper_bound | target_upper_bound | target_hits | elapsed_s | target_weight | build_profile | search_stats | note |",
"| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
```

For each summary, format:

```python
target_hits_text = "-"
if summary["target_hit_count"] not in {None, ""}:
    rate = summary["target_hit_rate"] or ""
    target_hits_text = f"{summary['target_hit_count']}/{summary['successful_seed_rows']}"
    if rate:
        target_hits_text = f"{target_hits_text} ({rate})"
target_weight_text = summary["run_target_weight_values"] or "none"
build_profile_text = summary["run_build_profile_values"] or "none"
observed_seeds_text = summary["run_seed_values"] or "none"
```

Then include those values in the row format.

- [ ] **Step 4: Update existing expected CSV rows**

Update `benchmarks/qec_code_random_window/tests/test_summarize.py` expected dictionaries to include:

```python
"run_build_profile_values": "",
```

after `run_target_weight_values` for fixture rows that do not contain `build_profile`.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_multiseed_summary -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit implementation**

Run:

```bash
git add benchmarks/qec_code_random_window/summarize.py benchmarks/qec_code_random_window/tests/test_summarize.py
git commit -m "bench: validate multi-seed random-window summaries"
```

---

### Task 3: Add Multi-Seed Make Target and Docs

**Files:**
- Modify: `Makefile`
- Modify: `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`
- Modify: `benchmarks/qec_code_random_window/README.md`
- Modify: `docs/showcases/qec-code-random-window-benchmark.md`

**Interfaces:**
- Consumes: existing `run_local.py --seeds` and `summarize.py`.
- Produces: `make qec-code-random-window-bench-no-target-multiseed-smoke`.

- [ ] **Step 1: Write failing Make/docs assertions**

In `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`:

- Add `qec-code-random-window-bench-no-target-multiseed-smoke` to the showcase-doc assertions.
- Add a test named `test_makefile_exposes_release_no_target_multiseed_smoke_pipeline` that checks:

```python
body = make_target_body(makefile, "qec-code-random-window-bench-no-target-multiseed-smoke")
self.assertIn("qec-code-random-window-bench-no-target-multiseed-smoke", makefile)
self.assertIn(
    "QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/no-target-multiseed-smoke",
    makefile,
)
self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES)", body)
self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)", body)
self.assertIn("cargo build --release -p qec-code", body)
self.assertIn("--qec-code-bin target/release/qec-code", body)
self.assertIn("--build-profile release", body)
self.assertIn("--seeds 7 11 17", body)
self.assertNotIn("--target-weight", body)
```

- [ ] **Step 2: Run the Make/docs test and verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

Expected: FAIL because the new Make target and docs are not present yet.

- [ ] **Step 3: Implement the Make target**

Modify `Makefile`:

- Add the target name to `.PHONY`.
- Add:

```make
QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/no-target-multiseed-smoke
```

- Add help text:

```make
	@echo "  qec-code-random-window-bench-no-target-multiseed-smoke - Run release/no-target random-window three-seed smoke"
```

- Add target body:

```make
qec-code-random-window-bench-no-target-multiseed-smoke:
	rm -rf $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)
	mkdir -p $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)
	python3 -m benchmarks.qec_code_random_window.validate_cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES)
	cargo build --release -p qec-code
	python3 -m benchmarks.qec_code_random_window.run_local --cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES) --out $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)/local-runs.jsonl --qec-code-bin target/release/qec-code --build-profile release --seeds 7 11 17
	python3 -m benchmarks.qec_code_random_window.summarize --cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES) --runs $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)/local-runs.jsonl --out-dir $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)/summary
```

- [ ] **Step 4: Update docs**

In `benchmarks/qec_code_random_window/README.md`, add the multiseed target to the entrypoint list:

```markdown
- `make qec-code-random-window-bench-no-target-multiseed-smoke` builds
  `target/release/qec-code`, runs the BB72/BB144 no-target smoke cases with
  seeds `7`, `11`, and `17`, and summarizes per-case stability fields while
  preserving one JSONL row per case/seed.
```

In `docs/showcases/qec-code-random-window-benchmark.md`, add the command in
"Run It", expected artifacts under
`benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/`, and mention
that its summary reports observed seeds, attempted/successful seed counts,
target-hit rates, elapsed min/median/max, and unset no-target weights.

- [ ] **Step 5: Run focused docs tests and commit**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

Expected: exits 0.

Commit:

```bash
git add Makefile benchmarks/qec_code_random_window/tests/test_make_targets_docs.py benchmarks/qec_code_random_window/README.md docs/showcases/qec-code-random-window-benchmark.md
git commit -m "bench: add multi-seed no-target smoke target"
```

---

### Task 4: Run Issue Verification and Final Review Prep

**Files:**
- No planned source edits unless verification exposes a defect.

**Interfaces:**
- Consumes: the completed branch from Tasks 1-3.
- Produces: passing verification evidence and a branch ready for final review.

- [ ] **Step 1: Run the issue positive check**

Run:

```bash
cargo build --release -p qec-code
python3 -m benchmarks.qec_code_random_window.run_local \
  --cases benchmarks/qec_code_random_window/cases.no-target-smoke.toml \
  --out /tmp/no-target-multiseed.jsonl \
  --qec-code-bin target/release/qec-code \
  --build-profile release \
  --seeds 7 11 17
python3 -m benchmarks.qec_code_random_window.summarize \
  --cases benchmarks/qec_code_random_window/cases.no-target-smoke.toml \
  --runs /tmp/no-target-multiseed.jsonl \
  --out-dir /tmp/no-target-multiseed-summary
python3 -m unittest benchmarks.qec_code_random_window.tests.test_multiseed_summary -q
```

Expected: all commands exit 0.

- [ ] **Step 2: Run the negative control**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_multiseed_summary.MultiSeedSummaryTest.test_rejects_missing_seed_or_mixed_build_profile -q
```

Expected: exits 0.

- [ ] **Step 3: Run broader required checks**

Run:

```bash
python3 -m unittest discover benchmarks/qec_code_random_window/tests -q
cargo test
```

Expected: both commands exit 0.

- [ ] **Step 4: Inspect diff and status**

Run:

```bash
git diff --check
git status --short
git log --oneline --decorate -8
```

Expected: `git diff --check` exits 0, status is clean after commits, and recent commits are scoped to the spec, tests, summary implementation, and Make/docs target.
