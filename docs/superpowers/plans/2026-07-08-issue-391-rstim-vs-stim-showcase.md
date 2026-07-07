# Issue 391 rstim-vs-Stim Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reviewer-readable `rstim`-vs-Stim simulator showcase and make the docs checker fail if the page loses either the speed or correctness command.

**Architecture:** Keep the page in the existing `docs/showcases/` contract and add a path-specific checker rule in `tools/check_showcase_docs.py` for the new page. The checker remains generic for all other showcase pages and only validates the two command substrings when the path is `docs/showcases/rstim-vs-stim-simulator.md`.

**Tech Stack:** Markdown showcase docs, Python 3 standard library checker, Rust workspace verification through `cargo test`.

## Global Constraints

- The showcase page path is exactly `docs/showcases/rstim-vs-stim-simulator.md`.
- The page must include second-level sections named exactly `What This Shows`, `Run It`, `Expected Result`, `Code`, `Verification`, and `Limits`.
- The canonical circuit input is the checked Stim-generated `.stim` fixture from issue #385.
- The precise claim boundary is: evidence applies to recorded workloads and recorded environments only.
- Bad, slow, failed, or incomplete performance output is still evidence when recorded honestly; it is not a documentation failure.
- Link the old umbrella issue #38 as context.
- Add the page to `docs/showcases/README.md`.
- Do not claim broad `rstim` performance parity.
- Do not require benchmark optimization before publishing the page.
- The docs checker must fail for this page if either `python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness` or `cargo run -p rstim --bin rstim -- perf run` is removed.

---

### Task 1: Add Checker Contract For rstim-vs-Stim Commands

**Files:**
- Modify: `tools/check_showcase_docs.py`

**Interfaces:**
- Consumes: Existing `validate_showcase_page(path: Path, repo_root: Path) -> list[str]`.
- Produces: Path-specific validation that appends `missing rstim-vs-Stim correctness command link` and `missing rstim-vs-Stim speed command link` for the new showcase page when the required command entry points are absent.

- [ ] **Step 1: Add failing self-test coverage**

In `tools/check_showcase_docs.py`, after `VALID_SHOWCASE`, add this fixture constant:

````python
RSTIM_VS_STIM_VALID_SHOWCASE = """# rstim-vs-Stim Simulator Evidence

## What This Shows

This fixture shows the specialized command contract.

## Run It

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \\
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \\
  --shots 20000 \\
  --out /tmp/rstim-vs-stim-correctness.json

cargo run -p rstim --bin rstim -- perf run \\
  --case stim-style-surface-sample-d11-r100-b1024 \\
  --warmup-rounds 0 \\
  --measure-rounds 1 \\
  --out /tmp/rstim-vs-stim-speed.jsonl
```

## Expected Result

The commands write reviewer-readable evidence.

## Code

See [`benchmarks/rstim_vs_stim_simulator/README.md`](benchmarks/rstim_vs_stim_simulator/README.md).

## Verification

Run the showcase checker.

## Limits

This fixture covers checker command validation only.
"""
````

Then, inside `run_self_test()` after the existing `boilerplate_limits` fixture and before `bad_link`, add:

```python
        rstim_vs_stim = write_fixture(
            root,
            "docs/showcases/rstim-vs-stim-simulator.md",
            RSTIM_VS_STIM_VALID_SHOWCASE,
        )
```

After the existing `if validate_showcase_page(valid, root):` block, add:

```python
        if validate_showcase_page(rstim_vs_stim, root):
            errors.append("rstim-vs-Stim showcase fixture should pass")
        rstim_vs_stim.write_text(
            RSTIM_VS_STIM_VALID_SHOWCASE.replace(
                "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness",
                "python3 -m benchmarks.rstim_vs_stim_simulator.missing_correctness",
            ),
            encoding="utf-8",
        )
        missing_correctness_errors = validate_showcase_page(rstim_vs_stim, root)
        if not any("correctness command" in error for error in missing_correctness_errors):
            errors.append(
                "rstim-vs-Stim fixture without correctness command did not fail: "
                f"{missing_correctness_errors}"
            )
        rstim_vs_stim.write_text(
            RSTIM_VS_STIM_VALID_SHOWCASE.replace(
                "cargo run -p rstim --bin rstim -- perf run",
                "cargo run -p rstim --bin rstim -- perf missing-run",
            ),
            encoding="utf-8",
        )
        missing_speed_errors = validate_showcase_page(rstim_vs_stim, root)
        if not any("speed command" in error for error in missing_speed_errors):
            errors.append(
                "rstim-vs-Stim fixture without speed command did not fail: "
                f"{missing_speed_errors}"
            )
        rstim_vs_stim.write_text(RSTIM_VS_STIM_VALID_SHOWCASE, encoding="utf-8")
```

- [ ] **Step 2: Run self-test to verify it fails**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
```

Expected: FAIL, with self-test errors mentioning missing correctness and speed command checks.

- [ ] **Step 3: Add specialized checker implementation**

Near the existing constants after `REQUIRED_INDEX_SECTIONS`, add:

```python
RSTIM_VS_STIM_SHOWCASE = Path("docs/showcases/rstim-vs-stim-simulator.md")
RSTIM_VS_STIM_COMMAND_REQUIREMENTS = (
    (
        "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness",
        "missing rstim-vs-Stim correctness command link",
    ),
    (
        "cargo run -p rstim --bin rstim -- perf run",
        "missing rstim-vs-Stim speed command link",
    ),
)
```

After `links_to_planning_docs`, add:

```python
def is_rstim_vs_stim_showcase(path: Path, repo_root: Path) -> bool:
    try:
        relative = path.resolve().relative_to(repo_root.resolve())
    except ValueError:
        return False
    return relative == RSTIM_VS_STIM_SHOWCASE


def validate_rstim_vs_stim_commands(path: Path, repo_root: Path, text: str) -> list[str]:
    if not is_rstim_vs_stim_showcase(path, repo_root):
        return []
    return [
        error
        for required_command, error in RSTIM_VS_STIM_COMMAND_REQUIREMENTS
        if required_command not in text
    ]
```

In `validate_showcase_page`, just before `return errors`, add:

```python
    errors.extend(validate_rstim_vs_stim_commands(path, repo_root, text))
```

- [ ] **Step 4: Run self-test to verify it passes**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
```

Expected: exits 0 and prints `ok: self-test`.

- [ ] **Step 5: Commit**

Run:

```sh
git add tools/check_showcase_docs.py
git commit -m "test: require rstim-vs-stim showcase commands"
```

### Task 2: Add Showcase Page And Index Entry

**Files:**
- Create: `docs/showcases/rstim-vs-stim-simulator.md`
- Modify: `docs/showcases/README.md`

**Interfaces:**
- Consumes: Existing fixture catalog, correctness verifier, perf commands, and showcase page contract.
- Produces: A linked showcase page whose checker validation passes and whose README index entry appears under benchmark workflows.

- [ ] **Step 1: Create the showcase page**

Create `docs/showcases/rstim-vs-stim-simulator.md` with exactly this content:

````markdown
# rstim-vs-Stim Simulator Evidence

This showcase is the reviewer-facing map for the `rstim`-vs-Stim simulator
evidence family. It explains the checked workload, the statistical correctness
workflow, the selected-case speed workflow, and the limits on any claim made
from those artifacts.

## What This Shows

The workload is the Stim-style surface-code sample case
`stim-style-surface-sample-d11-r100-b1024`. Its canonical circuit input is the
checked Stim-generated `.stim` fixture introduced by issue
[#385](https://github.com/nzy1997/rstim/issues/385), not a circuit regenerated
by `rstim`. The full fixture is
[`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`](benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim).

The evidence has two independent tracks:

- statistical sample-correctness checks compare Stim and `rstim` observable
  rates on shared checked circuit text;
- speed evidence reruns only the selected simulator comparison case and then
  summarizes raw records into reviewer-readable shots/s and report-only
  `rstim`-vs-Stim ratios.

The older umbrella issue
[#38](https://github.com/nzy1997/rstim/issues/38) is historical context for the
surface-code benchmark direction. This page narrows that umbrella to the
recorded simulator workloads, commands, and environments below.

## Run It

Validate the smoke fixture catalog:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml
```

Run the smoke correctness verifier:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --out /tmp/rstim-vs-stim-correctness.json
```

Run the selected speed case:

```sh
cargo run -p rstim --bin rstim -- perf run \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out /tmp/rstim-vs-stim-speed.jsonl
```

Summarize and render the speed evidence:

```sh
cargo run -p rstim --bin rstim -- perf summarize \
  --in /tmp/rstim-vs-stim-speed.jsonl \
  --out /tmp/rstim-vs-stim-summary.json
cargo run -p rstim --bin rstim -- perf report \
  --in /tmp/rstim-vs-stim-summary.json \
  --out /tmp/rstim-vs-stim-report.md
```

## Expected Result

The catalog validation command exits 0 and confirms that the smoke manifest
matches the checked fixtures.

The correctness verifier prints `PASS correctness smoke` for the current smoke
suite and writes `/tmp/rstim-vs-stim-correctness.json`. That JSON records each
case, selected rates and pair correlations, tolerances, sample counts, tool
status, stderr, and failure reasons. A future `WARN` or `FAIL` result should be
read as correctness evidence for that run, not hidden as a documentation
failure.

The selected speed run writes `/tmp/rstim-vs-stim-speed.jsonl` with records for
only `stim-style-surface-sample-d11-r100-b1024`. Available variants include
`stim-cli`, `rstim-interpreted`, and `rstim-compiled`; failed or unavailable
variants are represented with explicit statuses such as `tool_failed`,
`timed_out`, or `missing_variant`.

The summary JSON reports `median_shots_per_second` for completed sample
variants. The Markdown report contains the selected case label, `shots/s`, and
the phrase `report-only Stim comparison`. If `rstim` is slower than Stim, Stim
is unavailable, or a result is incomplete, that is still evidence when the
status and environment are recorded plainly.

## Code

Fixture catalog and canonical circuit input:

- [`benchmarks/rstim_vs_stim_simulator/README.md`](benchmarks/rstim_vs_stim_simulator/README.md)
- [`benchmarks/rstim_vs_stim_simulator/cases.smoke.toml`](benchmarks/rstim_vs_stim_simulator/cases.smoke.toml)
- [`benchmarks/rstim_vs_stim_simulator/cases.full.toml`](benchmarks/rstim_vs_stim_simulator/cases.full.toml)
- [`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`](benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim)
- [`benchmarks/rstim_vs_stim_simulator/validate_cases.py`](benchmarks/rstim_vs_stim_simulator/validate_cases.py)

Correctness evidence:

- [`benchmarks/rstim_vs_stim_simulator/verify_correctness.py`](benchmarks/rstim_vs_stim_simulator/verify_correctness.py)
- [`benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`](benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py)
- [`rstim/tests/sample_correctness_contract.rs`](rstim/tests/sample_correctness_contract.rs)

Speed evidence:

- [`rstim/src/perf/cases.rs`](rstim/src/perf/cases.rs)
- [`rstim/src/perf/runner.rs`](rstim/src/perf/runner.rs)
- [`rstim/src/perf/summary.rs`](rstim/src/perf/summary.rs)
- [`rstim/tests/cli_perf.rs`](rstim/tests/cli_perf.rs)
- [`rstim/tests/perf_summary.rs`](rstim/tests/perf_summary.rs)
- [`rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`](rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl)

Issue context:

- [`#38 Performance Benchmarks on Surface Codes`](https://github.com/nzy1997/rstim/issues/38)
- [`#385 Add a shared rstim-vs-Stim simulator fixture catalog`](https://github.com/nzy1997/rstim/issues/385)
- [`#386 Add a statistical sample-correctness verifier against Stim`](https://github.com/nzy1997/rstim/issues/386)
- [`#390 Report shots/s and rstim-vs-Stim ratios for sample speed evidence`](https://github.com/nzy1997/rstim/issues/390)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
```

Expected result: the command exits 0, and this page links to the speed command
and correctness command.

Negative controls for this page:

- removing the `Limits` section must fail with `missing required section:
  Limits`;
- removing `python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness`
  must fail with `missing rstim-vs-Stim correctness command link`;
- removing `cargo run -p rstim --bin rstim -- perf run` must fail with
  `missing rstim-vs-Stim speed command link`.

Run the checker self-test for the negative-control fixtures:

```sh
python3 tools/check_showcase_docs.py --self-test
```

## Limits

Evidence applies to recorded workloads and recorded environments only. A local
run on another machine, toolchain, Stim installation, or thermal state can
produce different timings and availability statuses.

The smoke correctness command is a statistical wiring and evidence check. It
does not prove all possible circuits, seeds, detector paths, or simulator
features agree with Stim.

The selected speed command is report-only `rstim`-vs-Stim context. It does not
make broad `rstim` performance parity claims, and it does not turn a
cross-machine Stim ratio into a CI gate. The same-run `rstim`
compiled-vs-interpreted comparisons remain the gating candidate.

The canonical fixture is checked Stim-generated circuit text. This page does
not claim that an `rstim` generator reproduces Stim's generator output.

Slow, bad, failed, or incomplete benchmark output is still valid evidence when
the raw record, summary, report, and environment make that status visible. This
documentation should publish that status plainly instead of blocking on
optimization work.
````

- [ ] **Step 2: Add the README index entry**

In `docs/showcases/README.md`, under `### Decoder And Benchmark Workflows` and
under the existing `Showcases:` list, add:

```markdown
- [`rstim-vs-Stim Simulator Evidence`](docs/showcases/rstim-vs-stim-simulator.md)
```

- [ ] **Step 3: Run the page checker**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
```

Expected: exits 0 and prints `ok: docs/showcases/rstim-vs-stim-simulator.md`.

- [ ] **Step 4: Run full showcase docs check**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases
```

Expected: exits 0 with `ok:` lines for all showcase Markdown files.

- [ ] **Step 5: Commit**

Run:

```sh
git add docs/showcases/rstim-vs-stim-simulator.md docs/showcases/README.md
git commit -m "docs: add rstim-vs-stim simulator showcase"
```

### Task 3: Final Verification

**Files:**
- No file changes expected.

**Interfaces:**
- Consumes: The checker contract and showcase page from Tasks 1 and 2.
- Produces: Fresh verification evidence for PR creation.

- [ ] **Step 1: Run checker self-test**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
```

Expected: exits 0 and prints `ok: self-test`.

- [ ] **Step 2: Run issue verification command**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
```

Expected: exits 0 and prints `ok: docs/showcases/rstim-vs-stim-simulator.md`.

- [ ] **Step 3: Run full showcase docs check**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases
```

Expected: exits 0 with no `error:` lines.

- [ ] **Step 4: Run repository Rust gate**

Run:

```sh
cargo test
```

Expected: exits 0. Existing warnings are acceptable if the test command exits
0.

- [ ] **Step 5: Confirm working tree**

Run:

```sh
git status --short --branch
```

Expected: branch contains only committed changes and no unrelated tracked file
modifications.
