# Issue 65 Memory-Z Sweep Fixture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fixed Stim/PyMatching issue #65 memory-Z sweep fixture, compare it statistically against rsinter/rmatching, and publish a comparison figure.

**Architecture:** Use one shared TOML spec for the Rust sweep and one checked-in JSON fixture for the Stim reference sweep. A small Python tool generates the Stim fixture and renders a plot from the fixed Stim rows plus a Rust `results.jsonl`; the Rust integration test reads the fixture, runs the Rust sweep, computes binomial confidence intervals with `rsinter::stats::fit_binomial`, and checks agreement.

**Tech Stack:** Rust integration tests, rsinter benchmark runner, Python 3, Stim/PyMatching/Sinter when available, Matplotlib, existing `fit_binomial(..., 9.0)` confidence intervals.

---

## File Structure

- Create `tools/issue65_memory_z_sweep.py`: generate Stim reference JSON and render comparison plots.
- Create `rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml`: shared rsinter/rmatching sweep spec with `input_type = "memory-z"`.
- Create `rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json`: generated reference fixture for the 15 Stim/PyMatching rows.
- Create `rsinter/tests/memory_z_sweep.rs`: Rust statistical parity test.
- Create `docs/figures/issue-65-memory-z-stim-vs-rsinter.png`: generated comparison figure.

---

### Task 1: Add The Shared Sweep Spec And Generator Tool

**Files:**
- Create: `rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml`
- Create: `tools/issue65_memory_z_sweep.py`

- [ ] **Step 1: Add the TOML sweep fixture**

Create `rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml`:

```toml
name = "issue65-memory-z-sweep"
version = 1
mode = "independent"

[[runner]]
name = "rmatching-memory-z-d3"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "memory-z"
distance = [3]
rounds = [9]
p = [0.008, 0.009, 0.010, 0.011, 0.012]
max_shots = 1000000
max_errors = 5000
batch_size = 256

[[runner]]
name = "rmatching-memory-z-d5"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "memory-z"
distance = [5]
rounds = [15]
p = [0.008, 0.009, 0.010, 0.011, 0.012]
max_shots = 1000000
max_errors = 5000
batch_size = 256

[[runner]]
name = "rmatching-memory-z-d7"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "memory-z"
distance = [7]
rounds = [21]
p = [0.008, 0.009, 0.010, 0.011, 0.012]
max_shots = 1000000
max_errors = 5000
batch_size = 256

[plot]
title = "Issue 65 Memory-Z Sweep"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["language", "runner", "params.distance"]
label_template = "{language} {runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
```

- [ ] **Step 2: Write the Python generator and plot tool**

Create `tools/issue65_memory_z_sweep.py` with these commands:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import time
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pymatching
import stim

DISTANCES = (3, 5, 7)
NOISES = (0.008, 0.009, 0.010, 0.011, 0.012)
MAX_SHOTS = 1_000_000
MAX_ERRORS = 5_000
MAX_LIKELIHOOD_FACTOR = 9.0


def fit_binomial(shots: int, errors: int) -> tuple[float, float, float]:
    if shots == 0:
        return 0.0, 0.5, 1.0
    best = errors / shots
    # Wilson interval is used only for the figure; the Rust test uses
    # rsinter::stats::fit_binomial for the actual pass/fail check.
    z = 2.0
    denom = 1 + z * z / shots
    center = (best + z * z / (2 * shots)) / denom
    half = z * math.sqrt((best * (1 - best) + z * z / (4 * shots)) / shots) / denom
    return max(0.0, center - half), best, min(1.0, center + half)


def make_circuit(distance: int, rounds: int, noise: float) -> stim.Circuit:
    return stim.Circuit.generated(
        "surface_code:rotated_memory_z",
        rounds=rounds,
        distance=distance,
        after_clifford_depolarization=noise,
        after_reset_flip_probability=noise,
        before_measure_flip_probability=noise,
        before_round_data_depolarization=noise,
    )


def collect_stim_reference(out: Path) -> None:
    rows = []
    for distance in DISTANCES:
        rounds = distance * 3
        for noise in NOISES:
            circuit = make_circuit(distance, rounds, noise)
            sampler = circuit.compile_detector_sampler()
            dem = circuit.detector_error_model(decompose_errors=True)
            matcher = pymatching.Matching.from_detector_error_model(dem)
            shots = 0
            errors = 0
            while shots < MAX_SHOTS and errors < MAX_ERRORS:
                batch = min(256, MAX_SHOTS - shots)
                dets, obs = sampler.sample(
                    shots=batch,
                    separate_observables=True,
                    bit_packed=True,
                )
                pred = matcher.decode_batch(
                    dets,
                    bit_packed_shots=True,
                    bit_packed_predictions=True,
                )
                errors += sum(bytes(a) != bytes(b) for a, b in zip(pred, obs))
                shots += batch
            low, best, high = fit_binomial(shots, errors)
            rows.append(
                {
                    "distance": distance,
                    "rounds": rounds,
                    "p": noise,
                    "shots": shots,
                    "logical_errors": errors,
                    "logical_error_rate": errors / shots,
                    "ci_low": low,
                    "ci_high": high,
                    "num_detectors": circuit.num_detectors,
                    "num_observables": circuit.num_observables,
                }
            )
    payload = {
        "metadata": {
            "generator": "tools/issue65_memory_z_sweep.py collect-stim",
            "created_at_unix": int(time.time()),
            "stim_version": stim.__version__,
            "pymatching_version": pymatching.__version__,
            "sinter_version": import_sinter_version(),
            "max_shots": MAX_SHOTS,
            "max_errors": MAX_ERRORS,
        },
        "rows": rows,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def import_sinter_version() -> str | None:
    try:
        import sinter
    except ModuleNotFoundError:
        return None
    return getattr(sinter, "__version__", "unknown")


def load_rust_rows(path: Path) -> list[dict[str, object]]:
    rows = []
    for line in path.read_text().splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return [
        {
            "distance": int(row["params"]["distance"]),
            "rounds": int(row["params"]["rounds"]),
            "p": float(row["params"]["p"]),
            "shots": int(row["metrics"]["shots_used"]),
            "logical_errors": int(row["metrics"]["logical_errors"]),
            "logical_error_rate": float(row["metrics"]["logical_error_rate"]),
            "source": "RStim/rmatching",
        }
        for row in rows
        if int(row["params"]["rounds"]) == int(row["params"]["distance"]) * 3
    ]


def plot_compare(stim_fixture: Path, rust_results: list[Path], out: Path) -> None:
    stim_rows = json.loads(stim_fixture.read_text())["rows"]
    rust_rows = []
    for path in rust_results:
        rust_rows.extend(load_rust_rows(path))
    fig, axes = plt.subplots(1, 3, figsize=(13, 4), sharey=True)
    for axis, distance in zip(axes, DISTANCES):
        for label, rows, color, marker in [
            ("Stim/PyMatching", stim_rows, "tab:blue", "o"),
            ("RStim/rmatching", rust_rows, "tab:orange", "s"),
        ]:
            selected = sorted(
                [row for row in rows if int(row["distance"]) == distance],
                key=lambda row: float(row["p"]),
            )
            xs = [float(row["p"]) for row in selected]
            ys = [float(row["logical_error_rate"]) for row in selected]
            lows = []
            highs = []
            for row, y in zip(selected, ys):
                low, _best, high = fit_binomial(
                    int(row["shots"]),
                    int(row["logical_errors"]),
                )
                lows.append(y - low)
                highs.append(high - y)
            axis.errorbar(xs, ys, yerr=[lows, highs], marker=marker, label=label, color=color)
        axis.set_title(f"d={distance}, r={distance * 3}")
        axis.set_xlabel("p")
        axis.grid(True, alpha=0.3)
    axes[0].set_ylabel("logical error rate")
    axes[0].legend()
    fig.tight_layout()
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=180)


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    collect = sub.add_parser("collect-stim")
    collect.add_argument("--out", type=Path, required=True)
    plot = sub.add_parser("plot")
    plot.add_argument("--stim-fixture", type=Path, required=True)
    plot.add_argument("--rust-results", type=Path, action="append", required=True)
    plot.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.cmd == "collect-stim":
        collect_stim_reference(args.out)
    elif args.cmd == "plot":
        plot_compare(args.stim_fixture, args.rust_results, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Run generator import smoke**

Run:

```bash
python3 tools/issue65_memory_z_sweep.py --help
```

Expected: prints `collect-stim` and `plot` subcommands.

- [ ] **Step 4: Commit**

```bash
git add tools/issue65_memory_z_sweep.py rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml
git commit -m "test: add issue 65 memory-z sweep tooling"
```

---

### Task 2: Generate The Fixed Stim/PyMatching Fixture

**Files:**
- Create: `rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json`

- [ ] **Step 1: Generate the fixture**

Run:

```bash
MPLCONFIGDIR=/tmp/issue65-mpl python3 tools/issue65_memory_z_sweep.py collect-stim \
  --out rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json
```

Expected: JSON with `metadata` and exactly 15 rows.

- [ ] **Step 2: Validate row count and stop rule**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p = Path("rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json")
data = json.loads(p.read_text())
assert len(data["rows"]) == 15
for row in data["rows"]:
    assert row["rounds"] == row["distance"] * 3
    assert row["shots"] <= 1_000_000
    assert row["logical_errors"] <= 5_000
print("ok")
PY
```

Expected: `ok`.

- [ ] **Step 3: Commit**

```bash
git add rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json
git commit -m "test: record issue 65 stim memory-z sweep"
```

---

### Task 3: Add Rust Statistical Parity Test

**Files:**
- Create: `rsinter/tests/memory_z_sweep.rs`

- [ ] **Step 1: Write the failing test**

Create `rsinter/tests/memory_z_sweep.rs`:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{BenchmarkResultRow, read_results_jsonl};
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::BenchmarkSpec;
use rsinter::stats::fit_binomial;
use serde::Deserialize;

const FIT_FACTOR: f64 = 9.0;

#[derive(Debug, Deserialize)]
struct StimFixture {
    rows: Vec<StimRow>,
}

#[derive(Debug, Deserialize)]
struct StimRow {
    distance: usize,
    rounds: usize,
    p: f64,
    shots: u64,
    logical_errors: u64,
    logical_error_rate: f64,
    num_detectors: usize,
    num_observables: usize,
}

#[test]
fn issue65_memory_z_rstim_ler_agrees_with_stim_reference_intervals() {
    let fixture = load_stim_fixture();
    assert_eq!(fixture.rows.len(), 15);

    let rust_rows = run_rust_issue65_sweep();
    assert_eq!(rust_rows.len(), 15);

    let rust_by_case = rust_rows
        .iter()
        .map(|row| (case_key_from_rust(row), row))
        .collect::<BTreeMap<_, _>>();

    for stim in &fixture.rows {
        let key = case_key(stim.distance, stim.rounds, stim.p);
        let rust = rust_by_case.get(&key).unwrap_or_else(|| panic!("missing Rust row for {key}"));
        assert_eq!(rust.status, "ok", "Rust row for {key} was not ok");
        assert_eq!(rust.case_summary["num_dets"], serde_json::json!(stim.num_detectors));
        assert_eq!(rust.case_summary["num_obs"], serde_json::json!(stim.num_observables));

        let rust_shots = count_metric(rust, "shots_used");
        let rust_errors = count_metric(rust, "logical_errors");
        let rust_fit = fit_binomial(rust_shots, rust_errors, FIT_FACTOR);
        let stim_fit = fit_binomial(stim.shots, stim.logical_errors, FIT_FACTOR);

        let rust_low = rust_fit.low.unwrap();
        let rust_high = rust_fit.high.unwrap();
        let stim_low = stim_fit.low.unwrap();
        let stim_high = stim_fit.high.unwrap();

        assert!(
            rust_low <= stim.logical_error_rate && stim.logical_error_rate <= rust_high,
            "Stim LER for {key}={} outside Rust CI [{}, {}]; rust errors/shots={}/{} stim errors/shots={}/{}",
            stim.logical_error_rate,
            rust_low,
            rust_high,
            rust_errors,
            rust_shots,
            stim.logical_errors,
            stim.shots,
        );
        assert!(
            rust_low <= stim_high && stim_low <= rust_high,
            "CI intervals do not overlap for {key}: Rust [{}, {}], Stim [{}, {}]",
            rust_low,
            rust_high,
            stim_low,
            stim_high,
        );
    }
}

fn load_stim_fixture() -> StimFixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json");
    let text = fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn run_rust_issue65_sweep() -> Vec<BenchmarkResultRow> {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/issue65_memory_z_sweep.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();
    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for runner in [
        "rmatching-memory-z-d3",
        "rmatching-memory-z-d5",
        "rmatching-memory-z-d7",
    ] {
        let results_path = artifact_root.join(runner).join("test-run").join("results.jsonl");
        let data = fs::read(results_path).unwrap();
        rows.extend(read_results_jsonl(&data[..]).unwrap());
    }
    rows
}

fn count_metric(row: &BenchmarkResultRow, key: &str) -> u64 {
    row.metrics[key].round() as u64
}

fn case_key_from_rust(row: &BenchmarkResultRow) -> String {
    case_key(
        row.params["distance"].as_u64().unwrap() as usize,
        row.params["rounds"].as_u64().unwrap() as usize,
        row.params["p"].as_f64().unwrap(),
    )
}

fn case_key(distance: usize, rounds: usize, p: f64) -> String {
    format!("d{distance}_r{rounds}_p{p:.3}")
}
```

- [ ] **Step 2: Run the test before the fixture exists if Task 2 was skipped**

Run:

```bash
cargo test -p rsinter --test memory_z_sweep
```

Expected before Task 2: FAIL because the fixture JSON is missing. Expected after Task 2: PASS if the statistical comparison agrees.

- [ ] **Step 3: Commit**

```bash
git add rsinter/tests/memory_z_sweep.rs
git commit -m "test: compare issue 65 memory-z sweep intervals"
```

---

### Task 4: Generate Rust Results And Comparison Figure

**Files:**
- Create: `docs/figures/issue-65-memory-z-stim-vs-rsinter.png`

- [ ] **Step 1: Run Rust sweep to a temp output**

Run:

```bash
cargo run -p rsinter -- bench run \
  --spec rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml \
  --language rust \
  --out /tmp/issue65-memory-z-rust
```

Expected: these three files exist:

- `/tmp/issue65-memory-z-rust/rmatching-memory-z-d3/test-run/results.jsonl`
- `/tmp/issue65-memory-z-rust/rmatching-memory-z-d5/test-run/results.jsonl`
- `/tmp/issue65-memory-z-rust/rmatching-memory-z-d7/test-run/results.jsonl`

- [ ] **Step 2: Render comparison figure**

Run:

```bash
MPLCONFIGDIR=/tmp/issue65-mpl python3 tools/issue65_memory_z_sweep.py plot \
  --stim-fixture rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json \
  --rust-results /tmp/issue65-memory-z-rust/rmatching-memory-z-d3/test-run/results.jsonl \
  --rust-results /tmp/issue65-memory-z-rust/rmatching-memory-z-d5/test-run/results.jsonl \
  --rust-results /tmp/issue65-memory-z-rust/rmatching-memory-z-d7/test-run/results.jsonl \
  --out docs/figures/issue-65-memory-z-stim-vs-rsinter.png
```

Expected: PNG file exists and shows both Stim/PyMatching and RStim/rmatching error bars for distances 3, 5, and 7.

- [ ] **Step 3: Commit**

```bash
git add docs/figures/issue-65-memory-z-stim-vs-rsinter.png
git commit -m "docs: plot issue 65 memory-z sweep comparison"
```

---

### Task 5: Final Verification And PR Update

**Files:**
- No new files unless verification requires a focused fix.

- [ ] **Step 1: Run focused checks**

Run:

```bash
cargo test -p rsinter --test memory_z_sweep
cargo test -p rsinter --test bench_registry --test bench_circuit_source --test bench_run
cargo test -p rstim --test stim_codegen --test cross_validate_dem
```

Expected: all pass.

- [ ] **Step 2: Run broad check**

Run:

```bash
cargo test -p rsinter -p rstim
```

Expected: all pass.

- [ ] **Step 3: Push and update PR body**

Run:

```bash
git status -sb
git push
```

Then update PR #74 to mention the fixture, statistical test, and figure:

```markdown
## Additional Issue #65 Sweep Evidence
- Added fixed Stim/PyMatching fixture for the 15-point memory-Z sweep.
- Added Rust interval-overlap regression test against the fixture.
- Added comparison figure: `docs/figures/issue-65-memory-z-stim-vs-rsinter.png`.
```
