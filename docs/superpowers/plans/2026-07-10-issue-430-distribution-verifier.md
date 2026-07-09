# Distribution Verifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the issue #430 small-circuit distribution verifier and make the checked catalog pass honestly against Stim and `rstim`.

**Architecture:** Add a dedicated Python verifier module beside the issue #429 catalog validator. The verifier runs both tools through their public `sample` CLI using catalog circuits on stdin, compares whole-line frequencies to expected probabilities, and emits stable JSON. A narrow Rust sampler fix initializes the Pauli-frame conjugate state for implicit `|0>` qubits so `rstim` matches Stim's initial-state measurement randomness.

**Tech Stack:** Python 3.11 stdlib (`argparse`, `collections.Counter`, `hashlib`, `json`, `math`, `random`, `shlex`, `subprocess`, `sys`, `tomllib`, `unittest`, `pathlib`), existing `benchmarks.rstim_vs_stim_simulator` package, Rust 2024 Cargo workspace, existing `rstim` sampler/frame simulator.

## Global Constraints

- Verifier module path is `benchmarks.rstim_vs_stim_simulator.verify_distributions`.
- Catalog path is `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`.
- Run both tools with `sample --shots <shots> --seed <seed> --out_format 01` and provide each catalog circuit on stdin.
- Compare each tool's observed distribution to the catalog's expected distribution; do not compare Stim and `rstim` shot-by-shot.
- Use a five-standard-deviation statistical tolerance derived from expected probability and sample count, with only a tiny `1e-12` numeric floor for exact 0/1 probabilities.
- Passing CLI output starts with exactly `PASS distribution correctness cases=8 mismatch=0` for the checked catalog.
- Negative control `--inject-rstim-bitflip-rate 0.20` must exit nonzero, print `FAIL statistical mismatch`, and write JSON evidence containing at least one mismatching case.
- JSON includes per-case observed frequencies for Stim and `rstim`, expected probabilities, tolerance, sample count, status, and the catalog provenance URL.
- JSON records command lists, per-run exit status, success flag, and stderr; do not record raw sample stdout or elapsed timing.
- Reuse the `rstim` binary fallback style from `verify_correctness.py`: release binary, debug binary, then offline Cargo run.
- Do not publish checked evidence, update the benchmark site, or add performance timing.
- Keep simulator changes scoped to Stim-compatible implicit initial `|0>` sampling.

---

## File Structure

- Modify `rstim/src/sim/frame.rs`: add a method to randomize initial conjugate Z frames.
- Modify `rstim/src/sampler.rs`: call the frame initialization method before interpreted frame sampling.
- Modify `rstim/src/compiled/sampler.rs`: call the frame initialization method before compiled frame sampling.
- Modify `rstim/tests/frame_sim.rs`: add sampler regressions for implicit initial-state randomness and Bell distribution support.
- Create `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`: verifier CLI, helpers, JSON evidence, report formatting.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`: verifier helper, runner, CLI, and negative-control tests.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the new verifier and negative control.

---

### Task 1: Initial-State Frame Sampler Correctness

**Files:**
- Modify: `rstim/src/sim/frame.rs`
- Modify: `rstim/src/sampler.rs`
- Modify: `rstim/src/compiled/sampler.rs`
- Modify: `rstim/tests/frame_sim.rs`

**Interfaces:**
- Produces: `FrameSimulator::randomize_initial_z_frames(&mut self, rng: &mut impl Rng)`
- Consumes: existing `BitTable::randomize_row(row, rng)` behavior.
- Later tasks rely on `rstim sample` matching Stim for catalog cases whose circuits start from implicit `|0>` qubits.

- [ ] **Step 1: Write failing Rust sampler tests**

Append these tests near the existing `sample_batch_bell_correlated` test in `rstim/tests/frame_sim.rs`:

```rust
#[test]
fn sample_batch_h_measurement_uses_implicit_zero_state_randomness() {
    let instrs = parse_lines("H 0\nM 0\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 256, &mut rng).unwrap();

    let ones = (0..256)
        .filter(|&shot| out.measurements.get(0, shot))
        .count();
    assert!(
        (64..=192).contains(&ones),
        "expected both H-measurement outcomes, got ones={ones}"
    );
}

#[test]
fn sample_batch_bell_distribution_uses_only_balanced_correlated_support() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 512, &mut rng).unwrap();

    let mut count_00 = 0usize;
    let mut count_11 = 0usize;
    for shot in 0..512 {
        let left = out.measurements.get(0, shot);
        let right = out.measurements.get(1, shot);
        match (left, right) {
            (false, false) => count_00 += 1,
            (true, true) => count_11 += 1,
            other => panic!("unexpected Bell sample {other:?} at shot {shot}"),
        }
    }

    assert!(
        (160..=352).contains(&count_00),
        "expected balanced Bell support, count_00={count_00}, count_11={count_11}"
    );
    assert!(
        (160..=352).contains(&count_11),
        "expected balanced Bell support, count_00={count_00}, count_11={count_11}"
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
cargo test -p rstim --test frame_sim sample_batch_h_measurement_uses_implicit_zero_state_randomness
```

Expected: fail because all `H 0; M 0` outcomes are currently `0`.

- [ ] **Step 3: Add frame initialization method**

In `rstim/src/sim/frame.rs`, add this public method inside `impl FrameSimulator` after `new`:

```rust
    pub(crate) fn randomize_initial_z_frames(&mut self, rng: &mut impl Rng) {
        for q in 0..self.num_qubits {
            self.z_table.randomize_row(q, rng);
        }
    }
```

- [ ] **Step 4: Call initialization from interpreted sampling**

In `rstim/src/sampler.rs`, update `sample_batch_interpreted` after constructing the frame:

```rust
    let mut frame = FrameSimulator::new(num_qubits, n_shots);
    frame.randomize_initial_z_frames(rng);
    frame
        .set_materialize_detector_observable_outputs(options.output_mode == SampleOutputMode::Full);
```

- [ ] **Step 5: Call initialization from compiled sampling**

In `rstim/src/compiled/sampler.rs`, update `sample_compiled_batch` after constructing the frame:

```rust
    let mut frame = FrameSimulator::new(compiled.num_qubits, n_shots);
    frame.randomize_initial_z_frames(rng);
    frame
        .set_materialize_detector_observable_outputs(options.output_mode == SampleOutputMode::Full);
```

- [ ] **Step 6: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test frame_sim sample_batch_ -- --nocapture
```

Expected: all filtered `sample_batch_` tests in `frame_sim` pass.

- [ ] **Step 7: Run CLI smoke for the fixed behavior**

Run:

```sh
printf 'H 0\nM 0\n' | cargo run --quiet -p rstim --bin rstim -- sample --shots 16 --seed 1 --out_format 01
```

Expected: output contains both `0` and `1` lines.

- [ ] **Step 8: Commit**

```sh
git add rstim/src/sim/frame.rs rstim/src/sampler.rs rstim/src/compiled/sampler.rs rstim/tests/frame_sim.rs
git commit -m "fix: initialize frame sampler implicit qubits"
```

---

### Task 2: Distribution Verifier Core And Tests

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`

**Interfaces:**
- Produces: `parse_01_samples(stdout: str, expected_bits: int, expected_shots: int) -> list[str]`
- Produces: `compare_distribution(samples: list[str], expected_distribution: dict[str, float]) -> dict[str, object]`
- Produces: `verify_case(case: dict[str, object], *, stim_command: list[str], rstim_command: list[str], shots: int, seeds: list[int], inject_rstim_bitflip_rate: float) -> dict[str, object]`
- Produces: `build_summary(args: argparse.Namespace) -> dict[str, object]`
- Produces: `format_report(summary: dict[str, object]) -> tuple[int, str]`
- Produces: `main(argv: list[str] | None = None) -> int`
- Consumes: issue #429 catalog schema and `validate_distribution_cases.validate_manifest`.

- [ ] **Step 1: Write failing verifier tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`:

```python
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_distributions import (
    build_sample_command,
    compare_distribution,
    format_report,
    main,
    parse_01_samples,
    verify_case,
)


def unit_case() -> dict[str, object]:
    return {
        "case_id": "unit_bell",
        "source_url": "https://example.test/source",
        "source_commit": "abc123",
        "source_line_start": 10,
        "source_line_end": 20,
        "circuit": "H 0\nCNOT 0 1\nM 0 1\n",
        "shots": 4,
        "tolerance": 1e-9,
        "expected_distribution": {"00": 0.5, "11": 0.5},
    }


class VerifyDistributionHelpersTest(unittest.TestCase):
    def test_parse_01_samples_requires_rectangular_output(self) -> None:
        self.assertEqual(parse_01_samples("00\n11\n", expected_bits=2, expected_shots=2), ["00", "11"])
        with self.assertRaisesRegex(ValueError, "expected 2 bits"):
            parse_01_samples("0\n11\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "expected 2 shots"):
            parse_01_samples("00\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "non-01"):
            parse_01_samples("0x\n11\n", expected_bits=2, expected_shots=2)

    def test_compare_distribution_accepts_five_sigma_frequencies(self) -> None:
        result = compare_distribution(["00"] * 50 + ["11"] * 50, {"00": 0.5, "11": 0.5})

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 100)
        self.assertEqual(result["observed_counts"], {"00": 50, "11": 50})
        self.assertAlmostEqual(result["observed_frequencies"]["00"], 0.5)

    def test_compare_distribution_flags_unexpected_observed_outcome(self) -> None:
        result = compare_distribution(["00"] * 90 + ["01"] * 10, {"00": 1.0})

        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertGreater(result["max_delta"], result["max_tolerance"])
        self.assertTrue(any("01" in reason for reason in result["failure_reasons"]))

    def test_build_sample_command_uses_stdin_compatible_cli(self) -> None:
        self.assertEqual(
            build_sample_command(["rstim"], shots=4, seed=7),
            ["rstim", "sample", "--shots", "4", "--seed", "7", "--out_format", "01"],
        )


class VerifyDistributionRunnerTest(unittest.TestCase):
    def test_verify_case_records_expected_observed_tolerance_and_provenance(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            return {
                "command": command,
                "exit_code": 0,
                "stderr": "",
                "success": True,
                "stdout": "00\n11\n00\n11\n",
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 4)
        self.assertEqual(result["expected_distribution"], {"00": 0.5, "11": 0.5})
        self.assertEqual(result["source_url"], "https://example.test/source")
        self.assertEqual(result["stim"]["observed_counts"], {"00": 2, "11": 2})
        self.assertEqual(result["rstim"]["observed_frequencies"], {"00": 0.5, "11": 0.5})
        self.assertEqual(result["stim"]["runs"][0]["command"][0], "stim")
        self.assertEqual(result["rstim"]["runs"][0]["stderr"], "")

    def test_verify_case_reports_rstim_negative_control_mismatch(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            return {
                "command": command,
                "exit_code": 0,
                "stderr": "",
                "success": True,
                "stdout": "00\n00\n00\n00\n",
                "stdin_source": "catalog:circuit",
            }

        case = unit_case()
        case["expected_distribution"] = {"00": 1.0}
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                case,
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=1.0,
            )

        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertEqual(result["stim"]["status"], "pass")
        self.assertEqual(result["rstim"]["status"], "statistical_mismatch")

    def test_verify_case_records_tool_failure_stderr(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            success = command[0] == "stim"
            return {
                "command": command,
                "exit_code": 0 if success else 2,
                "stderr": "" if success else "broken rstim",
                "success": success,
                "stdout": "00\n11\n00\n11\n" if success else "",
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "rstim_failed")
        self.assertIn("broken rstim", result["failure_reasons"][0])
        self.assertEqual(result["rstim"]["runs"][0]["stderr"], "broken rstim")


class VerifyDistributionCliTest(unittest.TestCase):
    def test_main_writes_json_and_prints_pass_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [unit_case()]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.verify_case"
                ) as mocked,
            ):
                mocked.return_value = {
                    "case_id": "unit_bell",
                    "status": "pass",
                    "sample_count": 4,
                    "failure_reasons": [],
                    "expected_distribution": {"00": 0.5, "11": 0.5},
                    "source_url": "https://example.test/source",
                    "stim": {"status": "pass"},
                    "rstim": {"status": "pass"},
                }
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/distribution_cases.toml",
                            "--shots",
                            "4",
                            "--out",
                            str(out),
                        ]
                    )

        self.assertEqual(code, 0)
        data = json.loads(out.read_text())
        self.assertEqual(data["status"], "pass")
        self.assertEqual(data["case_count"], 1)
        self.assertEqual(data["counts"]["pass"], 1)
        self.assertTrue(
            any("PASS distribution correctness cases=1 mismatch=0" in call.args[0] for call in stdout.call_args_list)
        )

    def test_format_report_returns_nonzero_for_mismatch(self) -> None:
        summary = {
            "status": "statistical_mismatch",
            "case_count": 1,
            "counts": {
                "pass": 0,
                "statistical_mismatch": 1,
                "stim_failed": 0,
                "rstim_failed": 0,
            },
        }

        exit_code, report = format_report(summary)

        self.assertEqual(exit_code, 1)
        self.assertEqual(report, "FAIL statistical mismatch cases=1 mismatch=1")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
```

Expected: fail or error because `verify_distributions.py` does not exist yet.

- [ ] **Step 3: Implement verifier module**

Create `benchmarks/rstim_vs_stim_simulator/verify_distributions.py` with the implementation matching these exact interfaces and statuses:

```python
from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import shlex
import subprocess
import sys
import tomllib
from collections import Counter
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator.validate_distribution_cases import (
    load_manifest,
    validate_manifest,
)
from benchmarks.rstim_vs_stim_simulator.verify_correctness import default_rstim_command


STATUS_PASS = "pass"
STATUS_MISMATCH = "statistical_mismatch"
STATUS_STIM_FAILED = "stim_failed"
STATUS_RSTIM_FAILED = "rstim_failed"
STATUS_TOOL_FAILED = "tool_failed"
STDDEV_FACTOR = 5.0
TOLERANCE_FLOOR = 1e-12
STDIN_SOURCE = "catalog:circuit"


def build_sample_command(tool_command: list[str], *, shots: int, seed: int) -> list[str]:
    return [
        *tool_command,
        "sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        "01",
    ]


def run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            input=circuit,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "exit_code": None,
            "stderr": str(error),
            "success": False,
            "stdout": "",
            "stdin_source": STDIN_SOURCE,
        }

    return {
        "command": command,
        "exit_code": completed.returncode,
        "stderr": completed.stderr,
        "success": completed.returncode == 0,
        "stdout": completed.stdout,
        "stdin_source": STDIN_SOURCE,
    }


def parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[str]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if len(lines) != expected_shots:
        raise ValueError(f"expected {expected_shots} shots, got {len(lines)}")
    for shot_index, line in enumerate(lines):
        if len(line) != expected_bits:
            raise ValueError(f"shot {shot_index}: expected {expected_bits} bits, got {len(line)}")
        if any(ch not in "01" for ch in line):
            raise ValueError(f"shot {shot_index}: output contains non-01 data")
    return lines


def _deterministic_bitflip_seed(case_id: str, seed: int) -> int:
    digest = hashlib.sha256(f"{case_id}:{seed}".encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "big")


def inject_bitflip(samples: list[str], *, rate: float, seed: int) -> list[str]:
    if not 0.0 <= rate <= 1.0:
        raise ValueError("rate must be between 0 and 1")
    rng = random.Random(seed)
    mutated: list[str] = []
    for sample in samples:
        chars = list(sample)
        for index, bit in enumerate(chars):
            if rng.random() < rate:
                chars[index] = "0" if bit == "1" else "1"
        mutated.append("".join(chars))
    return mutated


def _expected_bit_width(expected_distribution: dict[str, Any]) -> int:
    return len(next(iter(expected_distribution)))


def _stable_float_map(values: dict[str, Any]) -> dict[str, float]:
    return {key: float(values[key]) for key in sorted(values)}


def compare_distribution(
    samples: list[str],
    expected_distribution: dict[str, float],
    *,
    stddev_factor: float = STDDEV_FACTOR,
    tolerance_floor: float = TOLERANCE_FLOOR,
) -> dict[str, object]:
    sample_count = len(samples)
    counts = Counter(samples)
    expected = _stable_float_map(expected_distribution)
    outcomes = sorted(set(expected) | set(counts))
    observed_counts = {outcome: int(counts.get(outcome, 0)) for outcome in outcomes if counts.get(outcome, 0)}
    observed_frequencies = {
        outcome: (observed_counts.get(outcome, 0) / sample_count if sample_count else 0.0)
        for outcome in outcomes
        if observed_counts.get(outcome, 0)
    }

    failure_reasons: list[str] = []
    rows: list[dict[str, object]] = []
    max_delta = 0.0
    max_tolerance = 0.0
    for outcome in outcomes:
        probability = float(expected.get(outcome, 0.0))
        observed_count = int(counts.get(outcome, 0))
        observed_frequency = observed_count / sample_count if sample_count else 0.0
        tolerance = (
            stddev_factor * math.sqrt(max(0.0, probability * (1.0 - probability) / sample_count))
            + tolerance_floor
            if sample_count
            else 0.0
        )
        delta = abs(observed_frequency - probability)
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"outcome {outcome} exceeds tolerance: observed={observed_frequency:.6f}, "
                f"expected={probability:.6f}, delta={delta:.6f}, tolerance={tolerance:.6f}"
            )
        rows.append(
            {
                "outcome": outcome,
                "expected_probability": probability,
                "observed_count": observed_count,
                "observed_frequency": observed_frequency,
                "delta": delta,
                "tolerance": tolerance,
            }
        )

    return {
        "status": STATUS_PASS if not failure_reasons else STATUS_MISMATCH,
        "sample_count": sample_count,
        "observed_counts": dict(sorted(observed_counts.items())),
        "observed_frequencies": dict(sorted(observed_frequencies.items())),
        "outcomes": rows,
        "max_delta": max_delta,
        "max_tolerance": max_tolerance,
        "failure_reasons": failure_reasons,
    }


def _public_run(run: dict[str, object]) -> dict[str, object]:
    return {
        "command": run["command"],
        "exit_code": run["exit_code"],
        "stderr": run["stderr"],
        "success": run["success"],
        "stdin_source": run["stdin_source"],
    }


def _verify_tool(
    *,
    tool_name: str,
    command: list[str],
    case_id: str,
    circuit: str,
    expected_distribution: dict[str, float],
    shots: int,
    seeds: list[int],
    inject_bitflip_rate: float = 0.0,
) -> dict[str, object]:
    expected_bits = _expected_bit_width(expected_distribution)
    runs: list[dict[str, object]] = []
    samples: list[str] = []
    failure_reasons: list[str] = []

    for seed in seeds:
        run = run_tool(build_sample_command(list(command), shots=shots, seed=seed), circuit=circuit)
        runs.append(_public_run(run))
        if not bool(run["success"]):
            failure_reasons.append(f"seed {seed}: {run['stderr'] or f'{tool_name} failed'}")
            continue
        try:
            seed_samples = parse_01_samples(
                str(run["stdout"]),
                expected_bits=expected_bits,
                expected_shots=shots,
            )
        except ValueError as error:
            failure_reasons.append(f"seed {seed}: failed to parse {tool_name} output: {error}")
            continue
        if inject_bitflip_rate:
            seed_samples = inject_bitflip(
                seed_samples,
                rate=inject_bitflip_rate,
                seed=_deterministic_bitflip_seed(case_id, seed),
            )
        samples.extend(seed_samples)

    comparison = compare_distribution(samples, expected_distribution) if samples else {
        "status": STATUS_TOOL_FAILED,
        "sample_count": 0,
        "observed_counts": {},
        "observed_frequencies": {},
        "outcomes": [],
        "max_delta": 0.0,
        "max_tolerance": 0.0,
        "failure_reasons": [],
    }
    status = STATUS_TOOL_FAILED if failure_reasons else str(comparison["status"])
    return {
        "tool": tool_name,
        "status": status,
        "sample_count": comparison["sample_count"],
        "observed_counts": comparison["observed_counts"],
        "observed_frequencies": comparison["observed_frequencies"],
        "outcomes": comparison["outcomes"],
        "max_delta": comparison["max_delta"],
        "max_tolerance": comparison["max_tolerance"],
        "failure_reasons": [*failure_reasons, *list(comparison["failure_reasons"])],
        "runs": runs,
    }


def verify_case(
    case: dict[str, object],
    *,
    stim_command: list[str],
    rstim_command: list[str],
    shots: int,
    seeds: list[int],
    inject_rstim_bitflip_rate: float,
) -> dict[str, object]:
    case_id = str(case["case_id"])
    circuit = str(case["circuit"])
    expected_distribution = _stable_float_map(case["expected_distribution"])
    stim = _verify_tool(
        tool_name="stim",
        command=stim_command,
        case_id=case_id,
        circuit=circuit,
        expected_distribution=expected_distribution,
        shots=shots,
        seeds=seeds,
    )
    rstim = _verify_tool(
        tool_name="rstim",
        command=rstim_command,
        case_id=case_id,
        circuit=circuit,
        expected_distribution=expected_distribution,
        shots=shots,
        seeds=seeds,
        inject_bitflip_rate=inject_rstim_bitflip_rate,
    )

    failure_reasons = [*list(stim["failure_reasons"]), *list(rstim["failure_reasons"])]
    if stim["status"] == STATUS_TOOL_FAILED:
        status = STATUS_STIM_FAILED
    elif rstim["status"] == STATUS_TOOL_FAILED:
        status = STATUS_RSTIM_FAILED
    elif stim["status"] == STATUS_MISMATCH or rstim["status"] == STATUS_MISMATCH:
        status = STATUS_MISMATCH
    else:
        status = STATUS_PASS

    return {
        "case_id": case_id,
        "status": status,
        "source_url": case["source_url"],
        "source_commit": case["source_commit"],
        "source_line_start": case["source_line_start"],
        "source_line_end": case["source_line_end"],
        "expected_distribution": expected_distribution,
        "stddev_factor": STDDEV_FACTOR,
        "tolerance_floor": TOLERANCE_FLOOR,
        "shots_per_seed": shots,
        "seeds": list(seeds),
        "sample_count": min(int(stim["sample_count"]), int(rstim["sample_count"])),
        "failure_reasons": failure_reasons,
        "stim": stim,
        "rstim": rstim,
    }
```

Then add the remaining CLI functions in the same file:

```python
def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return parsed


def _parse_seeds(raw_value: str) -> list[int]:
    seeds: list[int] = []
    for chunk in raw_value.split(","):
        stripped = chunk.strip()
        if not stripped:
            continue
        try:
            seed = int(stripped)
        except ValueError as error:
            raise ValueError(f"invalid seed {stripped!r}") from error
        seeds.append(seed)
    if not seeds:
        raise ValueError("at least one seed is required")
    return seeds


def _command_from_arg(raw_command: str | None, *, default: list[str] | None = None) -> list[str]:
    if raw_command is None:
        if default is None:
            raise ValueError("command is required")
        return list(default)
    command = shlex.split(raw_command)
    if not command:
        raise ValueError("command must not be empty")
    return command


def _validate_probability(value: float, name: str) -> None:
    if not 0.0 <= value <= 1.0:
        raise ValueError(f"{name} must be between 0 and 1")


def _overall_status(case_results: Sequence[dict[str, object]]) -> str:
    statuses = [str(result["status"]) for result in case_results]
    if STATUS_STIM_FAILED in statuses:
        return STATUS_STIM_FAILED
    if STATUS_RSTIM_FAILED in statuses:
        return STATUS_RSTIM_FAILED
    if STATUS_MISMATCH in statuses:
        return STATUS_MISMATCH
    return STATUS_PASS


def build_summary(args: argparse.Namespace) -> dict[str, object]:
    manifest = load_manifest(args.cases)
    errors = validate_manifest(manifest)
    if errors:
        raise ValueError("\n".join(errors))
    _validate_probability(args.inject_rstim_bitflip_rate, "--inject-rstim-bitflip-rate")

    stim_command = _command_from_arg(args.stim)
    rstim_command = _command_from_arg(args.rstim, default=default_rstim_command())
    seeds = _parse_seeds(args.seeds)
    case_results = [
        verify_case(
            case,
            stim_command=stim_command,
            rstim_command=rstim_command,
            shots=args.shots,
            seeds=seeds,
            inject_rstim_bitflip_rate=args.inject_rstim_bitflip_rate,
        )
        for case in manifest["cases"]
    ]
    counts = {
        STATUS_PASS: sum(1 for result in case_results if result["status"] == STATUS_PASS),
        STATUS_MISMATCH: sum(1 for result in case_results if result["status"] == STATUS_MISMATCH),
        STATUS_STIM_FAILED: sum(1 for result in case_results if result["status"] == STATUS_STIM_FAILED),
        STATUS_RSTIM_FAILED: sum(1 for result in case_results if result["status"] == STATUS_RSTIM_FAILED),
    }
    return {
        "manifest_path": str(args.cases),
        "suite": manifest.get("suite"),
        "status": _overall_status(case_results),
        "case_count": len(case_results),
        "shots": args.shots,
        "seeds": seeds,
        "stim_command": stim_command,
        "rstim_command": rstim_command,
        "inject_rstim_bitflip_rate": args.inject_rstim_bitflip_rate,
        "counts": counts,
        "cases": case_results,
    }


def write_summary(path: Path, summary: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def format_report(summary: dict[str, object]) -> tuple[int, str]:
    status = str(summary["status"])
    case_count = int(summary["case_count"])
    counts = summary["counts"]
    mismatch = int(counts[STATUS_MISMATCH])
    if status == STATUS_PASS:
        return 0, f"PASS distribution correctness cases={case_count} mismatch={mismatch}"
    if status == STATUS_MISMATCH:
        return 1, f"FAIL statistical mismatch cases={case_count} mismatch={mismatch}"
    return (
        1,
        "FAIL tool failure "
        f"cases={case_count} mismatch={mismatch} "
        f"stim_failed={counts[STATUS_STIM_FAILED]} rstim_failed={counts[STATUS_RSTIM_FAILED]}",
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify rstim and Stim sample distributions against the small-circuit catalog."
    )
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--stim", default="stim")
    parser.add_argument("--rstim", default=None)
    parser.add_argument("--shots", type=_positive_int, required=True)
    parser.add_argument("--seeds", default="12345")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--inject-rstim-bitflip-rate", type=float, default=0.0)
    args = parser.parse_args(argv)

    try:
        summary = build_summary(args)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 1

    write_summary(args.out, summary)
    exit_code, report = format_report(summary)
    print(report)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/verify_distributions.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py
git commit -m "feat: add distribution verifier"
```

---

### Task 3: Verifier Documentation And End-To-End Checks

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Consumes: `benchmarks.rstim_vs_stim_simulator.verify_distributions` CLI.
- Produces: documented success and negative-control commands.

- [ ] **Step 1: Update README**

In `benchmarks/rstim_vs_stim_simulator/README.md`, after the catalog validation paragraph and before "Inspect Fixture Load", add:

```markdown
## Distribution Verification

Run the small-circuit distribution verifier against Stim and `rstim`:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --shots 100000 \
  --out /tmp/rstim-vs-stim-distributions.json
```

The expected verdict is `PASS distribution correctness cases=8 mismatch=0`.
The JSON report records each case's expected probabilities, observed Stim and
`rstim` frequencies, tolerance, sample count, status, command, exit status,
stderr, and source URL.

Run the statistical negative control:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --shots 100000 \
  --inject-rstim-bitflip-rate 0.20 \
  --out /tmp/rstim-vs-stim-distributions-bad.json
```

The expected negative-control verdict is `FAIL statistical mismatch`.
```

- [ ] **Step 2: Run the verifier success command**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml --shots 100000 --out /tmp/rstim-vs-stim-distributions.json
```

Expected: exit 0 and stdout exactly `PASS distribution correctness cases=8 mismatch=0`.

- [ ] **Step 3: Inspect success JSON**

Run:

```sh
python3 -m json.tool /tmp/rstim-vs-stim-distributions.json >/tmp/rstim-vs-stim-distributions.pretty.json
```

Expected: exit 0. Confirm the JSON contains `case_count = 8` and no executable case has a non-`pass` status.

- [ ] **Step 4: Run the negative-control verifier command**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml --shots 100000 --inject-rstim-bitflip-rate 0.20 --out /tmp/rstim-vs-stim-distributions-bad.json
```

Expected: exit nonzero and stdout starts with `FAIL statistical mismatch`.

- [ ] **Step 5: Inspect negative-control JSON**

Run:

```sh
python3 -m json.tool /tmp/rstim-vs-stim-distributions-bad.json >/tmp/rstim-vs-stim-distributions-bad.pretty.json
```

Expected: exit 0. Confirm at least one case has `status = "statistical_mismatch"`.

- [ ] **Step 6: Run required Python unit tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/README.md
git commit -m "docs: document distribution verifier"
```

---

## Final Verification

After all tasks complete, run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml --shots 100000 --out /tmp/rstim-vs-stim-distributions.json
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml --shots 100000 --inject-rstim-bitflip-rate 0.20 --out /tmp/rstim-vs-stim-distributions-bad.json
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
cargo test
```

Expected:

- success verifier exits 0 and prints `PASS distribution correctness cases=8 mismatch=0`;
- negative control exits nonzero, prints `FAIL statistical mismatch`, and writes mismatch evidence;
- unit tests pass;
- `cargo test` passes.

## Plan Self-Review

- The plan covers every issue #430 interface and verification requirement.
- The negative control fails only if `rstim` samples are actually inspected.
- The JSON omits raw sample stdout and timings to stay reviewable and in scope.
- The Rust sampler fix is tested before the verifier can claim passing catalog evidence.
- No unresolved marker text remains.
