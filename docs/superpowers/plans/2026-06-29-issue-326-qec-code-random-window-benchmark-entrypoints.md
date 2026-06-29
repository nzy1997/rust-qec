# QEC-Code Random-Window Benchmark Entrypoints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add auditable smoke and full Make targets plus showcase documentation for the qec-code random-window benchmark evidence pipeline.

**Architecture:** The root `Makefile` orchestrates the existing benchmark modules in the order validate -> local run -> summarize -> compare. Smoke writes a header-only baseline CSV and uses non-strict comparison; full imports external codeDistancePYPI baselines from `CODEDISTANCE_PAPER_RESULTS_DIR` and uses strict comparison.

**Tech Stack:** GNU Make, Python standard-library unittest and benchmark modules, Markdown showcase docs, existing Rust `qec-code` CLI binary.

## Global Constraints

- Do not require external codeDistancePYPI spreadsheets for `make qec-code-random-window-bench-smoke`.
- Do not pass `--strict-baselines` in the no-external-data smoke path.
- Generated outputs must land under ignored `benchmarks/out/qec_code_random_window/`.
- Full runs must allow paper results to be supplied by `CODEDISTANCE_PAPER_RESULTS_DIR`.
- The showcase must contain the exact command `make qec-code-random-window-bench-smoke`.
- The showcase must state that local runs execute only local `random-window-upper-bound`.
- Smoke comparison rows without paper data must show `NA` paper baseline fields, not fabricated provenance.
- Keep long benchmark campaigns outside normal tests.

---

### Task 1: Add Makefile And Showcase Contract Tests

**Files:**
- Create: `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`

**Interfaces:**
- Consumes: root `Makefile`, `docs/showcases/README.md`, future `docs/showcases/qec-code-random-window-benchmark.md`.
- Produces: unittest contract for Task 2 and Task 3.

- [ ] **Step 1: Write the failing test**

Create `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`:

```python
from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MAKEFILE = ROOT / "Makefile"
SHOWCASE = ROOT / "docs" / "showcases" / "qec-code-random-window-benchmark.md"
SHOWCASE_INDEX = ROOT / "docs" / "showcases" / "README.md"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def make_target_body(makefile: str, target: str) -> str:
    match = re.search(rf"^{re.escape(target)}:\n(?P<body>(?:\t.*\n)+)", makefile, re.MULTILINE)
    if match is None:
        raise AssertionError(f"missing Make target {target}")
    return match.group("body")


class QecRandomWindowBenchmarkDocsTest(unittest.TestCase):
    def test_makefile_exposes_smoke_pipeline_without_external_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-smoke")

        self.assertIn("benchmarks/qec_code_random_window/cases.smoke.toml", body)
        self.assertIn("benchmarks/out/qec_code_random_window/smoke", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.validate_cases", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.run_local", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.summarize", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.compare_paper", body)
        self.assertIn("case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row", body)
        self.assertNotIn("--strict-baselines", body)
        self.assertNotIn("CODEDISTANCE_PAPER_RESULTS_DIR", body)

    def test_makefile_exposes_full_pipeline_with_imported_strict_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-full")

        self.assertIn("benchmarks/qec_code_random_window/cases.full.toml", body)
        self.assertIn("benchmarks/out/qec_code_random_window/full", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.import_paper_baselines", body)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", body)
        self.assertIn("--strict-baselines", body)

    def test_showcase_documents_smoke_command_outputs_and_limits(self) -> None:
        showcase = read_text(SHOWCASE)
        index = read_text(SHOWCASE_INDEX)

        self.assertIn("make qec-code-random-window-bench-smoke", showcase)
        self.assertIn("random-window-upper-bound", showcase)
        self.assertIn("only the local `random-window-upper-bound`", showcase)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", showcase)
        self.assertIn("benchmarks/out/qec_code_random_window/", showcase)
        self.assertIn("`NA`", showcase)
        self.assertIn("qec-code random-window benchmark", index.lower())
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

Expected: FAIL because the new Make targets and showcase page do not exist.

- [ ] **Step 3: Commit**

```bash
git add benchmarks/qec_code_random_window/tests/test_make_targets_docs.py
git commit -m "test: cover qec random-window benchmark docs contract"
```

### Task 2: Add Root Make Targets

**Files:**
- Modify: `Makefile`

**Interfaces:**
- Consumes: existing benchmark modules and manifests under `benchmarks/qec_code_random_window/`.
- Produces: `qec-code-random-window-bench-smoke` and `qec-code-random-window-bench-full` targets.

- [ ] **Step 1: Implement Makefile variables and help entries**

Update `.PHONY` to include:

```make
qec-code-random-window-bench-smoke qec-code-random-window-bench-full
```

Add these variables below `SED_I`:

```make
QEC_CODE_RANDOM_WINDOW_OUT ?= benchmarks/out/qec_code_random_window
QEC_CODE_RANDOM_WINDOW_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/smoke
QEC_CODE_RANDOM_WINDOW_FULL_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/full
QEC_CODE_RANDOM_WINDOW_SMOKE_CASES := benchmarks/qec_code_random_window/cases.smoke.toml
QEC_CODE_RANDOM_WINDOW_FULL_CASES := benchmarks/qec_code_random_window/cases.full.toml
QEC_CODE_RANDOM_WINDOW_BASELINE_HEADER := case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row
```

Add help output lines:

```make
	@echo "  qec-code-random-window-bench-smoke - Run qec-code random-window smoke evidence pipeline"
	@echo "  qec-code-random-window-bench-full  - Run qec-code random-window full pipeline using CODEDISTANCE_PAPER_RESULTS_DIR"
```

- [ ] **Step 2: Implement the smoke target**

Add this target near the other benchmark targets:

```make
qec-code-random-window-bench-smoke:
	rm -rf $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)
	mkdir -p $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)
	python3 -m benchmarks.qec_code_random_window.validate_cases $(QEC_CODE_RANDOM_WINDOW_SMOKE_CASES)
	cargo build -p qec-code
	python3 -m benchmarks.qec_code_random_window.run_local --cases $(QEC_CODE_RANDOM_WINDOW_SMOKE_CASES) --out $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/local-runs.jsonl --qec-code-bin target/debug/qec-code
	python3 -m benchmarks.qec_code_random_window.summarize --cases $(QEC_CODE_RANDOM_WINDOW_SMOKE_CASES) --runs $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/local-runs.jsonl --out-dir $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/summary
	printf '%s\n' '$(QEC_CODE_RANDOM_WINDOW_BASELINE_HEADER)' > $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/paper-baselines.empty.csv
	python3 -m benchmarks.qec_code_random_window.compare_paper --cases $(QEC_CODE_RANDOM_WINDOW_SMOKE_CASES) --local-summary $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/summary/summary.csv --paper-baselines $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/paper-baselines.empty.csv --out-dir $(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)/comparison
```

- [ ] **Step 3: Implement the full target**

Add this target after the smoke target:

```make
qec-code-random-window-bench-full:
	rm -rf $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)
	mkdir -p $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)
	python3 -m benchmarks.qec_code_random_window.validate_cases $(QEC_CODE_RANDOM_WINDOW_FULL_CASES)
	cargo build -p qec-code
	python3 -m benchmarks.qec_code_random_window.run_local --cases $(QEC_CODE_RANDOM_WINDOW_FULL_CASES) --out $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/local-runs.jsonl --qec-code-bin target/debug/qec-code
	python3 -m benchmarks.qec_code_random_window.summarize --cases $(QEC_CODE_RANDOM_WINDOW_FULL_CASES) --runs $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/local-runs.jsonl --out-dir $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/summary
	python3 -m benchmarks.qec_code_random_window.import_paper_baselines --cases $(QEC_CODE_RANDOM_WINDOW_FULL_CASES) --out $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/paper-baselines.csv
	python3 -m benchmarks.qec_code_random_window.compare_paper --cases $(QEC_CODE_RANDOM_WINDOW_FULL_CASES) --local-summary $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/summary/summary.csv --paper-baselines $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/paper-baselines.csv --out-dir $(QEC_CODE_RANDOM_WINDOW_FULL_DIR)/comparison --strict-baselines
```

- [ ] **Step 4: Run the Makefile contract test**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs.QecRandomWindowBenchmarkDocsTest.test_makefile_exposes_smoke_pipeline_without_external_baselines benchmarks.qec_code_random_window.tests.test_make_targets_docs.QecRandomWindowBenchmarkDocsTest.test_makefile_exposes_full_pipeline_with_imported_strict_baselines -q
```

Expected: PASS after Task 2.

- [ ] **Step 5: Commit**

```bash
git add Makefile
git commit -m "bench: add qec random-window Make targets"
```

### Task 3: Add Showcase Page And Index Link

**Files:**
- Create: `docs/showcases/qec-code-random-window-benchmark.md`
- Modify: `docs/showcases/README.md`

**Interfaces:**
- Consumes: Make targets from Task 2 and the existing showcase page contract.
- Produces: user-facing showcase documentation discoverable from the index.

- [ ] **Step 1: Create the showcase page**

Create `docs/showcases/qec-code-random-window-benchmark.md` with this content:

```markdown
# QEC-Code Random-Window Benchmark

Run the local `qec-code` random-window upper-bound benchmark evidence pipeline
and compare its summary rows against imported codeDistancePYPI paper baselines
when those external spreadsheets are available.

## What This Shows

This showcase documents the benchmark evidence path for
`qec-code code css-distance random-window-upper-bound`. It runs local
random-window upper-bound searches over pinned case manifests, summarizes the
best local upper bound and elapsed-time distribution, and then joins the local
summary to canonical paper-baseline rows when there is a defensible
codeDistancePYPI match.

Local runs execute only the local `random-window-upper-bound` command. They do
not run QDistEvol, QDistRndMW, m4ri, Gurobi, SAT, or any other external paper
algorithm.

## Run It

Run the smoke pipeline from the repository root:

```sh
make qec-code-random-window-bench-smoke
```

Run the full pipeline only after obtaining the upstream paper-result
spreadsheets separately:

```sh
CODEDISTANCE_PAPER_RESULTS_DIR=/path/to/codeDistancePYPI/paper-results \
  make qec-code-random-window-bench-full
```

## Expected Result

The smoke target validates
`benchmarks/qec_code_random_window/cases.smoke.toml`, builds the local
`qec-code` binary, runs local random-window cases, writes a local summary, and
writes a comparison table under
`benchmarks/out/qec_code_random_window/smoke/`.

Smoke artifacts include:

- `local-runs.jsonl`: one local runner row per smoke case and seed.
- `summary/summary.csv` and `summary/summary.md`: local best upper bound and
  elapsed-time summary rows.
- `paper-baselines.empty.csv`: a header-only canonical baseline CSV used so
  smoke runs need no external spreadsheets.
- `comparison/comparison.csv` and `comparison/comparison.md`: local rows joined
  against the header-only smoke baseline table.

Rows without a paper match show `NA` in paper method, bound, elapsed-time,
delta, ratio, and provenance fields. `NA` means no defensible paper row was
provided to that comparison run; it is not a fabricated baseline and it is not
evidence that the paper has no result.

The full target writes the same artifact shape under
`benchmarks/out/qec_code_random_window/full/`, but first imports canonical
baseline rows from `CODEDISTANCE_PAPER_RESULTS_DIR`.

## Code

Pipeline entry points and generated-output policy:

- [`Makefile`](Makefile)
- [`.gitignore`](.gitignore)

Random-window benchmark modules and manifests:

- [`benchmarks/qec_code_random_window/cases.smoke.toml`](benchmarks/qec_code_random_window/cases.smoke.toml)
- [`benchmarks/qec_code_random_window/cases.full.toml`](benchmarks/qec_code_random_window/cases.full.toml)
- [`benchmarks/qec_code_random_window/validate_cases.py`](benchmarks/qec_code_random_window/validate_cases.py)
- [`benchmarks/qec_code_random_window/run_local.py`](benchmarks/qec_code_random_window/run_local.py)
- [`benchmarks/qec_code_random_window/summarize.py`](benchmarks/qec_code_random_window/summarize.py)
- [`benchmarks/qec_code_random_window/import_paper_baselines.py`](benchmarks/qec_code_random_window/import_paper_baselines.py)
- [`benchmarks/qec_code_random_window/compare_paper.py`](benchmarks/qec_code_random_window/compare_paper.py)
- [`benchmarks/qec_code_random_window/README.md`](benchmarks/qec_code_random_window/README.md)

## Verification

Run the smoke target:

```sh
make qec-code-random-window-bench-smoke
```

Confirm the comparison Markdown contains `NA` baseline fields for smoke rows
without paper data:

```sh
grep 'NA' benchmarks/out/qec_code_random_window/smoke/comparison/comparison.md
```

Run the docs checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md
```

Run the Makefile and showcase contract test:

```sh
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

## Limits

Smoke output is an implementation and wiring check. It is not a final
paper-quality performance claim and should not be cited as statistical evidence.

The smoke target intentionally uses a header-only baseline CSV and non-strict
comparison so it can run from a clean checkout without codeDistancePYPI
spreadsheets. This is why paper baseline fields can be `NA` even for cases that
require paper rows in the full manifest.

Full comparison provenance depends on the external codeDistancePYPI paper
results directory. The spreadsheets are not committed here; obtain them from
the upstream project and point `CODEDISTANCE_PAPER_RESULTS_DIR` at the local
`paper-results` or `paper results` directory before running the full target.
```

- [ ] **Step 2: Link the page from the showcase index**

In `docs/showcases/README.md`, add this bullet under `Decoder And Benchmark Workflows` `Showcases:`:

```markdown
- [`qec-code random-window benchmark`](docs/showcases/qec-code-random-window-benchmark.md)
```

- [ ] **Step 3: Run docs tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs.QecRandomWindowBenchmarkDocsTest.test_showcase_documents_smoke_command_outputs_and_limits -q
python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md
python3 tools/check_showcase_docs.py docs/showcases/README.md
```

Expected: PASS after Task 3.

- [ ] **Step 4: Commit**

```bash
git add docs/showcases/qec-code-random-window-benchmark.md docs/showcases/README.md
git commit -m "docs: showcase qec random-window benchmark"
```

### Task 4: Verify End-To-End Smoke And Final Suite

**Files:**
- Modify if needed: `Makefile`, `docs/showcases/qec-code-random-window-benchmark.md`, `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: verified branch ready for pull request.

- [ ] **Step 1: Run all qec random-window Python tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local benchmarks.qec_code_random_window.tests.test_summarize benchmarks.qec_code_random_window.tests.test_import_paper_baselines benchmarks.qec_code_random_window.tests.test_compare_paper benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

Expected: PASS.

- [ ] **Step 2: Run the smoke Make target**

Run:

```bash
make qec-code-random-window-bench-smoke
```

Expected: exit 0 and files exist:

```text
benchmarks/out/qec_code_random_window/smoke/summary/summary.md
benchmarks/out/qec_code_random_window/smoke/comparison/comparison.md
```

- [ ] **Step 3: Inspect smoke comparison NA fields**

Run:

```bash
grep 'NA' benchmarks/out/qec_code_random_window/smoke/comparison/comparison.md
```

Expected: output includes `NA` paper fields for smoke rows.

- [ ] **Step 4: Run showcase validation**

Run:

```bash
python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md
python3 tools/check_showcase_docs.py docs/showcases/README.md
```

Expected: PASS.

- [ ] **Step 5: Run Rust tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS. If a transient unrelated fixture-catalog temp-directory race appears, rerun the failing test once and record both outputs.

- [ ] **Step 6: Check ignored output and diff hygiene**

Run:

```bash
git check-ignore benchmarks/out/qec_code_random_window/smoke/summary/summary.md
git diff --check origin/master..HEAD
git status --short
```

Expected: summary output is ignored, diff check passes, and status has no untracked generated output outside ignored paths.

- [ ] **Step 7: Commit fixes if any**

If Task 4 required follow-up edits:

```bash
git add Makefile docs/showcases/qec-code-random-window-benchmark.md docs/showcases/README.md benchmarks/qec_code_random_window/tests/test_make_targets_docs.py
git commit -m "fix: complete qec random-window benchmark entrypoints"
```
