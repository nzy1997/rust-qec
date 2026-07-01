# Issue 354 Issue-225 Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local issue-225 acceleration evidence and closure-readiness report that validates existing random-window no-target smoke outputs and prints a greppable PASS decision.

**Architecture:** A committed JSON evidence manifest records the issue/PR chain, while `issue225_readiness.py` validates generated JSONL/CSV benchmark outputs and renders Markdown plus `summary.txt`. The root Makefile serially runs existing no-target ladder and multi-seed smoke targets before invoking the readiness report generator.

**Tech Stack:** Python standard library (`argparse`, `csv`, `json`, `dataclasses`, `pathlib`, `unittest`), GNU Make, existing qec-code random-window benchmark JSONL/CSV outputs, existing Rust `qec-code` release binary.

## Global Constraints

- Add `benchmarks/qec_code_random_window/issue225_readiness.py`.
- Add `benchmarks/qec_code_random_window/issue225_evidence.json`.
- Add `benchmarks/qec_code_random_window/tests/test_issue225_readiness.py`.
- Add Make target `qec-code-random-window-bench-issue225-readiness-smoke`.
- Write generated report artifacts under `benchmarks/out/qec_code_random_window/issue225-readiness-smoke/`.
- Do not commit generated benchmark outputs under `benchmarks/out/`.
- Reuse the existing no-target ladder and no-target multi-seed smoke targets.
- Required issue chain: #337, #338, #339, #343, #344, #345, #346, #351, #352, and #353.
- Required no-target ladder cases and best upper bounds: `surface_rotated_d5 = 5`, `toric_d5 = 5`, `bb72 = 6`, and `bb144 = 12`.
- Required multi-seed cases: `bb72_no_target_smoke` and `bb144_no_target_smoke`, both with seeds `7`, `11`, and `17`.
- Required no-target semantics: `target_weight = null`, `target_reached = false`, and `build_profile = release`.
- Required search counters: `weight_pruned_candidates`, `kernel_basis_generations`, `component_candidates_generated`, and `target_reached`.
- Required timing buckets: `kernel_basis_time_ns`, `span_filter_time_ns`, `witness_validation_time_ns`, and `total_search_time_ns`.
- Reject missing cases, non-release rows, non-null target weights, `target_reached = true`, missing timing fields, and loose upper bounds.
- Print and write a plainly greppable final decision line: `issue_225_readiness: PASS`.
- Do not close #225 automatically.
- Do not add a new distance-search algorithm.
- Do not require external `codeDistancePYPI`, QDistRnd, M4RI, Gurobi, SAT, or network access.
- Do not introduce hard wall-clock performance thresholds.

---

### Task 1: Add Issue-225 Readiness Checker, Evidence, Tests, And Make Target

**Files:**
- Create: `benchmarks/qec_code_random_window/issue225_readiness.py`
- Create: `benchmarks/qec_code_random_window/issue225_evidence.json`
- Create: `benchmarks/qec_code_random_window/tests/test_issue225_readiness.py`
- Modify: `Makefile`
- Modify: `benchmarks/qec_code_random_window/README.md`
- Modify: `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`

**Interfaces:**
- Consumes: `benchmarks/out/qec_code_random_window/no-target-ladder-smoke/local-runs.jsonl`.
- Consumes: `benchmarks/out/qec_code_random_window/no-target-ladder-smoke/summary/summary.csv`.
- Consumes: `benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/local-runs.jsonl`.
- Consumes: `benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/summary/summary.csv`.
- Consumes: `benchmarks/qec_code_random_window/issue225_evidence.json`.
- Produces: `benchmarks/out/qec_code_random_window/issue225-readiness-smoke/report.md`.
- Produces: `benchmarks/out/qec_code_random_window/issue225-readiness-smoke/summary.txt`.
- Produces: Python API `evaluate_readiness(evidence_path: Path, ladder_runs_path: Path, ladder_summary_path: Path, multiseed_runs_path: Path, multiseed_summary_path: Path) -> ReadinessReport`.
- Produces: CLI `python3 -m benchmarks.qec_code_random_window.issue225_readiness --evidence ... --ladder-runs ... --ladder-summary ... --multiseed-runs ... --multiseed-summary ... --out-dir ...`.

- [ ] **Step 1: Write the failing tests**

Create `benchmarks/qec_code_random_window/tests/test_issue225_readiness.py` with `Issue225ReadinessTest`. Build fixture helpers in the test file that write:

```python
def _stats(**overrides: object) -> dict[str, object]:
    stats = {
        "permutations_sampled": 10,
        "kernel_basis_generations": 500,
        "component_candidates_generated": 25,
        "zero_candidates_rejected": 0,
        "weight_pruned_candidates": 3,
        "stabilizer_span_candidates_rejected": 4,
        "witness_validation_candidates_rejected": 5,
        "valid_witnesses_found": 2,
        "best_witness_updates": 1,
        "target_reached": False,
        "permutation_time_ns": 100,
        "kernel_basis_time_ns": 200,
        "span_filter_time_ns": 300,
        "witness_validation_time_ns": 400,
        "best_update_time_ns": 50,
        "total_search_time_ns": 1200,
    }
    stats.update(overrides)
    return stats
```

Add positive fixture rows for ladder cases `surface_rotated_d5`, `toric_d5`, `bb72`, and `bb144` with best upper bounds `5`, `5`, `6`, and `12`, plus multi-seed rows for `bb72_no_target_smoke` and `bb144_no_target_smoke` with seeds `7`, `11`, and `17`. Every row must have `status = "ok"`, `build_profile = "release"`, `target_weight = None`, `command` without `--target-weight`, and `raw_cli_json.search_stats` from `_stats()`.

Test `test_accepts_good_fixture_and_formats_report` should call:

```python
report = issue225_readiness.evaluate_readiness(
    evidence_path=evidence,
    ladder_runs_path=ladder_runs,
    ladder_summary_path=ladder_summary,
    multiseed_runs_path=multiseed_runs,
    multiseed_summary_path=multiseed_summary,
)
markdown = report.to_markdown()
```

Assert `report.decision == "PASS"`, `issue_225_readiness: PASS` is in Markdown, all required issue tokens are in Markdown, the four ladder rows and upper bounds are present, `target_weight = null`, `target_reached = false`, `build_profile = release`, `weight_pruned_candidates`, `kernel_basis_generations`, `component_candidates_generated`, `kernel_basis_time_ns`, `span_filter_time_ns`, `witness_validation_time_ns`, `total_search_time_ns`, and `7;11;17` are present.

Test `test_rejects_missing_bb144_or_targeted_run` should mutate the good fixture twice: remove the `bb144` ladder row, and set the `bb72` ladder row `target_weight` to `6` plus add `--target-weight` to `command` and `search_stats.target_reached = True`. Each call to `evaluate_readiness(...)` must raise `Issue225ReadinessError`; assertions must name `bb144` with `missing` for the first case and `bb72` with `target_weight` or `target_reached` for the second.

Test `test_rejects_missing_timing_or_loose_upper_bound` should mutate the good fixture twice: remove `kernel_basis_time_ns` from `bb72` search stats, and set `bb144` ladder `upper_bound` to `13` while the summary best upper bound is also `13`. Each call must raise `Issue225ReadinessError`; assertions must name `bb72` with `kernel_basis_time_ns` for the first case and `bb144` with `best_upper_bound` for the second.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness -q
```

Expected: FAIL with `ImportError` or `AttributeError` because `benchmarks.qec_code_random_window.issue225_readiness` and its API do not exist yet.

- [ ] **Step 3: Add the evidence manifest**

Create `benchmarks/qec_code_random_window/issue225_evidence.json` containing:

```json
{
  "issue_225": {
    "issue": 225,
    "url": "https://github.com/nzy1997/rstim/issues/225",
    "summary": "random-window upper-bound goal and acceleration closure readiness"
  },
  "chain": [
    {
      "milestone": "M1: benchmark evidence and no-target semantics",
      "issue": 337,
      "issue_url": "https://github.com/nzy1997/rstim/issues/337",
      "title": "Add a release no-target issue-225 ladder profiling smoke",
      "pr": 340,
      "pr_url": "https://github.com/nzy1997/rstim/pull/340",
      "merged_at": "2026-06-30T09:46:34Z",
      "evidence": "Adds release/no-target ladder smoke for surface_rotated_d5, toric_d5, bb72, and bb144."
    },
    {
      "milestone": "M1: benchmark evidence and no-target semantics",
      "issue": 338,
      "issue_url": "https://github.com/nzy1997/rstim/issues/338",
      "title": "Report random-window search counters in CLI JSON",
      "pr": 341,
      "pr_url": "https://github.com/nzy1997/rstim/pull/341",
      "merged_at": "2026-06-30T10:02:25Z",
      "evidence": "Adds search_stats counters and benchmark summary aggregation."
    },
    {
      "milestone": "M1: benchmark evidence and no-target semantics",
      "issue": 339,
      "issue_url": "https://github.com/nzy1997/rstim/issues/339",
      "title": "Add multi-seed no-target stability reporting",
      "pr": 342,
      "pr_url": "https://github.com/nzy1997/rstim/pull/342",
      "merged_at": "2026-07-01T01:31:41Z",
      "evidence": "Adds BB72/BB144 no-target multi-seed summaries for seeds 7, 11, and 17."
    },
    {
      "milestone": "M2: diagnostics and pruning",
      "issue": 343,
      "issue_url": "https://github.com/nzy1997/rstim/issues/343",
      "title": "Add per-stage timing diagnostics to random-window search",
      "pr": 347,
      "pr_url": "https://github.com/nzy1997/rstim/pull/347",
      "merged_at": "2026-07-01T04:00:41Z",
      "evidence": "Adds kernel, span, witness, and total timing buckets."
    },
    {
      "milestone": "M2: diagnostics and pruning",
      "issue": 344,
      "issue_url": "https://github.com/nzy1997/rstim/issues/344",
      "title": "Replace inner-loop witness validation with CSS component checks",
      "pr": 349,
      "pr_url": "https://github.com/nzy1997/rstim/pull/349",
      "merged_at": "2026-07-01T06:09:01Z",
      "evidence": "Adds algebraic CSS component filtering before full witness construction."
    },
    {
      "milestone": "M2: diagnostics and pruning",
      "issue": 345,
      "issue_url": "https://github.com/nzy1997/rstim/issues/345",
      "title": "Prune candidates that cannot beat current best",
      "pr": 348,
      "pr_url": "https://github.com/nzy1997/rstim/pull/348",
      "merged_at": "2026-07-01T04:51:43Z",
      "evidence": "Adds current-best pruning and weight_pruned_candidates evidence."
    },
    {
      "milestone": "M2: diagnostics and pruning",
      "issue": 346,
      "issue_url": "https://github.com/nzy1997/rstim/issues/346",
      "title": "Introduce a reusable GF(2) workspace",
      "pr": 350,
      "pr_url": "https://github.com/nzy1997/rstim/pull/350",
      "merged_at": "2026-07-01T07:14:04Z",
      "evidence": "Reuses GF(2) workspace state for random-window kernel-basis generation."
    },
    {
      "milestone": "M3: bit-packed acceleration",
      "issue": 351,
      "issue_url": "https://github.com/nzy1997/rstim/issues/351",
      "title": "Add bit-packed GF(2) row primitives",
      "pr": 355,
      "pr_url": "https://github.com/nzy1997/rstim/pull/355",
      "merged_at": "2026-07-01T10:45:48Z",
      "evidence": "Adds dense GF(2) row packing, XOR, parity, popcount, and zero checks."
    },
    {
      "milestone": "M3: bit-packed acceleration",
      "issue": 352,
      "issue_url": "https://github.com/nzy1997/rstim/issues/352",
      "title": "Use bit-packed kernel-basis generation",
      "pr": 357,
      "pr_url": "https://github.com/nzy1997/rstim/pull/357",
      "merged_at": "2026-07-01T12:19:13Z",
      "evidence": "Routes random-window kernel-basis workspace through bit-packed GF(2) rows."
    },
    {
      "milestone": "M3: bit-packed acceleration",
      "issue": 353,
      "issue_url": "https://github.com/nzy1997/rstim/issues/353",
      "title": "Use bit-packed CSS span filtering",
      "pr": 356,
      "pr_url": "https://github.com/nzy1997/rstim/pull/356",
      "merged_at": "2026-07-01T12:52:16Z",
      "evidence": "Routes CSS component filtering through bit-packed kernel and stabilizer-span checks."
    }
  ]
}
```

- [ ] **Step 4: Implement the readiness module**

Implement `issue225_readiness.py` with:

```python
class Issue225ReadinessError(ValueError):
    """Validation error for issue-225 readiness inputs."""
```

`evaluate_readiness(...)` should load evidence JSON, JSONL rows, and CSV rows; accumulate validation errors; raise `Issue225ReadinessError("\\n".join(errors))` if any error exists; otherwise return a `ReadinessReport` dataclass with `decision = "PASS"` and normalized ladder, multiseed, counter, timing, and evidence data.

The validation must explicitly check:

- evidence contains every required issue exactly once and each has a positive integer PR plus non-empty issue/PR URLs;
- ladder rows contain all required case IDs;
- ladder summary contains all required case IDs and `best_upper_bound` exactly matches `5`, `5`, `6`, and `12`;
- multi-seed rows contain `bb72_no_target_smoke` and `bb144_no_target_smoke` with exactly seeds `{7, 11, 17}`;
- every required row has `status == "ok"`, `build_profile == "release"`, `target_weight is None`, no `--target-weight` command entry, `raw_cli_json.search_stats.target_reached is False`, required counters, and required timing fields.

`ReadinessReport.to_markdown()` should include the decision line, evidence chain grouped by milestone, no-target ladder table, multi-seed table, search counter table, and timing table. `write_outputs(out_dir)` should write `report.md` and `summary.txt`.

The CLI `main(argv)` should catch `Issue225ReadinessError`, print the message to stderr, and return `1`. On success it should write outputs, print `issue_225_readiness: PASS`, and return `0`.

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness.Issue225ReadinessTest.test_rejects_missing_bb144_or_targeted_run -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness.Issue225ReadinessTest.test_rejects_missing_timing_or_loose_upper_bound -q
```

Expected: all commands exit 0.

- [ ] **Step 6: Add Makefile and README wiring**

Modify `Makefile`:

- add `qec-code-random-window-bench-issue225-readiness-smoke` to `.PHONY`;
- add `QEC_CODE_RANDOM_WINDOW_ISSUE225_READINESS_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/issue225-readiness-smoke`;
- add `QEC_CODE_RANDOM_WINDOW_ISSUE225_EVIDENCE := benchmarks/qec_code_random_window/issue225_evidence.json`;
- add help text `qec-code-random-window-bench-issue225-readiness-smoke - Run issue-225 readiness report smoke`;
- add a target that serially invokes `$(MAKE) qec-code-random-window-bench-no-target-ladder-smoke`, then `$(MAKE) qec-code-random-window-bench-no-target-multiseed-smoke`, then runs `python3 -m benchmarks.qec_code_random_window.issue225_readiness ...`.

Modify `benchmarks/qec_code_random_window/README.md` to add the new Make target, output directory, PASS decision, and local-only/no-generated-output policy.

Modify `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py` to assert the new Make target, output directory, evidence manifest variable, module invocation, existing smoke target invocations, and README mention.

- [ ] **Step 7: Run full issue verification**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
make qec-code-random-window-bench-issue225-readiness-smoke
cargo test
git diff --check
```

Expected:

- unit tests exit 0;
- Make target exits 0;
- `benchmarks/out/qec_code_random_window/issue225-readiness-smoke/report.md` exists;
- report includes `issue_225_readiness: PASS`;
- report lists #337, #338, #339, #343, #344, #345, #346, #351, #352, and #353;
- ladder section records best upper bounds 5, 5, 6, and 12;
- no-target checks confirm `target_weight = null`, `target_reached = false`, and `build_profile = release`;
- timing section includes non-empty kernel, span, witness, and total timing buckets;
- generated benchmark outputs remain untracked.
