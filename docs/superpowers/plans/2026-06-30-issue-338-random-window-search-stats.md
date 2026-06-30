# Issue 338 Random-Window Search Stats Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Report random-window search counters in `qec-code` CLI JSON and aggregate those counters in benchmark summaries.

**Architecture:** Add a `RandomWindowSearchStats` serde struct in `qec-code/src/distance_bound.rs`, carry it through the existing `random_window_css_upper_bound` loop, and serialize it as `search_stats` on random-window results. Keep `run_local.py` unchanged because it already preserves raw CLI JSON, and teach `summarize.py` to validate and sum optional `raw_cli_json.search_stats` objects.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, Cargo integration tests, Python 3 standard library `unittest`, CSV/Markdown benchmark artifacts.

## Global Constraints

- Keep the existing randomized upper-bound result contract stable.
- Stats are diagnostic only; do not add performance thresholds.
- Integer counters must be non-negative; `target_reached` must be boolean.
- Do not optimize the search algorithm.
- Do not change the meaning of `upper_bound` or `bound_type`.
- Preserve raw CLI JSON in benchmark JSONL rows.
- When an automatic Superpowers choice is required, choose the recommended option.

---

## File Structure

- Modify `qec-code/src/distance_bound.rs`
  - owns `RandomWindowSearchStats`, stores optional stats on distance-bound results, increments counters in the random-window search, and sets `target_reached` on early target exit.
- Modify `qec-code/tests/distance_bound.rs`
  - adds the required pinned stats serialization and invariant test.
- Modify `qec-code/tests/cli.rs`
  - asserts CLI JSON contains `search_stats` for random-window runs.
- Modify `qec-code/doc/css_distance.md`
  - documents the new `search_stats` object in the JSON contract.
- Modify `benchmarks/qec_code_random_window/summarize.py`
  - validates optional search stats and emits aggregate CSV/Markdown fields.
- Modify `benchmarks/qec_code_random_window/tests/test_summarize.py`
  - updates existing expected CSV/Markdown shapes for the new summary fields.
- Create `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py`
  - adds focused positive and negative search-stat summary tests.

---

### Task 1: Rust Random-Window Search Stats

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/tests/distance_bound.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Produces: `RandomWindowSearchStats` with serde field names exactly matching `search_stats` children from issue #338.
- Produces: random-window result JSON with `search_stats`; randomized-upper-bound JSON remains without this field.

- [ ] **Step 1: Write the failing Rust tests**

Add `RandomWindowSearchStats` to the import list in `qec-code/tests/distance_bound.rs`.

Append this test near the existing random-window tests:

```rust
#[test]
fn random_window_upper_bound_reports_search_stats() {
    let css = css_from_built_in_code_id("surface_rotated:d=5");
    let target_result = random_window_css_upper_bound(
        &css,
        RandomWindowUpperBoundOptions {
            iterations: 20,
            restarts: 2,
            seed: 7,
            target_weight: Some(5),
        },
    )
    .unwrap();

    let json = serde_json::to_value(&target_result).unwrap();
    let search_stats = json["search_stats"]
        .as_object()
        .expect("random-window result should serialize search_stats");
    for field in [
        "permutations_sampled",
        "kernel_basis_generations",
        "component_candidates_generated",
        "zero_candidates_rejected",
        "stabilizer_span_candidates_rejected",
        "witness_validation_candidates_rejected",
        "valid_witnesses_found",
        "best_witness_updates",
    ] {
        assert!(
            search_stats[field].as_u64().is_some(),
            "{field} should serialize as a non-negative integer"
        );
    }

    let stats = target_result
        .search_stats
        .expect("random-window result should carry stats");
    assert!(stats.permutations_sampled > 0);
    assert!(stats.component_candidates_generated >= stats.valid_witnesses_found);
    assert!(stats.component_candidates_generated >= stats.best_witness_updates);
    assert!(stats.valid_witnesses_found >= stats.best_witness_updates);
    assert!(stats.target_reached);

    let no_target = random_window_css_upper_bound(
        &css,
        RandomWindowUpperBoundOptions {
            iterations: 2,
            restarts: 1,
            seed: 7,
            target_weight: None,
        },
    )
    .unwrap();
    let no_target_stats = no_target
        .search_stats
        .expect("random-window result should carry stats");
    assert!(no_target_stats.permutations_sampled > 0);
    assert!(!no_target_stats.target_reached);
}
```

In `qec-code/tests/cli.rs`, add `assert!(json["search_stats"].is_object());` to the first success block in `css_distance_random_window_upper_bound_cli_contract`.

- [ ] **Step 2: Run the Rust stats test and verify it fails**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_stats -q
```

Expected before implementation: compilation failure or assertion failure because `search_stats` is not available.

- [ ] **Step 3: Implement the stats struct and result field**

In `qec-code/src/distance_bound.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomWindowSearchStats {
    pub permutations_sampled: usize,
    pub kernel_basis_generations: usize,
    pub component_candidates_generated: usize,
    pub zero_candidates_rejected: usize,
    pub stabilizer_span_candidates_rejected: usize,
    pub witness_validation_candidates_rejected: usize,
    pub valid_witnesses_found: usize,
    pub best_witness_updates: usize,
    pub target_reached: bool,
}
```

Add this field to `DistanceBoundResult` after `provenance`:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_stats: Option<RandomWindowSearchStats>,
```

Set `search_stats: None` in `completed_with_method`. In
`completed_random_window_upper_bound`, assign `result.search_stats =
Some(RandomWindowSearchStats::default())` before returning. Add a private
`completed_random_window_upper_bound_with_stats` constructor that takes a
`RandomWindowSearchStats` and stores `Some(stats)`.

- [ ] **Step 4: Increment counters in the search loop**

In `random_window_css_upper_bound`, initialize:

```rust
let mut search_stats = RandomWindowSearchStats::default();
```

Increment `search_stats.permutations_sampled += 1` immediately after
`shuffled_columns`. Pass `&mut search_stats` into both
`consider_component_candidates` calls.

Change `consider_component_candidates` to accept
`search_stats: &mut RandomWindowSearchStats`. Inside it:

```rust
search_stats.kernel_basis_generations += 1;
let candidates =
    gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation)?;
search_stats.component_candidates_generated += candidates.len();
```

Then increment the rejection, valid, and update counters at the existing branch
points. Before each early return caused by `target_reached`, set
`search_stats.target_reached = true` and pass the stats into the completed
result helper. On the full-budget return, pass stats with `target_reached`
unchanged.

- [ ] **Step 5: Run focused Rust checks**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_stats -q
cargo test -p qec-code css_distance_random_window_upper_bound_cli_contract -q
```

Expected: both pass.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs qec-code/tests/cli.rs
git commit -m "feat: report random-window search stats"
```

---

### Task 2: Benchmark Summary Search Stats

**Files:**
- Modify: `benchmarks/qec_code_random_window/summarize.py`
- Modify: `benchmarks/qec_code_random_window/tests/test_summarize.py`
- Create: `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py`

**Interfaces:**
- Consumes: optional `raw_cli_json.search_stats` objects in JSONL rows.
- Produces: CSV fields named `search_stats_rows`, `search_stats_total_<counter>`, and `search_stats_target_reached_count`.

- [ ] **Step 1: Write failing Python search-stat tests**

Create `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py`:

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


class SearchStatsSummaryTest(unittest.TestCase):
    def write_manifest(self, path: Path) -> None:
        path.write_text(
            """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "stats_case"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 20
restarts = 2
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:stats"
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

    def row(self, *, stats: dict[str, object]) -> dict[str, object]:
        return {
            "case_id": "stats_case",
            "status": "ok",
            "seed": 7,
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

    def test_summarizes_search_stats(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            rows = [
                self.row(
                    stats={
                        "permutations_sampled": 2,
                        "kernel_basis_generations": 4,
                        "component_candidates_generated": 8,
                        "zero_candidates_rejected": 1,
                        "stabilizer_span_candidates_rejected": 2,
                        "witness_validation_candidates_rejected": 3,
                        "valid_witnesses_found": 2,
                        "best_witness_updates": 1,
                        "target_reached": True,
                    }
                ),
                self.row(
                    stats={
                        "permutations_sampled": 3,
                        "kernel_basis_generations": 6,
                        "component_candidates_generated": 10,
                        "zero_candidates_rejected": 0,
                        "stabilizer_span_candidates_rejected": 1,
                        "witness_validation_candidates_rejected": 4,
                        "valid_witnesses_found": 3,
                        "best_witness_updates": 2,
                        "target_reached": False,
                    }
                ),
            ]
            runs.write_text(
                "".join(json.dumps(row) + "\n" for row in rows),
                encoding="utf-8",
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(row["search_stats_rows"], "2")
            self.assertEqual(row["search_stats_total_permutations_sampled"], "5")
            self.assertEqual(row["search_stats_total_component_candidates_generated"], "18")
            self.assertEqual(row["search_stats_total_best_witness_updates"], "3")
            self.assertEqual(row["search_stats_target_reached_count"], "1")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("stats_rows=2", markdown)
            self.assertIn("permutations=5", markdown)
            self.assertIn("target_reached=1", markdown)

    def test_rejects_inconsistent_or_negative_counters(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": 1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 2,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.best_witness_updates", result.stderr)
            self.assertIn("search_stats.component_candidates_generated", result.stderr)

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": -1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 1,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.permutations_sampled", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the Python search-stat tests and verify they fail**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
```

Expected before implementation: failure because search-stat CSV fields and validation do not exist.

- [ ] **Step 3: Implement validation and aggregation in `summarize.py`**

Add constants after `CSV_FIELDS`:

```python
SEARCH_STAT_INT_FIELDS = [
    "permutations_sampled",
    "kernel_basis_generations",
    "component_candidates_generated",
    "zero_candidates_rejected",
    "stabilizer_span_candidates_rejected",
    "witness_validation_candidates_rejected",
    "valid_witnesses_found",
    "best_witness_updates",
]

SEARCH_STAT_CSV_FIELDS = [
    "search_stats_rows",
    *(f"search_stats_total_{field}" for field in SEARCH_STAT_INT_FIELDS),
    "search_stats_target_reached_count",
]
```

Extend `CSV_FIELDS` with `*SEARCH_STAT_CSV_FIELDS` before `summary_status`.

Add `_validate_search_stats(row, location)` that reads
`raw_cli_json.search_stats`, returns `None` when absent, requires every integer
counter to be a non-negative `int`, requires `target_reached` to be `bool`, and
raises `SummaryError` with messages containing `search_stats.<field>` for bad
fields. Store the result on `validated["search_stats"]`.

In `_summarize_case`, aggregate successful rows with stats:

```python
stats_rows = [row["search_stats"] for row in successful if row.get("search_stats") is not None]
search_stat_summary = {
    "search_stats_rows": len(stats_rows) if stats_rows else None,
    **{
        f"search_stats_total_{field}": sum(stats[field] for stats in stats_rows)
        if stats_rows
        else None
        for field in SEARCH_STAT_INT_FIELDS
    },
    "search_stats_target_reached_count": sum(1 for stats in stats_rows if stats["target_reached"])
    if stats_rows
    else None,
}
```

Merge `search_stat_summary` into the returned summary.

Update `write_summary_md` to add a `search_stats` column and render either `-`
or:

```text
stats_rows=<n>, permutations=<total>, candidates=<total>, target_reached=<count>
```

- [ ] **Step 4: Update existing summary expectations**

In `benchmarks/qec_code_random_window/tests/test_summarize.py`, add blank
strings for the new CSV fields to every expected row that lacks stats. Update
the Markdown header assertion to include `search_stats` before `note`.

- [ ] **Step 5: Run focused Python checks**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: both pass.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add benchmarks/qec_code_random_window/summarize.py benchmarks/qec_code_random_window/tests/test_summarize.py benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py
git commit -m "bench: summarize random-window search stats"
```

---

### Task 3: Documentation, Integration Checks, And PR Readiness

**Files:**
- Modify: `qec-code/doc/css_distance.md`
- Inspect: full branch diff

**Interfaces:**
- Consumes: Task 1 `search_stats` JSON shape.
- Produces: documented CLI JSON contract and final verification evidence.

- [ ] **Step 1: Update the JSON contract docs**

In `qec-code/doc/css_distance.md`, add `search_stats` to the sample JSON after
`provenance` and add a bullet:

```markdown
- `search_stats`: random-window diagnostic counters for sampled permutations,
  kernel basis generations, generated component candidates, rejection reasons,
  valid witnesses, best-witness updates, and whether `target_weight` ended the
  run early.
```

- [ ] **Step 2: Run doc-related Rust CLI tests**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_doc_contract -q
```

Expected: pass.

- [ ] **Step 3: Run required focused verification**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_reports_search_stats -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
```

Expected: both pass.

- [ ] **Step 4: Run full required Rust verification**

Run:

```bash
cargo test
```

Expected: pass.

- [ ] **Step 5: Run summary regression checks**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: pass.

- [ ] **Step 6: Run diff hygiene and commit docs if needed**

Run:

```bash
git diff --check
git status --short
```

If `qec-code/doc/css_distance.md` is modified, commit it:

```bash
git add qec-code/doc/css_distance.md
git commit -m "docs: describe random-window search stats"
```

Expected: no whitespace errors, and only intended files changed before commit.
