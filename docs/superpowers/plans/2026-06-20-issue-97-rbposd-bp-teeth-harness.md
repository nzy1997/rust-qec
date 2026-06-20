# Issue 97 rbposd BP Teeth Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic Rust teeth tests and Python differential-harness BP option mapping for `product_sum` and `serial`.

**Architecture:** Reuse the existing checked-in `bp_product_sum_serial_sensitive.json` parity fixture as the behavioral case. Add one Rust integration test that mutates BP method and schedule independently through the existing parity runner, and factor Python harness BP option mapping so OSD and LSD kwargs accept only the implemented upstream `ldpc` names.

**Tech Stack:** Rust 2024, Cargo workspace, `rbposd` integration tests, Python 3 `pytest`, upstream `ldpc` kwargs represented by `rbposd/scripts/parity_harness.py`.

## Global Constraints

- Prove non-default BP behavior with deterministic Rust tests, not parsing-only assertions.
- Map only supported BP options into upstream `ldpc` kwargs: `minimum_sum`, `product_sum`, `parallel`, and `serial`.
- Keep unsupported BP methods and schedules rejected explicitly.
- Reuse repo-owned fixtures and existing parity harness layers.
- Do not add decoder families, benchmark plot redesigns, or full benchmark suite expansion.
- Run `cargo test -p rbposd product_sum_serial_teeth_cases`.
- Run `python3 -m pytest rbposd/scripts/test_parity_harness.py -k bp_method`.
- Run `python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported_schedule`.
- Run the workspace `cargo test` gate before finishing.

---

## File Structure

- Modify `rbposd/tests/bp.rs`: add `product_sum_serial_teeth_cases`, using the existing fixture and parity runner.
- Modify `rbposd/scripts/test_parity_harness.py`: add failing tests for `product_sum + serial` mapping and unsupported schedule rejection.
- Modify `rbposd/scripts/parity_harness.py`: add a shared BP option mapper used by OSD and LSD kwargs.

## Task 1: Rust Teeth Test for ProductSum and Serial

**Files:**
- Modify: `rbposd/tests/bp.rs`

**Interfaces:**
- Consumes: `load_parity_case`, `parity_runner::run_case`, `ParityCase.config`, `BpVariantSpec::{MinimumSum, ProductSum}`, `ScheduleSpec::{Parallel, Serial}`.
- Produces: test `product_sum_serial_teeth_cases`.

- [ ] **Step 1: Write the failing Rust test**

Append this test after `product_sum_serial_changes_bp_snapshot_on_borrowed_case` in `rbposd/tests/bp.rs`:

```rust
#[test]
fn product_sum_serial_teeth_cases() {
    let sensitive_case = load_parity_case("bp_product_sum_serial_sensitive.json");
    assert_eq!(
        sensitive_case.config.bp_variant,
        parity_schema::BpVariantSpec::ProductSum
    );
    assert_eq!(
        sensitive_case.config.schedule,
        parity_schema::ScheduleSpec::Serial
    );

    let product_sum_serial = parity_runner::run_case(&sensitive_case);
    assert_eq!(product_sum_serial.matches_expected, Some(true));

    let mut minimum_sum_serial_case = sensitive_case.clone();
    minimum_sum_serial_case.config.bp_variant = parity_schema::BpVariantSpec::MinimumSum;
    let minimum_sum_serial = parity_runner::run_case(&minimum_sum_serial_case);

    let mut product_sum_parallel_case = sensitive_case.clone();
    product_sum_parallel_case.config.schedule = parity_schema::ScheduleSpec::Parallel;
    let product_sum_parallel = parity_runner::run_case(&product_sum_parallel_case);

    assert_ne!(
        product_sum_serial.actual,
        minimum_sum_serial.actual,
        "product_sum must change decoder behavior while schedule stays serial"
    );
    assert_ne!(
        product_sum_serial.actual,
        product_sum_parallel.actual,
        "serial schedule must change decoder behavior while bp method stays product_sum"
    );
}
```

- [ ] **Step 2: Run the focused test and verify red state**

Run:

```bash
cargo test -p rbposd product_sum_serial_teeth_cases
```

Expected before implementation: Cargo runs zero matching tests before the test
exists, or the new test fails if the existing sensitive fixture does not give
separate method and schedule teeth. If it fails because one selector does not
change the public result, adjust the test to use one documented parity case per
selector while keeping the same exact test name.

- [ ] **Step 3: Keep the test minimal and passing**

If Step 2 shows the existing fixture gives separate teeth, no production Rust
code is needed. If one selector lacks a public-result difference, add the
smallest checked-in parity fixture needed for that selector and update the test
to load that fixture. Do not change BP implementation logic for this issue.

- [ ] **Step 4: Re-run the focused Rust test**

Run:

```bash
cargo test -p rbposd product_sum_serial_teeth_cases
```

Expected after implementation: one test runs and passes.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add rbposd/tests/bp.rs rbposd/tests/fixtures/parity
git commit -m "test: add rbposd bp teeth cases"
```

## Task 2: Python Harness BP Method and Schedule Mapping

**Files:**
- Modify: `rbposd/scripts/test_parity_harness.py`
- Modify: `rbposd/scripts/parity_harness.py`

**Interfaces:**
- Consumes: `map_config_to_ldpc_kwargs`, `map_lsd_case_to_ldpc_kwargs`.
- Produces: shared mapping for `minimum_sum`, `product_sum`, `parallel`, and `serial`.

- [ ] **Step 1: Write failing Python mapping and rejection tests**

Add these tests after `test_map_config_to_ldpc_kwargs_maps_contract_fields` in
`rbposd/scripts/test_parity_harness.py`:

```python
    def test_map_config_to_ldpc_kwargs_maps_product_sum_serial_bp_method(self) -> None:
        config = {
            "max_bp_iterations": 7,
            "early_stop": True,
            "bp_variant": "product_sum",
            "schedule": "serial",
            "osd_variant": "osd0",
        }
        self.assertEqual(
            map_config_to_ldpc_kwargs(config),
            {
                "max_iter": 7,
                "bp_method": "product_sum",
                "schedule": "serial",
                "osd_method": "OSD_0",
                "osd_order": 0,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_lsd_case_to_ldpc_kwargs_maps_product_sum_serial_bp_method(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 9,
                "early_stop": True,
                "bp_variant": "product_sum",
                "schedule": "serial",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 1,
            },
        }

        self.assertEqual(
            map_lsd_case_to_ldpc_kwargs(case),
            {
                "max_iter": 9,
                "bp_method": "product_sum",
                "schedule": "serial",
                "lsd_method": "localized_statistics",
                "lsd_order": 1,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_config_to_ldpc_kwargs_rejects_unsupported_schedule(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": True,
            "bp_variant": "product_sum",
            "schedule": "flooding",
            "osd_variant": "osd0",
        }
        with self.assertRaisesRegex(ValueError, "Unsupported schedule: flooding"):
            map_config_to_ldpc_kwargs(config)
```

- [ ] **Step 2: Run Python tests and verify red state**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k bp_method
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported_schedule
```

Expected before implementation: the `bp_method` run fails because
`product_sum` is rejected; the `unsupported_schedule` run passes if the current
rejection already exists, or fails until the test import/name is correct.

- [ ] **Step 3: Implement the narrow shared BP mapping**

In `rbposd/scripts/parity_harness.py`, add these constants near
`DEFAULT_BP_CONFIG`:

```python
BP_METHOD_MAP = {
    "minimum_sum": "minimum_sum",
    "product_sum": "product_sum",
}

BP_SCHEDULE_MAP = {
    "parallel": "parallel",
    "serial": "serial",
}
```

Then add this helper before `map_config_to_ldpc_kwargs`:

```python
def map_bp_config_to_ldpc_kwargs(
    config: dict[str, Any], error_context: str = ""
) -> dict[str, Any]:
    bp_variant = config.get("bp_variant")
    if bp_variant not in BP_METHOD_MAP:
        raise ValueError(f"Unsupported bp_variant{error_context}: {bp_variant}")

    schedule = config.get("schedule")
    if schedule not in BP_SCHEDULE_MAP:
        raise ValueError(f"Unsupported schedule{error_context}: {schedule}")

    early_stop = config.get("early_stop")
    if early_stop is not True:
        raise ValueError(
            f"Unsupported early_stop value{error_context}: {early_stop}. "
            "Python ldpc parity harness currently requires early_stop=true."
        )

    return {
        "max_iter": int(config["max_bp_iterations"]),
        "bp_method": BP_METHOD_MAP[bp_variant],
        "schedule": BP_SCHEDULE_MAP[schedule],
    }
```

Replace the duplicated BP checks in `map_config_to_ldpc_kwargs` with:

```python
    decoder_kwargs = map_bp_config_to_ldpc_kwargs(config)
```

and return:

```python
    return {
        **decoder_kwargs,
        "osd_method": osd_method_map[osd_variant],
        "osd_order": 0,
        "input_vector_type": "syndrome",
    }
```

Replace the duplicated BP checks in `map_lsd_case_to_ldpc_kwargs` with:

```python
    decoder_kwargs = map_bp_config_to_ldpc_kwargs(config, " for LSD")
```

and return:

```python
    return {
        **decoder_kwargs,
        "lsd_method": "localized_statistics",
        "lsd_order": lsd_order,
        "input_vector_type": "syndrome",
    }
```

- [ ] **Step 4: Re-run the focused Python verification**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k bp_method
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported_schedule
```

Expected after implementation: both commands pass.

- [ ] **Step 5: Run the full Python harness test file**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py
```

Expected: all tests in the file pass.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add rbposd/scripts/parity_harness.py rbposd/scripts/test_parity_harness.py
git commit -m "test: map rbposd non-default bp harness kwargs"
```

## Task 3: Final Verification

**Files:**
- No code files; verification only.

**Interfaces:**
- Consumes: Task 1 and Task 2 commits.
- Produces: fresh verification evidence for PR creation.

- [ ] **Step 1: Run issue verification commands**

Run:

```bash
cargo test -p rbposd product_sum_serial_teeth_cases
python3 -m pytest rbposd/scripts/test_parity_harness.py -k bp_method
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported_schedule
```

Expected: all pass.

- [ ] **Step 2: Run package and workspace gates**

Run:

```bash
cargo test -p rbposd
python3 -m pytest rbposd/scripts/test_parity_harness.py
cargo test
git diff --check
```

Expected: all pass.

- [ ] **Step 3: Commit plan/documentation if not already committed**

Run:

```bash
git status --short
git log --oneline --decorate -5
```

Expected: only intentional files are changed, and implementation commits are on
`agent/issue-97-add-teeth-tests-and-differential-harness-coverag-run-1`.
