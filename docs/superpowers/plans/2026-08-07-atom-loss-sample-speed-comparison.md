# Atom-Loss Sample-Speed Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rstim-interpreted-atom-loss` as a fourth variant of the existing d=11/r=100/1024-shot sample-speed comparison and report its median-time ratio against `rstim-interpreted`.

**Architecture:** Keep the existing baseline fixture and three variants unchanged. Generate and check in a paired fixture that changes each two-qubit `DEPOLARIZE2(0.001)` layer to `DEPOLARIZE2(p)` and inserts a target-matched `LOSS(p)` layer, then extend the Rust perf case model so only the atom-loss variant loads that paired source and runs the interpreted executor path. Reuse the generic summary comparison machinery and add one case-scoped explanation to the Markdown report.

**Tech Stack:** Rust (`rstim` perf registry, runner, summary, report), Python 3 (`unittest` fixture generator tests), Stim circuit text, Cargo integration tests.

## Global Constraints

- The canonical baseline fixture and its `stim-cli`, `rstim-interpreted`, and `rstim-compiled` variants must remain unchanged.
- The per-event probability is exactly the stable decimal `0.0003334445062`, derived from `p = 1 - 0.999^(1/3)`.
- After every two-qubit `CX` layer in this fixture, `LOSS(p)` and `DEPOLARIZE2(p)` must use the exact `CX` target list; `LOSS` samples every target independently.
- No `LOSS` instruction may be added after a single-qubit gate.
- The atom-loss fixture must run only as `rstim-interpreted-atom-loss`; do not send `LOSS` to Stim or advertise the compiled fast path.
- The atom-loss-over-baseline ratio is report-only and must not become a performance gate.
- Do not refresh or commit generated timing evidence as part of this change.

---

### Task 1: Reproducible Paired Atom-Loss Fixture

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/build_atom_loss_fixture.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_build_atom_loss_fixture.py`
- Create: `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim`

**Interfaces:**
- Consumes: canonical Stim fixture text containing four `CX` / `DEPOLARIZE2(0.001)` layer pairs.
- Produces: `PER_EVENT_PROBABILITY`, `PER_EVENT_PROBABILITY_TEXT`, `transform_circuit(text: str) -> str`, and the checked paired fixture.

- [ ] **Step 1: Write the failing fixture transformation tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_build_atom_loss_fixture.py`:

```python
from __future__ import annotations

import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import build_atom_loss_fixture


PACKAGE_DIR = Path(__file__).resolve().parents[1]
BASELINE = PACKAGE_DIR / "fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
ATOM_LOSS = PACKAGE_DIR / "fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"


class AtomLossFixtureTest(unittest.TestCase):
    def test_per_event_probability_preserves_aggregate_error_rate(self) -> None:
        p = build_atom_loss_fixture.PER_EVENT_PROBABILITY
        self.assertEqual(build_atom_loss_fixture.PER_EVENT_PROBABILITY_TEXT, "0.0003334445062")
        self.assertAlmostEqual(1.0 - (1.0 - p) ** 3, 0.001, places=12)

    def test_transform_inserts_target_matched_independent_loss(self) -> None:
        source = "CX 0 1 2 3\nDEPOLARIZE2(0.001) 0 1 2 3\nTICK\n"
        self.assertEqual(
            build_atom_loss_fixture.transform_circuit(source),
            "CX 0 1 2 3\n"
            "LOSS(0.0003334445062) 0 1 2 3\n"
            "DEPOLARIZE2(0.0003334445062) 0 1 2 3\n"
            "TICK\n",
        )

    def test_transform_rejects_mismatched_two_qubit_noise_targets(self) -> None:
        source = "CX 0 1\nDEPOLARIZE2(0.001) 1 0\n"
        with self.assertRaisesRegex(ValueError, "targets do not match"):
            build_atom_loss_fixture.transform_circuit(source)

    def test_checked_fixture_is_exact_transformation_and_has_no_single_qubit_loss(self) -> None:
        baseline = BASELINE.read_text(encoding="utf-8")
        atom_loss = ATOM_LOSS.read_text(encoding="utf-8")
        self.assertEqual(atom_loss, build_atom_loss_fixture.transform_circuit(baseline))

        lines = atom_loss.splitlines()
        cx_indices = [index for index, line in enumerate(lines) if line.startswith("CX ")]
        self.assertEqual(len(cx_indices), 4)
        for index in cx_indices:
            targets = lines[index].removeprefix("CX ")
            self.assertEqual(lines[index + 1], f"LOSS(0.0003334445062) {targets}")
            self.assertEqual(lines[index + 2], f"DEPOLARIZE2(0.0003334445062) {targets}")
        self.assertFalse(
            any(
                line.startswith("H ") and lines[index + 1].startswith("LOSS(")
                for index, line in enumerate(lines[:-1])
            )
        )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests and verify the expected failure**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_build_atom_loss_fixture -v
```

Expected: import failure because `build_atom_loss_fixture.py` does not exist.

- [ ] **Step 3: Implement the deterministic transformer and CLI**

Create `benchmarks/rstim_vs_stim_simulator/build_atom_loss_fixture.py`:

```python
from __future__ import annotations

import argparse
from pathlib import Path


ORIGINAL_TWO_QUBIT_ERROR_PROBABILITY = 0.001
PER_EVENT_PROBABILITY = 1.0 - (1.0 - ORIGINAL_TWO_QUBIT_ERROR_PROBABILITY) ** (1.0 / 3.0)
PER_EVENT_PROBABILITY_TEXT = "0.0003334445062"
BASELINE_DEPOLARIZE2_PREFIX = "DEPOLARIZE2(0.001) "


def transform_circuit(text: str) -> str:
    lines = text.splitlines()
    transformed: list[str] = []
    two_qubit_layers = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if not line.startswith("CX "):
            if line.startswith("DEPOLARIZE2("):
                raise ValueError(f"orphan DEPOLARIZE2 layer at line {index + 1}")
            transformed.append(line)
            index += 1
            continue

        if index + 1 >= len(lines) or not lines[index + 1].startswith(BASELINE_DEPOLARIZE2_PREFIX):
            raise ValueError(f"CX layer at line {index + 1} is not followed by DEPOLARIZE2(0.001)")
        targets = line.removeprefix("CX ")
        depolarize_targets = lines[index + 1].removeprefix(BASELINE_DEPOLARIZE2_PREFIX)
        if targets != depolarize_targets:
            raise ValueError(f"CX and DEPOLARIZE2 targets do not match at line {index + 1}")

        transformed.extend(
            [
                line,
                f"LOSS({PER_EVENT_PROBABILITY_TEXT}) {targets}",
                f"DEPOLARIZE2({PER_EVENT_PROBABILITY_TEXT}) {targets}",
            ]
        )
        two_qubit_layers += 1
        index += 2

    if two_qubit_layers == 0:
        raise ValueError("fixture contains no CX / DEPOLARIZE2 layers")
    return "\n".join(transformed) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Build the paired atom-loss sample benchmark fixture.")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    source = args.input.read_text(encoding="utf-8")
    args.output.write_text(transform_circuit(source), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Generate the checked fixture**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.build_atom_loss_fixture \
  --input benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --output benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim
```

Expected: exit 0; the output contains four `LOSS(0.0003334445062)` layers and four `DEPOLARIZE2(0.0003334445062)` layers.

- [ ] **Step 5: Run the fixture tests and verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_build_atom_loss_fixture -v
```

Expected: four tests pass.

- [ ] **Step 6: Commit the fixture unit**

```sh
git add benchmarks/rstim_vs_stim_simulator/build_atom_loss_fixture.py benchmarks/rstim_vs_stim_simulator/tests/test_build_atom_loss_fixture.py benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim
git commit -m "test: add paired atom-loss benchmark fixture"
```

---

### Task 2: Fourth Perf Variant and Paired-Source Routing

**Files:**
- Modify: `rstim/src/perf/cases.rs:24-188,206-291`
- Modify: `rstim/src/perf/runner.rs:54-185,240-419`
- Modify: `rstim/src/perf.rs:7-13`
- Modify: `rstim/tests/perf_harness.rs:220-390,420-470`
- Modify: `rstim/tests/perf_runner.rs:200-275`
- Modify: `rstim/src/perf/runner.rs:421-570` (module-local tests)

**Interfaces:**
- Consumes: paired fixture from Task 1.
- Produces: `PerfVariant::RstimInterpretedAtomLoss`, `PerfAtomLossVariant`, `PerfComparisonKind::SamplerAtomLossVsInterpreted`, `PerfBenchmarkCase.atom_loss_variant`, and variant-specific source selection before timing begins.

- [ ] **Step 1: Write failing registry and routing tests**

In `rstim/tests/perf_harness.rs`, extend the public-case expectations to require:

```rust
vec![
    PerfVariant::StimCli,
    PerfVariant::RstimInterpreted,
    PerfVariant::RstimCompiled,
    PerfVariant::RstimInterpretedAtomLoss,
]
```

and:

```rust
vec![
    PerfComparisonKind::SamplerCompiledVsInterpreted,
    PerfComparisonKind::SamplerAtomLossVsInterpreted,
]
```

Add these assertions to `benchmark_cases_include_stim_style_surface_sample_contract`:

```rust
let atom_loss = case.atom_loss_variant.expect("paired atom-loss variant");
assert_eq!(atom_loss.per_event_probability, 0.0003334445062);
assert_eq!(atom_loss.aggregate_error_probability, 0.001);
assert!((1.0 - (1.0 - atom_loss.per_event_probability).powi(3) - 0.001).abs() < 1e-12);

let atom_loss_path = match atom_loss.source {
    PerfCircuitSource::Fixture { canonical_input_path, noise, .. } => {
        assert_eq!(noise.after_clifford_depolarization, atom_loss.per_event_probability);
        canonical_input_path
    }
    _ => panic!("atom-loss comparison must use a checked fixture"),
};
assert_eq!(
    atom_loss_path,
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
);
let atom_loss_text = std::fs::read_to_string(std::path::Path::new("..").join(atom_loss_path))
    .expect("checked atom-loss fixture");
let atom_loss_instrs = parse_lines(&atom_loss_text).expect("atom-loss fixture parses");
let baseline_stats = rstim::stats::summarize(&instrs);
let atom_loss_stats = rstim::stats::summarize(&atom_loss_instrs);
assert_eq!(atom_loss_stats.num_qubits, baseline_stats.num_qubits);
assert_eq!(atom_loss_stats.num_measurements, baseline_stats.num_measurements);
assert_eq!(atom_loss_stats.num_detectors, baseline_stats.num_detectors);
assert_eq!(atom_loss_stats.num_observables, baseline_stats.num_observables);
assert_eq!(atom_loss_stats.max_repeat_depth, baseline_stats.max_repeat_depth);
let compiled = compile_circuit(&atom_loss_instrs).expect("atom-loss fixture compiles to routing IR");
assert_eq!(choose_sampler_path(&compiled), SamplerPathDecision::Fallback);
```

Update imports to include `choose_sampler_path`, `SamplerPathDecision`, and `PerfAtomLossVariant` where used. Add `atom_loss_variant: None` to every ad hoc `PerfBenchmarkCase` literal outside the public case.

In `rstim/src/perf/runner.rs` module-local tests, add:

```rust
#[test]
fn circuit_text_for_variant_selects_only_the_paired_atom_loss_source() {
    let case = PerfBenchmarkCase {
        label: "paired-inline",
        workload: PerfWorkload::Sample,
        source: PerfCircuitSource::Inline { text: "M 0\n" },
        atom_loss_variant: Some(crate::perf::PerfAtomLossVariant {
            source: PerfCircuitSource::Inline { text: "LOSS(0) 0\nM 0\n" },
            per_event_probability: 0.0003334445062,
            aggregate_error_probability: 0.001,
        }),
        shots: Some(4),
        tier: PerfCaseTier::ReportOnly,
        requires_compiled: true,
        requires_fallback: false,
        comparisons: &[],
    };
    let paired = paired_atom_loss_text(case).unwrap().unwrap();
    assert_eq!(
        circuit_text_for_variant("M 0\n", Some(&paired), PerfVariant::RstimInterpreted).unwrap(),
        "M 0\n"
    );
    assert_eq!(
        circuit_text_for_variant(
            "M 0\n",
            Some(&paired),
            PerfVariant::RstimInterpretedAtomLoss,
        )
        .unwrap(),
        "LOSS(0) 0\nM 0\n"
    );
}
```

In `rstim/tests/perf_runner.rs`, add a small execution test using the same inline case and:

```rust
let records = run_case_measurements(
    case,
    "M 0\n",
    &[PerfVariant::RstimInterpretedAtomLoss],
    PerfRunOptions { warmup_rounds: 0, measured_rounds: 1 },
)
.expect("atom-loss variant record");
assert_eq!(records.len(), 1);
assert_eq!(records[0].tool_variant, "rstim-interpreted-atom-loss");
assert_eq!(records[0].status, rstim::perf::PerfRecordStatus::Completed);
```

- [ ] **Step 2: Run focused tests and verify they fail for the missing API**

Run:

```sh
cargo test -p rstim --test perf_harness benchmark_cases_include_stim_style_surface_sample_contract -- --exact
cargo test -p rstim --test perf_runner atom_loss_variant_records_the_paired_interpreted_run -- --exact
```

Expected: compile failures naming the missing enum variants, struct, and `atom_loss_variant` field.

- [ ] **Step 3: Add the case-model API and public registry entry**

In `rstim/src/perf/cases.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfAtomLossVariant {
    pub source: PerfCircuitSource,
    pub per_event_probability: f64,
    pub aggregate_error_probability: f64,
}
```

Add `atom_loss_variant: Option<PerfAtomLossVariant>` to `PerfBenchmarkCase`. Add these enum values and labels:

```rust
PerfComparisonKind::SamplerAtomLossVsInterpreted
    => "sampler_atom_loss_vs_interpreted"

PerfVariant::RstimInterpretedAtomLoss
    => "rstim-interpreted-atom-loss"
```

Map the comparison labels as:

```rust
PerfComparisonKind::SamplerAtomLossVsInterpreted => (
    PerfVariant::RstimInterpretedAtomLoss.label(),
    PerfVariant::RstimInterpreted.label(),
),
```

Append the atom-loss variant in both `expected_variant_labels` and `benchmark_case_variants` only when `case.atom_loss_variant.is_some()`. Add it to `benchmark_variants()`.

Define the paired fixture metadata:

```rust
const STIM_SURFACE_D11_R100_ATOM_LOSS_FIXTURE_PATH: &str =
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim";
const STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY: f64 = 0.0003334445062;
const STIM_STYLE_SURFACE_ATOM_LOSS_NOISE: PerfNoiseMetadata = PerfNoiseMetadata {
    before_round_data_depolarization: 0.0,
    after_clifford_depolarization: STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
};
const STIM_SURFACE_COMPARISONS: &[PerfComparisonKind] = &[
    PerfComparisonKind::SamplerCompiledVsInterpreted,
    PerfComparisonKind::SamplerAtomLossVsInterpreted,
];
```

Configure only `stim-style-surface-sample-d11-r100-b1024` with:

```rust
atom_loss_variant: Some(PerfAtomLossVariant {
    source: PerfCircuitSource::Fixture {
        case_id: "stim_surface_d11_r100_atom_loss",
        canonical_input_path: STIM_SURFACE_D11_R100_ATOM_LOSS_FIXTURE_PATH,
        noise: STIM_STYLE_SURFACE_ATOM_LOSS_NOISE,
    },
    per_event_probability: STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY,
    aggregate_error_probability: 0.001,
}),
comparisons: STIM_SURFACE_COMPARISONS,
```

Every other production and test case literal gets `atom_loss_variant: None`.

Re-export `PerfAtomLossVariant` from `rstim/src/perf.rs`.

- [ ] **Step 4: Route the fourth variant to the paired source outside the timer**

In `rstim/src/perf/runner.rs`, add:

```rust
fn paired_atom_loss_text(case: PerfBenchmarkCase) -> Result<Option<String>, String> {
    case.atom_loss_variant
        .map(|variant| source_text(variant.source))
        .transpose()
}

fn circuit_text_for_variant<'a>(
    baseline_text: &'a str,
    atom_loss_text: Option<&'a str>,
    variant: PerfVariant,
) -> Result<&'a str, String> {
    if variant == PerfVariant::RstimInterpretedAtomLoss {
        return atom_loss_text.ok_or_else(|| {
            "rstim-interpreted-atom-loss requires a paired atom-loss source".to_string()
        });
    }
    Ok(baseline_text)
}
```

Map both interpreted variants to the same backend:

```rust
PerfVariant::RstimInterpreted | PerfVariant::RstimInterpretedAtomLoss => {
    SamplingBackend::Interpreted
}
```

At the start of `run_case_measurements` and `run_selected_case_measurements`, load `let atom_loss_text = paired_atom_loss_text(case)?;`. Inside each variant loop, resolve:

```rust
let variant_text = circuit_text_for_variant(text, atom_loss_text.as_deref(), *variant)?;
```

Pass `variant_text` to `run_variant`. This keeps fixture I/O and source selection outside the measured interval; `run_variant` continues to start timing only after parsing.

- [ ] **Step 5: Run focused registry and runner tests**

Run:

```sh
cargo test -p rstim --test perf_harness -q
cargo test -p rstim --test perf_runner -q
cargo test -p rstim perf::runner::tests -q
```

Expected: all focused tests pass; the public case has four variants and the atom-loss circuit routes through the interpreted fallback.

- [ ] **Step 6: Commit the perf routing unit**

```sh
git add rstim/src/perf/cases.rs rstim/src/perf/runner.rs rstim/src/perf.rs rstim/tests/perf_harness.rs rstim/tests/perf_runner.rs
git commit -m "feat: benchmark atom-loss sample variant"
```

---

### Task 3: Ratio, Explanation, and Runner-Facing Documentation

**Files:**
- Modify: `rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`
- Modify: `rstim/tests/perf_summary.rs:390-445`
- Modify: `rstim/src/perf/report.rs:59-116`
- Modify: `rstim/tests/cli_perf.rs:350-445`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py:140-275`
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md:155-174,219-235`

**Interfaces:**
- Consumes: generic comparison machinery and the fourth variant from Task 2.
- Produces: `sampler_atom_loss_vs_interpreted` summary ratio, the probability explanation in `report.md`, and documented selected/suite runner behavior.

- [ ] **Step 1: Add failing summary and report expectations**

Append this measured record to `rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`:

```json
{"case_label":"stim-style-surface-sample-d11-r100-b1024","tool_variant":"rstim-interpreted-atom-loss","workload":"sample","tier":"report_only","measurement_index":0,"warmup":false,"qubits":1,"measurements":1,"detectors":0,"observables":0,"repeat_depth":1,"repeat_count":100,"shots":1024,"wall_time_ns":7500000,"peak_memory_bytes":2200,"status":"completed","failure_reason":null,"stderr":null}
```

Extend `summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio` in `rstim/tests/perf_summary.rs`:

```rust
let atom_loss = case
    .variants
    .iter()
    .find(|variant| variant.tool_variant == "rstim-interpreted-atom-loss")
    .expect("atom-loss variant");
assert_eq!(atom_loss.median_shots_per_second, Some(1024.0 * 1_000_000_000.0 / 7_500_000.0));

let atom_loss_comparison = case
    .comparisons
    .iter()
    .find(|comparison| comparison.kind == "sampler_atom_loss_vs_interpreted")
    .expect("atom-loss comparison");
assert_eq!(atom_loss_comparison.lhs_variant, "rstim-interpreted-atom-loss");
assert_eq!(atom_loss_comparison.rhs_variant, "rstim-interpreted");
assert_eq!(atom_loss_comparison.ratio, 1.5);

assert!(report.contains("sampler_atom_loss_vs_interpreted"));
assert!(report.contains("1.500000"));
assert!(report.contains("p = 1 - 0.999^(1/3) ~= 0.0003334445062"));
assert!(report.contains("probability of at least one error equal to `0.001`"));
```

In `rstim/tests/cli_perf.rs`, strengthen the public selected-case tests:

```rust
assert!(raw.contains("\"tool_variant\":\"rstim-interpreted-atom-loss\""));
assert_eq!(records.len(), 4);
```

and for the `perf ci` report:

```rust
assert!(summary.contains("rstim-interpreted-atom-loss"));
assert!(summary.contains("sampler_atom_loss_vs_interpreted"));
assert!(report.contains("rstim-interpreted-atom-loss"));
assert!(report.contains("p = 1 - 0.999^(1/3) ~= 0.0003334445062"));
```

- [ ] **Step 2: Run the focused tests and verify the report explanation is missing**

Run:

```sh
cargo test -p rstim --test perf_summary summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio -- --exact
cargo test -p rstim --test cli_perf perf_ci_case_with_public_label_writes_only_selected_artifacts -- --exact
```

Expected: the summary ratio is produced by the new comparison kind, while the report assertions in both tests fail until the case-scoped explanation is implemented.

- [ ] **Step 3: Render the probability explanation for this comparison**

In `rstim/src/perf/report.rs`, after the expected/present variant lines and before timing lines, add:

```rust
if case
    .expected_variants
    .iter()
    .any(|variant| variant == "rstim-interpreted-atom-loss")
{
    out.push_str(
        "- atom-loss probability: each two-qubit gate has one depolarization event and two independent per-atom loss events; using `p = 1 - 0.999^(1/3) ~= 0.0003334445062` keeps the probability of at least one error equal to `0.001`.\n",
    );
}
```

Do not change `PerfGateConfig` or `evaluate_summary`; the case remains `report_only`.

- [ ] **Step 4: Prove the Python suite merger preserves the fourth item**

In `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py`, make the fake selected-case summary include:

```python
"variants": (
    [{"tool_variant": "rstim-interpreted-atom-loss"}]
    if label == "stim-style-surface-sample-d11-r100-b1024"
    else []
),
"comparisons": (
    [{"kind": "sampler_atom_loss_vs_interpreted", "ratio": 1.5}]
    if label == "stim-style-surface-sample-d11-r100-b1024"
    else []
),
```

After loading the merged summary, assert:

```python
public_case = summary["cases"][2]
self.assertEqual(public_case["variants"][0]["tool_variant"], "rstim-interpreted-atom-loss")
self.assertEqual(public_case["comparisons"][0]["kind"], "sampler_atom_loss_vs_interpreted")
```

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v
```

Expected: all suite-runner tests pass without production Python changes.

- [ ] **Step 5: Document the fourth comparison item**

Update the “Selected Speed Runner” section of `benchmarks/rstim_vs_stim_simulator/README.md` to state that the selected public case now reports four items, that only the fourth uses the paired atom-loss fixture, and that the report prints the atom-loss/interpreted ratio. Include the exact probability contract:

```text
p = 1 - 0.999^(1/3) ~= 0.0003334445062
```

State that one depolarization plus two independent per-atom loss events preserve aggregate error probability `0.001`. Keep the existing command unchanged.

- [ ] **Step 6: Run focused summary, CLI, and Python tests**

Run:

```sh
cargo test -p rstim --test perf_summary -q
cargo test -p rstim --test cli_perf -q
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v
```

Expected: all tests pass; the report contains four variants, both ratios, and the probability explanation.

- [ ] **Step 7: Commit the reporting unit**

```sh
git add rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl rstim/tests/perf_summary.rs rstim/src/perf/report.rs rstim/tests/cli_perf.rs benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py benchmarks/rstim_vs_stim_simulator/README.md
git commit -m "docs: report atom-loss sample speed ratio"
```

---

### Task 4: Full Verification and Release Smoke Benchmark

**Files:**
- Verify only; do not add checked timing artifacts.

**Interfaces:**
- Consumes: completed fixture, perf variant, summary, report, and documentation.
- Produces: test output and a temporary release-profile benchmark bundle demonstrating the requested comparison.

- [ ] **Step 1: Run all focused benchmark tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_build_atom_loss_fixture -v
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_case benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v
cargo test -p rstim --test perf_harness -q
cargo test -p rstim --test perf_runner -q
cargo test -p rstim --test perf_summary -q
cargo test -p rstim --test perf_gate -q
cargo test -p rstim --test cli_perf -q
```

Expected: every focused Python and Rust test passes.

- [ ] **Step 2: Run the full rstim test suite**

Run:

```sh
cargo test -p rstim
```

Expected: all `rstim` unit, integration, and documentation tests pass.

- [ ] **Step 3: Run a temporary release-profile benchmark**

Run these lines in one foreground shell so the validated temporary path remains scoped to the command sequence:

```sh
smoke_dir="$(mktemp -d /tmp/rstim-atom-loss-speed.XXXXXX)"
test -n "${smoke_dir:?}"
test -d "${smoke_dir:?}"
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile release \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir "${smoke_dir:?}"
rg -n "rstim-interpreted-atom-loss|sampler_atom_loss_vs_interpreted|0.0003334445062" \
  "${smoke_dir:?}/raw.jsonl" "${smoke_dir:?}/summary.json" "${smoke_dir:?}/report.md"
```

Expected: the runner exits 0; `raw.jsonl` has four completed records for the selected case; `summary.json` contains `sampler_atom_loss_vs_interpreted`; `report.md` contains the ratio and probability explanation. Leave the temporary directory untracked and do not copy it into `benchmarks/rstim_vs_stim_simulator/results/`.

- [ ] **Step 4: Verify repository scope and cleanliness**

Run:

```sh
git diff --check
git status --short
git log -4 --oneline
```

Expected: no whitespace errors; only intentional commits from Tasks 1–3 are present; no generated timing bundle is tracked.

## Plan Self-Review

- Spec coverage: Task 1 covers the paired circuit and probability contract; Task 2 covers the fourth variant and executor routing; Task 3 covers the ratio, explanation, and existing runner preservation; Task 4 covers full and release-profile verification.
- Type consistency: `PerfAtomLossVariant`, `PerfVariant::RstimInterpretedAtomLoss`, `PerfComparisonKind::SamplerAtomLossVsInterpreted`, and `atom_loss_variant` use the same names in every task.
- Scope consistency: the original fixture and original three variants are never modified, the new comparison remains report-only, and no checked timing evidence is produced.
