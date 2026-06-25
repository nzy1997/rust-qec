# Issue 219 Benchmark Evidence Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a showcase page that maps existing benchmark and reproduction evidence without overclaiming statistical or algorithmic conclusions.

**Architecture:** Add one individual showcase page under `docs/showcases/` and link it from the existing showcase index. Extend the existing Python docs contract with a narrow negative-control test for the BB circuit command key typo required by issue #219.

**Tech Stack:** Markdown docs, Python standard-library `unittest` and `re`, existing showcase checker, existing Cargo benchmark spec/registry tests.

## Global Constraints

- Do not implement issue #124.
- Do not resolve issue #110.
- Do not create new benchmark results.
- Do not regenerate or edit checked-in benchmark artifacts.
- Do not add `benchmarks/bb_circuit_bposd/README.md`; link `docs/bb144_circuit_bposd_reproduction.md` instead because no committed BB circuit README exists on this branch.
- Separate smoke commands from full/manual campaigns.
- State when evidence is implementation smoke evidence rather than statistical reproduction.
- The typo `bb-circuit-bposd-memroy` must be rejected by a required verification command.
- Keep showcase links repo-relative from the workspace root.

---

## File Structure

- Create `docs/showcases/benchmark-evidence.md`: user-facing showcase page with required sections.
- Modify `docs/showcases/README.md`: add one index link to the new showcase under decoder and benchmark workflows.
- Modify `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`: add the BB circuit command-key docs contract and mutation negative control.

### Task 1: Add Benchmark Evidence Showcase And Docs Contract

**Files:**
- Create: `docs/showcases/benchmark-evidence.md`
- Modify: `docs/showcases/README.md`
- Modify: `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`

**Interfaces:**
- Consumes: `benchmarks/surface_decoder_compare/README.md`, `benchmarks/surface_decoder_compare/results/full/results.csv`, `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`, `benchmarks/surface_decoder/spec.toml`, `benchmarks/surface_decoder/full.toml`, `docs/bb144_circuit_bposd_reproduction.md`, and `docs/showcases/README.md`.
- Produces: showcase page `docs/showcases/benchmark-evidence.md`; docs contract tests `test_benchmark_evidence_showcase_links_required_evidence` and `test_benchmark_evidence_showcase_rejects_bb_circuit_command_typo`.

- [ ] **Step 1: Add the failing docs contract test**

In `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`, add `import re` above `import unittest`, add this constant next to the existing path constants:

```python
BENCHMARK_EVIDENCE_SHOWCASE_PATH = Path("docs/showcases/benchmark-evidence.md")
```

Then add these methods to `DocsContractTest` after `test_readme_and_makefile_document_rsinter_surface_benchmark_flow`:

```python
    def test_benchmark_evidence_showcase_links_required_evidence(self) -> None:
        doc = BENCHMARK_EVIDENCE_SHOWCASE_PATH.read_text()

        self.assertIn("benchmarks/surface_decoder_compare/README.md", doc)
        self.assertIn("docs/bb144_circuit_bposd_reproduction.md", doc)
        self.assertIn("benchmarks/surface_decoder_compare/results/full/results.csv", doc)
        self.assertIn("benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png", doc)
        self.assertIn("implementation smoke evidence", doc)
        self.assertIn("not statistical reproduction", doc)
        self.assertIn("bb-circuit-bposd-memory", doc)
        assert_valid_bb_circuit_command_keys(doc)

    def test_benchmark_evidence_showcase_rejects_bb_circuit_command_typo(self) -> None:
        doc = BENCHMARK_EVIDENCE_SHOWCASE_PATH.read_text().replace(
            "bb-circuit-bposd-memory",
            "bb-circuit-bposd-memroy",
            1,
        )

        with self.assertRaisesRegex(
            AssertionError, "unknown BB circuit command key: bb-circuit-bposd-memroy"
        ):
            assert_valid_bb_circuit_command_keys(doc)
```

Add this helper near the other module-level helpers:

```python
def assert_valid_bb_circuit_command_keys(text: str) -> None:
    known = {"bb-circuit-bposd-memory"}
    keys = set(re.findall(r"`(bb-circuit-bposd-[^`\\s]+)`", text))
    unknown = sorted(keys - known)
    if unknown:
        raise AssertionError(f"unknown BB circuit command key: {', '.join(unknown)}")
```

- [ ] **Step 2: Run the focused Python contract and verify RED**

Run:

```bash
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
```

Expected: FAIL because `docs/showcases/benchmark-evidence.md` does not exist yet. If the failure is anything other than a missing showcase file, fix the test before continuing.

- [ ] **Step 3: Add the showcase page**

Create `docs/showcases/benchmark-evidence.md` with this exact content:

```markdown
# Benchmark And Reproduction Evidence

This showcase maps the benchmark and reproduction evidence already committed
to the repository. It is a guide to evidence surfaces, not a new benchmark run
or an algorithmic comparison claim.

## What This Shows

The repository has two benchmark evidence tracks that answer different
questions:

- Surface-decoder comparison evidence in
  [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
  and the checked-in full-tier artifacts under
  [`benchmarks/surface_decoder_compare/results/full/`](benchmarks/surface_decoder_compare/results/full/).
- BB144 circuit-level BP-OSD reproduction evidence in
  [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md).

The surface-decoder comparison material demonstrates benchmark harness wiring,
tracked result artifacts, and smoke versus full campaign entry points. The
BB144 note records implementation smoke evidence for the `bb-circuit-bposd-memory`
path and separates that smoke result from the manual upstream-budget
reproduction command.

## Run It

Smoke commands are intended for local implementation checks:

```sh
make surface-decoder-compare-smoke
make bench-surface-smoke
cargo run -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 1 \
  --seed 12345 \
  --max-bp-iterations 10 \
  --osd-order 0
```

Full or manual campaigns are longer-running evidence-generation commands:

```sh
make surface-decoder-compare-full
make bench-surface-full
cargo run --release -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 50000 \
  --max-bp-iterations 10000 \
  --osd-order 7
```

## Expected Result

`make surface-decoder-compare-smoke` writes local smoke artifacts under
`benchmarks/surface_decoder_compare/results/smoke/`; that directory is ignored
and is for iteration only.

`make surface-decoder-compare-full` writes `results.csv` and
`surface_decoder_compare.png` under
[`benchmarks/surface_decoder_compare/results/full/`](benchmarks/surface_decoder_compare/results/full/).
The committed full-tier evidence currently includes
[`results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv) and
[`surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png).

`make bench-surface-smoke` and `make bench-surface-full` route through the
`rsinter` benchmark framework and write artifacts under
`benchmarks/out/surface_decoder/`.

The short BB144 command prints a four-column result line such as
`0.003	12	1	<num_failed_trials>`. That command is implementation smoke
evidence, not statistical reproduction. The upstream-budget BB144 command has
the same output shape with `50000` trials and is the command documented for a
statistical comparison attempt.

## Code

Primary evidence docs and commands:

- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
- [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md)
- [`Makefile`](Makefile)

Tracked surface-decoder comparison artifacts:

- [`benchmarks/surface_decoder_compare/results/full/results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv)
- [`benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png)

`rsinter` benchmark specs:

- [`benchmarks/surface_decoder/spec.toml`](benchmarks/surface_decoder/spec.toml)
- [`benchmarks/surface_decoder/full.toml`](benchmarks/surface_decoder/full.toml)

BB144 reference material:

- [`docs/figures/bb144_reference/small_ldpc.png`](docs/figures/bb144_reference/small_ldpc.png)
- [`docs/figures/bb144_reference/ldpc_vs_surface.png`](docs/figures/bb144_reference/ldpc_vs_surface.png)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/benchmark-evidence.md
```

Run the surface-decoder comparison docs contract:

```sh
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
```

That contract checks the required evidence links and owns the negative control
for the `bb-circuit-bposd-memroy` typo by rejecting unknown backtick-delimited
BB circuit command keys.

Run the `rsinter` benchmark spec and registry tests:

```sh
cargo test -p rsinter --test bench_specs --test bench_registry -q
```

These tests keep checked-in surface-decoder runner aliases and registry
expansion behavior current.

## Limits

The checked-in surface-decoder full-tier artifacts are evidence for the
committed run, not a promise about current local machine speed or a general
decoder ordering.

The surface-decoder smoke commands are implementation checks. They are not a
replacement for the full comparison campaign and should not be cited as
statistical evidence.

The BB144 lower-budget and one-trial commands are implementation smoke
evidence, not statistical reproduction. The BB144 note documents the
50,000-trial manual command for statistical comparison, but this showcase does
not add or claim a completed new 50,000-trial result.

This page does not implement new benchmark functionality, regenerate results,
or resolve open algorithmic questions about decoder behavior.
```

- [ ] **Step 4: Link the showcase from the index**

In `docs/showcases/README.md`, under the `Decoder And Benchmark Workflows`
paragraph and before `Primary code and docs:`, add:

```markdown
Showcases:

- [`Benchmark And Reproduction Evidence`](docs/showcases/benchmark-evidence.md)
```

- [ ] **Step 5: Run focused verification and verify GREEN**

Run:

```bash
python3 tools/check_showcase_docs.py docs/showcases/benchmark-evidence.md
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
cargo test -p rsinter --test bench_specs --test bench_registry -q
```

Expected: all three commands exit 0.

- [ ] **Step 6: Run the negative control explicitly**

Run:

```bash
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract.DocsContractTest.test_benchmark_evidence_showcase_rejects_bb_circuit_command_typo -q
```

Expected: exit 0. This proves the mutation-style typo fixture is rejected by the docs contract.

- [ ] **Step 7: Commit**

Run:

```bash
git add docs/showcases/benchmark-evidence.md docs/showcases/README.md benchmarks/surface_decoder_compare/tests/test_docs_contract.py docs/superpowers/plans/2026-06-25-issue-219-benchmark-evidence-showcase.md
git commit -m "docs: add benchmark evidence showcase"
```

Expected: a commit is created with only the showcase page, index link, docs contract test, and this implementation plan.
