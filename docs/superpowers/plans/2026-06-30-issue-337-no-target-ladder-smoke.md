# Issue 337 No-Target Ladder Smoke Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a release/no-target ladder smoke benchmark suite for issue-225 random-window cases.

**Architecture:** A new TOML manifest feeds the existing `validate_cases.py`, `run_local.py`, and `summarize.py` modules. The root `Makefile` exposes a release/no-target ladder target that validates the stricter no-target contract before running the existing local runner.

**Tech Stack:** GNU Make, Python standard-library TOML/JSON/unittest modules, existing Rust `qec-code` release binary, existing qec-code random-window benchmark modules.

## Global Constraints

- Keep `qec-code-random-window-bench-no-target-smoke` intact as the BB-only smoke path.
- Add `qec-code-random-window-bench-no-target-ladder-smoke`.
- Build or use `target/release/qec-code`.
- Use `benchmarks/qec_code_random_window/cases.no-target-ladder-smoke.toml`.
- Include at least `surface_rotated_d5`, `toric_d5`, `bb72`, and `bb144`.
- Omit `target_weight` for every case.
- Use bounded smoke budgets that finish locally in a few minutes.
- Write outputs under `benchmarks/out/qec_code_random_window/no-target-ladder-smoke/`.
- Every output row must record `build_profile = "release"` and `target_weight = null`.
- No recorded command may contain `--target-weight`.
- Do not optimize the random-window algorithm or add external tool requirements.

---

### Task 1: Add No-Target Ladder Validator Coverage

**Files:**
- Create: `benchmarks/qec_code_random_window/tests/test_no_target_ladder_suite.py`
- Modify: `benchmarks/qec_code_random_window/validate_cases.py`

**Interfaces:**
- Consumes: `validate_manifest(manifest: dict[str, Any]) -> list[str]`.
- Produces: `validate_no_target_ladder_manifest(manifest: dict[str, Any], required_case_ids: set[str] | None = None) -> list[str]`.

- [ ] **Step 1: Write the failing test**

Create `benchmarks/qec_code_random_window/tests/test_no_target_ladder_suite.py` with tests that import `validate_no_target_ladder_manifest`, load `cases.no-target-ladder-smoke.toml`, require the issue-225 case IDs, and mutate manifest copies to add `target_weight` and remove `bb144`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_no_target_ladder_suite.NoTargetLadderSuiteTest.test_rejects_target_weight_or_missing_required_case -q
```

Expected: FAIL because the test module or validator function is not implemented yet.

- [ ] **Step 3: Implement validator mode**

In `validate_cases.py`, add:

```python
NO_TARGET_LADDER_REQUIRED_CASE_IDS = {
    "surface_rotated_d5",
    "toric_d5",
    "bb72",
    "bb144",
}


def validate_no_target_ladder_manifest(
    manifest: dict[str, Any],
    required_case_ids: set[str] | None = None,
) -> list[str]:
    errors = validate_manifest(manifest)
    cases = manifest.get("cases")
    if not isinstance(cases, list):
        return errors

    required = required_case_ids or NO_TARGET_LADDER_REQUIRED_CASE_IDS
    present = {
        raw_case.get("case_id")
        for raw_case in cases
        if isinstance(raw_case, dict) and isinstance(raw_case.get("case_id"), str)
    }
    for missing_case_id in sorted(required - present):
        errors.append(f'no-target ladder manifest missing required case "{missing_case_id}"')

    for index, raw_case in enumerate(cases):
        if not isinstance(raw_case, dict):
            continue
        case_id = raw_case.get("case_id")
        case_label = f'case "{case_id}"' if isinstance(case_id, str) else f"case[{index}]"
        if "target_weight" in raw_case:
            errors.append(f'{case_label} must omit field "target_weight" for no-target ladder runs')
    return errors
```

Add a CLI flag:

```python
parser.add_argument(
    "--no-target-ladder-smoke",
    action="store_true",
    help="Require no-target issue-225 ladder smoke semantics.",
)
```

and choose `validate_no_target_ladder_manifest` when the flag is present.

- [ ] **Step 4: Run the negative-control test**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_no_target_ladder_suite.NoTargetLadderSuiteTest.test_rejects_target_weight_or_missing_required_case -q
```

Expected: PASS.

### Task 2: Add Manifest And Make Target

**Files:**
- Create: `benchmarks/qec_code_random_window/cases.no-target-ladder-smoke.toml`
- Modify: `Makefile`

**Interfaces:**
- Consumes: issue-225 case IDs and code IDs from `qec-code/tests/fixtures/distance/issue_225_ladder.json`.
- Produces: `make qec-code-random-window-bench-no-target-ladder-smoke`.

- [ ] **Step 1: Add the manifest**

Create `benchmarks/qec_code_random_window/cases.no-target-ladder-smoke.toml` with four cases:

```toml
manifest_version = 1
suite = "qec_code_random_window"
description = "Release no-target issue-225 ladder smoke cases for qec-code random-window upper-bound profiling."

[[cases]]
case_id = "surface_rotated_d5"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 500
restarts = 1
seed = 7
target_upper_bound = 5
source_issue = 225
baseline_key = "unmapped:surface_rotated_d5"
baseline_required = false

[[cases]]
case_id = "toric_d5"
code_id = "toric:d=5"
distance_side = "any"
iterations = 500
restarts = 1
seed = 7
target_upper_bound = 5
source_issue = 225
baseline_key = "unmapped:toric_d5"
baseline_required = false

[[cases]]
case_id = "bb72"
code_id = "bb72"
distance_side = "any"
iterations = 500
restarts = 1
seed = 7
target_upper_bound = 6
source_issue = 225
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
baseline_required = true

[[cases]]
case_id = "bb144"
code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
distance_side = "any"
iterations = 500
restarts = 1
seed = 7
target_upper_bound = 12
source_issue = 225
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
baseline_required = true
```

- [ ] **Step 2: Add Makefile variables and help**

Add `qec-code-random-window-bench-no-target-ladder-smoke` to `.PHONY`, add variables for the manifest and output directory, and add a help line naming the new target.

- [ ] **Step 3: Add the Make target**

Add:

```make
qec-code-random-window-bench-no-target-ladder-smoke:
	rm -rf $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)
	mkdir -p $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)
	python3 -m benchmarks.qec_code_random_window.validate_cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_CASES) --no-target-ladder-smoke
	cargo build --release -p qec-code
	python3 -m benchmarks.qec_code_random_window.run_local --cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_CASES) --out $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)/local-runs.jsonl --qec-code-bin target/release/qec-code --build-profile release
	python3 -m benchmarks.qec_code_random_window.summarize --cases $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_CASES) --runs $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)/local-runs.jsonl --out-dir $(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)/summary
```

- [ ] **Step 4: Run manifest and Makefile tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_no_target_ladder_suite -q
```

Expected: PASS except generated-output checks may skip before the Make target runs.

### Task 3: Update Docs And Run Verification

**Files:**
- Modify: `benchmarks/qec_code_random_window/README.md`
- Modify: `docs/showcases/qec-code-random-window-benchmark.md`
- Modify: `benchmarks/qec_code_random_window/tests/test_make_targets_docs.py`

**Interfaces:**
- Consumes: new Make target and manifest from Task 2.
- Produces: discoverable docs and existing docs-contract coverage for the new target.

- [ ] **Step 1: Update docs**

Mention the no-target ladder smoke target in the benchmark README and showcase page, including the output directory and the distinction from the BB-only no-target smoke target.

- [ ] **Step 2: Extend Makefile/docs tests**

Extend `test_make_targets_docs.py` to assert the new target uses release build, the new manifest, the new output directory, and no `--target-weight`.

- [ ] **Step 3: Run required verification**

Run:

```bash
make qec-code-random-window-bench-no-target-ladder-smoke
python3 -m unittest benchmarks.qec_code_random_window.tests.test_no_target_ladder_suite -q
cargo test
```

Expected: all commands exit 0. `local-runs.jsonl` and `summary/summary.csv` exist under `benchmarks/out/qec_code_random_window/no-target-ladder-smoke/`, rows contain the four required case IDs, `build_profile` is `release`, `target_weight` is null, commands omit `--target-weight`, and successful rows have positive integer `upper_bound`.
