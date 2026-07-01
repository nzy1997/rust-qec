# Issue 343 Random-Window Search Timing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-stage nanosecond timing diagnostics to random-window `search_stats` and aggregate those timings in benchmark summaries.

**Architecture:** Extend the existing `RandomWindowSearchStats` JSON object with timing fields and accumulate durations inside the existing random-window search loop. Keep `run_local.py` unchanged because it already preserves raw CLI JSON, and teach `summarize.py` to validate and summarize optional timing fields when they appear.

**Tech Stack:** Rust 2024, `std::time::Instant`, `serde`, `serde_json`, Cargo integration tests, Python 3 standard-library `unittest`, CSV and Markdown benchmark summaries.

## Global Constraints

- Extend `RandomWindowSearchStats` rather than creating a separate top-level JSON object.
- Timing fields must be non-negative integer nanoseconds.
- Include fields named `permutation_time_ns`, `kernel_basis_time_ns`, `span_filter_time_ns`, `witness_validation_time_ns`, `best_update_time_ns`, and `total_search_time_ns`.
- `total_search_time_ns` must be positive for a completed non-empty random-window run.
- Keep timing fields optional in the Python summarizer so old JSONL fixtures without timings still load.
- Reject negative timing fields, and reject rows where `total_search_time_ns` is smaller than the sum of named stage timings when all named stages are present.
- Keep `randomized-upper-bound` JSON stable and without random-window timing fields.
- Do not optimize the algorithm, change random-window sampling semantics, or change the meaning of `upper_bound`.
- Do not add hard runtime pass/fail thresholds or external profiler/reference-tool dependencies.
- When an automatic Superpowers choice is required, choose the recommended option.

---

## File Structure

- Modify `qec-code/src/distance_bound.rs`
  - Owns `RandomWindowSearchStats`, duration-to-nanosecond conversion, stage timing accumulation, and `total_search_time_ns` finalization.
- Modify `qec-code/tests/distance_bound.rs`
  - Adds `random_window_upper_bound_reports_search_timing` and keeps randomized serialization stable.
- Modify `benchmarks/qec_code_random_window/summarize.py`
  - Validates optional timing fields under `raw_cli_json.search_stats`, computes per-case timing totals, and writes compact Markdown timing notes.
- Create `benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py`
  - Adds focused timing summary positive coverage and the negative control requested by issue #343.
- Modify `benchmarks/qec_code_random_window/tests/test_summarize.py`
  - Updates exact expected CSV rows for the new timing CSV columns on fixtures that do not carry timings.

---

### Task 1: Rust Random-Window Timing Fields

**Files:**
- Modify: `qec-code/tests/distance_bound.rs`
- Modify: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Produces: `RandomWindowSearchStats` timing fields named exactly `permutation_time_ns`, `kernel_basis_time_ns`, `span_filter_time_ns`, `witness_validation_time_ns`, `best_update_time_ns`, and `total_search_time_ns`.
- Produces: random-window JSON with those fields under `search_stats`.
- Preserves: randomized-upper-bound JSON without `search_stats`.

- [ ] **Step 1: Write the failing Rust timing test**

In `qec-code/tests/distance_bound.rs`, append this test near
`random_window_upper_bound_reports_search_stats`:

```rust
#[test]
fn random_window_upper_bound_reports_search_timing() {
    let css = css_from_built_in_code_id("surface_rotated:d=5");
    let result = random_window_css_upper_bound(
        &css,
        RandomWindowUpperBoundOptions {
            iterations: 20,
            restarts: 1,
            seed: 7,
            target_weight: Some(5),
        },
    )
    .unwrap();

    let json = serde_json::to_value(&result).unwrap();
    let search_stats = json["search_stats"]
        .as_object()
        .expect("random-window result should serialize search_stats");
    let timing_fields = [
        "permutation_time_ns",
        "kernel_basis_time_ns",
        "span_filter_time_ns",
        "witness_validation_time_ns",
        "best_update_time_ns",
        "total_search_time_ns",
    ];
    for field in timing_fields {
        assert!(
            search_stats[field].as_u64().is_some(),
            "{field} should serialize as a non-negative integer"
        );
    }
    assert!(
        search_stats["total_search_time_ns"].as_u64().unwrap() > 0,
        "completed non-empty random-window run should report positive total search time"
    );

    let stats = result
        .search_stats
        .expect("random-window result should carry stats");
    let named_stage_sum = stats.permutation_time_ns
        + stats.kernel_basis_time_ns
        + stats.span_filter_time_ns
        + stats.witness_validation_time_ns
        + stats.best_update_time_ns;
    assert!(stats.total_search_time_ns >= named_stage_sum);

    let randomized = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: 20,
            restarts: 1,
            seed: 7,
            target_weight: Some(5),
        },
    )
    .unwrap();
    let randomized_json = serde_json::to_value(&randomized).unwrap();
    assert_eq!(randomized_json["method"], "randomized-upper-bound");
    assert!(randomized_json.get("search_stats").is_none());
}
```

- [ ] **Step 2: Run the Rust timing test and verify RED**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_timing -q
```

Expected before implementation: FAIL because the timing fields do not exist on
`RandomWindowSearchStats`.

- [ ] **Step 3: Add timing fields and helpers**

In `qec-code/src/distance_bound.rs`, add `use std::time::{Duration, Instant};`
near the existing imports.

Extend `RandomWindowSearchStats` with:

```rust
    pub permutation_time_ns: u64,
    pub kernel_basis_time_ns: u64,
    pub span_filter_time_ns: u64,
    pub witness_validation_time_ns: u64,
    pub best_update_time_ns: u64,
    pub total_search_time_ns: u64,
```

Add helpers below the struct:

```rust
fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}

fn add_elapsed_ns(total: &mut u64, started: Instant) {
    *total = total.saturating_add(duration_ns(started.elapsed()));
}

fn finish_search_timing(search_stats: &mut RandomWindowSearchStats, started: Instant) {
    search_stats.total_search_time_ns = duration_ns(started.elapsed());
}
```

- [ ] **Step 4: Time the random-window search loop**

In `random_window_css_upper_bound`, initialize the enclosing timer immediately
after `let mut search_stats = RandomWindowSearchStats::default();`:

```rust
let search_started = Instant::now();
```

Wrap permutation generation:

```rust
let permutation_started = Instant::now();
let permutation = shuffled_columns(width, &mut rng);
add_elapsed_ns(&mut search_stats.permutation_time_ns, permutation_started);
```

Before every successful completed-result return, call:

```rust
finish_search_timing(&mut search_stats, search_started);
```

That includes both early target returns and the full-budget return after
`best_witness.ok_or(...)`.

- [ ] **Step 5: Time candidate processing stages**

In `consider_component_candidates`, wrap kernel basis generation:

```rust
let kernel_started = Instant::now();
let candidates =
    gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation)?;
add_elapsed_ns(&mut search_stats.kernel_basis_time_ns, kernel_started);
```

Inside the candidate loop, time span filtering before the zero and stabilizer
component span checks:

```rust
let span_started = Instant::now();
let is_zero = !candidate.iter().any(|bit| *bit == 1);
if is_zero {
    add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
    search_stats.zero_candidates_rejected += 1;
    continue;
}
let in_component_span = gf2::try_in_reduced_row_span(stabilizer_component_span, &candidate)?;
add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
if in_component_span {
    search_stats.stabilizer_span_candidates_rejected += 1;
    continue;
}
```

Then time witness construction and validation:

```rust
let validation_started = Instant::now();
let witness = component_candidate_to_pauli(component, candidate)?;
let witness_is_valid =
    validate_witness_against_code_with_span(code, stabilizer_span, &witness).is_ok();
add_elapsed_ns(
    &mut search_stats.witness_validation_time_ns,
    validation_started,
);
if !witness_is_valid {
    search_stats.witness_validation_candidates_rejected += 1;
    continue;
}
```

Finally time the best-witness comparison and optional replacement:

```rust
let best_update_started = Instant::now();
let should_update = best_witness
    .as_ref()
    .is_none_or(|current| witness.weight() < current.weight());
if should_update {
    search_stats.best_witness_updates += 1;
    *best_witness = Some(witness);
}
add_elapsed_ns(&mut search_stats.best_update_time_ns, best_update_started);
```

- [ ] **Step 6: Run focused Rust checks and commit**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_timing -q
cargo test -p qec-code random_window_upper_bound_reports_search_stats -q
```

Expected: both commands exit 0.

Commit:

```bash
git add qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs
git commit -m "feat: report random-window search timing"
```

---

### Task 2: Benchmark Summary Timing Aggregation

**Files:**
- Modify: `benchmarks/qec_code_random_window/summarize.py`
- Create: `benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py`
- Modify: `benchmarks/qec_code_random_window/tests/test_summarize.py`

**Interfaces:**
- Consumes: optional timing fields inside `raw_cli_json.search_stats`.
- Produces: `summary.csv` columns `search_timing_rows` and `search_timing_total_<field>` for every timing field.
- Produces: `summary.md` timing text for cases with timing rows.

- [ ] **Step 1: Write the failing Python timing tests**

Create `benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py`:

```python
from __future__ import annotations

import csv
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class SearchTimingSummaryTest(unittest.TestCase):
    def write_manifest(self, path: Path) -> None:
        path.write_text(
            """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "timing_case"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 20
restarts = 2
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:timing"
baseline_required = false
""".lstrip(),
            encoding="utf-8",
        )

    def run_summarizer(
        self, manifest: Path, runs: Path, out_dir: Path
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

    def row(self, *, stats: dict[str, object], seed: int = 7) -> dict[str, object]:
        return {
            "case_id": "timing_case",
            "status": "ok",
            "seed": seed,
            "iterations": 20,
            "restarts": 2,
            "target_weight": 5,
            "upper_bound": 5,
            "elapsed_s": 1.25,
            "raw_cli_json": {
                "status": "completed",
                "method": "random-window-upper-bound",
                "search_stats": stats,
            },
        }

    def stats(self, **overrides: object) -> dict[str, object]:
        stats: dict[str, object] = {
            "permutations_sampled": 2,
            "kernel_basis_generations": 4,
            "component_candidates_generated": 8,
            "zero_candidates_rejected": 1,
            "stabilizer_span_candidates_rejected": 2,
            "witness_validation_candidates_rejected": 3,
            "valid_witnesses_found": 2,
            "best_witness_updates": 1,
            "target_reached": True,
            "permutation_time_ns": 100,
            "kernel_basis_time_ns": 200,
            "span_filter_time_ns": 300,
            "witness_validation_time_ns": 400,
            "best_update_time_ns": 50,
            "total_search_time_ns": 1200,
        }
        stats.update(overrides)
        return stats

    def test_summarizes_search_timing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            rows = [
                self.row(stats=self.stats()),
                self.row(
                    seed=11,
                    stats=self.stats(
                        permutation_time_ns=150,
                        kernel_basis_time_ns=250,
                        span_filter_time_ns=350,
                        witness_validation_time_ns=450,
                        best_update_time_ns=75,
                        total_search_time_ns=1500,
                        target_reached=False,
                    ),
                ),
            ]
            runs.write_text(
                "".join(json.dumps(row) + "\n" for row in rows),
                encoding="utf-8",
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(row["search_timing_rows"], "2")
            self.assertEqual(row["search_timing_total_permutation_time_ns"], "250")
            self.assertEqual(row["search_timing_total_kernel_basis_time_ns"], "450")
            self.assertEqual(row["search_timing_total_span_filter_time_ns"], "650")
            self.assertEqual(
                row["search_timing_total_witness_validation_time_ns"], "850"
            )
            self.assertEqual(row["search_timing_total_best_update_time_ns"], "125")
            self.assertEqual(row["search_timing_total_total_search_time_ns"], "2700")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("timing_rows=2", markdown)
            self.assertIn("total=0.003 ms", markdown)
            self.assertIn("kernel=0.001 ms", markdown)
            self.assertIn("witness=0.001 ms", markdown)

    def test_rejects_negative_or_inconsistent_timing(self) -> None:
        cases = [
            (
                self.stats(permutation_time_ns=-1),
                ["search_stats.permutation_time_ns"],
            ),
            (
                self.stats(total_search_time_ns=100),
                ["search_stats.total_search_time_ns", "search_stats.permutation_time_ns"],
            ),
        ]
        for bad_stats, expected_stderr in cases:
            with self.subTest(expected_stderr=expected_stderr):
                with tempfile.TemporaryDirectory() as tmp:
                    tmp_path = Path(tmp)
                    manifest = tmp_path / "cases.toml"
                    runs = tmp_path / "runs.jsonl"
                    out_dir = tmp_path / "summary"
                    self.write_manifest(manifest)
                    runs.write_text(
                        json.dumps(self.row(stats=bad_stats)) + "\n",
                        encoding="utf-8",
                    )

                    result = self.run_summarizer(manifest, runs, out_dir)

                    self.assertNotEqual(result.returncode, 0)
                    for expected in expected_stderr:
                        self.assertIn(expected, result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the timing summary test and verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing -q
```

Expected before implementation: FAIL because timing CSV fields and validation
do not exist.

- [ ] **Step 3: Add timing validation and aggregation to `summarize.py`**

Add below `SEARCH_STAT_CSV_FIELDS`:

```python
SEARCH_TIMING_FIELDS = [
    "permutation_time_ns",
    "kernel_basis_time_ns",
    "span_filter_time_ns",
    "witness_validation_time_ns",
    "best_update_time_ns",
    "total_search_time_ns",
]

SEARCH_TIMING_CSV_FIELDS = [
    "search_timing_rows",
    *(f"search_timing_total_{field}" for field in SEARCH_TIMING_FIELDS),
]
```

Change `CSV_FIELDS` composition to include timing fields after
`SEARCH_STAT_CSV_FIELDS`:

```python
CSV_FIELDS = [
    *CSV_FIELDS[:-1],
    *SEARCH_STAT_CSV_FIELDS,
    *SEARCH_TIMING_CSV_FIELDS,
    CSV_FIELDS[-1],
]
```

Inside `_validate_search_stats`, after validating `target_reached`, add:

```python
    present_timing_fields = [field for field in SEARCH_TIMING_FIELDS if field in search_stats]
    if present_timing_fields:
        for field in SEARCH_TIMING_FIELDS:
            value = search_stats.get(field)
            if not _is_int(value) or value < 0:
                raise _fail(location, f"search_stats.{field} must be a non-negative integer")
            validated[field] = value
        named_stage_sum = sum(
            validated[field]
            for field in SEARCH_TIMING_FIELDS
            if field != "total_search_time_ns"
        )
        if validated["total_search_time_ns"] < named_stage_sum:
            raise _fail(
                location,
                "search_stats.total_search_time_ns must be greater than or equal to "
                "search_stats.permutation_time_ns + search_stats.kernel_basis_time_ns + "
                "search_stats.span_filter_time_ns + search_stats.witness_validation_time_ns + "
                "search_stats.best_update_time_ns",
            )
```

In `_summarize_case`, compute timing rows and totals:

```python
    timing_rows = [
        stats for stats in stats_rows if all(field in stats for field in SEARCH_TIMING_FIELDS)
    ]
    search_timing_summary = {
        "search_timing_rows": len(timing_rows) if timing_rows else None,
        **{
            f"search_timing_total_{field}": sum(stats[field] for stats in timing_rows)
            if timing_rows
            else None
            for field in SEARCH_TIMING_FIELDS
        },
    }
```

Merge `**search_timing_summary` into the returned summary after
`**search_stat_summary`.

Add a formatting helper:

```python
def _format_ns_as_ms(value: object) -> str:
    if value in {None, ""}:
        return "-"
    assert isinstance(value, int)
    return f"{value / 1_000_000:.3f} ms"
```

In `write_summary_md`, append timing details to `search_stats_text` when
`search_timing_rows` is present:

```python
        if summary["search_timing_rows"] not in {None, ""}:
            timing_text = (
                f"timing_rows={summary['search_timing_rows']}, "
                f"total={_format_ns_as_ms(summary['search_timing_total_total_search_time_ns'])}, "
                f"kernel={_format_ns_as_ms(summary['search_timing_total_kernel_basis_time_ns'])}, "
                f"span={_format_ns_as_ms(summary['search_timing_total_span_filter_time_ns'])}, "
                f"witness={_format_ns_as_ms(summary['search_timing_total_witness_validation_time_ns'])}, "
                f"permutation={_format_ns_as_ms(summary['search_timing_total_permutation_time_ns'])}, "
                f"best_update={_format_ns_as_ms(summary['search_timing_total_best_update_time_ns'])}"
            )
            search_stats_text = (
                timing_text if search_stats_text == "-" else f"{search_stats_text}; {timing_text}"
            )
```

- [ ] **Step 4: Update exact CSV expectations without timings**

In `benchmarks/qec_code_random_window/tests/test_summarize.py`, update the
three exact expected dictionaries to include these blank fields after
`search_stats_target_reached_count`:

```python
                        "search_timing_rows": "",
                        "search_timing_total_permutation_time_ns": "",
                        "search_timing_total_kernel_basis_time_ns": "",
                        "search_timing_total_span_filter_time_ns": "",
                        "search_timing_total_witness_validation_time_ns": "",
                        "search_timing_total_best_update_time_ns": "",
                        "search_timing_total_total_search_time_ns": "",
```

- [ ] **Step 5: Run focused Python checks and commit**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: all commands exit 0.

Commit:

```bash
git add benchmarks/qec_code_random_window/summarize.py benchmarks/qec_code_random_window/tests/test_summarize.py benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py
git commit -m "bench: summarize random-window search timing"
```

---

### Task 3: Issue Verification and Branch Readiness

**Files:**
- No planned source edits unless verification exposes a defect.

**Interfaces:**
- Consumes: Rust timing fields and Python summary aggregation from Tasks 1 and 2.
- Produces: verification evidence and a branch ready for final review and PR creation.

- [ ] **Step 1: Run issue positive checks**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_timing -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing -q
make qec-code-random-window-bench-no-target-ladder-smoke
```

Expected: all commands exit 0, and
`benchmarks/out/qec_code_random_window/no-target-ladder-smoke/summary/summary.md`
contains `timing_rows=` in each successful case row.

- [ ] **Step 2: Run issue negative control**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing.SearchTimingSummaryTest.test_rejects_negative_or_inconsistent_timing -q
```

Expected: exits 0.

- [ ] **Step 3: Run broader verification**

Run:

```bash
python3 -m unittest discover benchmarks/qec_code_random_window/tests -q
cargo test
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 4: Inspect final status**

Run:

```bash
git status --short
git log --oneline --decorate -8
```

Expected: working tree is clean after commits and the branch contains scoped
commits for design, plan, Rust timing, and Python summary timing.
